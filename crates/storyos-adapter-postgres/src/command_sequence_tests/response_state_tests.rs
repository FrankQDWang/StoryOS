//! Which run of a command gives the Editor Session state (ADR 0106 section 3). The HTTP tests of
//! the two commands prove that the state equals a read of the Editor Session after the command.

use storyos_application::{AuthorEditSettlement, AuthorEditSettlementEffect, EditorSession};
use storyos_core::TransitionOutcome;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal,
};
use crate::shared_tests::command_sequence_support::{
    issued, stores, two_chapter_writer, with_new_request_ids,
};
use crate::shared_tests::command_sequence_undo::{edit_in_chapter_of, session_chapter_head};
use crate::shared_tests::create_volume_authority::{NamedEdit, named_edit_command};

/// The Revision of the base Snapshot of the state, if the state is present.
fn state_base(session: &Option<EditorSession>) -> Option<&str> {
    session
        .as_ref()
        .map(|session| session.base_snapshot.authoritative_revision_id.as_str())
}

/// The Revision of an applied Author Edit. Panics on any other effect.
fn edit_revision(settlement: &AuthorEditSettlement) -> &str {
    match &settlement.effect {
        AuthorEditSettlementEffect::AuthoritativeApplied { ids, .. } => &ids.revision_id,
        other => panic!("the Author Edit must apply, got {other:?}"),
    }
}

/// The first run of an applied Author Edit gives the state on the new Revision. A replay and an
/// edit with no effect give none.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn only_the_first_run_of_an_applied_author_edit_gives_the_editor_session_state() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _chapter_b, _revision_b, editor_session_id) =
        two_chapter_writer(&store, /*base*/ 0xd300).await;
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
    let replay = store
        .apply_author_edit(&with_new_request_ids_of(&command))
        .await
        .unwrap();
    let moved_head = session_chapter_head(&admin, &editor_session_id).await;
    let unchanged = named_edit_command(&store, &scope, edit("d30a", &moved_head, 2, "")).await;
    let no_effect = store.apply_author_edit(&unchanged).await.unwrap();

    assert_eq!(
        (
            state_base(&first.editor_session),
            state_base(&replay.editor_session),
            state_base(&no_effect.editor_session)
        ),
        (Some(edit_revision(&first)), None, None)
    );
}

/// The Editor Session state and the Revision of an applied Acceptance. Panics on any other
/// outcome.
fn session(
    settlement: storyos_application::AcceptProposalSettlement,
) -> (Option<EditorSession>, String) {
    match settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            applied.session.map(|session| *session),
            applied.ids.revision_id,
        ),
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

/// The first run of an applied Acceptance that moves the base gives the state on the new Revision.
/// A replay, and an Acceptance whose Editor Session keeps its base on another Chapter, give none.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn only_the_first_run_of_an_acceptance_that_moves_the_base_gives_the_editor_session_state() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    for (base, on_target_chapter) in [(0xd320, true), (0xd340, false)] {
        let (scope, input, _chapter_a_head) = acceptable_proposal(&store, &admin, base).await;
        if on_target_chapter {
            edit_in_chapter_of(
                &admin,
                input.editor_session_id.as_ref(),
                &input.expected_authoritative_revision_id,
            )
            .await;
        }
        let call = issued(&store, &scope, base + 9, &ACCEPT_PROPOSAL, input).await;
        let (first, revision) = session(accept_proposal(&store, &call).await.unwrap());
        let (replay, _) = session(
            accept_proposal(&store, &with_new_request_ids(&call))
                .await
                .unwrap(),
        );
        let expected = on_target_chapter.then_some(revision.as_str());
        assert_eq!(
            (state_base(&first), state_base(&replay)),
            (expected, None),
            "{base:#x}"
        );
    }
}
