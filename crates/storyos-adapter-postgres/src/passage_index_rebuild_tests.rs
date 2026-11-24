//! S7-E1b: the store test of a loss of each vector row of a Project. Test prefixes `a25`, `a26`,
//! and `a2f`: `damaged_evidence_tests` uses the Projects `a200` and `a210`.

use crate::statement_cache::Client;
use storyos_application::{PassageIndexWorkStore, ProjectScope};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::memory_entries::write_versions;
use crate::shared_tests::passage_index_work::{
    FAKE_ONLY, claim, configured, park_other_work, steps, wake,
};
use crate::shared_tests::passage_vectors::{chapter, digests, rows as digest_rows, texts};

/// A count and a digest of the ordered complete rows of each manuscript and Memory table of the
/// Project of `scope`: each Chapter, each Revision, and each Memory Entry version.
async fn manuscript_and_memory(admin: &Client, scope: &ProjectScope) -> Vec<String> {
    let mut state = Vec::new();
    for table in [
        "manuscript_objects",
        "authoritative_revisions",
        "authoritative_payloads",
        "authoritative_heads",
        "memory_entry_versions",
        "project_memory_heads",
    ] {
        let row = admin
            .query_one(
                &format!(
                    "SELECT '{table}|' || count(*) || '|' ||
                            md5(coalesce(string_agg(row_to_json(record)::text, ';'
                              ORDER BY row_to_json(record)::text), ''))
                       FROM storyos.{table} AS record WHERE record.project_id = $1::text::uuid"
                ),
                &[&scope.project_id.as_ref()],
            )
            .await
            .unwrap();
        state.push(row.get(/*idx*/ 0));
    }
    state
}

/// S7-E1b: a loss of each vector row of a Project. The rebuild mark and the index work make each
/// row again, and each Chapter, each Revision, and each Memory Entry stays the same.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_work_makes_each_lost_vector_row_again_and_changes_no_chapter_or_entry() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["a25", "a26"], "a2f", &texts()).await;
    let scope = chapter.scope.clone();
    let entries = [
        (1, 1, 1, "current", "character", "林舟", "渡口的船夫。"),
        (2, 1, 2, "current", "foreshadowing", "铜铃", "铃声。"),
    ];
    write_versions(&admin, &scope, &entries).await;
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    while let Some(claimed) = claim(&store, FAKE_ONLY).await {
        steps(&store, &claimed).await;
    }
    let vectors = digest_rows(&admin, &scope).await;
    let before = manuscript_and_memory(&admin, &scope).await;

    let lost = admin
        .execute(
            "DELETE FROM storyos.passage_vectors WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert!(store.mark_each_passage_index_work().await.unwrap() >= 1);
    park_other_work(&admin, &scope).await;
    while let Some(claimed) = claim(&store, FAKE_ONLY).await {
        steps(&store, &claimed).await;
    }

    assert_eq!(
        (
            lost,
            vectors.clone(),
            digest_rows(&admin, &scope).await,
            manuscript_and_memory(&admin, &scope).await
        ),
        (vectors.len() as u64, digests(&texts()), vectors, before)
    );
}
