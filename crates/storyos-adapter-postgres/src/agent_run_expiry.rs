use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, ProjectAssistanceRecord, ReferenceRecoveryDisposition,
    ReferenceRecoveryInspect,
};
use storyos_core::{
    AssistanceAvailability, ContinuationReferenceCondition, ExpiryRebuildBlock,
    ExpiryRebuildDisposition, ExpiryRebuildFacts, continuation_boundary_matches,
    decide_confirmed_expiry_rebuild,
};
use uuid::Uuid;

use crate::agent_run_continuation::{current_identity, parse_identity};

pub(crate) struct RecoveryDraft {
    pub recovery_id: String,
    pub predecessor_run_id: String,
    pub predecessor_attempt_id: String,
    pub predecessor_binding_id: Option<String>,
    pub disposition: &'static str,
    pub block_reason: Option<&'static str>,
    pub run_step_id: Option<String>,
    pub model_invocation_id: Option<String>,
    pub assembly_manifest_id: Option<String>,
    pub covered_content_included: bool,
    pub predecessor_terminal: bool,
}

pub(crate) struct RebuildDispatch {
    pub model_invocation_id: String,
    pub row: RecoveryDraft,
}

pub(crate) enum ExpiryAdmission {
    Settled,
    Dispatch(Option<Box<RebuildDispatch>>),
}

pub(crate) async fn plan_expiry_rebuild(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    author_message: &str,
    sufficiency: &str,
    assistance: Option<&ProjectAssistanceRecord>,
    assembly_manifest_id: &str,
) -> Result<ExpiryAdmission, CompleteAgentRunError> {
    let Some(prior) = load_latest_attempt(client, claim, conversation_id).await? else {
        return Ok(ExpiryAdmission::Dispatch(None));
    };
    let payload = serde_json::from_str::<serde_json::Value>(&prior.payload).map_err(unavailable)?;
    let identity = payload.get("produced_binding").and_then(parse_identity);
    let reference = if prior.dispatch_state == "uncertain" {
        ContinuationReferenceCondition::UnknownCreate
    } else {
        match payload
            .pointer("/produced_binding/reference_condition")
            .and_then(serde_json::Value::as_str)
        {
            Some("confirmed_expired") => ContinuationReferenceCondition::ConfirmedExpired,
            Some("confirmed_unusable") => ContinuationReferenceCondition::ConfirmedUnusable,
            _ => ContinuationReferenceCondition::Usable,
        }
    };
    if matches!(reference, ContinuationReferenceCondition::Usable) {
        return Ok(ExpiryAdmission::Dispatch(None));
    }
    let current = assistance.map(|record| current_identity(claim, conversation_id, record));
    let decision = decide_confirmed_expiry_rebuild(&ExpiryRebuildFacts {
        reference,
        prior_submissions_settled: prior.dispatch_state == "settled"
            && matches!(prior.run_status.as_str(), "completed" | "cancelled"),
        predecessor_fenced: prior.run_status == "cancelled",
        same_processing_boundary: match (&current, &identity) {
            (Some(current), Some(prior_identity)) => {
                continuation_boundary_matches(current, prior_identity)
            }
            _ => false,
        },
        current_authority: matches!(
            assistance.map(|record| record.availability),
            Some(AssistanceAvailability::Available)
        ),
        budget_covers_submission: !payload
            .pointer("/produced_binding/budget_exhausted")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        required_input_present: sufficiency == "complete" && !author_message.is_empty(),
        covered_copy_restricted: identity
            .as_ref()
            .is_some_and(|prior_identity| prior_identity.covered_copy_restricted),
        ordinary_correction: true,
        effective_model_context_changed: true,
        predecessor_terminal: prior.run_status == "completed",
    });
    let draft = RecoveryDraft {
        recovery_id: Uuid::now_v7().to_string(),
        predecessor_run_id: prior.run_id,
        predecessor_attempt_id: prior.attempt_id,
        predecessor_binding_id: prior.binding_id,
        disposition: match decision.disposition {
            ExpiryRebuildDisposition::NotApplicable => "not_applicable",
            ExpiryRebuildDisposition::UnknownCreate => "unknown_create",
            ExpiryRebuildDisposition::Blocked(_) => "blocked",
            ExpiryRebuildDisposition::Rebuilt => "rebuilt",
        },
        block_reason: match decision.disposition {
            ExpiryRebuildDisposition::Blocked(reason) => Some(block_labels(reason).0),
            ExpiryRebuildDisposition::NotApplicable
            | ExpiryRebuildDisposition::UnknownCreate
            | ExpiryRebuildDisposition::Rebuilt => None,
        },
        run_step_id: None,
        model_invocation_id: None,
        assembly_manifest_id: None,
        covered_content_included: decision.covered_content_included,
        predecessor_terminal: decision.predecessor_stays_terminal,
    };
    let capability = match decision.disposition {
        ExpiryRebuildDisposition::NotApplicable => {
            return Ok(ExpiryAdmission::Dispatch(None));
        }
        ExpiryRebuildDisposition::UnknownCreate => "expiry_rebuild_unknown_create",
        ExpiryRebuildDisposition::Blocked(reason) => block_labels(reason).1,
        ExpiryRebuildDisposition::Rebuilt if decision.new_run_step_and_invocation => {
            let model_invocation_id = Uuid::now_v7().to_string();
            return Ok(ExpiryAdmission::Dispatch(Some(Box::new(RebuildDispatch {
                model_invocation_id: model_invocation_id.clone(),
                row: RecoveryDraft {
                    run_step_id: Some(Uuid::now_v7().to_string()),
                    model_invocation_id: Some(model_invocation_id),
                    assembly_manifest_id: Some(assembly_manifest_id.to_owned()),
                    ..draft
                },
            }))));
        }
        ExpiryRebuildDisposition::Rebuilt => {
            return Err(unavailable(std::io::Error::other(
                "Confirmed expiry rebuild requires a new Run Step and Invocation",
            )));
        }
    };
    insert_recovery(
        client,
        claim,
        conversation_id,
        &draft,
        /*model_attempt_id*/ None,
    )
    .await?;
    crate::agent_run_work::update_run(
        client,
        claim,
        "refused",
        Some(&serde_json::json!({
            "kind": "execution_refused",
            "capability": capability
        })),
        /*clear_lease*/ true,
    )
    .await?;
    Ok(ExpiryAdmission::Settled)
}

pub(crate) async fn insert_recovery(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    row: &RecoveryDraft,
    model_attempt_id: Option<&str>,
) -> Result<(), CompleteAgentRunError> {
    let lossless_provider_reconstruction = false;
    let semantic_erasure = false;
    let opaque_reused = false;
    let model_attempt_id = model_attempt_id.map(str::to_owned);
    client
        .execute(
            "INSERT INTO storyos.context_reference_recoveries
               (owner_user_id, project_id, recovery_id, conversation_id,
                predecessor_run_id, successor_run_id, predecessor_model_attempt_id,
                predecessor_continuation_binding_id, disposition, block_reason,
                run_step_id, model_invocation_id, model_attempt_id, assembly_manifest_id,
                lossless_provider_reconstruction, semantic_erasure, opaque_reused,
                covered_content_included, predecessor_terminal)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9, $10, $11::text::uuid, $12::text::uuid, $13::text::uuid,
                     $14::text::uuid, $15, $16, $17, $18, $19)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &row.recovery_id,
                &conversation_id,
                &row.predecessor_run_id,
                &claim.run_id,
                &row.predecessor_attempt_id,
                &row.predecessor_binding_id,
                &row.disposition,
                &row.block_reason,
                &row.run_step_id,
                &row.model_invocation_id,
                &model_attempt_id,
                &row.assembly_manifest_id,
                &lossless_provider_reconstruction,
                &semantic_erasure,
                &opaque_reused,
                &row.covered_content_included,
                &row.predecessor_terminal,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

pub(crate) async fn load_reference_recovery(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    run_id: &str,
) -> Result<Option<ReferenceRecoveryInspect>, storyos_application::CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT recovery_id::text, disposition, block_reason, predecessor_run_id::text,
                    predecessor_continuation_binding_id::text, run_step_id::text,
                    model_invocation_id::text, model_attempt_id::text, assembly_manifest_id::text,
                    lossless_provider_reconstruction, semantic_erasure, opaque_reused,
                    covered_content_included, predecessor_terminal
               FROM storyos.context_reference_recoveries
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND successor_run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(read_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let disposition = match row.get::<_, String>(1).as_str() {
        "rebuilt" => ReferenceRecoveryDisposition::Rebuilt,
        "blocked" => ReferenceRecoveryDisposition::Blocked,
        "unknown_create" => ReferenceRecoveryDisposition::UnknownCreate,
        _ => {
            return Err(read_error(std::io::Error::other(
                "The reference recovery disposition is unknown",
            )));
        }
    };
    Ok(Some(ReferenceRecoveryInspect {
        recovery_id: row.get(0),
        disposition,
        block_reason: row.get(2),
        predecessor_run_id: row.get(3),
        predecessor_continuation_binding_id: row.get(4),
        run_step_id: row.get(5),
        model_invocation_id: row.get(6),
        model_attempt_id: row.get(7),
        assembly_manifest_id: row.get(8),
        lossless_provider_reconstruction: row.get(9),
        semantic_erasure: row.get(10),
        opaque_reused: row.get(11),
        covered_content_included: row.get(12),
        predecessor_terminal: row.get(13),
    }))
}

struct LatestAttempt {
    attempt_id: String,
    binding_id: Option<String>,
    dispatch_state: String,
    payload: String,
    run_id: String,
    run_status: String,
}

async fn load_latest_attempt(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
) -> Result<Option<LatestAttempt>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT attempt.model_attempt_id::text, attempt.continuation_binding_id::text,
                    attempt.dispatch_state, attempt.payload::text, run.run_id::text, run.status
               FROM storyos.model_attempts AS attempt
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id, run.run_id) =
                    (attempt.owner_user_id, attempt.project_id, attempt.run_id)
              WHERE attempt.owner_user_id = $1::text::uuid
                AND attempt.project_id = $2::text::uuid
                AND attempt.conversation_id = $3::text::uuid
                AND attempt.attempt_role = 'decision'
                AND run.run_id <> $4::text::uuid
              ORDER BY run.run_id DESC
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &conversation_id,
                &claim.run_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(LatestAttempt {
        attempt_id: row.get(0),
        binding_id: row.get(1),
        dispatch_state: row.get(2),
        payload: row.get(3),
        run_id: row.get(4),
        run_status: row.get(5),
    }))
}

fn block_labels(reason: ExpiryRebuildBlock) -> (&'static str, &'static str) {
    match reason {
        ExpiryRebuildBlock::PriorSubmissionUnsettled => (
            "prior_submission_unsettled",
            "expiry_rebuild_prior_submission_unsettled",
        ),
        ExpiryRebuildBlock::FencedPredecessor => {
            ("fenced_predecessor", "expiry_rebuild_fenced_predecessor")
        }
        ExpiryRebuildBlock::ProcessingBoundaryChanged => (
            "processing_boundary_changed",
            "expiry_rebuild_processing_boundary_changed",
        ),
        ExpiryRebuildBlock::AuthorityMissing => {
            ("authority_missing", "expiry_rebuild_authority_missing")
        }
        ExpiryRebuildBlock::BudgetInsufficient => {
            ("budget_insufficient", "expiry_rebuild_budget_insufficient")
        }
        ExpiryRebuildBlock::RequiredInputMissing => (
            "required_input_missing",
            "expiry_rebuild_required_input_missing",
        ),
    }
}

fn unavailable(error: impl std::error::Error + Send + Sync + 'static) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

fn read_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> storyos_application::CreateAgentRunError {
    storyos_application::CreateAgentRunError::Unavailable(Box::new(error))
}
