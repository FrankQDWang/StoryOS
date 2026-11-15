//! The `skill_instruction` items of a Run whose author selected Skills, and the `skill_catalog`
//! item of a Run with a pinned catalog, at each request of the Run (ADR 0089 sections 3 and 5,
//! ADR 0085, ADR 0093).

use std::sync::Arc;

use serde_json::{Value, json};
use storyos_application::{ConversationSelection, ProjectScope};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, SKILL_MD_PATH,
    SkillCatalogEntry, SkillPackage, SkillSet, skill_catalog_text,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_selected_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle, steering};
use crate::create_agent_run_tests::skill_tests::{fixture, set};

/// The two fixture Skills, the Skill set that offers them, and their selection in the reverse
/// order of their names.
fn selected() -> (
    Vec<SkillPackage>,
    Arc<SkillSet>,
    Vec<storyos_core::SkillPackageReference>,
) {
    let packages = vec![fixture("dialogue-style-guide"), fixture("internal-comms")];
    let selections = packages.iter().rev().map(SkillPackage::reference).collect();
    (packages.clone(), set(packages), selections)
}

/// The exact `SKILL.md` text of each package.
fn texts(packages: &[SkillPackage]) -> Vec<String> {
    packages
        .iter()
        .map(|package| {
            let file = package.files.iter().find(|file| file.path == SKILL_MD_PATH);
            String::from_utf8(file.unwrap().bytes.clone()).unwrap()
        })
        .collect()
}

/// The text of the `skill_catalog` item of a Run that pinned `packages`.
fn catalog(packages: &[SkillPackage]) -> String {
    let entries: Vec<_> = packages
        .iter()
        .map(|package| SkillCatalogEntry {
            reference: package.reference(),
            description: package.description.clone(),
        })
        .collect();
    skill_catalog_text(&entries)
}

/// For each Model Attempt of the Run in the order of its creation: its role, its RunStep, the
/// text of each `skill_instruction` item, and the text of each `skill_catalog` item that the fake
/// destination received.
async fn sent(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position,
                      jsonb_path_query_array(wire,
                        '$.context[*] ? (@.source_class == \"skill_instruction\").content'),
                      jsonb_path_query_array(wire,
                        '$.context[*] ? (@.source_class == \"skill_catalog\").content'))
                      ORDER BY model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// For each decision Context Assembly of the Run in RunStep order: its RunStep, and the Skill
/// requirements and the pinned catalog of its Operation Requirement.
async fn requirements(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(decision_position,
                      payload #> '{operation_requirement,skill_requirements}',
                      payload #> '{operation_requirement,skill_catalog}')
                      ORDER BY decision_position)::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The pinned SkillPackage References of `packages`, as the Operation Requirement records them.
fn pinned(packages: &[SkillPackage]) -> Value {
    packages
        .iter()
        .map(|package| json!({"name": package.name, "package_digest": package.package_digest}))
        .collect()
}

fn recorded(packages: &[SkillPackage]) -> Value {
    packages
        .iter()
        .map(|package| {
            json!({
                "skill_name": package.name,
                "package_digest": package.package_digest,
                "provenance": "author_selected",
            })
        })
        .collect()
}

/// The Conversation of the Run that `prefix` admitted.
fn conversation_of(prefix: &str) -> ConversationSelection {
    ConversationSelection::Existing {
        conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
    }
}

/// Completes the first Run of a new Project of the tool loop profile, then admits one Run with
/// `message` in `conversation` and the selection of the two fixture Skills.
async fn selected_run(
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_step_selects_the_catalog_and_only_a_selected_run_selects_the_exact_skill_md() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (packages, skills, _) = selected();
    let (scope, chapter_id, run_id) = selected_run(
        &store,
        &admin,
        ["441", "442"],
        "Help with this passage.",
        ConversationSelection::New,
    )
    .await;
    complete(&store, &admin, &scope, &run_id).await;
    let later = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "443",
        "Help again.",
        conversation_of("442"),
        (skills, Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &later).await;

    // The first request sends one section for each Skill in name order, then the catalog. The
    // request after the `finish` output continues with an unchanged input, so it sends no Skill
    // text and no catalog.
    let (catalog, pinned) = (catalog(&packages), pinned(&packages));
    assert_eq!(
        (
            sent(&admin, &run_id).await,
            requirements(&admin, &run_id).await,
            sent(&admin, &later).await,
            requirements(&admin, &later).await,
        ),
        (
            json!([
                ["decision", 0, texts(&packages), [catalog]],
                ["decision", 1, [], []]
            ]),
            json!([
                [0, recorded(&packages), pinned],
                [1, recorded(&packages), pinned]
            ]),
            json!([["decision", 0, [], [catalog]], ["decision", 1, [], []]]),
            json!([[0, null, pinned], [1, null, pinned]]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_sends_the_same_skill_and_catalog_text_and_a_summary_request_sends_none() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (packages, skills, selections) = selected();
    let (text, catalog) = (texts(&packages), [catalog(&packages)]);
    // The destination lost the closing reference of the Run before the selected Run, so the
    // selected Run sends its full input again.
    let (scope, chapter_id, first) = queued(&store, "444").await;
    complete(&store, &admin, &scope, &first).await;
    let expiring = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "445",
        "Help with this passage. SCRIPT:reference-expires",
        conversation_of("444"),
        (Arc::clone(&skills), Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &expiring).await;
    let expired = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "446",
        "Help again.",
        conversation_of("444"),
        (skills, selections),
    )
    .await;
    complete(&store, &admin, &scope, &expired).await;
    let compacting = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let new = ConversationSelection::New;
    let (scope, _, compacted) =
        selected_run(&store, &admin, ["447", "448"], &compacting, new.clone()).await;
    complete(&store, &admin, &scope, &compacted).await;
    let (scope, _, steered) =
        selected_run(&store, &admin, ["449", "44a"], "Help again.", new).await;
    let input = steering(&store, &scope, 0x4500, &steered, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &steered, &author)
        .await
        .unwrap();

    assert_eq!(
        (
            sent(&admin, &expired).await,
            sent(&admin, &compacted).await,
            sent(&admin, &steered).await,
            requirements(&admin, &steered).await,
        ),
        (
            json!([
                ["rejected_create", 0, text, catalog],
                ["decision", 0, text, catalog],
                ["decision", 1, [], []],
            ]),
            json!([
                ["decision", 0, text, catalog],
                ["compaction", 1, [], []],
                ["decision", 1, text, catalog],
            ]),
            // The steering input replaces the input of RunStep 1, and the requirement stays.
            json!([
                ["decision", 0, text, catalog],
                ["decision", 1, text, catalog],
                ["decision", 2, [], []]
            ]),
            json!([
                [0, recorded(&packages), pinned(&packages)],
                [1, recorded(&packages), pinned(&packages)],
                [2, recorded(&packages), pinned(&packages)],
            ]),
        )
    );
}
