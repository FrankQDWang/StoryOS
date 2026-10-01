use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN, CompactionInstallFacts,
    CompactionInstallRefusal, ContextSufficiency, CurrentPassageAssembly,
    HOST_FAKE_COMPACTION_OUTPUT, HOST_FAKE_COMPACTION_PRODUCER, HOST_FAKE_MAPPING_REVISION,
    InstructionBindingInput, active_context_input_digest, assemble_current_passage_context,
    decide_compaction_install,
};
use uuid::Uuid;

use crate::create_agent_run::context::load_working_target;

use super::{
    CompactionState, DecisionSource, json_string, load_decision_source, unavailable, work_error,
};

pub(super) async fn stage_compaction(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    author_message: &str,
) -> Result<(), CompleteAgentRunError> {
    let source = load_decision_source(client, claim).await?;
    let (revision_id, body) = current_target(client, claim, &source.chapter_id).await?;
    let admission = source
        .payload
        .pointer("/continuation/admission")
        .cloned()
        .ok_or_else(|| unavailable("Compaction requires the current admission"))?;
    let known_inputs = known_input_values(&source);
    let preserved_item_ids = item_ids(&source.payload);
    let producer_attempt_id = Uuid::now_v7().to_string();
    let producer_manifest_id = Uuid::now_v7().to_string();
    let copy = AssemblyCopy {
        requirement_role: "compaction",
        manifest_role: "compaction",
        requirement_id: Uuid::now_v7().to_string(),
        snapshot_id: Uuid::now_v7().to_string(),
        manifest_id: producer_manifest_id.clone(),
        destination_manifest_id: Uuid::now_v7().to_string(),
        outbound_manifest_id: Uuid::now_v7().to_string(),
        receipt_id: source.receipt_id.clone(),
    };
    insert_assembly_copy(client, claim, &copy).await?;
    let producer_invocation_id = Uuid::now_v7().to_string();
    insert_attempt(
        client,
        claim,
        &AttemptInsert {
            attempt_id: producer_attempt_id.clone(),
            destination_attempt_id: Uuid::now_v7().to_string(),
            outbound_event_id: Uuid::now_v7().to_string(),
            destination_manifest_id: copy.destination_manifest_id.clone(),
            outbound_manifest_id: copy.outbound_manifest_id.clone(),
            wire_projection_id: Uuid::now_v7().to_string(),
            invocation_id: producer_invocation_id.clone(),
            conversation_id: source.conversation_id.clone(),
            attempt_role: "compaction",
            payload: serde_json::json!({
                "execution_profile": source.payload.get("execution_profile").cloned().unwrap_or(serde_json::json!({})),
                "usage": { "kind": "unknown" },
                "producer": HOST_FAKE_COMPACTION_PRODUCER,
                "mapping_kind": "host_managed",
                "mapping_revision": HOST_FAKE_MAPPING_REVISION,
                "known_inputs": known_inputs,
                "output_text": HOST_FAKE_COMPACTION_OUTPUT,
                "loss_facts": [COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN],
                "admission": admission,
                "items": source.payload.get("items").cloned().unwrap_or(serde_json::json!([]))
            }),
        },
    )
    .await?;
    client
        .execute(
            "INSERT INTO storyos.active_context_compactions
               (owner_user_id, project_id, compaction_id, run_id, prior_model_attempt_id,
                prior_manifest_id, prior_run_step_id, producer_model_attempt_id,
                producer_manifest_id, producer_invocation_id, producer, mapping_kind,
                mapping_revision, known_inputs, output_text, usage_kind, loss_facts,
                input_digest, install_state, preserved_item_ids, admission)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11, 'host_managed', $12,
                     $13::text::jsonb, $14, 'unknown', $15::text::jsonb, $16, 'staged',
                     $17::text::jsonb, $18::text::jsonb)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &Uuid::now_v7().to_string(),
                &claim.run_id,
                &source.model_attempt_id,
                &source.manifest_id,
                &Uuid::now_v7().to_string(),
                &producer_attempt_id,
                &producer_manifest_id,
                &producer_invocation_id,
                &HOST_FAKE_COMPACTION_PRODUCER,
                &HOST_FAKE_MAPPING_REVISION,
                &known_inputs.to_string(),
                &HOST_FAKE_COMPACTION_OUTPUT,
                &serde_json::json!([COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN]).to_string(),
                &active_context_input_digest(author_message, &revision_id, &body),
                &serde_json::Value::Array(preserved_item_ids).to_string(),
                &admission.to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(())
}

pub(super) async fn install_or_refuse(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    author_message: &str,
    staged: &CompactionState,
) -> Result<(), CompleteAgentRunError> {
    let source = load_decision_source(client, claim).await?;
    let (revision_id, body) = current_target(client, claim, &source.chapter_id).await?;
    let assembled = assemble_current_passage_context(&assembly_input(
        claim,
        author_message,
        &source,
        revision_id.clone(),
        body.clone(),
    )?);
    let decision = decide_compaction_install(&CompactionInstallFacts {
        source_restricted: source_restricted(&source.payload),
        exact_required_satisfied: matches!(assembled.sufficiency, ContextSufficiency::Complete),
        staged_input_digest: staged.input_digest.clone(),
        current_input_digest: active_context_input_digest(author_message, &revision_id, &body),
    });
    let Err(refusal) = decision else {
        return install_later_request(client, claim, author_message, staged, &source).await;
    };
    let reason = match refusal {
        CompactionInstallRefusal::RestrictedSource => "restricted_source",
        CompactionInstallRefusal::ExactRequiredUnsatisfied => "exact_required_unsatisfied",
        CompactionInstallRefusal::ChangedInput => "changed_input",
    };
    let updated = client
        .execute(
            "UPDATE storyos.active_context_compactions
                SET install_state = 'refused', refusal_reason = $4
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND install_state = 'staged'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &reason,
            ],
        )
        .await
        .map_err(work_error)?;
    if updated != 1 {
        return Err(unavailable("The staged compaction did not refuse"));
    }
    Ok(())
}

async fn install_later_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    author_message: &str,
    staged: &CompactionState,
    source: &DecisionSource,
) -> Result<(), CompleteAgentRunError> {
    let attempt_id = Uuid::now_v7().to_string();
    let invocation_id = Uuid::now_v7().to_string();
    let run_step_id = Uuid::now_v7().to_string();
    let copy = AssemblyCopy {
        requirement_role: "later_request",
        manifest_role: "later_request",
        requirement_id: Uuid::now_v7().to_string(),
        snapshot_id: Uuid::now_v7().to_string(),
        manifest_id: Uuid::now_v7().to_string(),
        destination_manifest_id: Uuid::now_v7().to_string(),
        outbound_manifest_id: Uuid::now_v7().to_string(),
        receipt_id: source.receipt_id.clone(),
    };
    insert_assembly_copy(client, claim, &copy).await?;
    insert_attempt(
        client,
        claim,
        &AttemptInsert {
            attempt_id: attempt_id.clone(),
            destination_attempt_id: Uuid::now_v7().to_string(),
            outbound_event_id: Uuid::now_v7().to_string(),
            destination_manifest_id: copy.destination_manifest_id,
            outbound_manifest_id: copy.outbound_manifest_id,
            wire_projection_id: Uuid::now_v7().to_string(),
            invocation_id: invocation_id.clone(),
            conversation_id: source.conversation_id.clone(),
            attempt_role: "later_request",
            payload: serde_json::json!({
                "execution_profile": source.payload.get("execution_profile").cloned().unwrap_or(serde_json::json!({})),
                "usage": { "kind": "unknown" },
                "author_message": author_message,
                "chapter_id": source.chapter_id,
                "compaction_id": staged.compaction_id,
                "run_step_id": run_step_id,
                "items": source.payload.get("items").cloned().unwrap_or(serde_json::json!([])),
                "decision": null
            }),
        },
    )
    .await?;
    let updated = client
        .execute(
            "UPDATE storyos.active_context_compactions
                SET install_state = 'installed',
                    installed_run_step_id = $4::text::uuid,
                    installed_model_invocation_id = $5::text::uuid,
                    installed_model_attempt_id = $6::text::uuid
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND compaction_id = $3::text::uuid
                AND install_state = 'staged'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &staged.compaction_id,
                &run_step_id,
                &invocation_id,
                &attempt_id,
            ],
        )
        .await
        .map_err(work_error)?;
    if updated != 1 {
        return Err(unavailable("The staged compaction did not install"));
    }
    Ok(())
}

struct AssemblyCopy {
    requirement_role: &'static str,
    manifest_role: &'static str,
    requirement_id: String,
    snapshot_id: String,
    manifest_id: String,
    destination_manifest_id: String,
    outbound_manifest_id: String,
    receipt_id: String,
}

async fn insert_assembly_copy(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    copy: &AssemblyCopy,
) -> Result<(), CompleteAgentRunError> {
    let rebound = "jsonb_set(jsonb_set(jsonb_set(payload, \
        '{operation_requirement,run_id}', to_jsonb($3::text), true), \
        '{operation_requirement,operation_requirement_id}', to_jsonb($5::text), true), \
        '{operation_requirement,input_snapshot_id}', to_jsonb($6::text), true)";
    let requirement = client
        .execute(
            &format!(
                "INSERT INTO storyos.operation_requirements
                   (owner_user_id, project_id, operation_requirement_id, run_id,
                    input_snapshot_id, receipt_id, payload, requirement_role)
                 SELECT owner_user_id, project_id, $5::text::uuid, run_id,
                        $6::text::uuid, $4::text::uuid, {rebound}, $7
                   FROM storyos.operation_requirements
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND requirement_role = 'primary' AND decision_position = (SELECT active_decision_position FROM storyos.agent_runs
                      WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND run_id=$3::text::uuid)"
            ),
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &copy.receipt_id,
                &copy.requirement_id,
                &copy.snapshot_id,
                &copy.requirement_role,
            ],
        )
        .await
        .map_err(work_error)?;
    if requirement != 1 {
        return Err(unavailable("The primary Context Assembly is missing"));
    }
    let manifest = client
        .execute(
            &format!(
                "INSERT INTO storyos.context_assembly_manifests
                   (owner_user_id, project_id, context_assembly_manifest_id,
                    operation_requirement_id, run_id, sufficiency,
                    destination_context_manifest_id, outbound_disclosure_manifest_id,
                    payload, receipt_id, manifest_role)
                 SELECT owner_user_id, project_id, $7::text::uuid, $5::text::uuid, run_id,
                        sufficiency, $8::text::uuid, $9::text::uuid, {rebound},
                        $4::text::uuid, $10
                   FROM storyos.context_assembly_manifests
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND manifest_role = 'decision' AND decision_position = (SELECT active_decision_position FROM storyos.agent_runs
                      WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND run_id=$3::text::uuid)"
            ),
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &copy.receipt_id,
                &copy.requirement_id,
                &copy.snapshot_id,
                &copy.manifest_id,
                &copy.destination_manifest_id,
                &copy.outbound_manifest_id,
                &copy.manifest_role,
            ],
        )
        .await
        .map_err(work_error)?;
    if manifest != 1 {
        return Err(unavailable(
            "The primary Context Assembly Manifest is missing",
        ));
    }
    Ok(())
}

struct AttemptInsert {
    attempt_id: String,
    destination_attempt_id: String,
    outbound_event_id: String,
    destination_manifest_id: String,
    outbound_manifest_id: String,
    wire_projection_id: String,
    invocation_id: String,
    conversation_id: String,
    attempt_role: &'static str,
    payload: serde_json::Value,
}

async fn insert_attempt(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    attempt: &AttemptInsert,
) -> Result<(), CompleteAgentRunError> {
    client
        .execute(
            "INSERT INTO storyos.model_attempts
               (owner_user_id, project_id, run_id, model_attempt_id, destination_attempt_id,
                outbound_disclosure_event_id, destination_context_manifest_id,
                outbound_disclosure_manifest_id, wire_payload_projection_id,
                model_invocation_id, conversation_id, dispatch_state, payload, attempt_role)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11::text::uuid, 'settled',
                     $12::text::jsonb, $13)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &attempt.attempt_id,
                &attempt.destination_attempt_id,
                &attempt.outbound_event_id,
                &attempt.destination_manifest_id,
                &attempt.outbound_manifest_id,
                &attempt.wire_projection_id,
                &attempt.invocation_id,
                &attempt.conversation_id,
                &attempt.payload.to_string(),
                &attempt.attempt_role,
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(())
}

fn known_input_values(source: &DecisionSource) -> serde_json::Value {
    let mut values = vec![
        serde_json::json!({"kind": "model_attempt", "id": source.model_attempt_id}),
        serde_json::json!({"kind": "manifest", "id": source.manifest_id}),
    ];
    if let Some(selected) = source
        .manifest_payload
        .get("selected")
        .and_then(serde_json::Value::as_array)
    {
        for item in selected {
            values.push(serde_json::json!({
                "kind": "projection",
                "source_class": item.get("source_class").and_then(serde_json::Value::as_str).unwrap_or_default(),
                "source_version": item.get("source_version").and_then(serde_json::Value::as_str).unwrap_or_default()
            }));
        }
    }
    serde_json::Value::Array(values)
}

fn item_ids(payload: &serde_json::Value) -> Vec<serde_json::Value> {
    payload
        .get("items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("item_id").cloned())
        .collect()
}

fn source_restricted(payload: &serde_json::Value) -> bool {
    payload
        .pointer("/produced_binding/covered_copy_restricted")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        || payload
            .get("covered_copy_restricted")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
}

fn assembly_input(
    claim: &ClaimedAgentRun,
    author_message: &str,
    source: &DecisionSource,
    chapter_revision_id: String,
    chapter_body: String,
) -> Result<CurrentPassageAssembly, CompleteAgentRunError> {
    let requirement = source
        .manifest_payload
        .get("operation_requirement")
        .ok_or_else(|| unavailable("The primary operation requirement is missing"))?;
    Ok(CurrentPassageAssembly {
        operation_requirement_id: json_string(requirement, "operation_requirement_id")?,
        input_snapshot_id: json_string(requirement, "input_snapshot_id")?,
        run_id: claim.run_id.clone(),
        owner_user_id: claim.project_scope.owner_user_id.as_ref().to_owned(),
        project_id: claim.project_scope.project_id.as_ref().to_owned(),
        author_message: author_message.to_owned(),
        chapter_id: source.chapter_id.clone(),
        chapter_revision_id: Some(chapter_revision_id).filter(|revision| revision != "unavailable"),
        proposal_target_block_ids: requirement
            .get("proposal_target_block_ids")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            }),
        chapter_body,
        instruction: match requirement
            .pointer("/instruction/kind")
            .and_then(serde_json::Value::as_str)
        {
            Some("required_revision") => InstructionBindingInput::RequiredRevision {
                revision_id: json_string(requirement, "revision_id").or_else(|_| {
                    requirement
                        .pointer("/instruction/revision_id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| unavailable("The required instruction revision is missing"))
                })?,
                available: requirement
                    .pointer("/instruction/available")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            },
            _ => InstructionBindingInput::Absent,
        },
        destination_identity: json_string(requirement, "destination_identity")?,
    })
}

async fn current_target(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
) -> Result<(String, String), CompleteAgentRunError> {
    let (revision, body) = load_working_target(client, &claim.project_scope, chapter_id)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    Ok((revision.unwrap_or_else(|| "unavailable".to_owned()), body))
}
