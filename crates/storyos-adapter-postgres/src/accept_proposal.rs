//! `acceptProposal`: one Acceptance of selected pending Proposal Operations through the command
//! sequence (ADR 0076). A refusal before Admission retains its Pre-Admission Refusal Record
//! (ADR 0013).

use storyos_application::{
    AcceptProposalInput, AcceptProposalSettlement, AcceptanceRefusalBoundary,
    AcceptanceRefusalReason, ProjectCommandEnvelope, ProjectCommandError, RefusableCommandError,
};
use storyos_core::{
    AcceptProposal as CoreAccept, AcceptProposalConflict, AcceptProposalInvalid,
    AcceptProposalRefusal, AcceptanceBase, AcceptanceCondition, AcceptanceTargetResolution,
    AcceptanceValidation, ProposalBundlePolicy, ProposalOperationSelection,
    ProposalSelectionIntent, TransitionOutcome, accept_proposal as classify_acceptance,
    classify_proposal_selection,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActionDisposition, Admission, AppliedVariant, AuthoritativeRevision, Classification,
    CommandSpec, EditorAdmission, LockedProject, ProfileSequences, ProjectCommand, ProjectResponse,
    ReceiptHeads, ReceiptRefs, ReplayEffect, RevisionBase, RevisionMembers, RevisionWrite,
    ZeroAuthorityRows, ZeroOutcome, ZeroReceipt, settle_project_command, unavailable,
};
use crate::undo_compensation::ForwardCommand;

mod compensation;
#[path = "accept_proposal_facts.rs"]
mod facts;
pub(crate) use compensation::{
    AcceptanceCompensation, LoadedAcceptance, persist_reversal, record_unavailable, reversal_blocks,
};
use facts::load_proposal;

impl PostgresProjectReader {
    /// Settles one Acceptance. A refusal before Admission retains its Pre-Admission Refusal
    /// Record in its own Serializable transaction.
    pub async fn accept_proposal(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &AcceptProposalInput,
    ) -> Result<AcceptProposalSettlement, RefusableCommandError<AcceptanceRefusalReason>> {
        let command = AcceptProposal::new(input.clone());
        let (reason, boundary) = match settle_project_command(self, envelope, &command).await {
            Ok(settlement) => return Ok(settlement),
            Err(RefusableCommandError::RefusedBeforeAdmission(reason)) => {
                (reason, AcceptanceRefusalBoundary::WriterSession)
            }
            Err(RefusableCommandError::Command(ProjectCommandError::InvalidChallenge)) => (
                AcceptanceRefusalReason::InvalidChallenge,
                AcceptanceRefusalBoundary::Challenge,
            ),
            Err(RefusableCommandError::Command(error)) => {
                return Err(RefusableCommandError::Command(error));
            }
        };
        let reason = self
            .retain_acceptance_refusal(envelope, &input.proposal_id, reason, boundary)
            .await?;
        Err(RefusableCommandError::RefusedBeforeAdmission(reason))
    }
}

/// One Acceptance.
#[derive(Clone)]
pub(crate) struct AcceptProposal {
    input: AcceptProposalInput,
}

impl AcceptProposal {
    pub(crate) fn new(input: AcceptProposalInput) -> Self {
        Self { input }
    }
}

/// The new Revision text of an applied Acceptance and how it found its target.
pub(crate) struct AcceptedRevision {
    chapter_id: String,
    payload: String,
    resolution: AcceptanceTargetResolution,
}

impl ProjectCommand for AcceptProposal {
    const SPEC: CommandSpec = CommandSpec {
        kind: "acceptProposal",
        applied: &[AppliedVariant::Forward(ForwardCommand::AcceptProposal)],
        // The AuthoritativeRevision profile writes its own Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object('target_resolution', target_resolution)::text
               FROM storyos.acceptance_receipts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND acceptance_receipt_id = $3::text::uuid",
        ),
    };
    type Error = RefusableCommandError<AcceptanceRefusalReason>;
    type Profile = AuthoritativeRevision;
    type Response = ProjectResponse;
    /// The condition references of the Receipt.
    type ZeroEffect = Vec<String>;
    type Applied = ();
    type Plan = AcceptedRevision;
    type Effect = AcceptanceTargetResolution;
    type NoEffect = AcceptProposalInvalid;
    type Conflict = AcceptProposalConflict;
    type Refusal = AcceptProposalRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, Self::Error> {
        let input = &self.input;
        let loaded = load_proposal(client, &envelope.project_scope, input)
            .await?
            .ok_or(ProjectCommandError::MissingProject)?;
        if let Some(reason) = retained_refusal(client, envelope).await? {
            return Err(RefusableCommandError::RefusedBeforeAdmission(reason));
        }
        let selection = classify_proposal_selection(
            &input.selected_operation_ids,
            &loaded
                .operations
                .iter()
                .map(|operation| ProposalOperationSelection {
                    operation_id: operation.operation_id.clone(),
                    resolution: operation.resolution.clone(),
                    predecessor_operation_ids: operation.predecessor_operation_ids.clone(),
                })
                .collect::<Vec<_>>(),
            ProposalBundlePolicy::from_stored(&loaded.bundle_policy),
            ProposalSelectionIntent::Accept,
        );
        let validation = |value: Option<&str>| match value {
            Some("valid") => AcceptanceValidation::Valid,
            Some("conflicted") => AcceptanceValidation::Conflicted,
            _ => AcceptanceValidation::Other,
        };
        let receipt_named =
            loaded.receipt_id.as_deref() == Some(input.validation_receipt_id.as_str());
        let outcome = classify_acceptance(&CoreAccept {
            scope_matches: true,
            admission_valid: true,
            proposal_revision_current: loaded.current_revision_id == input.proposal_revision_id,
            retention_retained: true,
            generation_ready: loaded.generation == "ready",
            closure_open: loaded.closure == "open",
            validation: validation(Some(loaded.validation.as_str())),
            condition: match &loaded.condition {
                None => AcceptanceCondition::Absent,
                Some((_, Some(kind))) if kind == "proposal_recovery_conflict" => {
                    AcceptanceCondition::RecoveryConflict
                }
                Some((validation, _)) if validation == "conflicted" => {
                    AcceptanceCondition::ProposalConflict
                }
                Some(_) => AcceptanceCondition::Invalid,
            },
            validation_receipt: if receipt_named {
                validation(loaded.receipt_result.as_deref())
            } else {
                AcceptanceValidation::Other
            },
            validation_receipt_matches_revision: loaded.receipt_revision_id.as_deref()
                == Some(input.proposal_revision_id.as_str())
                && loaded.receipt_base_matches_revision,
            selected_operation_pending: selection.all_selected_pending,
            selection_duplicate_free: selection.duplicate_free,
            required_dependencies_met: selection.required_dependencies_met,
            bundle_closure_complete: selection.bundle_closure_complete,
            base: if loaded.base_is_head {
                AcceptanceBase::ExactHead
            } else {
                AcceptanceBase::EarlierHead {
                    target_blocks_present: loaded
                        .target_blocks_present(&input.selected_operation_ids),
                }
            },
            candidate_unaltered: loaded.candidate_unaltered(),
        });
        let outcome = match outcome {
            TransitionOutcome::Applied(resolution) => TransitionOutcome::Applied((
                (),
                AcceptedRevision {
                    payload: loaded.accepted_body(&input.selected_operation_ids),
                    chapter_id: loaded.chapter_id.clone(),
                    resolution,
                },
            )),
            TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
            TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
        };
        let head = vec![input.expected_authoritative_revision_id.clone()];
        Ok(Classification {
            outcome,
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: input.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(loaded.chapter_id),
                expected_authoritative_revision_id: Some(
                    input.expected_authoritative_revision_id.clone(),
                ),
                target_refs: Vec::new(),
            }),
            heads: ReceiptHeads {
                expected: head.clone(),
                prior: head.clone(),
                resulting: head,
            },
            zero_receipt: ZeroReceipt::Observed {
                fields: serde_json::Map::new(),
                refs: ReceiptRefs::default(),
                effect: Vec::new(),
            },
        })
    }

    async fn diagnose_missing_admission(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> Self::Error {
        let binding = &envelope.client_binding;
        let session = match client
            .query_opt(
                "SELECT client_session_binding_ref = $4
                        AND client_session_generation = $5::text::numeric
                        AND client_contract_revision = $6 AND security_policy_revision = $7
                   FROM storyos.editor_sessions
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND editor_session_id = $3::text::uuid",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &self.input.editor_session_id.as_ref(),
                    &binding.binding_ref,
                    &binding.session_generation.to_string(),
                    &binding.client_contract_revision,
                    &binding.security_policy_revision,
                ],
            )
            .await
        {
            Ok(session) => session,
            Err(error) => return unavailable(error).into(),
        };
        RefusableCommandError::RefusedBeforeAdmission(match session {
            None => AcceptanceRefusalReason::InvalidChallenge,
            Some(session) if session.get::<_, bool>(/*idx*/ 0) => {
                AcceptanceRefusalReason::StaleWriter
            }
            Some(_) => AcceptanceRefusalReason::SessionChanged,
        })
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ProfileSequences<Self>,
        plan: AcceptedRevision,
        (): (),
    ) -> Result<RevisionWrite<AcceptanceTargetResolution>, ProjectCommandError> {
        let input = &self.input;
        client
            .execute(
                "INSERT INTO storyos.acceptance_receipts
                   (owner_user_id, project_id, acceptance_receipt_id, proposal_id,
                    proposal_revision_id, validation_receipt_id, selected_operation_ids, result,
                    applied_authoritative_revision_id, target_resolution)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7::text[]::uuid[],
                         'authoritative_applied', $8::text::uuid, $9)",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &envelope.ids.receipt_id,
                    &input.proposal_id,
                    &input.proposal_revision_id,
                    &input.validation_receipt_id,
                    &input.selected_operation_ids,
                    &input.expected_authoritative_revision_id,
                    &plan.resolution.code(),
                ],
            )
            .await
            .map_err(unavailable)?;
        let updated = client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET resolution = 'applied', reservation_state = 'resolved'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($4::text[]::uuid[])
                    AND resolution = 'pending'",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &input.proposal_id,
                    &input.selected_operation_ids,
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated
            != u64::try_from(input.selected_operation_ids.len()).unwrap_or(/*default*/ 0)
        {
            return Err(ProjectCommandError::BindingConflict);
        }
        Ok(RevisionWrite {
            effect: plan.resolution,
            chapter_id: plan.chapter_id,
            prior_revision_id: input.expected_authoritative_revision_id.clone(),
            payload: plan.payload,
            members: RevisionMembers::CopyFrom(input.expected_authoritative_revision_id.clone()),
            disposition: ActionDisposition::Forward,
            editor_session_id: input.editor_session_id.as_ref().to_owned(),
            writer_base: RevisionBase::WhenOnPrior,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<AcceptanceTargetResolution, ReplayFault> {
        if replay.author_action_disposition.as_deref() != Some("forward") {
            return Err(ReplayFault::Unavailable(
                "the applied Acceptance has no Forward Author Action".into(),
            ));
        }
        replay
            .effect_text("target_resolution")?
            .as_deref()
            .and_then(AcceptanceTargetResolution::from_code)
            .ok_or_else(|| {
                ReplayFault::Unavailable("the applied Acceptance has no target resolution".into())
            })
    }

    fn zero_authority_rows(&self, _outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        ZeroAuthorityRows::Effect
    }

    /// Writes the Acceptance Receipt of a zero-authority outcome, and the condition of an invalid
    /// Acceptance. `apply` writes the applied Acceptance Receipt, because only the plan holds its
    /// target resolution.
    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<Vec<String>, ProjectCommandError> {
        let result = outcome.receipt_result_kind();
        client
            .execute(
                "INSERT INTO storyos.acceptance_receipts
                   (owner_user_id, project_id, acceptance_receipt_id, proposal_id,
                    proposal_revision_id, validation_receipt_id, selected_operation_ids, result)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7::text[]::uuid[], $8)",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &envelope.ids.receipt_id,
                    &self.input.proposal_id,
                    &self.input.proposal_revision_id,
                    &self.input.validation_receipt_id,
                    &self.input.selected_operation_ids,
                    &result,
                ],
            )
            .await
            .map_err(unavailable)?;
        match outcome {
            ZeroOutcome::NoEffect(_) => {}
            // Since ADR 0064 no Acceptance gives a conflict. Only earlier Receipts record one.
            ZeroOutcome::Conflicted(_) => {
                return Err(unavailable("only an invalid Acceptance writes a condition"));
            }
            ZeroOutcome::Refused(_) => return Ok(Vec::new()),
        }
        client
            .execute(
                "INSERT INTO storyos.proposal_validation_conditions
                   (owner_user_id, project_id, proposal_id, proposal_revision_id,
                    acceptance_receipt_id, validation)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, 'invalid')",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &self.input.proposal_id,
                    &self.input.proposal_revision_id,
                    &envelope.ids.receipt_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(Vec::new())
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<Vec<String>>, ReplayFault> {
        Ok(Some(replay.condition_refs.clone()))
    }
}

/// The reason of a Pre-Admission Refusal Record that the idempotency key already retains.
async fn retained_refusal(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<Option<AcceptanceRefusalReason>, ProjectCommandError> {
    client
        .query_opt(
            "SELECT reason FROM storyos.acceptance_refusals
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND idempotency_key = $3::text::uuid",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(unavailable)?
        .map(|row| crate::acceptance_refusal::parse_reason(row.get(/*idx*/ 0)))
        .transpose()
        .map_err(unavailable)
}
