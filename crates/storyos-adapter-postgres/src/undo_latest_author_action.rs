//! `undoLatestAuthorAction`: one Author Undo through the command sequence (ADR 0044). The
//! compensation adapter of the Forward command kind writes its Compensation records.

use std::convert::Infallible;

use storyos_application::{
    AuthorUndoFrontierPosition, EditorSessionError, ProjectCommandEnvelope, ProjectCommandError,
    UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
};
use storyos_core::{
    AuthorUndoFrontier, AuthorUndoFrontierKind, ProjectLifecycle, TransitionOutcome,
    UndoLatestAuthorAction as CoreUndo, UndoLatestAuthorActionConflict,
    UndoLatestAuthorActionUnavailable, undo_latest_author_action as classify_undo,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    Admission, AppliedVariant, Classification, CommandIsolation, CommandSpec, EditorAdmission,
    EditorWriter, LockedProject, MissingAdmission, ProfileSequences, ProjectCommand,
    ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReceiptRefs, ReceiptTime, ReplayEffect,
    ZeroAuthorityRows, ZeroOutcome, ZeroReceipt, settle_project_command, unavailable,
};
use crate::undo_compensation::{ForwardCommand, UndoRequest};
use crate::undo_frontier::{ObservedFrontier, load_observed_frontier};

mod profile;
mod replay;
pub(crate) use profile::{UndoCompensation, UndoPlan, UndoWrite};

impl PostgresProjectReader {
    /// Settles one Undo Latest Author Action.
    pub async fn undo_latest_author_action(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &UndoLatestAuthorActionInput,
    ) -> Result<UndoLatestAuthorActionSettlement, ProjectCommandError> {
        settle_project_command(
            self,
            envelope,
            &UndoLatestAuthorAction {
                input: input.clone(),
            },
        )
        .await
    }
}

/// One Author Undo of the latest Forward Author Action.
#[derive(Clone)]
pub(crate) struct UndoLatestAuthorAction {
    pub(crate) input: UndoLatestAuthorActionInput,
}

impl ProjectCommand for UndoLatestAuthorAction {
    const SPEC: CommandSpec = CommandSpec {
        kind: "undoLatestAuthorAction",
        applied: &[
            AppliedVariant::Compensation("authoritative_applied"),
            AppliedVariant::Compensation("draft_closure_changed"),
            AppliedVariant::Forward(ForwardCommand::UndoReversalRequired),
        ],
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The compensation adapters write their own Activity records.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(replay::UNDO_REPLAY_EFFECT),
    };
    type Error = ProjectCommandError;
    type Profile = UndoCompensation;
    type Response = ProjectResponse;
    type ZeroEffect = AuthorUndoFrontierPosition;
    type Applied = ();
    type Plan = UndoPlan;
    type Effect = ();
    type NoEffect = Infallible;
    type Conflict = UndoLatestAuthorActionConflict;
    type Refusal = UndoLatestAuthorActionUnavailable;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        match project.lifecycle {
            ProjectLifecycle::Active => {}
            ProjectLifecycle::Archived => return Err(ProjectCommandError::BindingConflict),
        }
        let request = UndoRequest::new(envelope, &self.input);
        let observed = load_observed_frontier(client, &request).await?;
        let retry_source = match &observed {
            Some(frontier @ (ObservedFrontier::Prose(_) | ObservedFrontier::Proposal(_))) => {
                crate::close_editor_flow_draft::load_frontier(client, &request, frontier.sequence())
                    .await?
            }
            _ => None,
        };
        let outcome = classify_undo(&CoreUndo {
            expected_author_undo_frontier_sequence: request.expected_author_undo_frontier_sequence,
            current_author_undo_frontier: observed.as_ref().map(|frontier| AuthorUndoFrontier {
                sequence: frontier.sequence(),
                kind: retry_source
                    .as_ref()
                    .filter(|source| source.kind != AuthorUndoFrontierKind::ReversibleDraftClose)
                    .map_or_else(|| frontier.kind(), |source| source.kind.clone()),
            }),
            expected_head_revision_id: request.expected_authoritative_revision_id.clone(),
            current_head_revision_id: observed
                .as_ref()
                .and_then(ObservedFrontier::prose_head)
                .unwrap_or_default()
                .to_owned(),
        });
        let admission = undo_admission(client, &request, observed.as_ref()).await?;
        let expected = request.expected_authoritative_revision_id.clone();
        let head = observed
            .as_ref()
            .and_then(ObservedFrontier::prose_head)
            .unwrap_or(expected.as_str())
            .to_owned();
        let zero_heads = ReceiptHeads {
            expected: vec![expected.clone()],
            prior: vec![head.clone()],
            resulting: vec![head],
        };
        let frontier = AuthorUndoFrontierPosition {
            current_author_undo_frontier_sequence: observed
                .as_ref()
                .map(ObservedFrontier::sequence),
        };
        let outcome = match outcome {
            TransitionOutcome::Applied(applied) => {
                match UndoPlan::new(client, &request, applied, observed).await? {
                    Some(plan) => TransitionOutcome::Applied(((), plan)),
                    None => TransitionOutcome::Refused(
                        UndoLatestAuthorActionUnavailable::SourceUnavailable,
                    ),
                }
            }
            TransitionOutcome::NoEffect(reason) => match reason {},
            TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
        };
        let heads = match &outcome {
            TransitionOutcome::Applied(((), plan)) => plan.heads(&expected),
            TransitionOutcome::NoEffect(_)
            | TransitionOutcome::Conflicted(_)
            | TransitionOutcome::Refused(_) => zero_heads,
        };
        Ok(Classification {
            outcome,
            admission,
            heads,
            zero_receipt: ZeroReceipt::Observed {
                fields: serde_json::Map::new(),
                refs: ReceiptRefs::default(),
                effect: frontier,
            },
        })
    }

    fn applied_variant(&self, (): &(), plan: &UndoPlan) -> AppliedVariant {
        plan.variant()
    }

    fn applied_receipt_payload(&self, (): &(), plan: &UndoPlan) -> String {
        serde_json::Value::Object(plan.receipt_payload()).to_string()
    }

    fn applied_receipt_refs(&self, (): &(), plan: &UndoPlan) -> ReceiptRefs {
        let (mut draft_artifact_refs, mut artifact_lifecycle_event_refs): (Vec<_>, Vec<_>) =
            plan.receipt_draft().into_iter().unzip();
        let created_at = match &plan.source_reopen {
            Some(source) => {
                draft_artifact_refs.push(source.draft_id.clone());
                artifact_lifecycle_event_refs.push(source.reopen_event_id.clone());
                ReceiptTime::Transaction
            }
            None => ReceiptTime::Clock,
        };
        ReceiptRefs {
            draft_artifact_refs,
            artifact_lifecycle_event_refs,
            created_at,
            ..ReceiptRefs::default()
        }
    }

    async fn apply(
        &self,
        _client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ProfileSequences<Self>,
        plan: UndoPlan,
        (): (),
    ) -> Result<UndoWrite<()>, ProjectCommandError> {
        Ok(UndoWrite {
            effect: (),
            request: UndoRequest::new(envelope, &self.input),
            plan,
        })
    }

    fn decode(&self, _replay: &CommandReplay) -> Result<(), ReplayFault> {
        Ok(())
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        match outcome {
            ZeroOutcome::Refused(_) => ZeroAuthorityRows::Effect,
            ZeroOutcome::NoEffect(_) | ZeroOutcome::Conflicted(_) => ZeroAuthorityRows::None,
        }
    }

    /// An unavailable Undo of an Acceptance writes its child Receipt. The frontier is the one
    /// that classification observed, because the transaction holds the Project lock.
    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<AuthorUndoFrontierPosition, ProjectCommandError> {
        let request = UndoRequest::new(envelope, &self.input);
        let observed = load_observed_frontier(client, &request).await?;
        if let Some(ObservedFrontier::Acceptance(loaded)) = &observed {
            crate::accept_proposal::record_unavailable(client, &request, loaded).await?;
        }
        Ok(AuthorUndoFrontierPosition {
            current_author_undo_frontier_sequence: observed
                .as_ref()
                .map(ObservedFrontier::sequence),
        })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<AuthorUndoFrontierPosition>, ReplayFault> {
        Ok(Some(AuthorUndoFrontierPosition {
            current_author_undo_frontier_sequence: replay::frontier_as_of_receipt(replay)?,
        }))
    }
}

/// The Admission of an Author Undo: the Chapter and the expected Revision of its frontier.
async fn undo_admission(
    client: &Client,
    request: &UndoRequest,
    observed: Option<&ObservedFrontier>,
) -> Result<Admission, ProjectCommandError> {
    let expected = request.expected_authoritative_revision_id.as_str();
    let session_chapter = crate::undo_frontier::editor_session_chapter(client, request).await?;
    let (chapter, expected_revision) = match observed {
        Some(ObservedFrontier::Acceptance(frontier)) => {
            (Some(frontier.chapter_id.as_str()), Some(expected))
        }
        Some(ObservedFrontier::Prose(frontier)) => (
            Some(frontier.chapter_id.as_str()),
            Some(frontier.resulting_revision_id.as_str()),
        ),
        Some(ObservedFrontier::Proposal(frontier)) => {
            (Some(frontier.chapter_id.as_str()), Some(expected))
        }
        Some(ObservedFrontier::AuthorWithdrawal(frontier)) => {
            (Some(frontier.chapter_id.as_str()), Some(expected))
        }
        Some(
            ObservedFrontier::Replan(decision)
            | ObservedFrontier::ReopenWithdrawnProposal(decision),
        ) => (Some(decision.chapter_id.as_str()), Some(expected)),
        Some(ObservedFrontier::ReopenRejectedOperations(frontier)) => {
            (Some(frontier.decision.chapter_id.as_str()), Some(expected))
        }
        Some(
            ObservedFrontier::Structure(_)
            | ObservedFrontier::CurrentChapter(_)
            | ObservedFrontier::DraftClose(_)
            | ObservedFrontier::Barrier { .. },
        )
        | None => match session_chapter.as_deref() {
            Some(chapter) if !expected.is_empty() => (Some(chapter), Some(expected)),
            Some(_) | None => (None, None),
        },
    };
    Ok(Admission::ExplicitEditorCommand(EditorAdmission {
        editor_session_id: request.editor_session_id.as_ref().to_owned(),
        chapter_object_id: chapter.map(str::to_owned),
        expected_authoritative_revision_id: expected_revision.map(str::to_owned),
        target_refs: Vec::new(),
        writer: EditorWriter::Current,
    }))
}

/// The `created_at` text of the Undo Receipt of `command`, as the Receipt records it.
pub(crate) async fn receipt_created_at(
    client: &Client,
    command: &UndoRequest,
) -> Result<String, ProjectCommandError> {
    Ok(client
        .query_one(
            "SELECT to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
               FROM storyos.domain_receipts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND receipt_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .get(/*idx*/ 0))
}

pub(crate) fn undo_database_error(error: tokio_postgres::Error) -> ProjectCommandError {
    unavailable(error)
}

pub(crate) fn undo_from_author_edit(
    error: storyos_application::AuthorEditError,
) -> ProjectCommandError {
    unavailable(error)
}

pub(crate) fn undo_from_session(error: EditorSessionError) -> ProjectCommandError {
    match error {
        EditorSessionError::BindingConflict | EditorSessionError::InvalidChallenge => {
            ProjectCommandError::BindingConflict
        }
        EditorSessionError::Unavailable(source) => ProjectCommandError::Unavailable(source),
    }
}
