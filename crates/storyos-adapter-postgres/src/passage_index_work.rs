//! The passage index work of a Chapter (ADR 0095 section 4): the marks in the transaction of each
//! change, and the fenced Worker claim of `storyos.passage_index_work`.

use std::time::Duration;

use storyos_application::{
    ClaimedPassageIndex, PASSAGE_INDEX_QUIET_TIME, PassageIndexWorkStore, PassageVectorKey,
    PassageVectorRow, ProjectId, ProjectReadError, ProjectScope, UserId,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_hybrid_search::DEPLOYMENT_REGISTRATION;
use crate::export_work::set_worker_scope;
use crate::passage_vectors::{delete_passage_vectors, insert_passage_vector};

fn read_error(error: impl std::error::Error + Send + Sync + 'static) -> ProjectReadError {
    ProjectReadError::unavailable(error)
}

fn seconds(duration: Duration) -> i64 {
    i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
}

/// The Chapters that one mark names.
pub(crate) enum MarkedChapters<'a> {
    /// A change of these Manuscript Objects. A Volume names each of its Chapters. The quiet time
    /// starts again.
    Changed(&'a [&'a str]),
    /// Each live Chapter of the Project, when the Project gets a new embedding use binding. The
    /// work can start at once.
    Live,
}

/// Marks the index work of the Chapters in the transaction of the change. There is no mark when
/// the deployment has no embedding configuration or the database has no vector table. There is
/// also no mark when the latest embedding use binding of the Project has another Embedding
/// Registration than the deployment. The setting `storyos.embedding_registration` of the
/// transaction gives the Registration of the deployment.
pub(crate) async fn mark_passage_index_work(
    client: &Client,
    scope: &ProjectScope,
    chapters: MarkedChapters<'_>,
) -> Result<(), tokio_postgres::Error> {
    let (objects, wait) = match chapters {
        MarkedChapters::Changed(objects) => (Some(objects), seconds(PASSAGE_INDEX_QUIET_TIME)),
        MarkedChapters::Live => (None, 0),
    };
    client
        .execute(
            &format!(
                "WITH binding AS (
               SELECT embedding_registration_revision
                 FROM storyos.project_external_use_binding_revisions
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND embedding_registration_revision IS NOT NULL
                ORDER BY project_model_use_binding_revision DESC
                LIMIT 1
             ), chapters AS (
               SELECT chapter.manuscript_object_id AS chapter_id, chapter.parent_volume_id,
                      row_number() OVER (ORDER BY volume.tree_order, chapter.tree_order,
                                                  chapter.manuscript_object_id) AS chapter_order,
                      EXISTS (
                        SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                         WHERE (removal.owner_user_id, removal.project_id, removal.chapter_id) =
                               (chapter.owner_user_id, chapter.project_id,
                                chapter.manuscript_object_id)
                      ) AS removed
                 FROM storyos.manuscript_objects AS chapter
                 LEFT JOIN storyos.manuscript_objects AS volume
                   ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                      (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
                WHERE chapter.owner_user_id = $1::text::uuid AND chapter.project_id = $2::text::uuid
                  AND chapter.object_kind = 'chapter'
             )
             INSERT INTO storyos.passage_index_work AS work
               (owner_user_id, project_id, chapter_id, embedding_registration_revision,
                chapter_order, not_before)
             SELECT $1::text::uuid, $2::text::uuid, chapters.chapter_id,
                    binding.embedding_registration_revision, chapters.chapter_order,
                    now() + ($4::bigint * interval '1 second')
               FROM chapters CROSS JOIN binding
              WHERE to_regclass('storyos.passage_vectors') IS NOT NULL
                AND binding.embedding_registration_revision::text = {DEPLOYMENT_REGISTRATION}
                AND CASE WHEN $3::text[] IS NULL THEN NOT chapters.removed
                         ELSE chapters.chapter_id::text = ANY($3::text[])
                           OR chapters.parent_volume_id::text = ANY($3::text[]) END
             ON CONFLICT (owner_user_id, project_id, chapter_id) DO UPDATE
               SET mark_generation = work.mark_generation + 1,
                   embedding_registration_revision = EXCLUDED.embedding_registration_revision,
                   chapter_order = EXCLUDED.chapter_order,
                   not_before = EXCLUDED.not_before"
            ),
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &objects,
                &wait,
            ],
        )
        .await
        .map(|_| ())
}

/// The fence check of a claim in the Project Scope of the claim. It starts the lease again.
async fn hold_fence(
    client: &Client,
    claim: &ClaimedPassageIndex,
) -> Result<bool, ProjectReadError> {
    let held = client
        .execute(
            "UPDATE storyos.passage_index_work
                SET lease_expires_at = clock_timestamp() + ($5::bigint * interval '1 second')
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND chapter_id = $3::text::uuid AND fence_token = $4
                AND lease_expires_at > clock_timestamp()",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.chapter_id,
                &claim.fence_token,
                &seconds(claim.lease),
            ],
        )
        .await
        .map_err(read_error)?;
    Ok(held == 1)
}

/// The condition of a work row that a claim can take: `$1` is the Embedding Registrations and
/// `$2` is the cut revision. The read of waiting work and the claim use this one condition.
macro_rules! waiting_work {
    () => {
        "to_regclass('storyos.passage_vectors') IS NOT NULL
         AND work.embedding_registration_revision::text = ANY($1::text[])
         AND (work.mark_generation > work.completed_generation
              OR work.indexed_cut_revision IS DISTINCT FROM $2)
         AND work.not_before <= clock_timestamp()
         AND (work.lease_expires_at IS NULL OR work.lease_expires_at <= clock_timestamp())"
    };
}

/// Whether a work row that a claim can take is in the worker scope of `client`.
async fn passage_index_row_waits(
    client: &impl tokio_postgres::GenericClient,
    registration_revisions: &[&str],
    cut_revision: &str,
) -> Result<bool, ProjectReadError> {
    Ok(client
        .query_one(
            concat!(
                "SELECT EXISTS (SELECT 1 FROM storyos.passage_index_work AS work WHERE ",
                waiting_work!(),
                ")"
            ),
            &[&registration_revisions, &cut_revision],
        )
        .await
        .map_err(read_error)?
        .get(/*idx*/ 0))
}

/// Claims the next pending work row in the worker scope of `client`, after its quiet time.
pub(crate) async fn claim_passage_index_row(
    client: &impl tokio_postgres::GenericClient,
    registration_revisions: &[&str],
    cut_revision: &str,
    lease: Duration,
) -> Result<Option<ClaimedPassageIndex>, ProjectReadError> {
    let claimed = client
        .query_opt(
            concat!(
                "WITH next_work AS (
                   SELECT owner_user_id, project_id, chapter_id
                     FROM storyos.passage_index_work AS work
                    WHERE ",
                waiting_work!(),
                "
                    ORDER BY work.not_before, work.chapter_order, work.chapter_id
                    FOR UPDATE SKIP LOCKED
                    LIMIT 1
                 )
                 UPDATE storyos.passage_index_work AS work
                    SET claim_generation = work.claim_generation + 1,
                        fence_token = work.claim_generation + 1,
                        lease_expires_at = clock_timestamp() + ($3::bigint * interval '1 second')
                   FROM next_work
                  WHERE (work.owner_user_id, work.project_id, work.chapter_id) =
                        (next_work.owner_user_id, next_work.project_id, next_work.chapter_id)
              RETURNING work.owner_user_id::text, work.project_id::text, work.chapter_id::text,
                        work.embedding_registration_revision::text, work.mark_generation,
                        work.fence_token"
            ),
            &[&registration_revisions, &cut_revision, &seconds(lease)],
        )
        .await
        .map_err(read_error)?;
    Ok(claimed.map(|row| ClaimedPassageIndex {
        project_scope: ProjectScope::new(
            UserId::new(row.get::<_, String>(/*idx*/ 0)),
            ProjectId::new(row.get::<_, String>(/*idx*/ 1)),
        ),
        chapter_id: row.get(/*idx*/ 2),
        embedding_registration_revision: row.get(/*idx*/ 3),
        mark_generation: row.get(/*idx*/ 4),
        fence_token: row.get(/*idx*/ 5),
        lease,
    }))
}

/// One write of a claim after its fence check.
enum FencedWrite<'a> {
    Vector(&'a PassageVectorRow<'a>),
    Remove(&'a [PassageVectorKey]),
    Complete { cut_revision: &'a str },
    Release { wait: Duration },
}

impl PostgresProjectReader {
    /// Runs `write` in one transaction of the Project Scope of the claim after the fence check.
    async fn fenced(
        &self,
        claim: &ClaimedPassageIndex,
        write: FencedWrite<'_>,
    ) -> Result<bool, ProjectReadError> {
        let scope = &claim.project_scope;
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(read_error)?;
        let client = &transaction.client;
        if !hold_fence(client, claim).await? {
            transaction.rollback().await.map_err(read_error)?;
            return Ok(false);
        }
        let work = [
            &scope.owner_user_id.as_ref(),
            &scope.project_id.as_ref(),
            &claim.chapter_id.as_str(),
        ];
        match write {
            FencedWrite::Vector(row) => insert_passage_vector(client, scope, row).await?,
            FencedWrite::Remove(keys) => {
                delete_passage_vectors(client, scope, &claim.chapter_id, keys).await?;
            }
            FencedWrite::Complete { cut_revision } => {
                client
                    .execute(
                        "UPDATE storyos.passage_index_work
                            SET completed_generation = $4, indexed_cut_revision = $5,
                                lease_expires_at = NULL
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                            AND chapter_id = $3::text::uuid",
                        &[
                            work[0],
                            work[1],
                            work[2],
                            &claim.mark_generation,
                            &cut_revision,
                        ],
                    )
                    .await
                    .map_err(read_error)?;
            }
            FencedWrite::Release { wait } => {
                client
                    .execute(
                        "UPDATE storyos.passage_index_work
                            SET not_before = clock_timestamp()
                                  + ($4::bigint * interval '1 second'),
                                lease_expires_at = NULL
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                            AND chapter_id = $3::text::uuid",
                        &[work[0], work[1], work[2], &seconds(wait)],
                    )
                    .await
                    .map_err(read_error)?;
            }
        }
        transaction.commit().await.map_err(read_error)?;
        Ok(true)
    }
}

impl PassageIndexWorkStore for PostgresProjectReader {
    async fn passage_index_work_waits(
        &self,
        registration_revisions: &[&str],
        cut_revision: &str,
    ) -> Result<bool, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_worker_scope(&transaction).await?;
        let waits =
            passage_index_row_waits(&transaction, registration_revisions, cut_revision).await?;
        transaction.commit().await.map_err(read_error)?;
        Ok(waits)
    }

    async fn claim_next_passage_index_work(
        &self,
        registration_revisions: &[&str],
        cut_revision: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedPassageIndex>, ProjectReadError> {
        self.require_release1_storage_activation_proof()
            .await
            .map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_worker_scope(&transaction).await?;
        let claimed =
            claim_passage_index_row(&transaction, registration_revisions, cut_revision, lease)
                .await?;
        transaction.commit().await.map_err(read_error)?;
        Ok(claimed)
    }

    async fn write_fenced_passage_vector(
        &self,
        claim: &ClaimedPassageIndex,
        row: &PassageVectorRow<'_>,
    ) -> Result<bool, ProjectReadError> {
        self.fenced(claim, FencedWrite::Vector(row)).await
    }

    async fn remove_fenced_passage_vectors(
        &self,
        claim: &ClaimedPassageIndex,
        keys: &[PassageVectorKey],
    ) -> Result<bool, ProjectReadError> {
        self.fenced(claim, FencedWrite::Remove(keys)).await
    }

    async fn complete_passage_index_work(
        &self,
        claim: &ClaimedPassageIndex,
        cut_revision: &str,
    ) -> Result<bool, ProjectReadError> {
        self.fenced(claim, FencedWrite::Complete { cut_revision })
            .await
    }

    async fn release_passage_index_work(
        &self,
        claim: &ClaimedPassageIndex,
        wait: Duration,
    ) -> Result<bool, ProjectReadError> {
        self.fenced(claim, FencedWrite::Release { wait }).await
    }

    /// The runtime role reads the Projects with an AgentRun in the worker scope: the policy
    /// `agent_runs_worker_select` gives that scope each Run of each Project. Each Project with an
    /// embedding use binding has a Run, because `createAgentRun` makes the binding. Then the mark
    /// of a new binding runs in the Project Scope of each Project, with the Embedding Registration
    /// of the deployment. So the mark also makes the row of a live Chapter that has none.
    async fn mark_each_passage_index_work(&self) -> Result<u64, ProjectReadError> {
        self.require_release1_storage_activation_proof()
            .await
            .map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_worker_scope(&transaction).await?;
        let projects = transaction
            .query(
                "SELECT DISTINCT owner_user_id::text, project_id::text FROM storyos.agent_runs",
                &[],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        for project in &projects {
            let scope = ProjectScope::new(
                UserId::new(project.get::<_, String>(/*idx*/ 0)),
                ProjectId::new(project.get::<_, String>(/*idx*/ 1)),
            );
            let transaction = self
                .begin_serializable_project_command_transaction(&scope)
                .await
                .map_err(read_error)?;
            mark_passage_index_work(&transaction.client, &scope, MarkedChapters::Live)
                .await
                .map_err(read_error)?;
            transaction.commit().await.map_err(read_error)?;
        }
        Ok(u64::try_from(projects.len()).unwrap_or(u64::MAX))
    }
}

#[cfg(test)]
#[path = "passage_index_work_tests.rs"]
pub(crate) mod tests;
