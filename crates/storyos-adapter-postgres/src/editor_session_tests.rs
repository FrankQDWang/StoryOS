use storyos_application::{EditorSessionId, EditorSessionStore, OpenEditorSession};

use crate::PostgresProjectReader;
use crate::create_volume_authority_tests::{NamedEdit, apply_named_edit};
use crate::set_current_chapter_authority_tests::{open_session_request, seed_two_chapters};

const OWNER: &str = "018f0000-0000-7001-8000-000000000001";

fn store() -> PostgresProjectReader {
    PostgresProjectReader::new(
        std::env::var("STORYOS_TEST_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
    )
}

/// The request of an exact retry: the same Command Challenge with new Server identities.
fn exact_retry(request: &OpenEditorSession, suffix: &str) -> OpenEditorSession {
    OpenEditorSession {
        editor_session_id: EditorSessionId::new(format!(
            "018f0000-0000-7001-8000-00000001{suffix}"
        )),
        snapshot_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
        ..request.clone()
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exact_retry_after_an_author_edit_returns_the_first_editor_session() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let store = store();
    let (scope, _volume_id, chapter_a, _chapter_b) =
        seed_two_chapters(&store, OWNER, "e120", "e121", "e122", "e123").await;
    let request = open_session_request(&store, &scope, "e124").await;
    let first = store.create_editor_session(&request).await.unwrap();
    apply_named_edit(
        &store,
        &scope,
        NamedEdit {
            editor_session_id: request.editor_session_id.as_ref(),
            chapter_id: &chapter_a,
            expected_revision_id: &first.base_snapshot.authoritative_revision_id,
            suffix: "e125",
            local_intent_sequence: 1,
            text: "Keep this sentence.",
            proposal_target: None,
        },
    )
    .await;
    assert_eq!(
        store
            .create_editor_session(&exact_retry(&request, "e126"))
            .await
            .unwrap(),
        first
    );
}
