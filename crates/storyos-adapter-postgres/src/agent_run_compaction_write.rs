use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    CompactionInstallFacts, CompactionKnownInput, CompactionSource, ContextSufficiency,
    CurrentPassageAssembly, InstructionBindingInput, StagedCompaction, active_context_input_digest,
    assemble_current_passage_context, decide_compaction_install, encode_assembly_record,
    later_request_record, summary_fits_item,
};
use uuid::Uuid;

use crate::agent_run_work::RunPhaseRow;
use crate::create_agent_run::context::load_working_target;

use super::{
    ChainSource, CompactionState, json_string, load_chain_source, parse_json, unavailable,
    work_error,
};

/// The summary Attempt that produced one compaction result.
pub(super) struct Producer {
    pub model_attempt_id: String,
    pub manifest_id: String,
    pub invocation_id: String,
    pub usage_kind: &'static str,
}

/// Inserts the compaction record of one summary result: staged, or refused with no summary. The
/// input digest binds the author message and the Working Target of the decision Context Assembly
/// that the later request copies.
pub(super) async fn insert_staged(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    source: &ChainSource,
    staged: &StagedCompaction,
    producer: &Producer,
) -> Result<(), CompleteAgentRunError> {
    let decision = run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| unavailable("Compaction requires the decision Context Assembly"))?;
    let (revision_id, body) = decision_target(decision);
    let admission = source
        .payload
        .pointer("/continuation/admission")
        .cloned()
        .ok_or_else(|| unavailable("Compaction requires the prior admission"))?;
    let (install_state, refusal_reason) = match staged.refusal {
        None => ("staged", None),
        Some(reason) => ("refused", Some(reason)),
    };
    client
        .execute(
            "INSERT INTO storyos.active_context_compactions
               (owner_user_id, project_id, compaction_id, run_id, prior_model_attempt_id,
                prior_manifest_id, prior_run_step_id, producer_model_attempt_id,
                producer_manifest_id, producer_invocation_id, producer, mapping_kind,
                mapping_revision, known_inputs, output_text, usage_kind, loss_facts,
                input_digest, install_state, preserved_item_ids, admission, refusal_reason)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, $10::text::uuid, $11, 'host_managed', $12,
                     $13::text::jsonb, $14, $15, $16::text::jsonb, $17, $18,
                     $19::text::jsonb, $20::text::jsonb, $21)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &Uuid::now_v7().to_string(),
                &claim.run_id,
                &source.model_attempt_id,
                &source.manifest_id,
                &Uuid::now_v7().to_string(),
                &producer.model_attempt_id,
                &producer.manifest_id,
                &producer.invocation_id,
                &staged.producer,
                &staged.mapping_revision,
                &known_input_values(&staged.known_inputs).to_string(),
                &staged.output_text,
                &producer.usage_kind,
                &serde_json::json!(staged.loss_facts).to_string(),
                &active_context_input_digest(
                    &run.author_message,
                    &revision_id,
                    body.unwrap_or_default(),
                ),
                &install_state,
                &serde_json::json!(staged.preserved_item_ids).to_string(),
                &admission.to_string(),
                &refusal_reason,
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(())
}

/// Decides the install of the staged summary for the decision request. An install commits the
/// Context Assembly of the later request and returns `true`. A refusal records its reason, and
/// the decision request then uses the ordinary continuation.
pub(super) async fn install_or_refuse(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    staged: &CompactionState,
) -> Result<bool, CompleteAgentRunError> {
    let source = load_chain_source(client, claim, &staged.prior_model_attempt_id).await?;
    let decision = run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| unavailable("Compaction requires the decision Context Assembly"))?;
    let (revision_id, body) = current_target(client, claim, &run.chapter_id).await?;
    let assembled = assemble_current_passage_context(&assembly_input(
        claim,
        run,
        &parse_json(&run.assembly_payload)?,
        revision_id.clone(),
        body.clone(),
    )?);
    // A decision Context Assembly with no Working Target item binds only the revision.
    let current_body = match decision_target(decision).1 {
        Some(_) => body.as_str(),
        None => "",
    };
    let checked = decide_compaction_install(&CompactionInstallFacts {
        source_restricted: source_restricted(&source.payload),
        exact_required_satisfied: matches!(assembled.sufficiency, ContextSufficiency::Complete),
        staged_input_digest: staged.input_digest.clone(),
        current_input_digest: active_context_input_digest(
            &run.author_message,
            &revision_id,
            current_body,
        ),
        summary_within_item_limit: summary_fits_item(decision, &staged.output_text),
    });
    let Err(refusal) = checked else {
        insert_later_request(client, claim, run, staged, decision).await?;
        return Ok(true);
    };
    refuse(client, claim, refusal.code()).await?;
    Ok(false)
}

/// Whether the source of the accepted install is still eligible before the later request goes
/// out. A source that became restricted refuses the install, and the decision request then uses
/// the ordinary continuation.
pub(super) async fn source_still_eligible(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    staged: &CompactionState,
) -> Result<bool, CompleteAgentRunError> {
    let source = load_chain_source(client, claim, &staged.prior_model_attempt_id).await?;
    if !source_restricted(&source.payload) {
        return Ok(true);
    }
    refuse(
        client,
        claim,
        storyos_core::CompactionInstallRefusal::RestrictedSource.code(),
    )
    .await?;
    Ok(false)
}

/// Whether the source of the staged install of the claimed AgentRun is restricted now.
pub(crate) async fn source_restricted_now(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    let staged = super::load_compaction_state(client, claim)
        .await?
        .ok_or_else(|| unavailable("The staged compaction is missing"))?;
    let source = load_chain_source(client, claim, &staged.prior_model_attempt_id).await?;
    Ok(source_restricted(&source.payload))
}

async fn refuse(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    reason: &str,
) -> Result<(), CompleteAgentRunError> {
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

/// The revision and the Working Target text of a decision Context Assembly. The text is `None`
/// when the assembly selects no Working Target item.
fn decision_target(
    decision: &storyos_core::CurrentPassageAssemblyRecord,
) -> (String, Option<&str>) {
    (
        decision
            .operation_requirement
            .chapter_revision_id
            .clone()
            .unwrap_or_else(|| "unavailable".to_owned()),
        decision
            .selected
            .iter()
            .find(|item| item.source_class == storyos_core::ContextSourceClass::WorkingTarget)
            .map(|item| item.content.as_str()),
    )
}

/// Commits the operation requirement and the Context Assembly Manifest of the later request. Its
/// new Step Snapshot selects the summary before the decision Context.
async fn insert_later_request(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    staged: &CompactionState,
    decision: &storyos_core::CurrentPassageAssemblyRecord,
) -> Result<(), CompleteAgentRunError> {
    let [requirement_id, snapshot_id, manifest_id] = [(); 3].map(|()| Uuid::now_v7().to_string());
    let mut record = later_request_record(decision, &staged.compaction_id, &staged.output_text);
    record.operation_requirement.operation_requirement_id = requirement_id.clone();
    record.operation_requirement.input_snapshot_id = snapshot_id.clone();
    let payload = encode_assembly_record(&record);
    let scope = [
        &claim.project_scope.owner_user_id.as_ref(),
        &claim.project_scope.project_id.as_ref(),
    ];
    let requirement = client
        .execute(
            "INSERT INTO storyos.operation_requirements
               (owner_user_id, project_id, operation_requirement_id, run_id, input_snapshot_id,
                receipt_id, payload, requirement_role, decision_position)
             SELECT owner_user_id, project_id, $4::text::uuid, run_id, $5::text::uuid,
                    receipt_id, $6::text::jsonb, 'later_request', decision_position
               FROM storyos.operation_requirements
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND operation_requirement_id = $3::text::uuid",
            &[
                scope[0],
                scope[1],
                &decision.operation_requirement.operation_requirement_id,
                &requirement_id,
                &snapshot_id,
                &payload["operation_requirement"].to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    let manifest = client
        .execute(
            "INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id,
                operation_requirement_id, run_id, sufficiency, payload, receipt_id,
                manifest_role, decision_position)
             SELECT owner_user_id, project_id, $4::text::uuid, $5::text::uuid, run_id,
                    sufficiency, $6::text::jsonb, receipt_id, 'later_request', decision_position
               FROM storyos.context_assembly_manifests
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND context_assembly_manifest_id = $3::text::uuid",
            &[
                scope[0],
                scope[1],
                &run.assembly_manifest_id,
                &manifest_id,
                &requirement_id,
                &payload.to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    if requirement != 1 || manifest != 1 {
        return Err(unavailable("The decision Context Assembly is missing"));
    }
    Ok(())
}

/// Records the install in the dispatch claim of the later request: its new RunStep, and the
/// Model Invocation and Model Attempt of the claim.
pub(crate) async fn mark_installed(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    model_attempt_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let updated = client
        .execute(
            "UPDATE storyos.active_context_compactions AS record
                SET install_state = 'installed',
                    installed_run_step_id = $5::text::uuid,
                    installed_model_invocation_id = attempt.model_invocation_id,
                    installed_model_attempt_id = attempt.model_attempt_id
               FROM storyos.model_attempts AS attempt
              WHERE record.owner_user_id = $1::text::uuid
                AND record.project_id = $2::text::uuid
                AND record.run_id = $3::text::uuid
                AND record.install_state = 'staged'
                AND (attempt.owner_user_id, attempt.project_id, attempt.model_attempt_id) =
                    (record.owner_user_id, record.project_id, $4::text::uuid)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &model_attempt_id,
                &Uuid::now_v7().to_string(),
            ],
        )
        .await
        .map_err(work_error)?;
    if updated != 1 {
        return Err(unavailable("The staged compaction did not install"));
    }
    Ok(())
}

/// The prior Attempt, its manifest, its selected projections, and its stream item ids.
pub(super) fn compaction_source(source: &ChainSource) -> CompactionSource {
    let text = |item: &serde_json::Value, field: &str| {
        item.get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let list = |value: &serde_json::Value, field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    CompactionSource {
        model_attempt_id: source.model_attempt_id.clone(),
        manifest_id: source.manifest_id.clone(),
        selected: list(&source.manifest_payload, "selected")
            .iter()
            .map(|item| (text(item, "source_class"), text(item, "source_version")))
            .collect(),
        item_ids: list(&source.payload, "items")
            .iter()
            .filter_map(|item| item.get("item_id")?.as_str().map(str::to_owned))
            .collect(),
    }
}

fn known_input_values(inputs: &[CompactionKnownInput]) -> serde_json::Value {
    inputs
        .iter()
        .map(|input| match input {
            CompactionKnownInput::ModelAttempt { id } => {
                serde_json::json!({"kind": "model_attempt", "id": id})
            }
            CompactionKnownInput::Manifest { id } => {
                serde_json::json!({"kind": "manifest", "id": id})
            }
            CompactionKnownInput::Projection {
                source_class,
                source_version,
            } => serde_json::json!({
                "kind": "projection",
                "source_class": source_class,
                "source_version": source_version
            }),
        })
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
    run: &RunPhaseRow,
    decision_payload: &serde_json::Value,
    chapter_revision_id: String,
    chapter_body: String,
) -> Result<CurrentPassageAssembly, CompleteAgentRunError> {
    let requirement = decision_payload
        .get("operation_requirement")
        .ok_or_else(|| unavailable("The primary operation requirement is missing"))?;
    Ok(CurrentPassageAssembly {
        operation_requirement_id: json_string(requirement, "operation_requirement_id")?,
        input_snapshot_id: json_string(requirement, "input_snapshot_id")?,
        run_id: claim.run_id.clone(),
        owner_user_id: claim.project_scope.owner_user_id.as_ref().to_owned(),
        project_id: claim.project_scope.project_id.as_ref().to_owned(),
        author_message: run.author_message.clone(),
        chapter_id: run.chapter_id.clone(),
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
