//! The Skills that the author selected in an earlier Run of the Project Conversation, at each
//! request of a later Run of that Conversation (ADR 0089 section 5, ADR 0085, ADR 0093).

use std::sync::Arc;

use serde_json::json;
use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, ConversationSelection, ProjectScope,
    SteerAgentRunInput,
};
use storyos_core::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX, SKILL_MD_PATH,
    SkillFileInput, SkillPackage, SkillSet, SkillUse,
};
use tokio_postgres::Client;

use super::{
    catalog, conversation_of, of_conversation, pinned, recorded, requirements, selected,
    selected_run, sent, texts,
};
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_selected_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::{
    AuthorAction, AuthorAt, Moment, cancellation, queued, settle,
};
use crate::command_sequence::tests::{STEER_AGENT_RUN, issued};
use crate::create_agent_run_tests::skill_tests::{package, set};

/// A Skill set of the deployment that has neither fixture Skill.
fn other_set() -> (SkillPackage, Arc<SkillSet>) {
    let outline = package(
        "plot-outline",
        vec![SkillFileInput {
            path: SKILL_MD_PATH.to_owned(),
            bytes:
                b"---\nname: plot-outline\ndescription: Use it to outline a plot.\n---\nOutline."
                    .to_vec(),
        }],
    );
    (outline.clone(), set(vec![outline]))
}

/// Completes a Run of a new Project, then a Run of a new Conversation with `first` and the
/// selection of the two fixture Skills. Then admits one later Run of that Conversation with
/// `message`, the Skill set `skills`, and the selections `own`.
async fn later_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 3],
    (first, message): (&str, &str),
    (skills, own): (Arc<SkillSet>, Vec<storyos_core::SkillPackageReference>),
) -> (ProjectScope, String) {
    let new = ConversationSelection::New;
    let (scope, chapter_id, selected) =
        selected_run(store, admin, [prefixes[0], prefixes[1]], first, new).await;
    complete(store, admin, &scope, &selected).await;
    let later = admit_selected_run(
        store,
        &scope,
        &chapter_id,
        prefixes[2],
        message,
        conversation_of(prefixes[1]),
        (skills, own),
    )
    .await;
    (scope, later)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_run_sends_each_skill_of_the_conversation_only_with_a_full_input() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (packages, skills, _) = selected();
    let (text, catalog) = (texts(&packages), [catalog(&packages)]);
    // The prior reference expires, and the deployment no longer has the two Skills. The text
    // comes from the Snapshots of the selections.
    let (outline, without) = other_set();
    let (expired_scope, expired) = later_run(
        &store,
        &admin,
        ["ca1", "ca2", "ca3"],
        (
            "Help with this passage. SCRIPT:reference-expires",
            "Help again.",
        ),
        (without, Vec::new()),
    )
    .await;
    complete(&store, &admin, &expired_scope, &expired).await;
    let compacting = format!(
        "{ACTIVE_COMPACTION_REQUEST_PREFIX} {ACTIVE_COMPACTION_POSITIONS_PREFIX} 1. Help again."
    );
    let (scope, compacted) = later_run(
        &store,
        &admin,
        ["ca4", "ca5", "ca6"],
        ("Help with this passage.", &compacting),
        (Arc::clone(&skills), Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &compacted).await;
    // The later Run selects one of the two Skills itself.
    let own = vec![packages[1].reference()];
    let (scope, steered) = later_run(
        &store,
        &admin,
        ["ca7", "ca8", "ca9"],
        ("Help with this passage.", "Help again."),
        (skills, own),
    )
    .await;
    let input = SteerAgentRunInput {
        run_id: steered.clone(),
        conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", "ca8"),
        author_message: "Keep the voice.".to_owned(),
        selection_references: None,
    };
    let input = issued(&store, &scope, 0xca00, &STEER_AGENT_RUN, input).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &steered, &author)
        .await
        .unwrap();
    let mixed = [
        recorded(&packages[1..]).as_array().unwrap().clone(),
        of_conversation(&packages[..1]).as_array().unwrap().clone(),
    ]
    .concat();
    let uses = |scope: ProjectScope, run_id: String| {
        let store = &store;
        async move {
            store
                .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
                .await
                .unwrap()
                .unwrap()
                .skill_uses
        }
    };

    let outline = std::slice::from_ref(&outline);
    let (other_catalog, other_pinned) = ([super::catalog(outline)], pinned(outline));

    assert_eq!(
        (
            uses(expired_scope, expired.clone()).await,
            sent(&admin, &expired).await,
            requirements(&admin, &expired).await,
            sent(&admin, &compacted).await,
            sent(&admin, &steered).await,
            requirements(&admin, &steered).await,
            uses(scope.clone(), steered.clone()).await,
        ),
        (
            // A full input that sends a Skill of the Conversation again adds no Skill use.
            Some(Vec::new()),
            // The first request continues the prior response and sends no text of a Skill of the
            // Conversation. The full input after the expired reference sends it.
            json!([
                ["rejected_create", 0, [], other_catalog],
                ["decision", 0, text, other_catalog],
                ["decision", 1, [], []],
            ]),
            json!([
                [0, of_conversation(&packages), other_pinned],
                [1, of_conversation(&packages), other_pinned],
            ]),
            // The summary request sends no project content. The later request has a full input.
            json!([
                ["decision", 0, [], catalog],
                ["compaction", 1, [], []],
                ["decision", 1, text, catalog],
            ]),
            // The steering input replaces the input of RunStep 1 after a prior response, so the
            // request sends the Skill of the Run again, and not the Skill of the Conversation.
            json!([
                ["decision", 0, &text[1..], catalog],
                ["decision", 1, &text[1..], catalog],
                ["decision", 2, [], []]
            ]),
            json!([
                [0, mixed, pinned(&packages)],
                [1, mixed, pinned(&packages)],
                [2, mixed, pinned(&packages)],
            ]),
            // The turn of the selection names the Skill. A later turn names only its own.
            Some(vec![SkillUse::AuthorSelected {
                name: packages[1].name.clone(),
            }]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn only_an_earlier_run_of_the_conversation_with_a_selection_gives_a_skill() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (packages, skills, selections) = selected();
    // A new Conversation of the same Project gets no Skill of another Conversation.
    let (scope, chapter_id, earlier) = selected_run(
        &store,
        &admin,
        ["caa", "cab"],
        "Help with this passage.",
        ConversationSelection::New,
    )
    .await;
    complete(&store, &admin, &scope, &earlier).await;
    let new = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "cac",
        "Help again.",
        ConversationSelection::New,
        (Arc::clone(&skills), Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &new).await;
    // The Agent loaded a Skill in the earlier Run. The full input of the later Run sends no text
    // of it.
    let (scope, chapter_id, first) = queued(&store, "cad").await;
    complete(&store, &admin, &scope, &first).await;
    let loaded = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "cae",
        &format!(
            "SCRIPT:load_skill {}\nSCRIPT:reference-expires",
            packages[0].name
        ),
        ConversationSelection::New,
        (Arc::clone(&skills), Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &loaded).await;
    let after_load = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "caf",
        "Help again.",
        conversation_of("cae"),
        (Arc::clone(&skills), Vec::new()),
    )
    .await;
    complete(&store, &admin, &scope, &after_load).await;
    // A cancelled Run gives its selection.
    let (scope, chapter_id, first) = queued(&store, "c9a").await;
    complete(&store, &admin, &scope, &first).await;
    let cancelled = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "c9b",
        "Help with this passage.",
        ConversationSelection::New,
        (Arc::clone(&skills), selections),
    )
    .await;
    let cancel = cancellation(&store, &scope, 0xc900, &cancelled).await;
    store
        .cancel_agent_run(&cancel.envelope, &cancel.input)
        .await
        .unwrap();
    let after_cancel = admit_selected_run(
        &store,
        &scope,
        &chapter_id,
        "c9c",
        "Help again.",
        conversation_of("c9b"),
        (skills, Vec::new()),
    )
    .await;
    let catalog = catalog(&packages);
    let status: String = admin
        .query_one(
            "SELECT status::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&cancelled],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            requirements(&admin, &new).await,
            sent(&admin, &after_load).await,
            requirements(&admin, &after_load).await,
            status,
            requirements(&admin, &after_cancel).await,
        ),
        (
            json!([[0, null, pinned(&packages)], [1, null, pinned(&packages)]]),
            json!([
                ["rejected_create", 0, [], [catalog]],
                ["decision", 0, [], [catalog]],
                ["decision", 1, [], []],
            ]),
            json!([[0, null, pinned(&packages)], [1, null, pinned(&packages)]]),
            "cancelled".to_owned(),
            json!([[0, of_conversation(&packages), pinned(&packages)]]),
        )
    );
}
