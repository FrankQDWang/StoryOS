//! The passage index work of a Chapter (ADR 0095 section 4). A change of the Chapter marks its work
//! in the transaction of the change. The work is not a Run: a Worker claims it with a lease and a
//! fence token, and each step sends at most one embedding request.

use std::future::Future;
use std::time::Duration;

use storyos_core::{EmbeddingRegistration, PASSAGE_CUT_REVISION};

use crate::passage_index::{PassagePlan, passage_input, plan_passages};
use crate::{
    DiagnosticField, DiagnosticId, EmbeddingDispatchStore, EmbeddingProviderAdapter,
    EmbeddingResult, PassageVectorKey, PassageVectorRow, PassageVectorStore, ProjectReadError,
    ProjectScope, dispatch_embedding,
};

/// The time with no change of a Chapter before its work starts. The same time passes before a new
/// try after a failed try. The coordinator set the initial value from the record of S7-C1.
pub const PASSAGE_INDEX_QUIET_TIME: Duration = Duration::from_secs(/*10 minutes*/ 600);

/// The lease of one claim. Each fenced write of the claim starts the lease again.
pub const PASSAGE_INDEX_LEASE: Duration = Duration::from_secs(/*default*/ 60);

/// One fenced Worker claim of the index work of one Chapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedPassageIndex {
    pub project_scope: ProjectScope,
    pub chapter_id: String,
    /// The Embedding Registration of the use binding of the Project at the last mark.
    pub embedding_registration_revision: String,
    /// The mark generation that the claim covers. A later mark gives a higher generation.
    pub mark_generation: i64,
    pub fence_token: i64,
    pub lease: Duration,
}

/// The persistence of the index work. Each operation is one committed transaction. A fenced
/// operation returns `false` and changes nothing when a later claim holds the work or the lease
/// ended, and it starts the lease again when it succeeds.
pub trait PassageIndexWorkStore: PassageVectorStore + EmbeddingDispatchStore {
    /// Claims the next pending work after its quiet time, in mark order and then in Chapter
    /// order. Claims only work whose Embedding Registration is one of `registration_revisions`.
    fn claim_next_passage_index_work(
        &self,
        registration_revisions: &[&str],
        cut_revision: &str,
        lease: Duration,
    ) -> impl Future<Output = Result<Option<ClaimedPassageIndex>, ProjectReadError>> + Send;

    fn write_fenced_passage_vector(
        &self,
        claim: &ClaimedPassageIndex,
        row: &PassageVectorRow<'_>,
    ) -> impl Future<Output = Result<bool, ProjectReadError>> + Send;

    fn remove_fenced_passage_vectors(
        &self,
        claim: &ClaimedPassageIndex,
        keys: &[PassageVectorKey],
    ) -> impl Future<Output = Result<bool, ProjectReadError>> + Send;

    /// Records that the rows cover the mark generation of the claim and `cut_revision`, and ends
    /// the lease. A later mark keeps the work pending.
    fn complete_passage_index_work(
        &self,
        claim: &ClaimedPassageIndex,
        cut_revision: &str,
    ) -> impl Future<Output = Result<bool, ProjectReadError>> + Send;

    /// Ends the lease and keeps the mark. The next claim starts after `wait`.
    fn release_passage_index_work(
        &self,
        claim: &ClaimedPassageIndex,
        wait: Duration,
    ) -> impl Future<Output = Result<bool, ProjectReadError>> + Send;

    /// Marks the work of each Chapter that has a work row, with no wait. Returns the number of
    /// marked rows.
    fn mark_each_passage_index_work(
        &self,
    ) -> impl Future<Output = Result<u64, ProjectReadError>> + Send;
}

/// The result of one step of a claim.
#[derive(Clone, Debug, PartialEq)]
pub enum PassageIndexStep {
    /// One request gave a vector, and its row is written. The claim continues.
    Written,
    /// No passage stays without a row. The work of the claim is complete.
    Completed,
    /// A request gave no vector. The mark stays, and the next try starts after the quiet time.
    Released(EmbeddingResult),
    /// A later claim holds the work, or the lease ended.
    StaleFence,
}

impl DiagnosticField for PassageIndexStep {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::Written => "written",
            Self::Completed => "completed",
            Self::Released(_) => "released",
            Self::StaleFence => "stale_fence",
        }
    }
}

/// Claims the next index work for the Embedding Registrations that the caller composes.
#[tracing::instrument(skip_all, fields(
    project_id = tracing::field::Empty,
    chapter_id = tracing::field::Empty,
    fence_token = tracing::field::Empty,
    outcome = tracing::field::Empty,
))]
pub async fn claim_next_passage_index_work(
    store: &impl PassageIndexWorkStore,
    registrations: &[&'static EmbeddingRegistration],
) -> Result<Option<ClaimedPassageIndex>, ProjectReadError> {
    let revisions: Vec<&str> = registrations
        .iter()
        .map(|registration| registration.revision)
        .collect();
    let claimed = if revisions.is_empty() {
        None
    } else {
        store
            .claim_next_passage_index_work(&revisions, PASSAGE_CUT_REVISION, PASSAGE_INDEX_LEASE)
            .await?
    };
    let span = tracing::Span::current();
    match &claimed {
        Some(claim) => {
            span.record("project_id", claim.project_scope.project_id.diagnostic());
            span.record("chapter_id", DiagnosticId(&claim.chapter_id).diagnostic());
            span.record("fence_token", claim.fence_token.diagnostic());
            span.record("outcome", "claimed");
        }
        None => {
            span.record("outcome", "none");
        }
    }
    Ok(claimed)
}

/// Marks the index work of each Chapter that has a work row, so that the work makes each vector
/// row again (`storyos-worker --rebuild-passage-index`). It sends no request. Each live Chapter of a
/// Project with the embedding use binding has a work row: the first binding marks each live
/// Chapter, and each later change marks its Chapters. The work of a Chapter that is not live only
/// removes its rows.
#[tracing::instrument(skip_all, fields(marked = tracing::field::Empty))]
pub async fn rebuild_passage_index(
    store: &impl PassageIndexWorkStore,
) -> Result<u64, ProjectReadError> {
    let marked = store.mark_each_passage_index_work().await?;
    tracing::Span::current().record("marked", marked.diagnostic());
    Ok(marked)
}

/// One step of a claim. It removes the rows that are not a current passage, and then sends the
/// request of the first passage with no row. It sends no request when each passage has a row, and
/// then it completes the work. A Chapter that is not live has no passage.
#[tracing::instrument(skip_all, fields(
    project_id = claim.project_scope.project_id.diagnostic(),
    chapter_id = DiagnosticId(&claim.chapter_id).diagnostic(),
    fence_token = claim.fence_token.diagnostic(),
    outcome = tracing::field::Empty,
))]
pub async fn step_passage_index_work(
    store: &impl PassageIndexWorkStore,
    adapter: &impl EmbeddingProviderAdapter,
    registrations: &[&'static EmbeddingRegistration],
    claim: &ClaimedPassageIndex,
) -> Result<PassageIndexStep, ProjectReadError> {
    let step = step(store, adapter, registrations, claim).await?;
    tracing::Span::current().record("outcome", step.diagnostic());
    Ok(step)
}

async fn step(
    store: &impl PassageIndexWorkStore,
    adapter: &impl EmbeddingProviderAdapter,
    registrations: &[&'static EmbeddingRegistration],
    claim: &ClaimedPassageIndex,
) -> Result<PassageIndexStep, ProjectReadError> {
    let scope = &claim.project_scope;
    let facts = store
        .read_passage_index_facts(scope, &claim.chapter_id)
        .await?;
    let (plan, chapter) = match facts.chapter {
        Some(chapter) => (
            plan_passages(
                &chapter,
                &claim.embedding_registration_revision,
                &facts.stored,
            ),
            Some(chapter),
        ),
        None => (
            PassagePlan {
                current: Vec::new(),
                stale: facts.stored.clone(),
            },
            None,
        ),
    };
    if !plan.stale.is_empty()
        && !store
            .remove_fenced_passage_vectors(claim, &plan.stale)
            .await?
    {
        return Ok(PassageIndexStep::StaleFence);
    }
    let next = plan
        .current
        .iter()
        .find(|(key, _)| !facts.stored.contains(key));
    let (Some((key, text)), Some(chapter)) = (next, chapter) else {
        let completed = store
            .complete_passage_index_work(claim, PASSAGE_CUT_REVISION)
            .await?;
        return Ok(if completed {
            PassageIndexStep::Completed
        } else {
            PassageIndexStep::StaleFence
        });
    };
    let registration = registrations
        .iter()
        .copied()
        .find(|registration| registration.revision == claim.embedding_registration_revision);
    let input = passage_input(&chapter, &claim.chapter_id, key, text);
    let held = match dispatch_embedding(store, adapter, registration, scope, &input).await? {
        EmbeddingResult::Vector(embedding) => {
            let row = PassageVectorRow {
                chapter_id: &claim.chapter_id,
                key,
                embedding: &embedding,
            };
            (store.write_fenced_passage_vector(claim, &row).await?)
                .then_some(PassageIndexStep::Written)
        }
        result => (store
            .release_passage_index_work(claim, PASSAGE_INDEX_QUIET_TIME)
            .await?)
            .then_some(PassageIndexStep::Released(result)),
    };
    Ok(held.unwrap_or(PassageIndexStep::StaleFence))
}
