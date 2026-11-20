//! The shared test support from `skill_instruction_context_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::create_agent_run_skill::{fixture, set};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::admit_selected_run;
use crate::shared_tests::model_gateway::{ReplacedInputLabel, sent_context};
use serde_json::{Value, json};
use std::sync::Arc;
use storyos_application::{ConversationSelection, ProjectScope};
use storyos_core::{SKILL_MD_PATH, SkillCatalogEntry, SkillPackage, SkillSet, skill_catalog_text};
use tokio_postgres::Client;

/// The two fixture Skills, the Skill set that offers them, and their selection in the reverse
/// order of their names.
pub(crate) fn selected() -> (
    Vec<SkillPackage>,
    Arc<SkillSet>,
    Vec<storyos_core::SkillPackageReference>,
) {
    let packages = vec![fixture("dialogue-style-guide"), fixture("internal-comms")];
    let selections = packages.iter().rev().map(SkillPackage::reference).collect();
    (packages.clone(), set(packages), selections)
}

/// The exact `SKILL.md` text of each package.
pub(crate) fn texts(packages: &[SkillPackage]) -> Vec<String> {
    packages
        .iter()
        .map(|package| {
            let file = package.files.iter().find(|file| file.path == SKILL_MD_PATH);
            String::from_utf8(file.unwrap().bytes.clone()).unwrap()
        })
        .collect()
}

/// The text of the `skill_catalog` item of a Run that pinned `packages`.
pub(crate) fn catalog(packages: &[SkillPackage]) -> String {
    let entries: Vec<_> = packages
        .iter()
        .map(|package| SkillCatalogEntry {
            reference: package.reference(),
            description: package.description.clone(),
        })
        .collect();
    skill_catalog_text(&entries)
}

/// For each Model Attempt of the Run in the order of its creation: its role, the position of its
/// RunStep, the text of each `skill_instruction` item, and the text of each `skill_catalog` item
/// that the fake destination received.
pub(crate) async fn sent(admin: &Client, run_id: &str) -> Value {
    sent_context(
        admin,
        run_id,
        &["skill_instruction", "skill_catalog"],
        ReplacedInputLabel::Omit,
    )
    .await
}

/// For each decision Context Assembly of the Run in RunStep order: its RunStep, and the Skill
/// requirements and the pinned catalog of its Operation Requirement.
pub(crate) async fn requirements(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(decision_position,
                      payload #> '{operation_requirement,skill_requirements}',
                      payload #> '{operation_requirement,skill_catalog}')
                      ORDER BY decision_position)::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'",
        &[&run_id],
    )
    .await
}

/// The pinned SkillPackage References of `packages`, as the Operation Requirement records them.
pub(crate) fn pinned(packages: &[SkillPackage]) -> Value {
    packages
        .iter()
        .map(|package| json!({"name": package.name, "package_digest": package.package_digest}))
        .collect()
}

pub(crate) fn recorded(packages: &[SkillPackage]) -> Value {
    with_provenance(packages, "author_selected")
}

/// The requirements of the Skills of the Conversation `packages` of a later Run.
pub(crate) fn of_conversation(packages: &[SkillPackage]) -> Value {
    with_provenance(packages, "conversation_selected")
}

pub(crate) fn with_provenance(packages: &[SkillPackage], provenance: &str) -> Value {
    packages
        .iter()
        .map(|package| {
            json!({
                "skill_name": package.name,
                "package_digest": package.package_digest,
                "provenance": provenance,
            })
        })
        .collect()
}

/// Completes the first Run of a new Project of the tool loop profile, then admits one Run with
/// `message` in `conversation` and the selection of the two fixture Skills.
pub(crate) async fn selected_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    message: &str,
    conversation: ConversationSelection,
) -> (ProjectScope, String, String) {
    let (scope, chapter_id, first) = queued(store, prefixes[0]).await;
    complete(store, admin, &scope, &first).await;
    let (_, skills, selections) = selected();
    let run_id = admit_selected_run(
        store,
        &scope,
        &chapter_id,
        prefixes[1],
        message,
        conversation,
        (skills, selections),
    )
    .await;
    (scope, chapter_id, run_id)
}
