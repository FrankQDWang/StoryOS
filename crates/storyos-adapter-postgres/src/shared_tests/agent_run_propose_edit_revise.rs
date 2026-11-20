//! The shared test support from `agent_run_propose_edit_revise_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_propose_edit::{REVISE, complete};
use crate::shared_tests::create_volume_authority::{NamedEdit, adjusted_edit_command};
use crate::shared_tests::model_gateway::{Admission, queued_run_offering};
use crate::shared_tests::set_current_chapter_authority::open_session_request;
use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditProposalTarget, EditorSessionStore, ProjectScope,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, DeploymentDestination, SelectionReference,
    SelectionSnapshot, TextPoint,
};
use tokio_postgres::Client;

pub(crate) const SENTENCES: [&str; 4] = ["Rain fell.", "Wind rose.", "Night came.", "Dawn broke."];

/// A new Project of `destination` whose Chapter has one Block for each sentence, and one pending
/// Proposal on the last Block. `lead` gives the identities: `lead` for the Project, `lead + 1`
/// for the Run that opened the Proposal, `lead + 2` to `lead + 6` for later Runs, and
/// `lead + 7` for the Editor Session and the Author Edit.
pub(crate) struct Fixture {
    pub(crate) scope: ProjectScope,
    pub(crate) chapter_id: String,
    head: String,
    blocks: Vec<String>,
    destination: DeploymentDestination,
    lead: u16,
    pub(crate) source_run: String,
}

impl Fixture {
    pub(crate) async fn new(
        store: &PostgresProjectReader,
        admin: &Client,
        lead: u16,
        destination: DeploymentDestination,
    ) -> Self {
        let prefix = |offset: u16| format!("{:03x}", lead + offset);
        let (scope, chapter_id, first) =
            queued_run_offering(store, &prefix(0), destination.clone(), None).await;
        complete(store, admin, &scope, &first).await;
        let request = open_session_request(store, &scope, &format!("{}0", prefix(7))).await;
        let session = store.create_editor_session(&request).await.unwrap();
        let base = session.base_snapshot.authoritative_revision_id;
        let first_block = blocks(admin, &base).await.remove(0);
        let mut splits = Vec::new();
        let mut split_block = first_block.clone();
        for sentence in &SENTENCES[..3] {
            let new_block = uuid::Uuid::now_v7().to_string();
            splits.push(AuthorEditPrimitive::SplitBlock {
                manuscript_block_id: split_block,
                offset: sentence.len() as u32,
                new_manuscript_block_id: new_block.clone(),
            });
            split_block = new_block;
        }
        let edit = NamedEdit {
            editor_session_id: request.editor_session_id.as_ref(),
            chapter_id: &chapter_id,
            expected_revision_id: &base,
            suffix: &format!("{}1", prefix(7)),
            local_intent_sequence: 1,
            text: "",
            proposal_target: None,
        };
        let command = adjusted_edit_command(store, &scope, edit, |command| {
            // The Author Edit class keeps the Shared Challenges of the Project for the Runs.
            command.challenge_binding.challenge_rate_policy_revision =
                storyos_application::ChallengeRateClass::AuthorEdit
                    .policy_revision()
                    .to_owned();
            command.author_edit_units = vec![AuthorEditUnit {
                normalized_primitives: std::iter::once(
                    AuthorEditPrimitive::ReplaceBlockSelection {
                        manuscript_block_id: first_block,
                        from: 0,
                        to: 0,
                        text: SENTENCES.concat(),
                    },
                )
                .chain(splits)
                .collect(),
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 0,
                    to: 0,
                },
            }];
        })
        .await;
        store.apply_author_edit(&command).await.unwrap();
        let head: String = admin
            .query_one(
                "SELECT current_revision_id::text FROM storyos.authoritative_heads
                  WHERE manuscript_object_id = $1::text::uuid",
                &[&chapter_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        let blocks = blocks(admin, &head).await;
        assert_eq!(blocks.len(), SENTENCES.len());
        let mut fixture = Self {
            scope,
            chapter_id,
            blocks,
            head,
            destination,
            lead,
            source_run: String::new(),
        };
        let message = format!("{REVISE} SCRIPT:partial_location");
        fixture.source_run = fixture.run(store, admin, 1, &message, Vec::new()).await;
        fixture
    }

    /// Admits one Run with `references` in a new Conversation, with the identities of
    /// `lead + offset`.
    pub(crate) async fn admit(
        &self,
        store: &PostgresProjectReader,
        offset: u16,
        message: &str,
        references: Vec<SelectionReference>,
    ) -> String {
        let prefix = format!("{:03x}", self.lead + offset);
        Admission {
            destination: self.destination.clone(),
            message,
            references: Some(references),
            ..Default::default()
        }
        .admit(store, &self.scope, &self.chapter_id, &prefix)
        .await
    }

    /// Admits and completes one Run with `references` in a new Conversation.
    pub(crate) async fn run(
        &self,
        store: &PostgresProjectReader,
        admin: &Client,
        offset: u16,
        message: &str,
        references: Vec<SelectionReference>,
    ) -> String {
        let run_id = self.admit(store, offset, message, references).await;
        complete(store, admin, &self.scope, &run_id).await;
        run_id
    }

    /// A reference to all of the text from Block `first` to Block `last`.
    pub(crate) fn chapter_text(&self, first: usize, last: usize) -> SelectionReference {
        SelectionReference::ChapterText {
            chapter_id: self.chapter_id.clone(),
            base_authoritative_revision_id: self.head.clone(),
            start: TextPoint {
                manuscript_block_id: self.blocks[first].clone(),
                offset: 0,
            },
            end: TextPoint {
                manuscript_block_id: self.blocks[last].clone(),
                offset: SENTENCES[last].len() as u32,
            },
        }
    }

    /// A reference to all of the text of each pending candidate of the Proposal, in Block order.
    pub(crate) async fn candidates(&self, admin: &Client) -> Vec<SelectionReference> {
        admin
            .query(
                "SELECT proposal.proposal_id::text, operation.operation_id::text,
                        head.current_revision_id::text, operation.candidate_text
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head USING (owner_user_id, project_id, proposal_id)
                   JOIN storyos.proposal_operations AS operation
                     USING (owner_user_id, project_id, proposal_id)
                   JOIN storyos.manuscript_revision_members AS member
                     ON member.revision_id = $2::text::uuid
                    AND member.manuscript_block_id = operation.manuscript_block_id
                  WHERE proposal.source_run_id = $1::text::uuid
                  ORDER BY member.block_order",
                &[&self.source_run, &self.head],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| SelectionReference::ProposalCandidateText {
                chapter_id: self.chapter_id.clone(),
                proposal_id: row.get(/*idx*/ 0),
                operation_id: row.get(/*idx*/ 1),
                revision_id: row.get(/*idx*/ 2),
                start_offset: 0,
                end_offset: row.get::<_, &str>(/*idx*/ 3).encode_utf16().count() as u32,
            })
            .collect()
    }

    /// The Editor Session of the fixture, and the Chapter head that its commands expect.
    pub(crate) fn writer(&self) -> (storyos_application::EditorSessionId, &str) {
        let prefix = format!("{:03x}", self.lead + 7);
        let editor_session_id = format!("018f0000-0000-7001-8000-00000001{prefix}0");
        (
            storyos_application::EditorSessionId::new(editor_session_id),
            &self.head,
        )
    }

    /// The id of Block `index` of the fixture head.
    pub(crate) fn block(&self, index: usize) -> &str {
        &self.blocks[index]
    }

    /// Applies the Author Edit that puts `text` before the text of the first Chapter Block.
    /// `sequence` is the local intent sequence of the Editor Session of the fixture, from 2.
    pub(crate) async fn edit_chapter(
        &self,
        store: &PostgresProjectReader,
        sequence: u64,
        text: &str,
    ) {
        let primitive = AuthorEditPrimitive::ReplaceBlockSelection {
            manuscript_block_id: self.blocks[0].clone(),
            from: 0,
            to: 0,
            text: text.to_owned(),
        };
        let command = self.chapter_edit(store, sequence, vec![primitive]).await;
        store.apply_author_edit(&command).await.unwrap();
    }

    /// The Author Edit of the Chapter head of the fixture with one unit for each of `primitives`:
    /// a Block text replacement or a join of two Blocks. `sequence` is the local intent sequence
    /// of the Editor Session of the fixture, from 2.
    pub(crate) async fn chapter_edit(
        &self,
        store: &PostgresProjectReader,
        sequence: u64,
        primitives: Vec<AuthorEditPrimitive>,
    ) -> ApplyAuthorEditCommand {
        let units = primitives
            .into_iter()
            .map(|primitive| {
                // The selection of a unit is the range of its replacement, or the end of the left
                // Block of its join.
                let (from, to) = match &primitive {
                    AuthorEditPrimitive::ReplaceBlockSelection { from, to, .. } => (*from, *to),
                    AuthorEditPrimitive::JoinBlocks {
                        left_manuscript_block_id,
                        ..
                    } => {
                        let left = self
                            .blocks
                            .iter()
                            .position(|id| id == left_manuscript_block_id);
                        let end = SENTENCES[left.expect("a Block of the fixture")].len() as u32;
                        (end, end)
                    }
                    other => panic!("{other:?}"),
                };
                AuthorEditUnit {
                    normalized_primitives: vec![primitive],
                    selection_snapshot: SelectionSnapshot {
                        coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                        from,
                        to,
                    },
                }
            })
            .collect();
        let prefix = format!("{:03x}", self.lead + 7);
        let (editor_session_id, _) = self.writer();
        let edit = NamedEdit {
            editor_session_id: editor_session_id.as_ref(),
            chapter_id: &self.chapter_id,
            expected_revision_id: &self.head,
            suffix: &format!("{prefix}{sequence}"),
            local_intent_sequence: sequence,
            text: "",
            proposal_target: None,
        };
        adjusted_edit_command(store, &self.scope, edit, |command| {
            command.challenge_binding.challenge_rate_policy_revision =
                storyos_application::ChallengeRateClass::AuthorEdit
                    .policy_revision()
                    .to_owned();
            command.author_edit_units = units;
        })
        .await
    }

    /// The Author Edit that makes `change` to the candidate text of `reference`. `sequence` is
    /// the local intent sequence of the Editor Session of the fixture, from 2.
    pub(crate) async fn candidate_edit(
        &self,
        store: &PostgresProjectReader,
        admin: &Client,
        reference: &SelectionReference,
        sequence: u64,
        change: CandidateChange<'_>,
    ) -> ApplyAuthorEditCommand {
        let target = reference.candidate_target().expect("candidate reference");
        let row = admin
            .query_one(
                "SELECT manuscript_block_id::text, candidate_text FROM storyos.proposal_operations
                  WHERE operation_id = $1::text::uuid",
                &[&target.operation_id],
            )
            .await
            .unwrap();
        let manuscript_block_id = row.get(/*idx*/ 0);
        let (to, text) = match change {
            CandidateChange::Prefix(text) => (0, text),
            CandidateChange::RemoveAll => (
                row.get::<_, &str>(/*idx*/ 1).encode_utf16().count() as u32,
                "",
            ),
        };
        let prefix = format!("{:03x}", self.lead + 7);
        let (editor_session_id, _) = self.writer();
        let edit = NamedEdit {
            editor_session_id: editor_session_id.as_ref(),
            chapter_id: &self.chapter_id,
            expected_revision_id: &self.head,
            suffix: &format!("{prefix}{sequence}"),
            local_intent_sequence: sequence,
            text,
            proposal_target: Some(AuthorEditProposalTarget {
                proposal_id: target.proposal_id,
                operation_id: target.operation_id,
                revision_id: target.revision_id,
                manuscript_block_id,
            }),
        };
        adjusted_edit_command(store, &self.scope, edit, |command| {
            command.challenge_binding.challenge_rate_policy_revision =
                storyos_application::ChallengeRateClass::AuthorEdit
                    .policy_revision()
                    .to_owned();
            command.author_edit_units = vec![AuthorEditUnit {
                normalized_primitives: vec![AuthorEditPrimitive::ReplaceSelection {
                    from: 0,
                    to,
                    text: text.to_owned(),
                }],
                selection_snapshot: SelectionSnapshot {
                    coordinate_profile: storyos_core::UTF16_COORDINATE_PROFILE.to_owned(),
                    from: 0,
                    to,
                },
            }];
        })
        .await
    }
}

/// One Author Edit of a candidate text.
pub(crate) enum CandidateChange<'a> {
    /// Puts the text before the candidate text.
    Prefix(&'a str),
    /// Removes all of the candidate text.
    RemoveAll,
}

/// The Blocks of the Authoritative Revision `revision_id` in order.
pub(crate) async fn blocks(admin: &Client, revision_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT manuscript_block_id::text FROM storyos.manuscript_revision_members
              WHERE revision_id = $1::text::uuid ORDER BY block_order",
            &[&revision_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}
