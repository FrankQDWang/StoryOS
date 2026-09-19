use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, ProjectId,
    ProjectReadError, ProjectScope, UserId,
};
use storyos_core::{
    ExecutionCapability, FakeAttemptOutcome, FakeDecisionKind, FakeDispatchPlan,
    HOST_FAKE_EXECUTION_PROFILE, HOST_FAKE_MAPPING_REVISION, StreamItemRole, StreamItemState,
    host_fake_wire_digest, plan_fake_model_decision,
};
use uuid::Uuid;

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

    async fn complete_agent_run(
        &self,
        claim: &ClaimedAgentRun,
    ) -> Result<CompleteAgentRun, CompleteAgentRunError> {
        loop {
            let transaction = self
                .begin_project_command_transaction(&claim.project_scope)
                .await
                .map_err(complete_challenge_error)?;
            let phase = settle_one_phase(&transaction.client, claim).await;
            match phase {
                Ok(WorkPhase::Done(result)) => {
                    transaction
                        .commit()
                        .await
                        .map_err(complete_challenge_error)?;
                    return Ok(result);
                }
                Ok(WorkPhase::Hold(kind)) => {
                    transaction
                        .commit()
                        .await
                        .map_err(complete_challenge_error)?;
                    hold_if_requested(kind).await;
                }
                Err(error) => {
                    let _rollback = transaction.rollback().await;
                    return Err(error);
                }
            }
        }
    }
}

enum WorkPhase {
    Done(CompleteAgentRun),
    Hold(&'static str),
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
                ORDER BY run.run_id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             UPDATE storyos.agent_runs AS run
                SET claim_generation = run.claim_generation + 1,
                    fence_token = run.claim_generation + 1,
                    lease_expires_at = clock_timestamp()
                      + ($1::bigint * interval '1 second'),
                    wakeup_pending = false,
                    status = 'claimed'
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

async fn settle_one_phase(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let Some(run) = client
        .query_opt(
            "SELECT run.status, run.author_message, run.chapter_id::text,
                    run.conversation_id::text, run.settlement,
                    assembly.sufficiency, assembly.context_assembly_manifest_id::text,
                    assembly.destination_context_manifest_id::text,
                    attempt.model_attempt_id::text, attempt.decision_id::text,
                    attempt.continuation_binding_id::text, attempt.dispatch_state,
                    attempt.payload::text
               FROM storyos.agent_runs AS run
               JOIN storyos.context_assembly_manifests AS assembly
                 ON (assembly.owner_user_id, assembly.project_id, assembly.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
               LEFT JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
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
    let status: String = run.get(0);
    if matches!(status.as_str(), "completed" | "waiting" | "refused") {
        return Ok(WorkPhase::Done(CompleteAgentRun::AlreadySettled));
    }
    let author_message: String = run.get(1);
    let chapter_id: String = run.get(2);
    let conversation_id: String = run.get(3);
    let sufficiency: String = run.get(5);
    let assembly_manifest_id: String = run.get(6);
    let destination_manifest: Option<String> = run.get(7);
    let attempt_id: Option<String> = run.get(8);
    let decision_id: Option<String> = run.get(9);
    let continuation_id: Option<String> = run.get(10);
    let assistance = read_assistance_record(client, &claim.project_scope)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let plan = plan_fake_model_decision(&author_message);
    let blocked = sufficiency != "complete"
        || !matches!(
            assistance.as_ref().map(|record| record.availability),
            Some(storyos_core::AssistanceAvailability::Available)
        );
    if attempt_id.is_none() {
        if blocked || matches!(plan, FakeDispatchPlan::RefuseWithoutDispatch { .. }) {
            let settlement = match plan {
                FakeDispatchPlan::RefuseWithoutDispatch { capability } => {
                    serde_json::json!({
                        "kind": "execution_refused",
                        "capability": match capability {
                            ExecutionCapability::Tool => "tool",
                            ExecutionCapability::Mcp => "mcp",
                            ExecutionCapability::Research => "research",
                            ExecutionCapability::Embedding => "embedding",
                            ExecutionCapability::Memory => "memory",
                            ExecutionCapability::Skill => "skill",
                            ExecutionCapability::Subrun => "subrun",
                            ExecutionCapability::Eval => "eval",
                        }
                    })
                }
                _ => serde_json::json!({"kind":"execution_refused","capability":"blocked_context"}),
            };
            update_run(
                client,
                claim,
                "refused",
                Some(&settlement),
                /*clear_lease*/ true,
            )
            .await?;
            return Ok(WorkPhase::Done(CompleteAgentRun::Settled));
        }
        persist_uncertain_attempt(
            client,
            claim,
            &conversation_id,
            &author_message,
            &chapter_id,
            &assembly_manifest_id,
        )
        .await?;
        return Ok(WorkPhase::Hold("dispatch"));
    }
    if destination_manifest.is_none() {
        return Err(CompleteAgentRunError::Unavailable(Box::new(
            std::io::Error::other("The Destination Manifest is missing after dispatch claim"),
        )));
    }
    let payload: serde_json::Value = serde_json::from_str(&run.get::<_, String>(12))
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let items_empty = payload
        .get("items")
        .and_then(serde_json::Value::as_array)
        .is_none_or(Vec::is_empty);
    if decision_id.is_none() {
        let FakeDispatchPlan::Dispatch { items, outcome } = plan else {
            return Err(CompleteAgentRunError::Unavailable(Box::new(
                std::io::Error::other("A dispatch claim cannot refuse after I/O began"),
            )));
        };
        let next = persist_stream_and_decision(
            client,
            claim,
            attempt_id.as_deref().expect("attempt exists"),
            &author_message,
            &chapter_id,
            &assembly_manifest_id,
            &items,
            &outcome,
            /*include_decision*/ !items_empty,
        )
        .await?;
        return Ok(next);
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
        client
            .execute(
                "UPDATE storyos.model_attempts
                    SET continuation_binding_id = $4::text::uuid,
                        dispatch_state = 'settled'
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND decision_id = $5::text::uuid
                    AND continuation_binding_id IS NULL",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
                    &binding,
                    &decision,
                ],
            )
            .await
            .map_err(complete_database_error)?;
        update_run(
            client,
            claim,
            "completed",
            /*settlement*/ None,
            /*clear_lease*/ true,
        )
        .await?;
        return Ok(WorkPhase::Done(CompleteAgentRun::Settled));
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

#[allow(clippy::too_many_arguments)]
async fn persist_stream_and_decision(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    attempt_id: &str,
    author_message: &str,
    chapter_id: &str,
    assembly_manifest_id: &str,
    items: &[storyos_core::NativeStreamItem],
    outcome: &FakeAttemptOutcome,
    include_decision: bool,
) -> Result<WorkPhase, CompleteAgentRunError> {
    let (decision_id, status, hold) = match (include_decision, outcome) {
        (false, FakeAttemptOutcome::NoDecision { .. }) => (None, "completed", None),
        (false, FakeAttemptOutcome::Decision { .. }) => (None, "claimed", Some("stream")),
        (true, FakeAttemptOutcome::NoDecision { .. }) => (None, "completed", None),
        (
            true,
            FakeAttemptOutcome::Decision {
                selected,
                advances_continuation,
                ..
            },
        ) => {
            let id = (*selected).then(|| Uuid::now_v7().to_string());
            let hold = (*selected && *advances_continuation).then_some("decision");
            let status = if hold.is_some() {
                "claimed"
            } else {
                "completed"
            };
            (id, status, hold)
        }
    };
    let opened_proposal = match (decision_id.as_deref(), outcome) {
        (
            Some(decision_id),
            FakeAttemptOutcome::Decision {
                kind: FakeDecisionKind::ProseChange { text, .. },
                selected: true,
                ..
            },
        ) => {
            if author_message.starts_with("Revise this phrase:") {
                crate::open_inline_proposal::open_selected_inline_change(
                    client,
                    claim,
                    chapter_id,
                    decision_id,
                    text,
                )
                .await?
            } else {
                crate::open_block_proposal::open_selected_prose_change(
                    client,
                    claim,
                    chapter_id,
                    decision_id,
                    text,
                )
                .await?
            }
        }
        _ => None,
    };
    let payload = encode_payload(
        author_message,
        chapter_id,
        assembly_manifest_id,
        attempt_id,
        items,
        outcome,
        decision_id.as_deref(),
        opened_proposal.as_deref(),
    );
    client
        .execute(
            "UPDATE storyos.model_attempts
                SET decision_id = $4::text::uuid,
                    dispatch_state = $5,
                    payload = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &decision_id,
                &if hold.is_some() {
                    "uncertain"
                } else {
                    "settled"
                },
                &payload.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    update_run(
        client,
        claim,
        status,
        /*settlement*/ None,
        /*clear_lease*/ hold.is_none(),
    )
    .await?;
    Ok(match hold {
        Some(kind) => WorkPhase::Hold(kind),
        None => WorkPhase::Done(CompleteAgentRun::Settled),
    })
}

async fn persist_uncertain_attempt(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    conversation_id: &str,
    author_message: &str,
    chapter_id: &str,
    assembly_manifest_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let model_attempt_id = Uuid::now_v7().to_string();
    let destination_attempt_id = Uuid::now_v7().to_string();
    let outbound_disclosure_event_id = Uuid::now_v7().to_string();
    let destination_context_manifest_id = Uuid::now_v7().to_string();
    let outbound_disclosure_manifest_id = Uuid::now_v7().to_string();
    let wire_payload_projection_id = Uuid::now_v7().to_string();
    let model_invocation_id = Uuid::now_v7().to_string();
    let digest = host_fake_wire_digest(author_message, chapter_id);
    let payload = serde_json::json!({
        "execution_profile": {
            "profile_revision": HOST_FAKE_EXECUTION_PROFILE,
            "mapping_revision": HOST_FAKE_MAPPING_REVISION,
            "network_io": false,
            "provider_bound": "unknown"
        },
        "wire": {
            "digest": digest,
            "author_message": author_message,
            "chapter_id": chapter_id
        },
        "items": [],
        "decision": null,
        "usage": { "kind": "unknown" },
        "evidence": evidence_values(&model_attempt_id, author_message, assembly_manifest_id)
    });
    client
        .execute(
            "UPDATE storyos.context_assembly_manifests
                SET destination_context_manifest_id = $4::text::uuid,
                    outbound_disclosure_manifest_id = $5::text::uuid
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND destination_context_manifest_id IS NULL",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &destination_context_manifest_id,
                &outbound_disclosure_manifest_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id,
                destination_attempt_id, outbound_disclosure_event_id,
                destination_context_manifest_id, outbound_disclosure_manifest_id,
                wire_payload_projection_id, model_invocation_id, conversation_id,
                dispatch_state, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, 'uncertain',
                     $12::text::jsonb)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &model_attempt_id,
                &destination_attempt_id,
                &outbound_disclosure_event_id,
                &destination_context_manifest_id,
                &outbound_disclosure_manifest_id,
                &wire_payload_projection_id,
                &model_invocation_id,
                &conversation_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_payload(
    author_message: &str,
    chapter_id: &str,
    assembly_manifest_id: &str,
    attempt_id: &str,
    items: &[storyos_core::NativeStreamItem],
    outcome: &FakeAttemptOutcome,
    decision_id: Option<&str>,
    opened_proposal: Option<&str>,
) -> serde_json::Value {
    let encoded_items: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let phase = match item.state {
                StreamItemState::Provisional => "provisional",
                StreamItemState::Complete => "complete",
                StreamItemState::Incomplete => "incomplete",
                StreamItemState::Failed => "failed",
                StreamItemState::Cancelled => "cancelled",
                StreamItemState::Unknown => "unknown",
            };
            serde_json::json!({
                "item_id": item.item_id,
                "role": match item.role {
                    StreamItemRole::Assistant => "assistant",
                    StreamItemRole::Tool => "tool",
                    StreamItemRole::Hosted => "hosted",
                },
                "state": phase,
                "phase": phase,
                "text": item.text,
                "summary": item.summary,
                "call_id": item.call_id,
                "arguments": item.arguments,
                "refusal": item.refusal,
                "hosted_report": item.hosted_report
            })
        })
        .collect();
    let decision = match (outcome, decision_id) {
        (
            FakeAttemptOutcome::Decision {
                kind,
                selected,
                advances_continuation,
            },
            Some(decision_id),
        ) => Some(match kind {
            FakeDecisionKind::Advisory { text } => serde_json::json!({
                "kind": "advisory",
                "decision_id": decision_id,
                "selected": selected,
                "text": text,
                "authoritative": false,
                "advances_continuation": advances_continuation
            }),
            FakeDecisionKind::ProseChange {
                text,
                producer_input,
            } => serde_json::json!({
                "kind": "prose_change",
                "decision_id": decision_id,
                "selected": selected,
                "text": text,
                "producer_input": producer_input,
                "authoritative": false,
                "advances_continuation": advances_continuation,
                "opened_proposal": match opened_proposal {
                    Some(proposal_id) => serde_json::json!({
                        "kind": "present",
                        "proposal_id": proposal_id
                    }),
                    None => serde_json::json!({ "kind": "absent" }),
                }
            }),
            FakeDecisionKind::Clarification { question } => serde_json::json!({
                "kind": "clarification",
                "decision_id": decision_id,
                "selected": selected,
                "question": question,
                "required_reply": question,
                "authoritative": false,
                "advances_continuation": advances_continuation
            }),
        }),
        _ => None,
    };
    serde_json::json!({
        "execution_profile": {
            "profile_revision": HOST_FAKE_EXECUTION_PROFILE,
            "mapping_revision": HOST_FAKE_MAPPING_REVISION,
            "network_io": false,
            "provider_bound": "unknown"
        },
        "wire": {
            "digest": host_fake_wire_digest(author_message, chapter_id),
            "author_message": author_message,
            "chapter_id": chapter_id
        },
        "items": encoded_items,
        "decision": decision,
        "usage": { "kind": "unknown" },
        "evidence": evidence_values(attempt_id, author_message, assembly_manifest_id)
    })
}

fn evidence_values(
    attempt_id: &str,
    author_message: &str,
    assembly_manifest_id: &str,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "kind": "sent_content",
            "attempt_id": attempt_id,
            "availability": "current",
            "content": author_message
        }),
        serde_json::json!({
            "kind": "stored_reference",
            "attempt_id": attempt_id,
            "availability": "current",
            "reference_id": assembly_manifest_id
        }),
        serde_json::json!({
            "kind": "provider_report",
            "attempt_id": attempt_id,
            "availability": "current",
            "report": "host_fake_no_provider_usage"
        }),
        serde_json::json!({
            "kind": "provider_opaque",
            "attempt_id": attempt_id,
            "availability": "unknown",
            "unknown_facts": ["provider_internal_content"]
        }),
    ]
}

async fn update_run(
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

async fn hold_if_requested(kind: &str) {
    let key = match kind {
        "dispatch" => "STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH",
        "stream" => "STORYOS_TEST_FAKE_STREAM_HOLD_PATH",
        "decision" => "STORYOS_TEST_FAKE_DECISION_HOLD_PATH",
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

fn complete_challenge_error(
    error: storyos_application::ProjectCommandChallengeError,
) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

fn complete_database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
