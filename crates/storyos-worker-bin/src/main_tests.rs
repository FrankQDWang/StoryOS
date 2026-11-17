//! The one-shot commands of the Worker binary with a store that records each call. Each call that
//! sends a request or settles work is unreachable.

use std::sync::Mutex;
use std::time::Duration;

use storyos_application::{
    AgentRunWorkStore, ArchiveExportWorkStore, ClaimedAgentRun, ClaimedArchiveExport,
    ClaimedExportWork, ClaimedPassageIndex, ClaimedReadableExport, CompleteAgentRunError,
    CompleteArchiveExport, CompleteArchiveExportError, CompleteReadableExport,
    CompleteReadableExportError, DestinationRequest, DispatchClaim, DispatchRecord, EmbeddingClaim,
    EmbeddingDispatchStore, EmbeddingProjection, EmbeddingRecord, EmbeddingResult, EmbeddingRoute,
    ExportWorkStore, ModelDispatchStore, NextDispatchWork, PassageIndexFacts,
    PassageIndexWorkStore, PassageVectorKey, PassageVectorRow, PassageVectorStore,
    ProjectReadError, ProjectScope, QuestionSearch, ReadableExportWorkStore, ResponseReference,
    StreamStop, ToolCallRequest, ToolDispatchClaim, ToolDispatchRecord, ToolDispatchStore,
    ToolWork, WirePayloadProjection,
};
use storyos_core::{
    AGENT_PLAN_EMBEDDING_REGISTRATION, EmbeddingInput, EmbeddingRefusal, EmbeddingRegistration,
    HOST_FAKE_EMBEDDING_REGISTRATION, ModelAdapter, NativeStreamItem,
};

use super::one_shot;

/// A store with no work. It records each claim and each rebuild mark.
#[derive(Default)]
struct RecordingStore {
    calls: Mutex<Vec<String>>,
}

impl RecordingStore {
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl ExportWorkStore for RecordingStore {
    async fn claim_next_export_work(&self) -> Result<Option<ClaimedExportWork>, ProjectReadError> {
        self.record("export".to_owned());
        Ok(None)
    }
}

impl ReadableExportWorkStore for RecordingStore {
    async fn claim_next_readable_export(
        &self,
    ) -> Result<Option<ClaimedReadableExport>, ProjectReadError> {
        unreachable!("the Worker claims export work as one claim")
    }

    async fn complete_readable_export(
        &self,
        _claim: &ClaimedReadableExport,
    ) -> Result<CompleteReadableExport, CompleteReadableExportError> {
        unreachable!("the store has no export work")
    }
}

impl ArchiveExportWorkStore for RecordingStore {
    async fn claim_next_archive_export(
        &self,
    ) -> Result<Option<ClaimedArchiveExport>, ProjectReadError> {
        unreachable!("the Worker claims export work as one claim")
    }

    async fn complete_archive_export(
        &self,
        _claim: &ClaimedArchiveExport,
    ) -> Result<CompleteArchiveExport, CompleteArchiveExportError> {
        unreachable!("the store has no export work")
    }
}

impl AgentRunWorkStore for RecordingStore {
    async fn claim_next_agent_run(
        &self,
        adapters: &[ModelAdapter],
    ) -> Result<Option<ClaimedAgentRun>, ProjectReadError> {
        self.record(format!("agent_run {adapters:?}"));
        Ok(None)
    }
}

impl ToolDispatchStore for RecordingStore {
    async fn next_tool_call(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<Option<ToolWork>, CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn record_question_search(
        &self,
        _claim: &ClaimedAgentRun,
        _search: &QuestionSearch,
        _result: EmbeddingResult,
    ) -> Result<(), CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn commit_tool_dispatch_claim(
        &self,
        _claim: &ClaimedAgentRun,
        _request: &ToolCallRequest,
        _projection: &WirePayloadProjection,
    ) -> Result<Option<ToolDispatchClaim>, CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn record_tool_call(
        &self,
        _claim: &ClaimedAgentRun,
        _record: ToolDispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }
}

impl ModelDispatchStore for RecordingStore {
    async fn next_dispatch_work(
        &self,
        _claim: &ClaimedAgentRun,
        _adapters: &'static [ModelAdapter],
    ) -> Result<NextDispatchWork, CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn commit_dispatch_claim(
        &self,
        _claim: &ClaimedAgentRun,
        _request: &DestinationRequest,
        _projection: &WirePayloadProjection,
    ) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn append_model_stream_events(
        &self,
        _claim: &ClaimedAgentRun,
        _dispatch: &DispatchClaim,
        _events: &[NativeStreamItem],
        _reference: Option<&ResponseReference>,
    ) -> Result<Option<StreamStop>, CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn renew_agent_run_lease(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<Option<Duration>, CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }

    async fn record(
        &self,
        _claim: &ClaimedAgentRun,
        _record: DispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        unreachable!("the store has no AgentRun")
    }
}

impl EmbeddingDispatchStore for RecordingStore {
    async fn embedding_route(
        &self,
        _scope: &ProjectScope,
    ) -> Result<Option<EmbeddingRoute>, ProjectReadError> {
        unreachable!("a one-shot command sends no embedding request")
    }

    async fn commit_embedding_claim(
        &self,
        _scope: &ProjectScope,
        _registration: Option<&'static EmbeddingRegistration>,
        _input: &EmbeddingInput,
        _projection: &EmbeddingProjection,
    ) -> Result<Result<EmbeddingClaim, EmbeddingRefusal>, ProjectReadError> {
        unreachable!("a one-shot command sends no embedding request")
    }

    async fn record_embedding(
        &self,
        _scope: &ProjectScope,
        _claim: &EmbeddingClaim,
        _record: &EmbeddingRecord<'_>,
    ) -> Result<(), ProjectReadError> {
        unreachable!("a one-shot command sends no embedding request")
    }
}

impl PassageVectorStore for RecordingStore {
    async fn vector_store_available(
        &self,
        _scope: &ProjectScope,
    ) -> Result<bool, ProjectReadError> {
        unreachable!("a one-shot command reads no Chapter")
    }

    async fn read_passage_index_facts(
        &self,
        _scope: &ProjectScope,
        _chapter_id: &str,
    ) -> Result<PassageIndexFacts, ProjectReadError> {
        unreachable!("a one-shot command reads no Chapter")
    }

    async fn remove_passage_vectors(
        &self,
        _scope: &ProjectScope,
        _chapter_id: &str,
        _keys: &[PassageVectorKey],
    ) -> Result<u64, ProjectReadError> {
        unreachable!("a one-shot command writes no vector row")
    }

    async fn write_passage_vector(
        &self,
        _scope: &ProjectScope,
        _row: &PassageVectorRow<'_>,
    ) -> Result<(), ProjectReadError> {
        unreachable!("a one-shot command writes no vector row")
    }
}

impl PassageIndexWorkStore for RecordingStore {
    async fn claim_next_passage_index_work(
        &self,
        registration_revisions: &[&str],
        _cut_revision: &str,
        _lease: Duration,
    ) -> Result<Option<ClaimedPassageIndex>, ProjectReadError> {
        self.record(format!("index {registration_revisions:?}"));
        Ok(None)
    }

    async fn write_fenced_passage_vector(
        &self,
        _claim: &ClaimedPassageIndex,
        _row: &PassageVectorRow<'_>,
    ) -> Result<bool, ProjectReadError> {
        unreachable!("the store has no index work")
    }

    async fn remove_fenced_passage_vectors(
        &self,
        _claim: &ClaimedPassageIndex,
        _keys: &[PassageVectorKey],
    ) -> Result<bool, ProjectReadError> {
        unreachable!("the store has no index work")
    }

    async fn complete_passage_index_work(
        &self,
        _claim: &ClaimedPassageIndex,
        _cut_revision: &str,
    ) -> Result<bool, ProjectReadError> {
        unreachable!("the store has no index work")
    }

    async fn release_passage_index_work(
        &self,
        _claim: &ClaimedPassageIndex,
        _wait: Duration,
    ) -> Result<bool, ProjectReadError> {
        unreachable!("the store has no index work")
    }

    async fn mark_each_passage_index_work(&self) -> Result<u64, ProjectReadError> {
        self.record("rebuild".to_owned());
        Ok(3)
    }
}

async fn run(argument: &str) -> (&'static str, Vec<String>) {
    let store = RecordingStore::default();
    let (result, stage) = one_shot(&[argument.to_owned()], &store)
        .await
        .expect("the argument names a one-shot command");
    result.expect("the command succeeds");
    (stage, store.calls.into_inner().unwrap())
}

#[tokio::test]
async fn the_binary_claims_the_index_work_of_each_embedding_adapter_that_it_composes() {
    let index = format!(
        "index {:?}",
        [
            HOST_FAKE_EMBEDDING_REGISTRATION.revision,
            AGENT_PLAN_EMBEDDING_REGISTRATION.revision
        ]
    );
    let agent_run = format!(
        "agent_run {:?}",
        <super::WorkerAdapters as storyos_application::ModelProviderAdapter>::ADAPTERS
    );
    assert_eq!(
        run("--claim-only").await,
        ("claim_only", vec!["export".to_owned(), agent_run, index])
    );
}

#[tokio::test]
async fn the_rebuild_flag_marks_the_index_work_and_sends_no_request() {
    assert_eq!(
        run("--rebuild-passage-index").await,
        ("rebuild_passage_index", vec!["rebuild".to_owned()])
    );
}
