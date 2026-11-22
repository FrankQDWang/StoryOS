//! Failed Memory work (S7-E1, the delivery continuation `S7-EVD-003`): the destination rejects each
//! request of a capture and of a pass. The author still commits an edit, and a Run of the author
//! completes with the Memory Index of the entries of the time before. The lead of this test is `a1`.

use serde_json::json;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AuthorEditSettlementEffect, CreatePurpose, DestinationRequest, ModelProviderAdapter,
    ModelStreamSink, NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest,
    ProjectScope, complete_agent_run,
};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_read_chapter::project;
use crate::shared_tests::agent_run_write_memory::first_index;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::replace_text;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::memory_capture::start;
use crate::shared_tests::memory_capture_loop::{record, script, settlement};
use crate::shared_tests::memory_capture_tools::conversation_source;
use crate::shared_tests::memory_consolidation::{capture, settled};
use crate::shared_tests::memory_consolidation_merge::{flag_lines, seed};
use crate::shared_tests::model_gateway::{RuntimeClock, claim_run};

/// The fake destination, but it rejects each request of a capture and of a pass.
struct RejectsMemoryWork;

enum Exchange {
    Fake(<FakeDestination as ModelProviderAdapter>::Prepared),
    Rejected,
}

impl ModelProviderAdapter for RejectsMemoryWork {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = Exchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let prepared = FakeDestination.prepare(request).await?;
        let memory_work = matches!(request, DestinationRequest::Create(create)
            if matches!(create.purpose,
                CreatePurpose::MemoryCapture(_) | CreatePurpose::MemoryConsolidation));
        Ok(PreparedRequest {
            prepared: if memory_work {
                Exchange::Rejected
            } else {
                Exchange::Fake(prepared.prepared)
            },
            projection: prepared.projection,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        match prepared {
            Exchange::Fake(prepared) => FakeDestination.exchange(prepared, sink).await,
            Exchange::Rejected => Observation::Rejected {
                reason: "invalid_request".to_owned(),
            },
        }
    }
}

/// Completes the background Run `run_id` with the destination that rejects Memory work.
async fn complete_rejected(admin: &Client, scope: &ProjectScope, run_id: &str) {
    let store = stores().await.0;
    for _ in 0..=storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS {
        let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
        complete_agent_run(
            &store,
            &RejectsMemoryWork,
            &NoContractFaults,
            &RuntimeClock,
            &claim,
        )
        .await
        .unwrap();
        if status(admin, run_id).await != "queued" {
            return;
        }
    }
}

/// The status of the Run `run_id`.
async fn status(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT status FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn with_failed_memory_work_an_edit_commits_and_a_run_has_the_index_of_before() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, _, session) =
        project(&store, &admin, "a10", 0xa1f0, &["Lin ran."]).await;
    seed(
        &admin,
        &scope,
        &[(1, "character", "author_request", "林舟")],
    )
    .await;
    let source = conversation_source(&admin, &scope).await;
    // One earlier capture leaves a fact that waits, so that a pass can start.
    let fact = json!([{"type": "character", "title": "阿禾", "text": "卖鱼的人。"}]);
    capture(&store, &admin, &scope, &source, fact.clone()).await;
    let lines_before = flag_lines(&admin, &scope).await;

    let failed = start(&store, &scope, &source, &script(&[record(fact)]))
        .await
        .unwrap();
    complete_rejected(&admin, &scope, &failed.run_id).await;
    let pass = crate::shared_tests::memory_consolidation::start_pass(&store, &scope)
        .await
        .unwrap();
    complete_rejected(&admin, &scope, &pass.run_id).await;
    let edit = replace_text(
        &store,
        &admin,
        &scope,
        (&session, 2, "a1f3"),
        &chapter_a,
        (3, "Wei"),
    )
    .await;
    let committed = store.apply_author_edit(&edit).await.unwrap();
    let run = crate::shared_tests::memory_conflict_flag_show::run(
        &store,
        &admin,
        (&scope, &chapter_a),
        "a11",
        "继续写。",
    )
    .await;

    assert_eq!(
        (
            settlement(&admin, &failed.run_id).await,
            settled(&admin, &pass.run_id).await,
            matches!(
                committed.effect,
                AuthorEditSettlementEffect::AuthoritativeApplied { .. }
            ),
            status(&admin, &run).await,
            first_index(&admin, &run).await,
            flag_lines(&admin, &scope).await,
        ),
        (
            json!(["refused", "settled", "failed", "destination_refused", []]),
            json!(["refused", "failed", "destination_refused", [], ["waiting"]]),
            true,
            "completed".to_owned(),
            // The Index of entry 1 only: the fact that waits and the failed work add no line.
            json!([
                "{\"type\":\"character\"}\n{\"author_part\":true,\"description\":\"述。\",\"id\":1,\"title\":\"林舟\"}"
            ]),
            lines_before,
        )
    );
}
