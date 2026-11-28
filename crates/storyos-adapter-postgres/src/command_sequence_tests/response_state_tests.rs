//! The Editor Session state that the first run of a command gives (ADR 0106 section 3).

use storyos_application::{
    EditorClientBinding, EditorSession, EditorSessionId, EditorSessionLookup, ProjectScope,
    get_editor_session,
};
use storyos_core::TransitionOutcome;

use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal,
};
use crate::shared_tests::command_sequence_support::{
    CLIENT_CONTRACT_REVISION, SECURITY_POLICY_REVISION, issued, stores, two_chapter_writer,
    with_new_request_ids,
};
use crate::shared_tests::command_sequence_undo::{edit_in_chapter_of, session_chapter_head};
use crate::shared_tests::create_volume_authority::{NamedEdit, named_edit_command};

/// A read of the Editor Session through the binding of the test Challenges.
async fn read_session(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    editor_session_id: &str,
) -> Option<EditorSession> {
    get_editor_session(
        store,
        &EditorSessionLookup {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            client_binding: EditorClientBinding {
                binding_ref: "sha256:session-a".to_owned(),
                session_generation: 1,
                client_contract_revision: CLIENT_CONTRACT_REVISION.to_owned(),
                security_policy_revision: SECURITY_POLICY_REVISION.to_owned(),
            },
        },
    )
    .await
    .unwrap()
}

/// The first run of an applied Author Edit gives the Editor Session that a read right after it
/// gives. A replay and an edit with no effect give none.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_author_edit_gives_the_editor_session_that_a_read_after_it_gives() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _chapter_b, _revision_b, editor_session_id) =
        two_chapter_writer(&store, /*base*/ 0xd300).await;
    let opened = read_session(&store, &scope, &editor_session_id).await;
    let head = session_chapter_head(&admin, &editor_session_id).await;
    let edit = |suffix, expected_revision_id, local_intent_sequence, text| NamedEdit {
        editor_session_id: &editor_session_id,
        chapter_id: &chapter_a,
        expected_revision_id,
        suffix,
        local_intent_sequence,
        text,
        proposal_target: None,
    };
    let command = named_edit_command(&store, &scope, edit("d309", &head, 1, "x")).await;

    let first = store.apply_author_edit(&command).await.unwrap();
    let after = read_session(&store, &scope, &editor_session_id).await;
    let replay = store
        .apply_author_edit(&with_new_request_ids_of(&command))
        .await
        .unwrap();
    let moved_head = session_chapter_head(&admin, &editor_session_id).await;
    let unchanged = named_edit_command(&store, &scope, edit("d30a", &moved_head, 2, "")).await;
    let no_effect = store.apply_author_edit(&unchanged).await.unwrap();

    assert_ne!(after, opened);
    assert_eq!(first.editor_session, after);
    assert_eq!(
        (replay.editor_session, no_effect.editor_session),
        (None, None)
    );
}

/// The Editor Session of an applied Acceptance. Panics on any other outcome.
fn session(settlement: storyos_application::AcceptProposalSettlement) -> Option<EditorSession> {
    match settlement.outcome {
        TransitionOutcome::Applied(applied) => applied.session,
        other => panic!("the Acceptance must apply, got {other:?}"),
    }
}

/// The Author Edit command with new request identities, as an exact retry sends it.
fn with_new_request_ids_of(
    command: &storyos_application::ApplyAuthorEditCommand,
) -> storyos_application::ApplyAuthorEditCommand {
    let mut retry = command.clone();
    retry.ids = storyos_application::AuthorCommandAdmissionIds {
        command_id: uuid::Uuid::now_v7().to_string(),
        author_command_admission_id: uuid::Uuid::now_v7().to_string(),
        receipt_id: uuid::Uuid::now_v7().to_string(),
    };
    retry
}

/// The first run of an applied Acceptance that moved the base gives the Editor Session that a
/// read right after it gives. A replay, and an Acceptance whose Editor Session keeps its base on
/// another Chapter, give none.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_acceptance_gives_the_editor_session_that_a_read_after_it_gives() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    for (base, on_target_chapter) in [(0xd320, true), (0xd340, false)] {
        let (scope, input, _chapter_a_head) = acceptable_proposal(&store, &admin, base).await;
        let editor_session_id = input.editor_session_id.as_ref().to_owned();
        if on_target_chapter {
            edit_in_chapter_of(
                &admin,
                &editor_session_id,
                &input.expected_authoritative_revision_id,
            )
            .await;
        }
        let opened = read_session(&store, &scope, &editor_session_id).await;
        let call = issued(&store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
        let first = session(accept_proposal(&store, &call).await.unwrap());
        let after = read_session(&store, &scope, &editor_session_id).await;
        let replay = session(
            accept_proposal(&store, &with_new_request_ids(&call))
                .await
                .unwrap(),
        );
        if on_target_chapter {
            assert_ne!(after, opened);
            assert_eq!(first, after);
        } else {
            // The base stays on Chapter A, so the first run gives none.
            let base = |session: &Option<EditorSession>| {
                session
                    .as_ref()
                    .map(|session| session.base_snapshot.clone())
            };
            assert_eq!((base(&after), &first), (base(&opened), &None));
        }
        assert_eq!(replay, None, "{base:#x}");
    }
}
