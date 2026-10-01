use storyos_application::{
    ActiveCompactionInspect, ActiveCompactionInstallState, ActiveCompactionKnownInput,
    ActiveCompactionMappingKind, AgentRunContinuationAdmission, ClaimedAgentRun,
    CompleteAgentRunError, CreateAgentRunError, ProjectScope,
};

pub(crate) enum CompactionAdvance {
    Hold,
    ReadyToComplete,
}

pub(crate) async fn advance_active_compaction(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    author_message: &str,
) -> Result<CompactionAdvance, CompleteAgentRunError> {
    let Some(staged) = load_compaction_state(client, claim).await? else {
        write::stage_compaction(client, claim, author_message).await?;
        return Ok(CompactionAdvance::Hold);
    };
    if staged.install_state == "staged" {
        write::install_or_refuse(client, claim, author_message, &staged).await?;
    }
    Ok(CompactionAdvance::ReadyToComplete)
}

pub(crate) async fn load_active_compaction(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<ActiveCompactionInspect>, CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT compaction_id::text, install_state, prior_model_attempt_id::text,
                    prior_manifest_id::text, prior_run_step_id::text,
                    producer_model_attempt_id::text, producer_manifest_id::text,
                    producer_invocation_id::text, producer, mapping_kind, mapping_revision,
                    known_inputs::text, output_text, usage_kind, loss_facts::text,
                    refusal_reason, installed_run_step_id::text,
                    installed_model_invocation_id::text, installed_model_attempt_id::text,
                    preserved_item_ids::text, admission::text
               FROM storyos.active_context_compactions
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(read_error)?;
    row.map(|row| inspect_row(&row)).transpose()
}

struct CompactionState {
    compaction_id: String,
    install_state: String,
    input_digest: String,
}

async fn load_compaction_state(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<CompactionState>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT compaction_id::text, install_state, input_digest
               FROM storyos.active_context_compactions
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(work_error)?;
    Ok(row.map(|row| CompactionState {
        compaction_id: row.get(0),
        install_state: row.get(1),
        input_digest: row.get(2),
    }))
}

struct DecisionSource {
    receipt_id: String,
    conversation_id: String,
    chapter_id: String,
    model_attempt_id: String,
    manifest_id: String,
    payload: serde_json::Value,
    manifest_payload: serde_json::Value,
}

async fn load_decision_source(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<DecisionSource, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT run.receipt_id::text, run.conversation_id::text, run.chapter_id::text,
                    attempt.model_attempt_id::text, attempt.payload::text,
                    assembly.context_assembly_manifest_id::text, assembly.payload::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND attempt.attempt_role = 'decision' AND attempt.decision_position = run.active_decision_position
               JOIN storyos.context_assembly_manifests AS assembly
                 ON (assembly.owner_user_id, assembly.project_id, assembly.run_id) =
                    (run.owner_user_id, run.project_id, run.run_id)
                AND assembly.manifest_role = 'decision' AND assembly.decision_position = run.active_decision_position
              WHERE run.owner_user_id = $1::text::uuid
                AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid
                AND run.fence_token = $4",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &claim.fence_token,
            ],
        )
        .await
        .map_err(work_error)?
        .ok_or_else(|| unavailable("The submitted Attempt is missing"))?;
    let payload = parse_json(&row.get::<_, String>(4))?;
    let manifest_payload = parse_json(&row.get::<_, String>(6))?;
    Ok(DecisionSource {
        receipt_id: row.get(0),
        conversation_id: row.get(1),
        chapter_id: row.get(2),
        model_attempt_id: row.get(3),
        manifest_id: row.get(5),
        payload,
        manifest_payload,
    })
}

#[path = "agent_run_compaction_write.rs"]
mod write;

fn inspect_row(row: &tokio_postgres::Row) -> Result<ActiveCompactionInspect, CreateAgentRunError> {
    let known_inputs = parse_json(&row.get::<_, String>(11)).map_err(read_error)?;
    let loss_facts = parse_json(&row.get::<_, String>(14)).map_err(read_error)?;
    let preserved = parse_json(&row.get::<_, String>(19)).map_err(read_error)?;
    let admission = parse_json(&row.get::<_, String>(20)).map_err(read_error)?;
    Ok(ActiveCompactionInspect {
        compaction_id: row.get(0),
        install_state: match row.get::<_, String>(1).as_str() {
            "staged" => ActiveCompactionInstallState::Staged,
            "installed" => ActiveCompactionInstallState::Installed,
            "refused" => ActiveCompactionInstallState::Refused,
            _ => {
                return Err(read_error(std::io::Error::other(
                    "The compaction state is unknown",
                )));
            }
        },
        prior_model_attempt_id: row.get(2),
        prior_manifest_id: row.get(3),
        prior_run_step_id: row.get(4),
        producer_model_attempt_id: row.get(5),
        producer_manifest_id: row.get(6),
        producer_invocation_id: row.get(7),
        producer: row.get(8),
        mapping_kind: match row.get::<_, String>(9).as_str() {
            "host_managed" => ActiveCompactionMappingKind::HostManaged,
            "native" => ActiveCompactionMappingKind::Native,
            _ => {
                return Err(read_error(std::io::Error::other(
                    "The compaction mapping is unknown",
                )));
            }
        },
        mapping_revision: row.get(10),
        known_inputs: parse_known_inputs(&known_inputs)?,
        output_text: row.get(12),
        usage_kind: row.get(13),
        loss_facts: string_array(&loss_facts)?,
        refusal_reason: row.get(15),
        installed_run_step_id: row.get(16),
        installed_model_invocation_id: row.get(17),
        installed_model_attempt_id: row.get(18),
        preserved_item_ids: string_array(&preserved)?,
        admission: admission_from_json(&admission)?,
    })
}

fn parse_known_inputs(
    value: &serde_json::Value,
) -> Result<Vec<ActiveCompactionKnownInput>, CreateAgentRunError> {
    let items = value
        .as_array()
        .ok_or_else(|| read_error(std::io::Error::other("Compaction inputs are damaged")))?;
    items
        .iter()
        .map(
            |item| match item.get("kind").and_then(serde_json::Value::as_str) {
                Some("model_attempt") => Ok(ActiveCompactionKnownInput::ModelAttempt {
                    id: json_string(item, "id").map_err(read_error)?,
                }),
                Some("manifest") => Ok(ActiveCompactionKnownInput::Manifest {
                    id: json_string(item, "id").map_err(read_error)?,
                }),
                Some("projection") => Ok(ActiveCompactionKnownInput::Projection {
                    source_class: json_string(item, "source_class").map_err(read_error)?,
                    source_version: json_string(item, "source_version").map_err(read_error)?,
                }),
                _ => Err(read_error(std::io::Error::other(
                    "A compaction input kind is unknown",
                ))),
            },
        )
        .collect()
}

fn string_array(value: &serde_json::Value) -> Result<Vec<String>, CreateAgentRunError> {
    value
        .as_array()
        .ok_or_else(|| read_error(std::io::Error::other("A compaction list is damaged")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| read_error(std::io::Error::other("A compaction list is damaged")))
        })
        .collect()
}

fn admission_from_json(
    value: &serde_json::Value,
) -> Result<AgentRunContinuationAdmission, CreateAgentRunError> {
    Ok(AgentRunContinuationAdmission {
        processing_destination_identity: json_string(value, "processing_destination_identity")
            .map_err(read_error)?,
        evidence_revision: json_string(value, "evidence_revision").map_err(read_error)?,
        model_registration_revision: json_string(value, "model_registration_revision")
            .map_err(read_error)?,
        adapter_mapping: json_string(value, "adapter_mapping").map_err(read_error)?,
        project_model_use_binding_revision: json_string(
            value,
            "project_model_use_binding_revision",
        )
        .map_err(read_error)?,
        external_compatibility_decision: json_string(value, "external_compatibility_decision")
            .map_err(read_error)?,
    })
}

fn json_string(value: &serde_json::Value, field: &str) -> Result<String, CompleteAgentRunError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| unavailable("A compaction field is missing"))
}

fn parse_json(value: &str) -> Result<serde_json::Value, CompleteAgentRunError> {
    serde_json::from_str(value).map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))
}

fn unavailable(message: &str) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(message)))
}

fn work_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

fn read_error(error: impl std::error::Error + Send + Sync + 'static) -> CreateAgentRunError {
    CreateAgentRunError::Unavailable(Box::new(error))
}
