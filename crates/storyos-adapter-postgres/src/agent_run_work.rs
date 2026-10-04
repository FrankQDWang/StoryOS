use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, CommittedCancellation, CompleteAgentRun,
    CompleteAgentRunError, DestinationRequest, DispatchClaim, ProjectId, ProjectReadError,
    ProjectScope, RequestAttempt, UserId,
};
use storyos_core::{ExecutionCapability, requested_execution_capability, stream_batch_plan};
use uuid::Uuid;

use crate::agent_run_create_dispatch::PriorContext;

use super::*;
use crate::update_project_assistance::read_assistance_record;

impl AgentRunWorkStore for PostgresProjectReader {
    async fn claim_next_agent_run(&self) -> Result<Option<ClaimedAgentRun>, ProjectReadError> {
        self.require_release1_storage_activation_proof()
            .await
            .map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        crate::export_work::set_worker_scope(&transaction).await?;
        let lease_seconds = i64::try_from(self.readable_export_lease_ttl.as_secs())
            .map_err(ProjectReadError::unavailable)?;
        let claimed = claim_agent_run_row(&transaction, lease_seconds).await?;
        transaction.commit().await.map_err(read_error)?;
        Ok(claimed)
    }
}

pub(crate) enum WorkPhase {
    Done(CompleteAgentRun),
    Hold(&'static str),
    Dispatch(Box<DestinationRequest>),
    Abort(CommittedCancellation),
}

async fn claim_agent_run_row(
    transaction: &tokio_postgres::Transaction<'_>,
    lease_seconds: i64,
) -> Result<Option<ClaimedAgentRun>, ProjectReadError> {
    let claimed = transaction
        .query_opt(
            "WITH next_work AS (
               SELECT owner_user_id, project_id, run_id
                 FROM storyos.agent_runs AS run
                WHERE run.status = 'queued'
                   OR (
                     run.status = 'claimed'
                     AND run.claim_generation > 0
                     AND run.lease_expires_at IS NOT NULL
                     AND run.lease_expires_at <= clock_timestamp()
                   )
                   OR (
                     run.status = 'cancelled'
                     AND run.wakeup_pending
                     AND (run.lease_expires_at IS NULL
                          OR run.lease_expires_at <= clock_timestamp())
                   )
                ORDER BY run.status = 'cancelled', run.run_id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             UPDATE storyos.agent_runs AS run
                SET claim_generation = run.claim_generation + 1,
                    fence_token = run.claim_generation + 1,
                    lease_expires_at = clock_timestamp()
                      + ($1::bigint * interval '1 second'),
                    wakeup_pending = run.status = 'cancelled' AND run.wakeup_pending,
                    status = CASE WHEN run.status = 'cancelled' THEN 'cancelled' ELSE 'claimed' END
               FROM next_work
              WHERE run.owner_user_id = next_work.owner_user_id
                AND run.project_id = next_work.project_id
                AND run.run_id = next_work.run_id
          RETURNING run.owner_user_id::text,
                    run.project_id::text,
                    run.run_id::text,
                    run.fence_token",
            &[&lease_seconds],
        )
        .await
        .map_err(read_error)?;
    Ok(claimed.map(|row| ClaimedAgentRun {
        project_scope: ProjectScope::new(
            UserId::new(row.get::<_, String>(0)),
            ProjectId::new(row.get::<_, String>(1)),
        ),
        run_id: row.get(2),
        fence_token: row.get(3),
    }))
}

pub(crate) struct RunPhaseRow {
    pub status: String,
    pub author_message: String,
    pub chapter_id: String,
    pub conversation_id: String,
    pub sufficiency: String,
    pub assembly_manifest_id: String,
    pub destination_manifest_id: Option<String>,
    pub attempt_id: Option<String>,
    pub decision_id: Option<String>,
    pub continuation_id: Option<String>,
    pub attempt_payload: Option<String>,
    pub decision_position: String,
    pub assembly_payload: String,
}

impl RunPhaseRow {
    /// A requeued Run has given back this claim, like a settled Run.
    pub(crate) fn settled(&self) -> bool {
        matches!(
            self.status.as_str(),
            "queued" | "completed" | "waiting" | "refused" | "paused" | "cancelled"
        )
    }
}

/// Locks the fenced AgentRun row with its active decision Context Assembly and Model Attempt.
pub(crate) async fn load_run_phase(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<RunPhaseRow, CompleteAgentRunError> {
    let Some(row) = client
        .query_opt(
            "SELECT run.status, COALESCE((SELECT item->>'content' FROM jsonb_array_elements(assembly.payload->'selected') AS item WHERE item->>'source_class'='author_instruction'),run.author_message) AS author_message,
                    run.chapter_id::text AS chapter_id, run.conversation_id::text AS conversation_id,
                    assembly.sufficiency, assembly.context_assembly_manifest_id::text AS assembly_manifest_id,
                    assembly.destination_context_manifest_id::text AS destination_manifest_id,
                    attempt.model_attempt_id::text AS attempt_id, attempt.decision_id::text AS decision_id,
                    attempt.continuation_binding_id::text AS continuation_id,
                    attempt.payload::text AS attempt_payload,
                    run.active_decision_position::text AS decision_position,
                    assembly.payload::text AS assembly_payload
               FROM storyos.agent_runs AS run
               JOIN storyos.context_assembly_manifests AS assembly
                 ON (assembly.owner_user_id, assembly.project_id, assembly.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND assembly.manifest_role = 'decision' AND assembly.decision_position=run.active_decision_position
               LEFT JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND attempt.attempt_role = 'decision' AND attempt.decision_position=run.active_decision_position
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid
                AND run.fence_token = $4
              FOR UPDATE OF run",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(complete_database_error)?
    else {
        return Err(CompleteAgentRunError::StaleFence);
    };
    Ok(RunPhaseRow {
        status: row.get("status"),
        author_message: row.get("author_message"),
        chapter_id: row.get("chapter_id"),
        conversation_id: row.get("conversation_id"),
        sufficiency: row.get("sufficiency"),
        assembly_manifest_id: row.get("assembly_manifest_id"),
        destination_manifest_id: row.get("destination_manifest_id"),
        attempt_id: row.get("attempt_id"),
        decision_id: row.get("decision_id"),
        continuation_id: row.get("continuation_id"),
        attempt_payload: row.get("attempt_payload"),
        decision_position: row.get("decision_position"),
        assembly_payload: row.get("assembly_payload"),
    })
}

pub(crate) enum CreateAdmission {
    Settled,
    Dispatch(Option<Box<crate::agent_run_expiry::RebuildDispatch>>),
}

/// Refuses a blocked or capability request before dispatch, or admits one new Model Attempt.
pub(crate) async fn admit_create(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    assistance: Option<&storyos_application::ProjectAssistanceRecord>,
) -> Result<CreateAdmission, CompleteAgentRunError> {
    let rebuild = match crate::agent_run_expiry::plan_expiry_rebuild(
        client,
        claim,
        &run.conversation_id,
        &run.author_message,
        &run.sufficiency,
        assistance,
        &run.assembly_manifest_id,
    )
    .await?
    {
        crate::agent_run_expiry::ExpiryAdmission::Settled => return Ok(CreateAdmission::Settled),
        crate::agent_run_expiry::ExpiryAdmission::Dispatch(rebuild) => rebuild,
    };
    let capability = requested_execution_capability(&run.author_message);
    let blocked = run.sufficiency != "complete"
        || !matches!(
            assistance.map(|record| record.availability),
            Some(storyos_core::AssistanceAvailability::Available)
        );
    if !blocked && capability.is_none() {
        return Ok(CreateAdmission::Dispatch(rebuild));
    }
    let capability = match capability {
        Some(ExecutionCapability::Tool) => "tool",
        Some(ExecutionCapability::Mcp) => "mcp",
        Some(ExecutionCapability::Research) => "research",
        Some(ExecutionCapability::Embedding) => "embedding",
        Some(ExecutionCapability::Memory) => "memory",
        Some(ExecutionCapability::Skill) => "skill",
        Some(ExecutionCapability::Subrun) => "subrun",
        Some(ExecutionCapability::Eval) => "eval",
        None => "blocked_context",
    };
    update_run(
        client,
        claim,
        "refused",
        Some(&serde_json::json!({"kind": "execution_refused", "capability": capability})),
        /*clear_lease*/ true,
    )
    .await?;
    Ok(CreateAdmission::Settled)
}

pub(crate) async fn settle_one_phase(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    lease_seconds: i64,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let run = load_run_phase(client, claim).await?;
    if run.status == "cancelled" {
        return crate::agent_run_recovery::settle_cancelled(client, claim, &run).await;
    }
    if run.settled() {
        return Ok(WorkPhase::Done(CompleteAgentRun::AlreadySettled));
    }
    if run.attempt_id.is_none()
        && run.decision_position == "0"
        && crate::agent_run_steering::advance(client, claim, lease_seconds).await?
    {
        return Ok(WorkPhase::Hold("steering"));
    }
    let assistance = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let record: serde_json::Value = serde_json::from_str(&run.assembly_payload)
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let Some(attempt_id) = run.attempt_id.clone() else {
        return Ok(
            match admit_create(client, claim, &run, assistance.as_ref()).await? {
                CreateAdmission::Settled => WorkPhase::Done(CompleteAgentRun::Settled),
                CreateAdmission::Dispatch(rebuild) => {
                    WorkPhase::Dispatch(Box::new(DestinationRequest::Create(
                        crate::agent_run_create_dispatch::create_request(
                            client,
                            claim,
                            &run,
                            &record,
                            RequestAttempt::New,
                            match rebuild {
                                Some(_) => PriorContext::Rebuild,
                                None => PriorContext::Continue,
                            },
                        )
                        .await?,
                    )))
                }
            },
        );
    };
    let RunPhaseRow {
        author_message,
        chapter_id,
        destination_manifest_id: destination_manifest,
        decision_id,
        continuation_id,
        ..
    } = &run;
    if destination_manifest.is_none() {
        return Err(CompleteAgentRunError::Unavailable(Box::new(
            std::io::Error::other("The Destination Manifest is missing after dispatch claim"),
        )));
    }
    let payload: serde_json::Value =
        serde_json::from_str(run.attempt_payload.as_deref().unwrap_or("null"))
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    if payload.get("original_result_retrieval").is_some()
        || payload.get("unknown_create_successor").is_some()
    {
        return crate::agent_run_recovery::advance(
            client,
            claim,
            &run,
            &payload,
            assistance.as_ref(),
        )
        .await;
    }
    if decision_id.is_none() {
        return Ok(WorkPhase::Dispatch(Box::new(DestinationRequest::Create(
            crate::agent_run_create_dispatch::create_request(
                client,
                claim,
                &run,
                &record,
                RequestAttempt::Claimed(DispatchClaim {
                    model_attempt_id: attempt_id,
                }),
                PriorContext::Continue,
            )
            .await?,
        ))));
    }
    if payload.pointer("/decision/locations").is_none()
        && stream_batch_plan(author_message).is_some()
        && let Some(decision) = decision_id.as_deref()
    {
        let (_proposal_id, work) = crate::stream_proposal_generation::apply_streamed_proposal(
            client,
            claim,
            chapter_id,
            decision,
            author_message,
        )
        .await?;
        if matches!(work, crate::stream_proposal_generation::StreamWork::Hold) {
            return requeue_generation(client, claim).await;
        }
    }
    if let Some(decision) = decision_id
        && continuation_id.is_none()
        && payload
            .get("decision")
            .and_then(|value| value.get("advances_continuation"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    {
        let binding = Uuid::now_v7().to_string();
        let mut next_payload = payload.clone();
        if let Some(wire) = crate::agent_run_continuation::parse_wire(&payload) {
            next_payload["produced_binding"] =
                crate::agent_run_continuation::encode_produced_binding(
                    &binding,
                    &attempt_id,
                    &wire.admission,
                );
        }
        client
            .execute(
                "UPDATE storyos.model_attempts
                    SET continuation_binding_id = $4::text::uuid,
                        dispatch_state = 'settled',
                        payload = $6::text::jsonb
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND decision_id = $5::text::uuid
                    AND continuation_binding_id IS NULL
                    AND attempt_role = 'decision'",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
                    &binding,
                    &decision,
                    &next_payload.to_string(),
                ],
            )
            .await
            .map_err(complete_database_error)?;
        return complete_or_compact(client, claim, author_message).await;
    }
    complete_or_compact(client, claim, author_message).await
}

pub(crate) async fn complete_or_compact(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    author_message: &str,
) -> Result<WorkPhase, CompleteAgentRunError> {
    if storyos_core::requests_active_compaction(author_message) {
        let advance =
            crate::agent_run_compaction::advance_active_compaction(client, claim, author_message)
                .await?;
        if matches!(
            advance,
            crate::agent_run_compaction::CompactionAdvance::Hold
        ) {
            return Ok(WorkPhase::Hold("compaction_stage"));
        }
    }
    update_run(
        client,
        claim,
        "completed",
        /*settlement*/ None,
        /*clear_lease*/ true,
    )
    .await?;
    Ok(WorkPhase::Done(CompleteAgentRun::Settled))
}

pub(crate) async fn update_run(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    status: &str,
    settlement: Option<&serde_json::Value>,
    clear_lease: bool,
) -> Result<(), CompleteAgentRunError> {
    let settlement = settlement.map(ToString::to_string);
    client
        .execute(
            "UPDATE storyos.agent_runs
                SET status = $4,
                    settlement = COALESCE($5::text::jsonb, settlement),
                    lease_expires_at = CASE WHEN $6 THEN NULL ELSE lease_expires_at END,
                    wakeup_pending = false
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND fence_token = $7",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &status,
                &settlement,
                &clear_lease,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

pub(crate) async fn requeue_generation(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<WorkPhase, CompleteAgentRunError> {
    update_run(
        client, claim, "queued", /*settlement*/ None, /*clear_lease*/ true,
    )
    .await?;
    Ok(WorkPhase::Done(CompleteAgentRun::Settled))
}

pub(crate) async fn hold_if_requested(kind: &str) {
    let key = match kind {
        "decision" => "STORYOS_TEST_FAKE_DECISION_HOLD_PATH",
        "compaction_stage" => "STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH",
        "successor_fence" => "STORYOS_TEST_FAKE_SUCCESSOR_FENCE_HOLD_PATH",
        "successor_late" => "STORYOS_TEST_FAKE_SUCCESSOR_LATE_HOLD_PATH",
        _ => return,
    };
    let Ok(path) = std::env::var(key) else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    if !path.exists() {
        return;
    }
    while path.exists() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

pub(crate) fn complete_challenge_error(
    error: storyos_application::ProjectCommandChallengeError,
) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

pub(crate) fn complete_database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
