use storyos_application::{
    ActionApplied, ProjectCommandEnvelope, ProjectCommandError, ProposalWithdrawn,
    WithdrawProposalInput, WithdrawProposalSettlement, WithdrawalNote,
};
use storyos_core::{
    WithdrawProposal as CoreWithdraw, WithdrawProposalConflict, WithdrawProposalNoEffect,
    WithdrawProposalRefusal, WithdrawalCause, withdraw_proposal,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault, StateAxis};
use crate::command_sequence::{
    ActionOnly, Admission, AppliedVariant, Classification, CommandIsolation, CommandSpec,
    EditorAdmission, EditorWriter, LockedProject, MissingAdmission, ProfileSequences,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect, ZeroReceipt,
    settle_project_command, unavailable,
};
use crate::undo_compensation::ForwardCommand;

mod compensation;
pub(crate) use compensation::{AuthorWithdrawalCompensation, ObservedAuthorWithdrawal};

impl PostgresProjectReader {
    /// Settles one author Withdrawal of an open Proposal.
    pub async fn withdraw_proposal(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &WithdrawProposalInput,
    ) -> Result<WithdrawProposalSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The State Axes of the open Proposal Revision that a Withdrawal keeps.
pub(crate) struct OpenRevision {
    generation: String,
    validation: String,
}

impl ProjectCommand for WithdrawProposalInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "withdrawProposal",
        applied: &[AppliedVariant::Forward(ForwardCommand::WithdrawProposal)],
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'withdrawal_event_id', withdrawal_event_id::text,
                      'preserved_generation', preserved_generation,
                      'preserved_validation', preserved_validation)::text
               FROM storyos.proposal_withdrawals
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND withdrawal_receipt_id = $3::text::uuid",
        ),
    };
    type Error = ProjectCommandError;
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    /// The identity of the new Proposal Withdrawal record.
    type Applied = String;
    type Plan = OpenRevision;
    type Effect = ProposalWithdrawn;
    type NoEffect = WithdrawProposalNoEffect;
    type Conflict = WithdrawProposalConflict;
    type Refusal = WithdrawProposalRefusal;

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
                        chapter_head.current_revision_id::text
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
        let chapter_object_id = row
            .get::<_, Option<String>>(/*idx*/ 4)
            .ok_or(ProjectCommandError::MissingProject)?;
        let closure = row.get::<_, String>(/*idx*/ 3);
        let outcome = withdraw_proposal(&CoreWithdraw {
            scope_matches: true,
            cause: WithdrawalCause::Author,
            admission_valid: true,
            producer_matches: false,
            proposal_revision_current: row.get::<_, String>(/*idx*/ 0) == self.proposal_revision_id,
            closure_open: closure == "open",
            terminal_supersession: closure == "superseded",
            expected_target_matches_head: row.get::<_, Option<String>>(/*idx*/ 5).as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        let open = OpenRevision {
            generation: row.get(/*idx*/ 1),
            validation: row.get(/*idx*/ 2),
        };
        let head = vec![self.expected_authoritative_revision_id.clone()];
        Ok(Classification {
            outcome: outcome.map_applied(|()| (Uuid::now_v7().to_string(), open)),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(chapter_object_id),
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

    fn applied_receipt_payload(&self, _applied: &String, _plan: &OpenRevision) -> String {
        r#"{"transition":"withdraw"}"#.to_owned()
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequence: &ProfileSequences<Self>,
        open: OpenRevision,
        withdrawal_event_id: String,
    ) -> Result<ProposalWithdrawn, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let updated = client
            .execute(
                "UPDATE storyos.proposal_revisions
                    SET closure = 'withdrawn'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND revision_id = $4::text::uuid
                    AND closure = 'open'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &self.proposal_revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        Ok(ProposalWithdrawn {
            preserved_generation: open.generation,
            preserved_validation: open.validation,
            withdrawal_event_id,
        })
    }

    async fn apply_after_authority(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        applied: &ActionApplied<ProposalWithdrawn>,
    ) -> Result<(), ProjectCommandError> {
        let scope = &envelope.project_scope;
        let note = match &self.withdrawal_note {
            WithdrawalNote::Omitted => None,
            WithdrawalNote::Present { text } => Some(text.as_str()),
        };
        client
            .execute(
                "INSERT INTO storyos.proposal_withdrawals
                   (owner_user_id, project_id, withdrawal_event_id, proposal_id,
                    proposal_revision_id, withdrawal_reason, author_note,
                    withdrawal_receipt_id, author_action_sequence, preserved_generation,
                    preserved_validation)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, 'author_withdrew', $6, $7::text::uuid,
                         $8::text::numeric, $9, $10)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &applied.effect.withdrawal_event_id,
                    &self.proposal_id,
                    &self.proposal_revision_id,
                    &note,
                    &envelope.ids.receipt_id,
                    &applied.author_action_sequence.to_string(),
                    &applied.effect.preserved_generation,
                    &applied.effect.preserved_validation,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProposalWithdrawn, ReplayFault> {
        replay.require_receipt_text("transition", "withdraw")?;
        match (
            replay.effect_text("withdrawal_event_id")?,
            replay.effect_text("preserved_generation")?,
            replay.effect_text("preserved_validation")?,
        ) {
            (Some(withdrawal_event_id), Some(preserved_generation), Some(preserved_validation)) => {
                Ok(ProposalWithdrawn {
                    preserved_generation: StateAxis::Generation.preserved(preserved_generation)?,
                    preserved_validation: StateAxis::Validation.preserved(preserved_validation)?,
                    withdrawal_event_id,
                })
            }
            (Some(_), None, None) => Err(ReplayFault::HistoricalAcknowledgementUnavailable),
            _ => Err(ReplayFault::Unavailable(
                "the applied Withdrawal record is missing or damaged".into(),
            )),
        }
    }
}
