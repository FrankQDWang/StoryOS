use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProposalReopened, ReopenWithdrawnProposalInput,
    ReopenWithdrawnProposalSettlement,
};
use storyos_core::{
    ReopenWithdrawnProposal as CoreReopen, ReopenWithdrawnProposalConflict,
    ReopenWithdrawnProposalNoEffect, ReopenWithdrawnProposalRefusal, reopen_withdrawn_proposal,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault, StateAxis};
use crate::command_sequence::{
    ActionOnly, Admission, AppliedVariant, Classification, CommandSpec, EditorAdmission,
    LockedProject, ProfileSequences, ProjectCommand, ProjectResponse, ReceiptHeads, ReceiptRefs,
    ReplayEffect, ZeroReceipt, settle_project_command, unavailable,
};
use crate::undo_compensation::ForwardCommand;

mod compensation;
pub(crate) use compensation::ReopenWithdrawnCompensation;

impl PostgresProjectReader {
    /// Settles one author reopen of a withdrawn Proposal.
    pub async fn reopen_withdrawn_proposal(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ReopenWithdrawnProposalInput,
    ) -> Result<ReopenWithdrawnProposalSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The withdrawn Proposal Revision that a reopen copies into a new pending open revision.
pub(crate) struct WithdrawnRevision {
    revision_id: String,
    generation: String,
    candidate_text: String,
    base_authoritative_revision_id: String,
    operation_resolution: String,
}

/// Whether the Withdrawal event names `head_revision_id` or a Revision that it keeps withdrawn
/// (ADR 0069). A recorded Compensation of a candidate edit, a Replan, or a reopen of rejected
/// Operations keeps the closure of a withdrawn parent. The walk goes from each such appended
/// Revision to its parent.
async fn head_keeps_withdrawal(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    proposal_id: &str,
    head_revision_id: &str,
    withdrawal_event_id: &str,
) -> Result<bool, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let row = client
        .query_one(
            "WITH RECURSIVE kept(revision_id) AS (
                 SELECT $4::text::uuid
               UNION ALL
                 SELECT appended.parent_revision_id
                   FROM kept
                   JOIN storyos.proposal_revisions AS appended
                     ON (appended.owner_user_id, appended.project_id, appended.proposal_id,
                         appended.revision_id) =
                        ($1::text::uuid, $2::text::uuid, $3::text::uuid, kept.revision_id)
                   JOIN storyos.proposal_revisions AS parent
                     ON (parent.owner_user_id, parent.project_id, parent.proposal_id,
                         parent.revision_id) =
                        (appended.owner_user_id, appended.project_id, appended.proposal_id,
                         appended.parent_revision_id)
                    AND parent.closure = 'withdrawn'
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id) =
                        (appended.owner_user_id, appended.project_id)
                    AND receipt.result_payload->>'proposal_revision_id' =
                        appended.revision_id::text
                    AND receipt.result_payload->>'source_proposal_revision_id' =
                        appended.parent_revision_id::text
                   JOIN storyos.author_action_entries AS compensation
                     ON (compensation.owner_user_id, compensation.project_id,
                         compensation.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                    AND compensation.disposition = 'compensation'
                   JOIN storyos.author_action_entries AS source
                     ON (source.owner_user_id, source.project_id, source.author_action_sequence) =
                        (compensation.owner_user_id, compensation.project_id,
                         compensation.compensated_source_sequence)
                    AND source.disposition = 'forward'
                   JOIN storyos.domain_receipts AS source_receipt
                     ON (source_receipt.owner_user_id, source_receipt.project_id,
                         source_receipt.receipt_id) =
                        (source.owner_user_id, source.project_id, source.receipt_id)
                    AND source_receipt.command_kind IN
                          ('applyAuthorEdit', 'replanProposal', 'reopenRejectedOperations')
             ) CYCLE revision_id SET looped USING path
             SELECT EXISTS (
               SELECT 1
                 FROM kept
                 JOIN storyos.proposal_withdrawals AS withdrawal
                   ON (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id,
                       withdrawal.proposal_revision_id) =
                      ($1::text::uuid, $2::text::uuid, $3::text::uuid, kept.revision_id)
                WHERE NOT kept.looped
                  AND withdrawal.withdrawal_event_id = $5::text::uuid
             )",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
                &head_revision_id,
                &withdrawal_event_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(row.get(/*idx*/ 0))
}

impl ProjectCommand for ReopenWithdrawnProposalInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "reopenWithdrawnProposal",
        applied: &[AppliedVariant::Forward(
            ForwardCommand::ReopenWithdrawnProposal,
        )],
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'resulting_proposal_revision_id', resulting_proposal_revision_id::text,
                      'preserved_generation', preserved_generation,
                      'preserved_operation_resolution', preserved_operation_resolution)::text
               FROM storyos.proposal_withdrawal_reopenings
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND reopen_receipt_id = $3::text::uuid",
        ),
    };
    type Error = ProjectCommandError;
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    /// The identity of the new pending open Proposal Revision.
    type Applied = String;
    type Plan = WithdrawnRevision;
    type Effect = ProposalReopened;
    type NoEffect = ReopenWithdrawnProposalNoEffect;
    type Conflict = ReopenWithdrawnProposalConflict;
    type Refusal = ReopenWithdrawnProposalRefusal;

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
                        proposal.chapter_id::text, revision.candidate_text,
                        revision.base_authoritative_revision_id::text,
                        chapter_head.current_revision_id::text,
                        (
                          SELECT operation.resolution
                            FROM storyos.proposal_operations AS operation
                           WHERE operation.owner_user_id = proposal.owner_user_id
                             AND operation.project_id = proposal.project_id
                             AND operation.proposal_id = proposal.proposal_id
                           ORDER BY operation.operation_id
                           LIMIT 1
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
        let operation_resolution = row
            .get::<_, Option<String>>(/*idx*/ 7)
            .ok_or(ProjectCommandError::MissingProject)?;
        let current_revision_id = row.get::<_, String>(/*idx*/ 0);
        let closure = row.get::<_, String>(/*idx*/ 2);
        let outcome = reopen_withdrawn_proposal(&CoreReopen {
            scope_matches: true,
            admission_valid: true,
            proposal_revision_current: current_revision_id == self.proposal_revision_id,
            closure_withdrawn: closure == "withdrawn",
            terminal_supersession: closure == "superseded",
            withdrawal_event_matches: head_keeps_withdrawal(
                client,
                envelope,
                &self.proposal_id,
                &current_revision_id,
                &self.withdrawal_event_id,
            )
            .await?,
            expected_target_matches_head: row.get::<_, Option<String>>(/*idx*/ 6).as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        let withdrawn = WithdrawnRevision {
            revision_id: current_revision_id,
            generation: row.get(/*idx*/ 1),
            candidate_text: row.get(/*idx*/ 4),
            base_authoritative_revision_id: row.get(/*idx*/ 5),
            operation_resolution,
        };
        let head = vec![self.expected_authoritative_revision_id.clone()];
        Ok(Classification {
            outcome: outcome.map_applied(|()| (Uuid::now_v7().to_string(), withdrawn)),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(row.get(/*idx*/ 3)),
                expected_authoritative_revision_id: Some(
                    self.expected_authoritative_revision_id.clone(),
                ),
                target_refs: Vec::new(),
            }),
            heads: ReceiptHeads {
                expected: head.clone(),
                prior: head.clone(),
                resulting: head,
            },
            zero_receipt: ZeroReceipt::Reason,
        })
    }

    fn applied_receipt_payload(&self, _applied: &String, _plan: &WithdrawnRevision) -> String {
        r#"{"transition":"reopen_withdrawn"}"#.to_owned()
    }

    fn applied_receipt_refs(&self, applied: &String, _plan: &WithdrawnRevision) -> ReceiptRefs {
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
        withdrawn: WithdrawnRevision,
        resulting_proposal_revision_id: String,
    ) -> Result<ProposalReopened, ProjectCommandError> {
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.proposal_revisions
                   (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                    closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                         'pending', 'open', $6, $7::text::uuid, $8::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &resulting_proposal_revision_id,
                    &withdrawn.generation,
                    &withdrawn.candidate_text,
                    &withdrawn.base_authoritative_revision_id,
                    &withdrawn.revision_id,
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
                    &withdrawn.revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if head_updates != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        client
            .execute(
                "INSERT INTO storyos.proposal_withdrawal_reopenings
                   (owner_user_id, project_id, reopen_event_id, proposal_id,
                    source_proposal_revision_id, resulting_proposal_revision_id,
                    withdrawal_event_id, preserved_generation, preserved_operation_resolution,
                    reopen_receipt_id, author_action_sequence)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7::text::uuid, $8, $9,
                         $10::text::uuid, $11::text::numeric)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &self.proposal_id,
                    &withdrawn.revision_id,
                    &resulting_proposal_revision_id,
                    &self.withdrawal_event_id,
                    &withdrawn.generation,
                    &withdrawn.operation_resolution,
                    &envelope.ids.receipt_id,
                    &sequence.0.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(ProposalReopened {
            resulting_proposal_revision_id,
            preserved_generation: withdrawn.generation,
            preserved_operation_resolution: withdrawn.operation_resolution,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProposalReopened, ReplayFault> {
        replay.require_receipt_text("transition", "reopen_withdrawn")?;
        match (
            replay.effect_text("resulting_proposal_revision_id")?,
            replay.effect_text("preserved_generation")?,
            replay.effect_text("preserved_operation_resolution")?,
        ) {
            (
                Some(resulting_proposal_revision_id),
                Some(preserved_generation),
                Some(preserved_operation_resolution),
            ) => Ok(ProposalReopened {
                resulting_proposal_revision_id,
                preserved_generation: StateAxis::Generation.preserved(preserved_generation)?,
                preserved_operation_resolution: StateAxis::OperationResolution
                    .preserved(preserved_operation_resolution)?,
            }),
            (Some(_), None, None) => Err(ReplayFault::HistoricalAcknowledgementUnavailable),
            _ => Err(ReplayFault::Unavailable(
                "the applied reopening record is missing or damaged".into(),
            )),
        }
    }
}
