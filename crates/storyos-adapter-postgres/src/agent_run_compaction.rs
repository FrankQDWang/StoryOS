use storyos_application::{
    ActiveCompactionInspect, ActiveCompactionInstallState, ActiveCompactionKnownInput,
    ActiveCompactionMappingKind, AgentRunContinuationAdmission, ClaimedAgentRun,
    CompleteAgentRunError, CreateAgentRunError, DestinationRequest, DispatchClaim, ProjectScope,
    RequestAttempt,
};

use crate::agent_run_work::RunPhaseRow;
use storyos_core::{CompactionRecordState, CompactionStep, next_compaction_step};

pub(crate) enum CompactionAdvance {
    /// Send the summary request.
    Dispatch(Box<DestinationRequest>),
    /// Send the decision request. It is the later request when the summary installed.
    Decide { later_request: bool },
}

/// Advances the active compaction of the claimed AgentRun before its first decision request.
pub(crate) async fn advance_active_compaction(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<CompactionAdvance, CompleteAgentRunError> {
    let staged = load_compaction_state(client, claim).await?;
    let summary = summary::load_summary_attempt(client, claim).await?;
    let record = match (
        staged.as_ref().map(|state| state.install_state.as_str()),
        &summary,
    ) {
        (Some("staged"), _) if run.later_request => CompactionRecordState::Accepted,
        (Some("staged"), _) => CompactionRecordState::Staged,
        (Some("installed"), _) => CompactionRecordState::Installed,
        (Some("refused"), _) => CompactionRecordState::Refused,
        (Some(_), _) => return Err(unavailable("The compaction state is unknown")),
        (None, Some(_)) => CompactionRecordState::SummaryClaimed,
        (None, None) => CompactionRecordState::Absent,
    };
    let chain = match record {
        CompactionRecordState::Absent => summary::prior_chain(client, claim, run).await?,
        _ => None,
    };
    let ordinary = Ok(CompactionAdvance::Decide {
        later_request: false,
    });
    match (
        next_compaction_step(&run.author_message, chain.is_some(), record),
        staged,
        summary,
        chain,
    ) {
        (CompactionStep::RequestSummary, _, _, Some(chain)) => {
            // A refused summary request leaves the ordinary continuation for the decision request.
            if !summary::admitted(client, claim, run).await? {
                return ordinary;
            }
            summary::commit_assembly(client, claim, run).await?;
            Ok(CompactionAdvance::Dispatch(Box::new(
                summary::summary_request(client, claim, run, RequestAttempt::New, chain.reference)
                    .await?,
            )))
        }
        (CompactionStep::ReobserveSummary, _, Some(summary), _) => {
            let attempt = RequestAttempt::Claimed(DispatchClaim {
                model_attempt_id: summary.model_attempt_id,
            });
            Ok(CompactionAdvance::Dispatch(Box::new(
                summary::summary_request(client, claim, run, attempt, summary.prior_reference)
                    .await?,
            )))
        }
        (CompactionStep::InstallOrRefuse, Some(staged), _, _) => Ok(CompactionAdvance::Decide {
            later_request: write::install_or_refuse(client, claim, run, &staged).await?,
        }),
        // The source can become restricted after the install check. Check it again before the
        // later request goes out.
        (CompactionStep::LaterRequest, Some(staged), _, _)
            if record == CompactionRecordState::Accepted =>
        {
            Ok(CompactionAdvance::Decide {
                later_request: write::source_still_eligible(client, claim, &staged).await?,
            })
        }
        (CompactionStep::LaterRequest, _, _, _) => Ok(CompactionAdvance::Decide {
            later_request: true,
        }),
        (CompactionStep::RequestSummary, _, _, None)
        | (CompactionStep::ReobserveSummary, _, None, _)
        | (CompactionStep::InstallOrRefuse, None, _, _)
        | (CompactionStep::OrdinaryRequest, _, _, _) => ordinary,
    }
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
    prior_model_attempt_id: String,
    output_text: String,
}

async fn load_compaction_state(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<CompactionState>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT compaction_id::text, install_state, input_digest,
                    prior_model_attempt_id::text, output_text
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
        compaction_id: row.get(/*idx*/ 0),
        install_state: row.get(/*idx*/ 1),
        input_digest: row.get(/*idx*/ 2),
        prior_model_attempt_id: row.get(/*idx*/ 3),
        output_text: row.get(/*idx*/ 4),
    }))
}

/// The prior decision Attempt of the Conversation that a compaction summarizes, with the Context
/// Assembly Manifest that its request disclosed.
struct ChainSource {
    model_attempt_id: String,
    manifest_id: String,
    payload: serde_json::Value,
    manifest_payload: serde_json::Value,
}

async fn load_chain_source(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    model_attempt_id: &str,
) -> Result<ChainSource, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT attempt.payload::text, assembly.context_assembly_manifest_id::text,
                    assembly.payload::text
               FROM storyos.model_attempts AS attempt
               JOIN storyos.context_assembly_manifests AS assembly
                 ON (assembly.owner_user_id, assembly.project_id,
                     assembly.destination_context_manifest_id) =
                    (attempt.owner_user_id, attempt.project_id,
                     attempt.destination_context_manifest_id)
              WHERE attempt.owner_user_id = $1::text::uuid
                AND attempt.project_id = $2::text::uuid
                AND attempt.model_attempt_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &model_attempt_id,
            ],
        )
        .await
        .map_err(work_error)?
        .ok_or_else(|| unavailable("The prior decision Attempt is missing"))?;
    Ok(ChainSource {
        model_attempt_id: model_attempt_id.to_owned(),
        manifest_id: row.get(/*idx*/ 1),
        payload: parse_json(&row.get::<_, String>(/*idx*/ 0))?,
        manifest_payload: parse_json(&row.get::<_, String>(/*idx*/ 2))?,
    })
}

#[path = "agent_run_compaction_write.rs"]
mod write;
pub(crate) use write::{mark_installed, source_restricted_now};

#[path = "agent_run_compaction_summary.rs"]
pub(crate) mod summary;

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
