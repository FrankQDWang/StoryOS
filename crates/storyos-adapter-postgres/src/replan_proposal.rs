use std::convert::Infallible;

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProposalReplanned, ReplanProposalInput,
    ReplanProposalSettlement,
};
use storyos_contracts::ReplanSourceCondition;
use storyos_core::{
    ReplanProposal as CoreReplan, ReplanProposalConflict, ReplanProposalRefusal, replan_proposal,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault, StateAxis};
use crate::command_sequence::{
    ActionOnly, Admission, AppliedResult, Classification, CommandIsolation, CommandSpec,
    EditorAdmission, EditorWriter, LockedProject, MissingAdmission, ProfileSequences,
    ProjectCommand, ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReceiptRefs, ReplayEffect,
    ZeroReceipt, settle_project_command, unavailable,
};

mod compensation;
pub(crate) use compensation::ReplanCompensation;

impl PostgresProjectReader {
    /// Settles one author Replan of a conflicted Proposal.
    pub async fn replan_proposal(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ReplanProposalInput,
    ) -> Result<ReplanProposalSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The conflicted Proposal Revision that a Replan copies into a new pending revision.
pub(crate) struct ConflictedRevision {
    revision_id: String,
    generation: String,
    closure: String,
    candidate_text: String,
    candidate_blocks: Option<String>,
}

fn source_condition_parts(condition: &ReplanSourceCondition) -> (&'static str, &str) {
    match condition {
        ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref,
        } => ("proposal_conflict", proposal_conflict_ref.as_str()),
        ReplanSourceCondition::ProposalRecoveryConflict {
            proposal_recovery_conflict_ref,
        } => (
            "proposal_recovery_conflict",
            proposal_recovery_conflict_ref.as_str(),
        ),
    }
}

impl ProjectCommand for ReplanProposalInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "replanProposal",
        applied_result: AppliedResult::command("proposal_revised"),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'state_event_id', replan_event_id::text,
                      'resulting_proposal_revision_id', resulting_proposal_revision_id::text,
                      'preserved_generation', preserved_generation,
                      'preserved_closure', preserved_closure)::text
               FROM storyos.proposal_replans
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND replan_receipt_id = $3::text::uuid",
        ),
    };
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    /// The identity of the new pending Proposal Revision.
    type Applied = String;
    type Plan = ConflictedRevision;
    type Effect = ProposalReplanned;
    type NoEffect = Infallible;
    type Conflict = ReplanProposalConflict;
    type Refusal = ReplanProposalRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let (condition_kind, condition_ref) = source_condition_parts(&self.source_condition);
        let row = client
            .query_opt(
                "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                        proposal.chapter_id::text, revision.candidate_text,
                        revision.candidate_blocks::text,
                        chapter_head.current_revision_id::text,
                        EXISTS (
                          SELECT 1
                            FROM storyos.proposal_validation_conditions AS condition
                           WHERE (condition.owner_user_id, condition.project_id,
                                  condition.proposal_id, condition.proposal_revision_id) =
                                 (revision.owner_user_id, revision.project_id,
                                  revision.proposal_id, revision.revision_id)
                             AND condition.validation = 'conflicted'
                             AND condition.condition_kind = $4
                             AND condition.conflict_id = $5::text::uuid
                        ),
                        EXISTS (
                          SELECT 1
                            FROM storyos.proposal_operations AS operation
                           WHERE (operation.owner_user_id, operation.project_id,
                                  operation.proposal_id) =
                                 (proposal.owner_user_id, proposal.project_id,
                                  proposal.proposal_id)
                             AND operation.operation_id = $6::text::uuid
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
                    &condition_kind,
                    &condition_ref,
                    &self.replacement_operation_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .ok_or(ProjectCommandError::MissingProject)?;
        let current_revision_id = row.get::<_, String>(/*idx*/ 0);
        let closure = row.get::<_, String>(/*idx*/ 2);
        let outcome = replan_proposal(&CoreReplan {
            scope_matches: true,
            admission_valid: true,
            proposal_revision_current: current_revision_id == self.conflicted_proposal_revision_id,
            expected_head_current: current_revision_id == self.expected_current_proposal_head,
            closure_open: closure == "open",
            source_condition_matches: row.get(/*idx*/ 7),
            replacement_operations_preserve_identity: row.get(/*idx*/ 8),
            expected_target_matches_head: row.get::<_, Option<String>>(/*idx*/ 6).as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        let conflicted = ConflictedRevision {
            revision_id: current_revision_id,
            generation: row.get(/*idx*/ 1),
            closure,
            candidate_text: row.get(/*idx*/ 4),
            candidate_blocks: row.get(/*idx*/ 5),
        };
        let head = vec![self.expected_authoritative_revision_id.clone()];
        Ok(Classification {
            outcome: outcome.map_applied(|()| (Uuid::now_v7().to_string(), conflicted)),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(row.get(/*idx*/ 3)),
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

    fn applied_receipt_payload(&self, _applied: &String, _plan: &ConflictedRevision) -> String {
        r#"{"transition":"replan"}"#.to_owned()
    }

    fn applied_receipt_refs(&self, applied: &String, _plan: &ConflictedRevision) -> ReceiptRefs {
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
        conflicted: ConflictedRevision,
        resulting_proposal_revision_id: String,
    ) -> Result<ProposalReplanned, ProjectCommandError> {
        let scope = &envelope.project_scope;
        // A Resolved Replan matched the Chapter head to the expected revision.
        client
            .execute(
                "INSERT INTO storyos.proposal_revisions
                   (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                    closure, candidate_text, candidate_blocks, base_authoritative_revision_id,
                    parent_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                         'pending', $6, $7, $8::text::jsonb, $9::text::uuid, $10::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &resulting_proposal_revision_id,
                    &conflicted.generation,
                    &conflicted.closure,
                    &conflicted.candidate_text,
                    &conflicted.candidate_blocks,
                    &self.expected_authoritative_revision_id,
                    &conflicted.revision_id,
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
                    &conflicted.revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if head_updates != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let state_event_id = Uuid::now_v7().to_string();
        let (condition_kind, condition_ref) = source_condition_parts(&self.source_condition);
        client
            .execute(
                "INSERT INTO storyos.proposal_replans
                   (owner_user_id, project_id, replan_event_id, proposal_id,
                    source_proposal_revision_id, resulting_proposal_revision_id,
                    source_condition_kind, source_condition_ref, replan_receipt_id,
                    author_action_sequence, preserved_generation, preserved_closure)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7, $8::text::uuid, $9::text::uuid,
                         $10::text::numeric, $11, $12)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &state_event_id,
                    &self.proposal_id,
                    &self.conflicted_proposal_revision_id,
                    &resulting_proposal_revision_id,
                    &condition_kind,
                    &condition_ref,
                    &envelope.ids.receipt_id,
                    &sequence.0.to_string(),
                    &conflicted.generation,
                    &conflicted.closure,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(ProposalReplanned {
            resulting_proposal_revision_id,
            preserved_generation: conflicted.generation,
            preserved_closure: conflicted.closure,
            state_event_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProposalReplanned, ReplayFault> {
        replay.require_receipt_text("transition", "replan")?;
        match (
            replay.effect_text("state_event_id")?,
            replay.effect_text("resulting_proposal_revision_id")?,
            replay.effect_text("preserved_generation")?,
            replay.effect_text("preserved_closure")?,
        ) {
            (
                Some(state_event_id),
                Some(resulting_proposal_revision_id),
                Some(preserved_generation),
                Some(preserved_closure),
            ) => Ok(ProposalReplanned {
                resulting_proposal_revision_id,
                preserved_generation: StateAxis::Generation.preserved(preserved_generation)?,
                preserved_closure: StateAxis::Closure.preserved(preserved_closure)?,
                state_event_id,
            }),
            (Some(_), Some(_), None, None) => {
                Err(ReplayFault::HistoricalAcknowledgementUnavailable)
            }
            _ => Err(ReplayFault::Unavailable(
                "the applied replan record is missing or damaged".into(),
            )),
        }
    }
}
