use std::convert::Infallible;

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, RejectedOperationsReopened,
    ReopenRejectedOperationsInput, ReopenRejectedOperationsSettlement,
};
use storyos_core::{
    ReopenRejectedOperations as CoreReopen, ReopenRejectedOperationsConflict,
    ReopenRejectedOperationsRefusal, reopen_rejected_operations,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActionOnly, Admission, AppliedResult, Classification, CommandIsolation, CommandSpec,
    EditorAdmission, EditorWriter, LockedProject, MissingAdmission, ProfileSequences,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReceiptRefs, ReplayEffect,
    ZeroReceipt, settle_project_command, unavailable,
};

impl PostgresProjectReader {
    /// Settles one author reopen of a rejected Proposal Operation.
    pub async fn reopen_rejected_operations(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ReopenRejectedOperationsInput,
    ) -> Result<ReopenRejectedOperationsSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The current Proposal Revision that a reopen copies into a new pending revision.
pub(crate) struct RejectedRevision {
    revision_id: String,
    generation: String,
    closure: String,
    candidate_text: String,
    base_authoritative_revision_id: String,
}

impl ProjectCommand for ReopenRejectedOperationsInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "reopenRejectedOperations",
        applied_result: AppliedResult::command("proposal_revised"),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'state_event_id', reopening.reopen_event_id::text,
                      'resulting_proposal_revision_id',
                      reopening.resulting_proposal_revision_id::text,
                      'preserved_generation', revision.generation,
                      'preserved_closure', revision.closure)::text
               FROM storyos.proposal_operation_reopenings AS reopening
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (reopening.owner_user_id, reopening.project_id, reopening.proposal_id,
                     reopening.resulting_proposal_revision_id)
              WHERE reopening.owner_user_id = $1::text::uuid
                AND reopening.project_id = $2::text::uuid
                AND reopening.reopen_receipt_id = $3::text::uuid",
        ),
    };
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    /// The identity of the new pending Proposal Revision.
    type Applied = String;
    type Plan = RejectedRevision;
    type Effect = RejectedOperationsReopened;
    type NoEffect = Infallible;
    type Conflict = ReopenRejectedOperationsConflict;
    type Refusal = ReopenRejectedOperationsRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let row = client
            .query_opt(
                "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                        proposal.chapter_id::text, operation.resolution, revision.candidate_text,
                        revision.base_authoritative_revision_id::text,
                        chapter_head.current_revision_id::text,
                        EXISTS (
                          SELECT 1
                            FROM storyos.proposal_operation_resolutions AS resolution
                           WHERE resolution.owner_user_id = proposal.owner_user_id
                             AND resolution.project_id = proposal.project_id
                             AND resolution.resolution_event_id = $4::text::uuid
                             AND resolution.proposal_id = proposal.proposal_id
                             AND resolution.operation_id = $5::text::uuid
                             AND resolution.resulting_resolution = 'rejected'
                             AND NOT EXISTS (
                               SELECT 1
                                 FROM storyos.proposal_operation_reopenings AS reopening
                                WHERE reopening.owner_user_id = resolution.owner_user_id
                                  AND reopening.project_id = resolution.project_id
                                  AND reopening.rejection_event_id =
                                      resolution.resolution_event_id
                             )
                        ),
                        NOT EXISTS (
                          SELECT 1
                            FROM storyos.proposal_operations AS reserved
                           WHERE reserved.owner_user_id = proposal.owner_user_id
                             AND reserved.project_id = proposal.project_id
                             AND reserved.manuscript_block_id = operation.manuscript_block_id
                             AND reserved.reservation_state = 'unresolved'
                             AND reserved.proposal_id <> proposal.proposal_id
                        )
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id)
                   LEFT JOIN storyos.proposal_operations AS operation
                     ON (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                    AND operation.operation_id = $5::text::uuid
                   LEFT JOIN storyos.authoritative_heads AS chapter_head
                     ON (chapter_head.owner_user_id, chapter_head.project_id,
                         chapter_head.manuscript_object_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.chapter_id)
                  WHERE proposal.owner_user_id = $1::text::uuid
                    AND proposal.project_id = $2::text::uuid
                    AND proposal.proposal_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &self.rejection_event_id,
                    &self.selected_rejected_operation_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .ok_or(ProjectCommandError::MissingProject)?;
        let current_revision_id = row.get::<_, String>(/*idx*/ 0);
        let closure = row.get::<_, String>(/*idx*/ 2);
        let outcome = reopen_rejected_operations(&CoreReopen {
            scope_matches: true,
            admission_valid: true,
            proposal_revision_current: current_revision_id == self.proposal_revision_id,
            closure_open: closure == "open",
            selected_operations_rejected: row.get::<_, Option<String>>(/*idx*/ 4).as_deref()
                == Some("rejected"),
            rejection_event_matches: row.get(/*idx*/ 8),
            reservation_available: row.get(/*idx*/ 9),
            expected_target_matches_head: row.get::<_, Option<String>>(/*idx*/ 7).as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        let rejected = RejectedRevision {
            revision_id: current_revision_id,
            generation: row.get(/*idx*/ 1),
            closure,
            candidate_text: row.get(/*idx*/ 5),
            base_authoritative_revision_id: row.get(/*idx*/ 6),
        };
        let head = vec![self.expected_authoritative_revision_id.clone()];
        Ok(Classification {
            outcome: outcome.map_applied(|()| (Uuid::now_v7().to_string(), rejected)),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(row.get(/*idx*/ 3)),
                expected_authoritative_revision_id: Some(
                    self.expected_authoritative_revision_id.clone(),
                ),
                writer: EditorWriter::Current,
            }),
            heads: ReceiptHeads {
                expected: head.clone(),
                prior: head.clone(),
                resulting: head,
            },
            zero_receipt: ZeroReceipt::Reason,
        })
    }

    fn applied_receipt_payload(&self, _applied: &String, _plan: &RejectedRevision) -> String {
        r#"{"transition":"reopen_rejected"}"#.to_owned()
    }

    fn applied_receipt_refs(&self, applied: &String, _plan: &RejectedRevision) -> ReceiptRefs {
        ReceiptRefs {
            proposal_revision_ids: vec![applied.clone()],
            ..ReceiptRefs::default()
        }
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        sequence: &ProfileSequences<Self>,
        rejected: RejectedRevision,
        resulting_proposal_revision_id: String,
    ) -> Result<RejectedOperationsReopened, ProjectCommandError> {
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.proposal_revisions
                   (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                    closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                         'pending', $6, $7, $8::text::uuid, $9::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &resulting_proposal_revision_id,
                    &rejected.generation,
                    &rejected.closure,
                    &rejected.candidate_text,
                    &rejected.base_authoritative_revision_id,
                    &rejected.revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        let head_updates = client
            .execute(
                "UPDATE storyos.proposal_heads
                    SET current_revision_id = $4::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &resulting_proposal_revision_id,
                    &rejected.revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if head_updates != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let reopened = client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET resolution = 'pending', reservation_state = 'unresolved'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND operation_id = $4::text::uuid
                    AND resolution = 'rejected'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &self.selected_rejected_operation_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if reopened != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let state_event_id = Uuid::now_v7().to_string();
        client
            .execute(
                "INSERT INTO storyos.proposal_operation_reopenings
                   (owner_user_id, project_id, reopen_event_id, proposal_id,
                    source_proposal_revision_id, resulting_proposal_revision_id, operation_id,
                    rejection_event_id, prior_resolution, resulting_resolution,
                    reopen_receipt_id, author_action_sequence)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                         'rejected', 'pending', $9::text::uuid, $10::text::numeric)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &state_event_id,
                    &self.proposal_id,
                    &self.proposal_revision_id,
                    &resulting_proposal_revision_id,
                    &self.selected_rejected_operation_id,
                    &self.rejection_event_id,
                    &envelope.ids.receipt_id,
                    &sequence.0.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(RejectedOperationsReopened {
            resulting_proposal_revision_id,
            preserved_generation: rejected.generation,
            preserved_closure: rejected.closure,
            state_event_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<RejectedOperationsReopened, ReplayFault> {
        match (
            replay.effect_text("resulting_proposal_revision_id"),
            replay.effect_text("preserved_generation"),
            replay.effect_text("preserved_closure"),
            replay.effect_text("state_event_id"),
        ) {
            (
                Some(resulting_proposal_revision_id),
                Some(preserved_generation),
                Some(preserved_closure),
                Some(state_event_id),
            ) => Ok(RejectedOperationsReopened {
                resulting_proposal_revision_id,
                preserved_generation,
                preserved_closure,
                state_event_id,
            }),
            _ => Err(ReplayFault::HistoricalAcknowledgementUnavailable),
        }
    }
}
