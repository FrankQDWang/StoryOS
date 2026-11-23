//! The query `getSubrun` and the wire shapes of the reads of the Subruns of a Run (ADR 0103
//! section 3A, "The reads").

use storyos_application::inspect_subrun;
use storyos_core::{SubrunState, SubrunStep};

use super::create_agent_run::agent_run_read_error;
use super::*;

pub(super) async fn get_subrun(
    State(state): State<Arc<ServerState>>,
    Path((project_id, subrun_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<contracts::GetSubrunResponse>, ApiError> {
    let scope = authenticate_scope(
        &state,
        &headers,
        &project_id,
        RequestOriginPolicy::SensitiveSafeReadWithRefererFallback,
    )?;
    valid_uuid(&subrun_id)?;
    let reader = project_reader(&state).await?;
    // Each identity that is not a Subrun of a Run of the author of the Project answers as an
    // AgentRun that does not exist.
    let Some(record) = inspect_subrun(&reader, &scope, &subrun_id)
        .await
        .map_err(agent_run_read_error)?
    else {
        return Err(resource_unavailable());
    };
    Ok(Json(contracts::GetSubrunResponse {
        schema_id: contracts::GET_SUBRUN_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: Uuid::now_v7().to_string(),
        project_scope: contract_scope(&scope),
        subrun_id,
        title: record.title,
        brief: record.brief,
        state: subrun_state(record.state),
        step_count: record.step_count,
        steps: record.steps.into_iter().map(subrun_step).collect(),
        answer: record.answer,
    }))
}

pub(super) fn subrun_state(state: SubrunState) -> contracts::SubrunStateInspect {
    match state {
        SubrunState::Working => contracts::SubrunStateInspect::Working,
        SubrunState::Completed => contracts::SubrunStateInspect::Completed,
        SubrunState::NoResult => contracts::SubrunStateInspect::NoResult,
        SubrunState::Stopped => contracts::SubrunStateInspect::Stopped,
    }
}

fn subrun_step(step: SubrunStep) -> contracts::SubrunStepInspect {
    match step {
        SubrunStep::Text { text } => contracts::SubrunStepInspect::Text { text },
        SubrunStep::ChapterRead { chapter_id, title } => {
            contracts::SubrunStepInspect::ChapterRead { chapter_id, title }
        }
        SubrunStep::KeywordSearch { query, matches } => {
            contracts::SubrunStepInspect::KeywordSearch { query, matches }
        }
        SubrunStep::HybridSearch { question, passages } => {
            contracts::SubrunStepInspect::HybridSearch { question, passages }
        }
        SubrunStep::MemoryRead { title } => contracts::SubrunStepInspect::MemoryRead { title },
        SubrunStep::MemorySearch { query, entries } => {
            contracts::SubrunStepInspect::MemorySearch { query, entries }
        }
    }
}
