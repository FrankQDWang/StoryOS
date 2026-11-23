//! The shared test support from `agent_run_load_skill_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::Admission;
use serde_json::Value;
use std::sync::Arc;
use storyos_application::ProjectScope;
use storyos_core::{SKILL_MD_PATH, SkillPackage, SkillSet};
use tokio_postgres::Client;

pub(crate) fn skill_md(package: &SkillPackage) -> String {
    let file = package.files.iter().find(|file| file.path == SKILL_MD_PATH);
    String::from_utf8(file.unwrap().bytes.clone()).unwrap()
}

/// Completes the first Run of a new Project of the tool loop profile, then admits and completes
/// one Run with `message` that pins the catalog of `skills` and selects no Skill.
pub(crate) async fn loaded_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    message: &str,
    skills: Arc<SkillSet>,
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) = queued(store, prefixes[0]).await;
    complete(store, admin, &scope, &first).await;
    let run_id = Admission {
        message,
        skills: (skills, Vec::new()),
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, prefixes[1])
    .await;
    complete(store, admin, &scope, &run_id).await;
    (scope, run_id)
}

/// The status of the Run, and each ToolCall as its function name, state, refusal, Host result,
/// and whether it has no Destination Attempt and pins the current Host Tool Registration.
pub(crate) async fn calls(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(run.status, (SELECT jsonb_agg(jsonb_build_array(
                      call.function_name, call.state, call.refusal, call.result_text,
                      COALESCE(call.destination_attempt_id IS NULL AND call.attempt IS NULL
                        AND call.registration_id = 'host_tools'
                        AND call.tool_registration_revision::text = $2, false))
                      ORDER BY call.decision_position, call.call_position)
                      FROM storyos.tool_calls AS call WHERE call.run_id = run.run_id))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
        &[&run_id, &storyos_core::HOST_TOOL_REGISTRATION.revision],
    )
    .await
}

/// The Tool sources of the Operation Requirement and each selected `skill_instruction` item of
/// the Context Assembly with `role` at `position`.
pub(crate) async fn record(admin: &Client, run_id: &str, role: &str, position: i32) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(payload #> '{operation_requirement,tool_results}',
                      jsonb_path_query_array(payload,
                        '$.selected[*] ? (@.source_class == \"skill_instruction\")'))::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = $2
                AND decision_position = $3::text::numeric",
        &[&run_id, &role, &position.to_string()],
    )
    .await
}

pub(crate) const TOOLS: [&str; 13] = [
    "propose_edit",
    "submit_research",
    "read_tool_result",
    "load_skill",
    "read_skill_resource",
    "read_chapter",
    "search_manuscript",
    "write_memory",
    "read_memory_entry",
    "search_memory",
    "update_plan",
    "start_subrun",
    "finish",
];
