//! `propose_edit` calls that revise a referenced pending candidate or grow its Proposal over
//! Joinable Blocks (ADR 0062, ADR 0071), with the fake of each profile.

use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ApplyAuthorEditCommand,
    AuthorEditProposalTarget, EditorSessionStore, ProjectScope,
};
use storyos_core::{
    AuthorEditPrimitive, AuthorEditUnit, DeploymentDestination, SelectionReference,
    SelectionSnapshot, TextPoint,
};
use tokio_postgres::Client;

use super::tests::{REVISE, complete, opened, proposal_records, propose_edit_calls};
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_referenced_run, queued_run_offering, stores};
use crate::create_volume_authority_tests::{NamedEdit, adjusted_edit_command};
use crate::set_current_chapter_authority_tests::open_session_request;

const SENTENCES: [&str; 4] = ["Rain fell.", "Wind rose.", "Night came.", "Dawn broke."];

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

    /// Admits and completes one Run with `references` in a new Conversation.
    async fn run(
        &self,
        store: &PostgresProjectReader,
        admin: &Client,
        offset: u16,
        message: &str,
        references: Vec<SelectionReference>,
    ) -> String {
        let prefix = format!("{:03x}", self.lead + offset);
        let run_id = admit_referenced_run(
            store,
            &self.scope,
            &self.chapter_id,
            &prefix,
            self.destination.clone(),
            message,
            references,
        )
        .await;
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

    /// The Author Edit that puts `text` before the candidate text of `reference`. `sequence` is
    /// the local intent sequence of the Editor Session of the fixture, from 2.
    pub(crate) async fn candidate_edit(
        &self,
        store: &PostgresProjectReader,
        admin: &Client,
        reference: &SelectionReference,
        sequence: u64,
        text: &str,
    ) -> ApplyAuthorEditCommand {
        let target = reference.candidate_target().expect("candidate reference");
        let manuscript_block_id = admin
            .query_one(
                "SELECT manuscript_block_id::text FROM storyos.proposal_operations
                  WHERE operation_id = $1::text::uuid",
                &[&target.operation_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        let prefix = format!("{:03x}", self.lead + 7);
        let editor_session_id = format!("018f0000-0000-7001-8000-00000001{prefix}0");
        let edit = NamedEdit {
            editor_session_id: &editor_session_id,
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
        })
        .await
    }
}

/// The Blocks of the Authoritative Revision `revision_id` in order.
async fn blocks(admin: &Client, revision_id: &str) -> Vec<String> {
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

/// The locations of the public `prose_change` of the Run.
async fn locations(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> Value {
    let record = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let AgentRunDecisionInspect::ProseChange {
        locations: Some(locations),
        ..
    } = record.decision
    else {
        panic!("{:?}", record.decision);
    };
    json!(locations)
}

/// `value` with each UUID replaced by the order of its first appearance, so that the records of
/// two Projects compare.
fn without_identities(value: &Value, seen: &mut Vec<String>) -> Value {
    match value {
        Value::String(text) if uuid::Uuid::parse_str(text).is_ok() => {
            let index = seen
                .iter()
                .position(|known| known == text)
                .unwrap_or_else(|| {
                    seen.push(text.clone());
                    seen.len() - 1
                });
            json!(format!("id-{index}"))
        }
        Value::Array(items) => items
            .iter()
            .map(|item| without_identities(item, seen))
            .collect(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, item)| (key.clone(), without_identities(item, seen)))
            .collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

/// The status and the locations of the Run and the Proposal records of the fixture, with no
/// identities.
async fn outcome(
    store: &PostgresProjectReader,
    admin: &Client,
    fixture: &Fixture,
    run_id: &str,
) -> Value {
    let mut seen = Vec::new();
    // The panel opens the current revision of a location, or else the revision of its outcome.
    // A later RunStep of the tool loop references the joined candidates, so their locations
    // have a current revision. It is the revision of the outcome.
    let mut locations = locations(store, &fixture.scope, run_id).await;
    for location in locations.as_array_mut().unwrap() {
        let opened = match &location["current"] {
            Value::Null => location["outcome"]["revision_id"].clone(),
            current => current["revision_id"].clone(),
        };
        location["current"] = opened;
    }
    let status = admin
        .query_one(
            "SELECT status FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get::<_, String>(/*idx*/ 0);
    let records = json!([
        status,
        locations,
        proposal_records(admin, &fixture.source_run).await,
    ]);
    without_identities(&records, &mut seen)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_revises_the_candidate_or_grows_its_proposal_as_the_reply_form_does() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let grow = format!("{REVISE} SCRIPT:grow_proposal");
    // Each case gives the leads of the reply form and the tool loop Projects.
    let cases = [
        ("revise", [0xd18, 0xd10], REVISE, false),
        ("grow", [0xd38, 0xd30], grow.as_str(), true),
    ];
    for (name, [reply_lead, tool_lead], message, joined) in cases {
        let mut outcomes = Vec::new();
        let mut runs = Vec::new();
        for (lead, destination) in [
            (reply_lead, DeploymentDestination::HostFake),
            (tool_lead, DeploymentDestination::HostFakeToolLoop),
        ] {
            let fixture = Fixture::new(&store, &admin, lead, destination).await;
            let mut references = fixture.candidates(&admin).await;
            if joined {
                references.insert(0, fixture.chapter_text(1, 2));
            }
            let run_id = fixture.run(&store, &admin, 2, message, references).await;
            outcomes.push(outcome(&store, &admin, &fixture, &run_id).await);
            runs.push((fixture, run_id));
        }
        let (fixture, run_id) = &runs[1];
        let records = proposal_records(&admin, &fixture.source_run).await;
        let kinds: Vec<Value> = locations(&store, &fixture.scope, run_id)
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|location| location["outcome"]["kind"].clone())
            .collect();
        let blocks = if joined { 3 } else { 1 };
        let bundle = if joined { "atomic" } else { "none" };

        assert_eq!(outcomes[0], outcomes[1], "{name}");
        assert_eq!(
            (
                kinds,
                records[0]["proposal"].clone(),
                records[0]["revisions"].as_array().unwrap().len(),
                records[0]["operations"].as_array().unwrap().len(),
                propose_edit_calls(&admin, run_id).await,
            ),
            (
                vec![json!("revised"); blocks],
                json!(["block_edit", bundle]),
                2,
                blocks,
                vec![opened(0)],
            ),
            "{name}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_that_breaks_the_growth_rule_is_refused_and_no_proposal_changes() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let tool_loop = DeploymentDestination::HostFakeToolLoop;
    let fixture = Fixture::new(&store, &admin, 0x430, tool_loop.clone()).await;
    let refused = json!([
        0,
        "refused",
        "change_shape",
        "The Host refused this call because it changes a pending candidate together with \
         blocks that cannot join its Proposal, so no Proposal opened."
    ]);
    let before = proposal_records(&admin, &fixture.source_run).await;
    let mut observed = Vec::new();
    // A Block that is not joinable, and a joinable Block that leaves a gap.
    for (offset, script) in [(2, "grow_unjoinable"), (3, "grow_gap")] {
        let mut references = fixture.candidates(&admin).await;
        references.insert(0, fixture.chapter_text(1, 2));
        let message = format!("{REVISE} SCRIPT:{script}");
        let run_id = fixture
            .run(&store, &admin, offset, &message, references)
            .await;
        observed.push((
            script,
            propose_edit_calls(&admin, &run_id).await,
            proposal_records(&admin, &run_id).await,
            proposal_records(&admin, &fixture.source_run).await,
        ));
    }
    // Two candidates of a grown Proposal.
    let grown = Fixture::new(&store, &admin, 0x438, tool_loop).await;
    let mut references = grown.candidates(&admin).await;
    references.insert(0, grown.chapter_text(1, 2));
    let grow = format!("{REVISE} SCRIPT:grow_proposal");
    grown.run(&store, &admin, 2, &grow, references).await;
    let grown_before = proposal_records(&admin, &grown.source_run).await;
    let message = format!("{REVISE} SCRIPT:propose_two_candidates");
    let references = grown.candidates(&admin).await[1..].to_vec();
    let run_id = grown.run(&store, &admin, 3, &message, references).await;
    observed.push((
        "two_candidates",
        propose_edit_calls(&admin, &run_id).await,
        proposal_records(&admin, &run_id).await,
        proposal_records(&admin, &grown.source_run).await,
    ));

    assert_eq!(
        observed,
        vec![
            (
                "grow_unjoinable",
                vec![refused.clone()],
                json!([]),
                before.clone()
            ),
            ("grow_gap", vec![refused.clone()], json!([]), before),
            ("two_candidates", vec![refused], json!([]), grown_before),
        ]
    );
}
