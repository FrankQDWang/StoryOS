//! The store tests of the passage index work (ADR 0095 section 4). The tests move the times of the
//! work rows with the admin connection, and they park the work of other Projects. Test prefix
//! leads `f3`, `2f`, and `59`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use storyos_adapter_fake_destination::{
    EMBEDDING_NOT_SUBMITTED_MARKER, EMBEDDING_REJECTION_MARKER, EMBEDDING_UNKNOWN_MARKER,
    FakeEmbeddings,
};
use storyos_application::{
    ClaimedPassageIndex, EmbeddingObservation, EmbeddingProviderAdapter, EmbeddingRoute,
    NoEmbeddingDestination, PassageIndexStep, PassageIndexWorkStore, PassageVectorKey,
    PassageVectorRow, PreDispatchRefusal, PreparedEmbeddingRequest, ProjectId, ProjectScope,
    claim_next_passage_index_work, step_passage_index_work,
};
use storyos_core::{
    AGENT_PLAN_EMBEDDING_REGISTRATION, EmbeddingInput, EmbeddingRegistration,
    HOST_FAKE_EMBEDDING_REGISTRATION, PASSAGE_CUT_REVISION, TransitionOutcome,
};
use tokio_postgres::Client;

use super::{MarkedChapters, claim_passage_index_row, mark_passage_index_work};
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_embedding_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::cancellation;
use crate::command_sequence::tests::{titled_chapter, undo_named};
use crate::delete_structure_tests::settle_delete_chapter;
use crate::passage_vectors::tests::{chapter, digests, head, prepend, rows as digest_rows, texts};
use crate::project_embedding_binding::tests::{FAKE, embedding_project};

const FAKE_ONLY: &[&EmbeddingRegistration] = &[&HOST_FAKE_EMBEDDING_REGISTRATION];

/// The stores of a deployment with the fake embedding configuration.
pub(crate) async fn configured() -> (PostgresProjectReader, Client) {
    let (store, admin) = stores().await;
    (store.with_embedding_registration(FAKE), admin)
}

/// Gives the open transaction of `admin` the fake embedding configuration of the deployment.
pub(crate) async fn deploy(admin: &Client) {
    admin
        .execute(
            "SELECT set_config('storyos.embedding_registration', $1, true)",
            &[&HOST_FAKE_EMBEDDING_REGISTRATION.revision],
        )
        .await
        .unwrap();
}

/// The work rows of the Project in Chapter order: the Chapter, its order, its mark generation,
/// its completed generation, and whether its quiet time runs.
pub(crate) async fn work(
    admin: &Client,
    scope: &ProjectScope,
) -> Vec<(String, i64, i64, i64, bool)> {
    admin
        .query(
            "SELECT chapter_id::text, chapter_order, mark_generation, completed_generation,
                    not_before > clock_timestamp()
               FROM storyos.passage_index_work WHERE project_id = $1::text::uuid
              ORDER BY chapter_order",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect()
}

/// Ends the quiet time of each work row of the Project.
async fn wake(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.passage_index_work
                SET not_before = clock_timestamp() - interval '1 second'
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// Moves the work of each other Project one day later, so that a claim takes only this Project.
async fn park_other_work(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.passage_index_work
                SET not_before = clock_timestamp() + interval '1 day'
              WHERE project_id <> $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// Ends each lease of the Project.
async fn expire(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.passage_index_work
                SET lease_expires_at = clock_timestamp() - interval '1 second'
              WHERE project_id = $1::text::uuid AND lease_expires_at IS NOT NULL",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// The vector rows of the Project.
async fn rows(admin: &Client, scope: &ProjectScope) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.passage_vectors WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(0)
}

/// The settlements of the Embedding Attempts of the Project, in order.
async fn settlements(admin: &Client, scope: &ProjectScope) -> Vec<Option<String>> {
    admin
        .query(
            "SELECT settlement FROM storyos.embedding_attempts
              WHERE project_id = $1::text::uuid ORDER BY embedding_attempt_id",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect()
}

async fn claim(
    store: &PostgresProjectReader,
    registrations: &[&'static EmbeddingRegistration],
) -> Option<ClaimedPassageIndex> {
    claim_next_passage_index_work(store, registrations)
        .await
        .unwrap()
}

async fn step(store: &PostgresProjectReader, claim: &ClaimedPassageIndex) -> PassageIndexStep {
    step_with(store, &FakeEmbeddings, claim).await
}

/// One step of the claim with `adapter`, with a bounded wait: an action of the author that waits
/// for the work ends the test.
async fn step_with(
    store: &PostgresProjectReader,
    adapter: &impl EmbeddingProviderAdapter,
    claim: &ClaimedPassageIndex,
) -> PassageIndexStep {
    let step = step_passage_index_work(store, adapter, FAKE_ONLY, claim);
    tokio::time::timeout(Duration::from_secs(/*secs*/ 60), step)
        .await
        .expect("the step ends")
        .unwrap()
}

/// The fake embedding destination with a deterministic hold in the exchange. The first exchange
/// runs `action` to its end before the fake gives the vector, so the action occurs during the
/// work of the claim.
struct During<'a> {
    action: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'a>>>>,
}

impl<'a> During<'a> {
    fn new(action: impl Future<Output = ()> + Send + 'a) -> Self {
        Self {
            action: Mutex::new(Some(Box::pin(action))),
        }
    }
}

impl EmbeddingProviderAdapter for During<'_> {
    const REGISTRATIONS: &'static [&'static EmbeddingRegistration] = FakeEmbeddings::REGISTRATIONS;
    type Prepared = <FakeEmbeddings as EmbeddingProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, PreDispatchRefusal> {
        FakeEmbeddings.prepare(registration, route, input).await
    }

    async fn exchange(&self, prepared: Self::Prepared) -> EmbeddingObservation {
        let action = self.action.lock().unwrap().take();
        if let Some(action) = action {
            action.await;
        }
        FakeEmbeddings.exchange(prepared).await
    }
}

async fn run_status(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT status::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0)
}

/// Whether work that a claim of `revisions` can take waits.
async fn waits(store: &PostgresProjectReader, revisions: &[&str]) -> bool {
    store
        .passage_index_work_waits(revisions, PASSAGE_CUT_REVISION)
        .await
        .unwrap()
}

/// The steps of one claim until a step does not write.
async fn steps(store: &PostgresProjectReader, claim: &ClaimedPassageIndex) -> Vec<&'static str> {
    let mut steps = Vec::new();
    for _ in 0..8 {
        let result = step(store, claim).await;
        steps.push(match result {
            PassageIndexStep::Written => "written",
            PassageIndexStep::Completed => "completed",
            PassageIndexStep::Released(_) => "released",
            PassageIndexStep::StaleFence => "stale_fence",
        });
        if result != PassageIndexStep::Written {
            break;
        }
    }
    steps
}

async fn latest_forward(admin: &Client, scope: &ProjectScope) -> u64 {
    let sequence: String = admin
        .query_one(
            "SELECT max(author_action_sequence)::text FROM storyos.author_action_entries
              WHERE project_id = $1::text::uuid AND disposition = 'forward'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(0);
    sequence.parse().unwrap()
}

/// The Volume of the Chapter and the tree revision of the Project.
async fn volume_and_tree(admin: &Client, scope: &ProjectScope, chapter_id: &str) -> (String, u64) {
    let row = admin
        .query_one(
            "SELECT chapter.parent_volume_id::text, project.tree_revision
               FROM storyos.manuscript_objects AS chapter
               JOIN storyos.projects AS project USING (owner_user_id, project_id)
              WHERE project.project_id = $1::text::uuid
                AND chapter.manuscript_object_id = $2::text::uuid",
            &[&scope.project_id.as_ref(), &chapter_id],
        )
        .await
        .unwrap();
    (row.get(0), u64::try_from(row.get::<_, i64>(1)).unwrap())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_change_of_a_chapter_marks_its_work_in_the_transaction_of_the_change() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let mut chapter = chapter(&store, &admin, ["f30", "f31"], "f32", &texts()).await;
    let scope = chapter.scope.clone();
    let one = chapter.chapter_id.clone();
    let row = |id: &str, order, generation| (id.to_owned(), order, generation, 0, true);
    // The new binding marked the live Chapter, and the Author Edit of the fixture marked it again.
    assert_eq!(work(&admin, &scope).await, vec![row(&one, 1, 2)]);

    let command = prepend(&store, &chapter, ("f322", 2), 0, "新的一句。").await;
    store.apply_author_edit(&command).await.unwrap();
    assert_eq!(work(&admin, &scope).await, vec![row(&one, 1, 3)]);

    chapter.head = head(&admin, &one).await;
    let frontier = latest_forward(&admin, &scope).await;
    let undone = undo_named(
        &store,
        &scope,
        &chapter.editor_session_id,
        frontier,
        &chapter.head,
        "f323",
    )
    .await;
    assert!(matches!(undone.outcome, TransitionOutcome::Applied(_)));
    assert_eq!(work(&admin, &scope).await, vec![row(&one, 1, 4)]);

    let (volume, tree) = volume_and_tree(&admin, &scope, &one).await;
    let two = titled_chapter(&store, &scope, "f324", &volume, "第二章", tree).await;
    assert_eq!(
        work(&admin, &scope).await,
        vec![row(&one, 1, 4), row(&two, 2, 1)]
    );

    let deleted = settle_delete_chapter(&store, &scope, "f325", &two, tree + 1).await;
    assert!(matches!(deleted.outcome, TransitionOutcome::Applied(_)));
    let delete_sequence = latest_forward(&admin, &scope).await;
    assert_eq!(
        work(&admin, &scope).await,
        vec![row(&one, 1, 4), row(&two, 2, 2)]
    );

    let restored = undo_named(
        &store,
        &scope,
        &chapter.editor_session_id,
        delete_sequence,
        &head(&admin, &one).await,
        "f326",
    )
    .await;
    assert!(matches!(restored.outcome, TransitionOutcome::Applied(_)));
    let marked = vec![row(&one, 1, 4), row(&two, 2, 3)];
    assert_eq!(work(&admin, &scope).await, marked);

    // The binding stays after the deployment loses its embedding configuration or gets another
    // Embedding Registration. Then an Author Edit changes the head and writes no mark.
    let other = Some(&AGENT_PLAN_EMBEDDING_REGISTRATION);
    for (registration, named) in [(None, ("f327", 3)), (other, ("f328", 4))] {
        chapter.head = head(&admin, &one).await;
        let deployment = store.clone().with_embedding_registration(registration);
        let command = prepend(&deployment, &chapter, named, 0, "又一句。").await;
        deployment.apply_author_edit(&command).await.unwrap();
        assert_ne!(head(&admin, &one).await, chapter.head);
        assert_eq!(work(&admin, &scope).await, marked);
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn one_claim_holds_the_work_after_the_quiet_time_and_its_steps_complete_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let mut chapter = chapter(&store, &admin, ["f33", "f34"], "f35", &texts()).await;
    let scope = chapter.scope.clone();
    park_other_work(&admin, &scope).await;
    // The quiet time of the Author Edit runs.
    assert_eq!(claim(&store, FAKE_ONLY).await, None);
    wake(&admin, &scope).await;
    // A caller that composes another adapter, or no adapter, claims nothing.
    assert_eq!(
        claim(&store, &[&AGENT_PLAN_EMBEDDING_REGISTRATION]).await,
        None
    );
    assert_eq!(claim(&store, &[]).await, None);

    let first = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(
        (
            &first.project_scope,
            first.chapter_id.as_str(),
            first.embedding_registration_revision.as_str(),
            first.mark_generation,
            first.fence_token,
        ),
        (
            &scope,
            chapter.chapter_id.as_str(),
            HOST_FAKE_EMBEDDING_REGISTRATION.revision,
            2,
            1
        )
    );
    // One Worker holds the claim until its lease ends. The next claim has a higher fence token.
    assert_eq!(claim(&store, FAKE_ONLY).await, None);
    expire(&admin, &scope).await;
    let second = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(second.fence_token, 2);

    // The earlier claim cannot write, and a claim with the Scope of another Project cannot write a
    // row of this Project.
    let key = PassageVectorKey {
        embedding_registration_revision: second.embedding_registration_revision.clone(),
        cut_revision: PASSAGE_CUT_REVISION.to_owned(),
        passage_digest: format!("sha256:{}", "0".repeat(64)),
    };
    let zero = vec![0.0_f32; HOST_FAKE_EMBEDDING_REGISTRATION.dimensions];
    let row = PassageVectorRow {
        chapter_id: &chapter.chapter_id,
        key: &key,
        embedding: &zero,
    };
    let other = ClaimedPassageIndex {
        project_scope: ProjectScope::new(
            scope.owner_user_id.clone(),
            ProjectId::new("018f0000-0000-7001-8000-00000000f3ff"),
        ),
        ..second.clone()
    };
    assert_eq!(
        [
            store
                .write_fenced_passage_vector(&first, &row)
                .await
                .unwrap(),
            store
                .remove_fenced_passage_vectors(&first, std::slice::from_ref(&key))
                .await
                .unwrap(),
            store
                .complete_passage_index_work(&first, PASSAGE_CUT_REVISION)
                .await
                .unwrap(),
            store
                .release_passage_index_work(&first, Duration::ZERO)
                .await
                .unwrap(),
            store
                .write_fenced_passage_vector(&other, &row)
                .await
                .unwrap(),
        ],
        [false; 5]
    );
    assert_eq!(rows(&admin, &scope).await, 0);

    // Each step sends one request, and the last step completes the work.
    assert_eq!(
        steps(&store, &second).await,
        ["written", "written", "written", "completed"]
    );
    assert_eq!(
        (
            rows(&admin, &scope).await,
            settlements(&admin, &scope).await.len()
        ),
        (3, 3)
    );
    assert_eq!(claim(&store, FAKE_ONLY).await, None);

    // An edit marks the Chapter again. The next work removes the row that is not current and
    // sends one request for the changed passage.
    let command = prepend(&store, &chapter, ("f352", 2), 0, "新的一句。").await;
    store.apply_author_edit(&command).await.unwrap();
    chapter.head = head(&admin, &chapter.chapter_id).await;
    wake(&admin, &scope).await;
    let third = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!((third.mark_generation, third.fence_token), (3, 3));
    assert_eq!(step(&store, &third).await, PassageIndexStep::Written);
    // A change during the work marks the Chapter again, and the completion keeps that mark.
    admin.batch_execute("BEGIN").await.unwrap();
    deploy(&admin).await;
    mark_passage_index_work(
        &admin,
        &scope,
        MarkedChapters::Changed(&[&chapter.chapter_id]),
    )
    .await
    .unwrap();
    admin.batch_execute("COMMIT").await.unwrap();
    assert_eq!(step(&store, &third).await, PassageIndexStep::Completed);
    let id = chapter.chapter_id.clone();
    assert_eq!(work(&admin, &scope).await, vec![(id, 1, 4, 3, true)]);
    assert_eq!(
        (
            rows(&admin, &scope).await,
            settlements(&admin, &scope).await.len()
        ),
        (3, 4)
    );
    wake(&admin, &scope).await;
    let fourth = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(steps(&store, &fourth).await, ["completed"]);
    assert_eq!(settlements(&admin, &scope).await.len(), 4);
    assert_eq!(claim(&store, FAKE_ONLY).await, None);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_request_with_no_vector_keeps_the_mark_and_the_next_try_waits_for_the_quiet_time() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let mut texts = texts();
    texts[4].push_str(EMBEDDING_REJECTION_MARKER);
    let mut chapter = chapter(&store, &admin, ["f36", "f37"], "f38", &texts).await;
    let scope = chapter.scope.clone();
    let id = chapter.chapter_id.clone();
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    let first = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(
        steps(&store, &first).await,
        ["written", "written", "released"]
    );
    // The mark stays, the lease ended, and the next try waits for the quiet time.
    assert_eq!(
        work(&admin, &scope).await,
        vec![(id.clone(), 1, 2, 0, true)]
    );
    assert_eq!(claim(&store, FAKE_ONLY).await, None);

    // Each later try sends only the request of the passage with no row.
    let mut sequence = 2;
    for marker in ["", EMBEDDING_UNKNOWN_MARKER, EMBEDDING_NOT_SUBMITTED_MARKER] {
        if !marker.is_empty() {
            let suffix = format!("f38{sequence}");
            let command = prepend(&store, &chapter, (&suffix, sequence), 4, marker).await;
            store.apply_author_edit(&command).await.unwrap();
            chapter.head = head(&admin, &id).await;
            sequence += 1;
        }
        wake(&admin, &scope).await;
        let again = claim(&store, FAKE_ONLY).await.unwrap();
        assert_eq!(steps(&store, &again).await, ["released"]);
    }
    let stored = |text: &str| Some(text.to_owned());
    assert_eq!(
        settlements(&admin, &scope).await,
        [
            stored("vector"),
            stored("vector"),
            stored("rejected"),
            stored("rejected"),
            stored("outcome_unknown"),
            stored("not_submitted"),
        ]
    );
    // The rows of the passages that got a vector stay.
    assert_eq!(rows(&admin, &scope).await, 2);
    assert_eq!(work(&admin, &scope).await, vec![(id, 1, 4, 0, true)]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_manuscript_that_exists_gets_its_work_in_chapter_order_with_the_first_binding() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    // A deployment with no embedding configuration makes no binding and writes no mark.
    let (scope, one, run_id) = embedding_project(&store, &admin, ["f39", "f3a"], None).await;
    complete(&store, &admin, &scope, &run_id).await;
    let (volume, tree) = volume_and_tree(&admin, &scope, &one).await;
    let two = titled_chapter(&store, &scope, "f3b0", &volume, "第二章", tree).await;
    let three = titled_chapter(&store, &scope, "f3b1", &volume, "第三章", tree + 1).await;
    assert_eq!(work(&admin, &scope).await, Vec::new());

    let run_id = admit_embedding_run(&store, &scope, &one, "f3c", FAKE).await;
    complete(&store, &admin, &scope, &run_id).await;
    let live = |id: &str, order| (id.to_owned(), order, 1, 0, false);
    assert_eq!(
        work(&admin, &scope).await,
        vec![live(&one, 1), live(&two, 2), live(&three, 3)]
    );
    park_other_work(&admin, &scope).await;
    let mut claims = Vec::new();
    while let Some(claimed) = claim(&store, FAKE_ONLY).await {
        claims.push(claimed);
    }
    let order: Vec<&str> = claims
        .iter()
        .map(|claim| claim.chapter_id.as_str())
        .collect();
    assert_eq!(order, [one.as_str(), two.as_str(), three.as_str()]);

    // Work that covers another cut revision is pending again.
    assert!(
        store
            .complete_passage_index_work(&claims[0], "storyos.passage-cut.v0")
            .await
            .unwrap()
    );
    let again = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(
        (again.chapter_id.as_str(), again.fence_token),
        (one.as_str(), 2)
    );

    // A database with no vector table writes no mark and claims no work.
    expire(&admin, &scope).await;
    let before = work(&admin, &scope).await;
    admin
        .batch_execute(
            "BEGIN; ALTER TABLE storyos.passage_vectors RENAME TO passage_vectors_hidden",
        )
        .await
        .unwrap();
    deploy(&admin).await;
    mark_passage_index_work(&admin, &scope, MarkedChapters::Changed(&[&one]))
        .await
        .unwrap();
    let marked = work(&admin, &scope).await;
    let revisions = [HOST_FAKE_EMBEDDING_REGISTRATION.revision];
    let hidden = claim_passage_index_row(
        &admin,
        &revisions,
        PASSAGE_CUT_REVISION,
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    admin.batch_execute("ROLLBACK").await.unwrap();
    assert_eq!((marked, hidden), (before, None));
    // With the vector table, the same claim takes the work.
    assert!(claim(&store, FAKE_ONLY).await.is_some());
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_edit_during_the_work_does_not_wait_and_the_work_sends_only_the_changed_passage() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["2f0", "2f1"], "2f2", &texts()).await;
    let scope = chapter.scope.clone();
    let id = chapter.chapter_id.clone();
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    let claimed = claim(&store, FAKE_ONLY).await.unwrap();
    for _ in 0..2 {
        assert_eq!(step(&store, &claimed).await, PassageIndexStep::Written);
    }

    // The author edits the first passage during the request of the last passage. The edit does not
    // wait for the work, and the step writes the row of the text that it read.
    let command = prepend(&store, &chapter, ("2f22", 2), 0, "新的一句。").await;
    let during = During::new(async {
        store.apply_author_edit(&command).await.unwrap();
    });
    assert_eq!(
        step_with(&store, &during, &claimed).await,
        PassageIndexStep::Written
    );
    assert_ne!(head(&admin, &id).await, chapter.head);
    assert_eq!(digest_rows(&admin, &scope).await, digests(&texts()));
    assert_eq!(
        work(&admin, &scope).await,
        vec![(id.clone(), 1, 3, 0, true)]
    );

    // The next steps remove the row that is not current and send one request for the changed
    // passage. The mark of the edit keeps the work open.
    assert_eq!(steps(&store, &claimed).await, ["written", "completed"]);
    let mut changed = texts();
    changed[0] = format!("新的一句。{}", changed[0]);
    assert_eq!(digest_rows(&admin, &scope).await, digests(&changed));
    assert_eq!(settlements(&admin, &scope).await.len(), 4);
    assert_eq!(
        work(&admin, &scope).await,
        vec![(id.clone(), 1, 3, 2, true)]
    );

    // After the quiet time, the next work sends no request.
    wake(&admin, &scope).await;
    let next = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(steps(&store, &next).await, ["completed"]);
    assert_eq!(settlements(&admin, &scope).await.len(), 4);
    assert_eq!(work(&admin, &scope).await, vec![(id, 1, 3, 3, false)]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancel_of_a_run_during_the_work_does_not_change_the_work() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["2f3", "2f4"], "2f5", &texts()).await;
    let scope = chapter.scope.clone();
    let run_id = admit_embedding_run(&store, &scope, &chapter.chapter_id, "2f6", FAKE).await;
    let cancel = cancellation(&store, &scope, 0xf3f5, &run_id).await;
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    let claimed = claim(&store, FAKE_ONLY).await.unwrap();
    let before = work(&admin, &scope).await;

    // The author cancels the Run during the first request. The cancel is a command on the Run, and
    // the index work is not a Run.
    let during = During::new(async {
        let settled = store
            .cancel_agent_run(&cancel.envelope, &cancel.input)
            .await
            .unwrap();
        assert!(matches!(settled.outcome, TransitionOutcome::Applied(_)));
    });
    assert_eq!(
        step_with(&store, &during, &claimed).await,
        PassageIndexStep::Written
    );
    assert_eq!(run_status(&admin, &run_id).await, "cancelled");
    assert_eq!(work(&admin, &scope).await, before);
    // The claim keeps its fence token, and its steps complete the work.
    assert_eq!(
        steps(&store, &claimed).await,
        ["written", "written", "completed"]
    );
    assert_eq!(digest_rows(&admin, &scope).await, digests(&texts()));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_editor_and_each_run_work_while_each_embedding_request_fails() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["2f7", "2f8"], "2f9", &texts()).await;
    let scope = chapter.scope.clone();
    let id = chapter.chapter_id.clone();
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    let first = claim(&store, FAKE_ONLY).await.unwrap();
    let released = step_with(&store, &NoEmbeddingDestination, &first).await;
    assert!(matches!(released, PassageIndexStep::Released(_)));

    // An Author Edit and a Run settle while the embedding destination is not available.
    let command = prepend(&store, &chapter, ("2f92", 2), 0, "新的一句。").await;
    store.apply_author_edit(&command).await.unwrap();
    assert_ne!(head(&admin, &id).await, chapter.head);
    let run_id = admit_embedding_run(&store, &scope, &id, "2fd", FAKE).await;
    complete(&store, &admin, &scope, &run_id).await;
    assert_eq!(run_status(&admin, &run_id).await, "completed");

    // The next try after the quiet time fails again, and the mark stays.
    wake(&admin, &scope).await;
    let again = claim(&store, FAKE_ONLY).await.unwrap();
    let released = step_with(&store, &NoEmbeddingDestination, &again).await;
    assert!(matches!(released, PassageIndexStep::Released(_)));
    assert_eq!(rows(&admin, &scope).await, 0);
    assert_eq!(work(&admin, &scope).await, vec![(id, 1, 3, 0, true)]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stopped_worker_leaves_whole_rows_and_the_next_claim_continues_the_chapter() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["2fa", "2fb"], "2fc", &texts()).await;
    let scope = chapter.scope.clone();
    let id = chapter.chapter_id.clone();
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    let first = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(step(&store, &first).await, PassageIndexStep::Written);
    // The Worker stops after the first passage. The one row is the vector of one passage.
    drop(first);
    let written = digest_rows(&admin, &scope).await;
    assert_eq!(written.len(), 1);
    assert!(digests(&texts()).contains(&written[0]));
    assert_eq!(
        work(&admin, &scope).await,
        vec![(id.clone(), 1, 2, 0, false)]
    );

    // The lease holds the work until it ends. The next claim continues the Chapter and sends no
    // request again for the written passage.
    assert_eq!(claim(&store, FAKE_ONLY).await, None);
    expire(&admin, &scope).await;
    let next = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(next.fence_token, 2);
    assert_eq!(
        steps(&store, &next).await,
        ["written", "written", "completed"]
    );
    assert_eq!(digest_rows(&admin, &scope).await, digests(&texts()));
    assert_eq!(settlements(&admin, &scope).await.len(), 3);
    assert_eq!(work(&admin, &scope).await, vec![(id, 1, 2, 2, false)]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_rebuild_marks_each_work_row_and_the_work_makes_each_row_again() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["f3d", "2fe"], "2ff", &texts()).await;
    let scope = chapter.scope.clone();
    let id = chapter.chapter_id.clone();
    park_other_work(&admin, &scope).await;
    wake(&admin, &scope).await;
    let claimed = claim(&store, FAKE_ONLY).await.unwrap();
    assert_eq!(steps(&store, &claimed).await.len(), 4);
    // A Chapter that a deployment with no embedding configuration made has no work row.
    let (volume, tree) = volume_and_tree(&admin, &scope, &id).await;
    let unconfigured = store.clone().with_embedding_registration(None);
    let two = titled_chapter(&unconfigured, &scope, "f3fc", &volume, "第二章", tree).await;
    assert_eq!(work(&admin, &scope).await.len(), 1);

    // A loss of the vector rows. The rebuild marks the work with no wait, makes the missing row,
    // and sends no request.
    admin
        .execute(
            "DELETE FROM storyos.passage_vectors WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    // The flag `--rebuild-passage-index` of the Worker binary runs this mark.
    assert!(store.mark_each_passage_index_work().await.unwrap() >= 1);
    assert_eq!(settlements(&admin, &scope).await.len(), 3);
    assert_eq!(
        work(&admin, &scope).await,
        vec![(id.clone(), 1, 3, 2, false), (two.clone(), 2, 1, 0, false)]
    );

    // The work then makes each row again.
    park_other_work(&admin, &scope).await;
    while let Some(claimed) = claim(&store, FAKE_ONLY).await {
        steps(&store, &claimed).await;
    }
    assert_eq!(digest_rows(&admin, &scope).await, digests(&texts()));
    assert_eq!(settlements(&admin, &scope).await.len(), 6);
    assert_eq!(
        work(&admin, &scope).await,
        vec![(id, 1, 3, 3, false), (two, 2, 1, 1, false)]
    );
}

/// The claim asks this read first, so that an idle turn of the Worker makes no claim span. The read
/// sees only work that a claim can take. Work in its quiet time, work of another registration,
/// held work, and completed work do not wait.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_read_of_waiting_work_sees_only_work_that_a_claim_can_take() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = configured().await;
    let chapter = chapter(&store, &admin, ["591", "592"], "593", &texts()).await;
    let scope = chapter.scope.clone();
    park_other_work(&admin, &scope).await;
    let fake = [HOST_FAKE_EMBEDDING_REGISTRATION.revision];
    let quiet = waits(&store, &fake).await;
    wake(&admin, &scope).await;
    let due = waits(&store, &fake).await;
    let other = waits(&store, &[AGENT_PLAN_EMBEDDING_REGISTRATION.revision]).await;
    let claimed = claim(&store, FAKE_ONLY).await.unwrap();
    let held = waits(&store, &fake).await;
    steps(&store, &claimed).await;

    assert_eq!(
        (quiet, due, other, held, waits(&store, &fake).await),
        (false, true, false, false, false)
    );
}

#[path = "passage_index_rebuild_tests.rs"]
mod rebuild;
