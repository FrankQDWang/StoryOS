use std::convert::Infallible;

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProposalOperationsRejected,
    RejectProposalOperationsInput, RejectProposalOperationsSettlement, RejectionNote,
};
use storyos_core::{
    ProposalBundlePolicy, ProposalOperationSelection, ProposalSelectionIntent, ReceiptResult,
    RejectProposalOperations as CoreReject, RejectProposalOperationsConflict,
    RejectProposalOperationsRefusal, classify_proposal_selection, reject_proposal_operations,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault, StateAxis};
use crate::command_sequence::{
    ActionOnly, ActionSequence, Admission, AppliedVariant, Classification, CommandIsolation,
    CommandSpec, EditorAdmission, EditorWriter, LockedProject, MissingAdmission, ProjectCommand,
    ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect, ZeroAuthorityRows,
    ZeroOutcome, ZeroReceipt, settle_project_command, unavailable,
};
use crate::undo_compensation::ForwardCommand;

impl PostgresProjectReader {
    /// Settles one author rejection of selected pending Proposal Operations.
    pub async fn reject_proposal_operations(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &RejectProposalOperationsInput,
    ) -> Result<RejectProposalOperationsSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The Proposal State Axes of the current Proposal Revision that a rejection preserves.
pub(crate) struct RejectedRevision {
    generation: String,
    validation: String,
    closure: String,
}

/// The outcome that one rejection record keeps.
enum RejectionRecord<'a> {
    /// An applied rejection and the Proposal State Axes that its acknowledgement preserves.
    Applied(&'a RejectedRevision),
    Zero(ReceiptResult),
}

/// Inserts the rejection record that every outcome writes with its Receipt.
async fn insert_rejection_record(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    input: &RejectProposalOperationsInput,
    record: RejectionRecord<'_>,
) -> Result<(), ProjectCommandError> {
    let scope = &envelope.project_scope;
    let selected_operation_id = input
        .selected_pending_operation_ids
        .first()
        .ok_or(ProjectCommandError::BindingConflict)?;
    let (result, rejection_reason, preserved) = match record {
        RejectionRecord::Applied(rejected) => (
            RejectProposalOperationsInput::SPEC.applied.result_kind(),
            Some("author_declined"),
            Some(rejected),
        ),
        RejectionRecord::Zero(result) => (result.code(), None, None),
    };
    let rejection_note = match &input.rejection_note {
        RejectionNote::Omitted => None,
        RejectionNote::Present { text } => Some(text.as_str()),
    };
    client
        .execute(
            "INSERT INTO storyos.proposal_rejection_receipts
               (owner_user_id, project_id, rejection_receipt_id, proposal_id,
                proposal_revision_id, selected_operation_id, result, rejection_reason,
                rejection_note, preserved_generation, preserved_validation, preserved_closure)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7, $8, $9, $10, $11, $12)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &input.proposal_id,
                &input.proposal_revision_id,
                selected_operation_id,
                &result,
                &rejection_reason,
                &rejection_note,
                &preserved.map(|rejected| rejected.generation.as_str()),
                &preserved.map(|rejected| rejected.validation.as_str()),
                &preserved.map(|rejected| rejected.closure.as_str()),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

impl ProjectCommand for RejectProposalOperationsInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "rejectProposalOperations",
        applied: AppliedVariant::Forward(ForwardCommand::RejectProposalOperations),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'resolution_event_id', resolution.resolution_event_id::text,
                      'preserved_generation', rejection.preserved_generation,
                      'preserved_validation', rejection.preserved_validation,
                      'preserved_closure', rejection.preserved_closure)::text
               FROM storyos.proposal_rejection_receipts AS rejection
               JOIN storyos.proposal_operation_resolutions AS resolution
                 ON (resolution.owner_user_id, resolution.project_id,
                     resolution.rejection_receipt_id, resolution.operation_id) =
                    (rejection.owner_user_id, rejection.project_id,
                     rejection.rejection_receipt_id, rejection.selected_operation_id)
              WHERE rejection.owner_user_id = $1::text::uuid
                AND rejection.project_id = $2::text::uuid
                AND rejection.rejection_receipt_id = $3::text::uuid",
        ),
    };
    type Error = ProjectCommandError;
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = ();
    type Plan = RejectedRevision;
    type Effect = ProposalOperationsRejected;
    type NoEffect = Infallible;
    type Conflict = RejectProposalOperationsConflict;
    type Refusal = RejectProposalOperationsRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let row = client
            .query_opt(
                "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                        revision.closure, proposal.chapter_id::text,
                        chapter_head.current_revision_id::text, proposal.bundle_policy
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id)
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
                ],
            )
            .await
            .map_err(unavailable)?
            .ok_or(ProjectCommandError::MissingProject)?;
        let operations = client
            .query(
                "SELECT operation_id::text, resolution,
                        COALESCE(predecessor_operation_ids::text[], '{}')
                   FROM storyos.proposal_operations
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                  ORDER BY operation_id",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .into_iter()
            .map(|operation| ProposalOperationSelection {
                operation_id: operation.get(/*idx*/ 0),
                resolution: operation.get(/*idx*/ 1),
                predecessor_operation_ids: operation.get(/*idx*/ 2),
            })
            .collect::<Vec<_>>();
        let selection = classify_proposal_selection(
            &self.selected_pending_operation_ids,
            &operations,
            ProposalBundlePolicy::from_stored(&row.get::<_, String>(/*idx*/ 6)),
            ProposalSelectionIntent::Reject,
        );
        let rejected = RejectedRevision {
            generation: row.get(/*idx*/ 1),
            validation: row.get(/*idx*/ 2),
            closure: row.get(/*idx*/ 3),
        };
        let outcome = reject_proposal_operations(&CoreReject {
            scope_matches: true,
            admission_valid: true,
            proposal_revision_current: row.get::<_, String>(/*idx*/ 0) == self.proposal_revision_id,
            closure_open: rejected.closure == "open",
            selected_operations_pending: selection.all_selected_pending,
            selection_duplicate_free: selection.duplicate_free,
            required_dependencies_met: selection.required_dependencies_met,
            bundle_closure_complete: selection.bundle_closure_complete,
            expected_target_matches_head: row.get::<_, Option<String>>(/*idx*/ 5).as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        let head = vec![self.expected_authoritative_revision_id.clone()];
        Ok(Classification {
            outcome: outcome.map_applied(|()| ((), rejected)),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(row.get(/*idx*/ 4)),
                expected_authoritative_revision_id: Some(
                    self.expected_authoritative_revision_id.clone(),
                ),
                target_refs: Vec::new(),
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

    fn applied_receipt_payload(&self, _applied: &(), _plan: &RejectedRevision) -> String {
        r#"{"rejection_reason":"author_declined"}"#.to_owned()
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        ActionSequence(author_action_sequence): &ActionSequence,
        rejected: RejectedRevision,
        _applied: (),
    ) -> Result<ProposalOperationsRejected, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let updated = client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET resolution = 'rejected', reservation_state = 'resolved'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($4::text[]::uuid[])
                    AND resolution = 'pending'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &self.selected_pending_operation_ids,
                ],
            )
            .await
            .map_err(unavailable)?;
        if usize::try_from(updated).ok() != Some(self.selected_pending_operation_ids.len()) {
            return Err(ProjectCommandError::BindingConflict);
        }
        insert_rejection_record(client, envelope, self, RejectionRecord::Applied(&rejected))
            .await?;
        let mut resolution_event_id = None;
        for operation_id in &self.selected_pending_operation_ids {
            let event_id = Uuid::now_v7().to_string();
            client
                .execute(
                    "INSERT INTO storyos.proposal_operation_resolutions
                       (owner_user_id, project_id, resolution_event_id, proposal_id,
                        proposal_revision_id, operation_id, prior_resolution,
                        resulting_resolution, rejection_receipt_id, author_action_sequence)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                             $5::text::uuid, $6::text::uuid, 'pending', 'rejected',
                             $7::text::uuid, $8::text::numeric)",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &event_id,
                        &self.proposal_id,
                        &self.proposal_revision_id,
                        operation_id,
                        &envelope.ids.receipt_id,
                        &author_action_sequence.to_string(),
                    ],
                )
                .await
                .map_err(unavailable)?;
            resolution_event_id.get_or_insert(event_id);
        }
        let resolution_event_id =
            resolution_event_id.ok_or(ProjectCommandError::BindingConflict)?;
        Ok(ProposalOperationsRejected {
            preserved_generation: rejected.generation,
            preserved_validation: rejected.validation,
            preserved_closure: rejected.closure,
            resolution_event_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProposalOperationsRejected, ReplayFault> {
        replay.require_receipt_text("rejection_reason", "author_declined")?;
        match (
            replay.effect_text("resolution_event_id")?,
            replay.effect_text("preserved_generation")?,
            replay.effect_text("preserved_validation")?,
            replay.effect_text("preserved_closure")?,
        ) {
            (
                Some(resolution_event_id),
                Some(preserved_generation),
                Some(preserved_validation),
                Some(preserved_closure),
            ) => Ok(ProposalOperationsRejected {
                preserved_generation: StateAxis::Generation.preserved(preserved_generation)?,
                preserved_validation: StateAxis::Validation.preserved(preserved_validation)?,
                preserved_closure: StateAxis::Closure.preserved(preserved_closure)?,
                resolution_event_id,
            }),
            (Some(_), None, None, None) => Err(ReplayFault::HistoricalAcknowledgementUnavailable),
            _ => Err(ReplayFault::Unavailable(
                "the applied rejection record is missing or damaged".into(),
            )),
        }
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        match outcome {
            ZeroOutcome::NoEffect(reason) => match **reason {},
            ZeroOutcome::Conflicted(_) | ZeroOutcome::Refused(_) => ZeroAuthorityRows::Effect,
        }
    }

    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<(), ProjectCommandError> {
        let result = match outcome {
            ZeroOutcome::NoEffect(reason) => match **reason {},
            ZeroOutcome::Conflicted(_) => ReceiptResult::Conflicted,
            ZeroOutcome::Refused(_) => ReceiptResult::Refused,
        };
        insert_rejection_record(client, envelope, self, RejectionRecord::Zero(result)).await
    }

    fn decode_zero_authority_effect(
        &self,
        _replay: &CommandReplay,
    ) -> Result<Option<()>, ReplayFault> {
        Ok(Some(()))
    }
}
