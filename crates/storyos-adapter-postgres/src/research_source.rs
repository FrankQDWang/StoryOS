//! The Source Snapshots and Source Gaps of research ToolCalls (S5-06). The settlement of a
//! ToolCall writes them in its own transaction, so a crash leaves no half-captured result. The
//! store has no update or delete path, and the runtime role has only SELECT and INSERT on both
//! tables. The revisions of one locator in a Project form one lineage. Each revision has its own
//! identity and capture time; no stored or computed rank can change after a read.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, CreateAgentRunError, ProjectScope,
    ResearchSourceReadStore, ResearchSourceText, ResearchSources,
};
use storyos_contracts::{SourceGapInspect, SourceSnapshotInspect, UnknownResearchCallInspect};
use storyos_core::{
    ResearchCapture, SourceGapReason, SourceKind, ToolAttemptSettlement, ToolCallClass,
    ToolCallState, tool_call_class,
};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::agent_run_work::complete_database_error;
use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// Writes the capture of one settled ToolCall of the claimed Run.
pub(crate) async fn capture(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    tool_call_id: &str,
    capture: &ResearchCapture,
) -> Result<(), CompleteAgentRunError> {
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    let int = |value: usize| {
        i32::try_from(value).map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))
    };
    for (position, source) in capture.snapshots.iter().enumerate() {
        client
            .execute(
                "INSERT INTO storyos.source_snapshots
                   (owner_user_id, project_id, source_snapshot_id, run_id, tool_call_id,
                    entry_position, kind, locator, title, published, text, text_bytes,
                    result_offset, content_digest)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
                &[
                    &owner,
                    &project,
                    &Uuid::now_v7().to_string(),
                    &claim.run_id,
                    &tool_call_id,
                    &int(position)?,
                    &source.kind.as_str(),
                    &source.locator,
                    &source.title,
                    &source.published,
                    &source.text,
                    &int(source.text.len())?,
                    &int(source.span.offset)?,
                    &source.digest,
                ],
            )
            .await
            .map_err(complete_database_error)?;
    }
    for (position, gap) in capture.gaps.iter().enumerate() {
        client
            .execute(
                "INSERT INTO storyos.source_gaps
                   (owner_user_id, project_id, source_gap_id, run_id, tool_call_id, gap_position,
                    locator, reason, result_offset, result_bytes)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6, $7, $8, $9, $10)",
                &[
                    &owner,
                    &project,
                    &Uuid::now_v7().to_string(),
                    &claim.run_id,
                    &tool_call_id,
                    &int(position)?,
                    &gap.locator,
                    &gap.reason.as_str(),
                    &gap.span.map(|span| int(span.offset)).transpose()?,
                    &gap.span.map(|span| int(span.bytes)).transpose()?,
                ],
            )
            .await
            .map_err(complete_database_error)?;
    }
    Ok(())
}

/// The Snapshot columns of the query responses. Each column is immutable, so a response never
/// changes when a later capture of the same locator commits.
const SNAPSHOT_COLUMNS: &str = "snapshot.source_snapshot_id::text, snapshot.tool_call_id::text,
    snapshot.kind, snapshot.locator, snapshot.title, snapshot.published,
    to_char(snapshot.captured_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
    snapshot.content_digest, snapshot.text_bytes::text, snapshot.result_offset::text";

fn snapshot(row: &tokio_postgres::Row) -> Result<SourceSnapshotInspect, CreateAgentRunError> {
    let damaged = || agent_run_parse_error(std::io::Error::other("The Source Snapshot is damaged"));
    Ok(SourceSnapshotInspect {
        source_snapshot_id: row.get(/*idx*/ 0),
        tool_call_id: row.get(/*idx*/ 1),
        kind: match SourceKind::parse(row.get(/*idx*/ 2)).ok_or_else(damaged)? {
            SourceKind::SearchResult => storyos_contracts::SourceSnapshotKind::SearchResult,
            SourceKind::Page => storyos_contracts::SourceSnapshotKind::Page,
        },
        locator: row.get(/*idx*/ 3),
        title: row.get(/*idx*/ 4),
        published: row.get(/*idx*/ 5),
        captured_at: row.get(/*idx*/ 6),
        content_digest: row.get(/*idx*/ 7),
        text_bytes: row.get(/*idx*/ 8),
        result_offset: row.get(/*idx*/ 9),
    })
}

impl ResearchSourceReadStore for PostgresProjectReader {
    async fn list_research_sources(
        &self,
        scope: &ProjectScope,
        run_id: &str,
    ) -> Result<Option<ResearchSources>, CreateAgentRunError> {
        self.in_read_transaction(scope, async |client| {
            let params: [&(dyn tokio_postgres::types::ToSql + Sync); 3] = [
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ];
            let run = client
                .query_opt(
                    "SELECT 1 FROM storyos.agent_runs
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid",
                    &params,
                )
                .await
                .map_err(agent_run_database_error)?;
            if run.is_none() {
                return Ok(None);
            }
            let snapshots = client
                .query(
                    &format!(
                        "SELECT {SNAPSHOT_COLUMNS}
                           FROM storyos.source_snapshots AS snapshot
                           JOIN storyos.tool_calls AS call
                             USING (owner_user_id, project_id, tool_call_id)
                          WHERE snapshot.owner_user_id = $1::text::uuid
                            AND snapshot.project_id = $2::text::uuid
                            AND snapshot.run_id = $3::text::uuid
                          ORDER BY call.decision_position, call.call_position,
                                   snapshot.entry_position"
                    ),
                    &params,
                )
                .await
                .map_err(agent_run_database_error)?
                .iter()
                .map(snapshot)
                .collect::<Result<_, _>>()?;
            let gaps = client
                .query(
                    "SELECT gap.source_gap_id::text, gap.tool_call_id::text, gap.locator,
                            gap.reason,
                            to_char(gap.recorded_at AT TIME ZONE 'UTC',
                                    'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                            gap.result_offset::text, gap.result_bytes::text
                       FROM storyos.source_gaps AS gap
                       JOIN storyos.tool_calls AS call
                         USING (owner_user_id, project_id, tool_call_id)
                      WHERE gap.owner_user_id = $1::text::uuid
                        AND gap.project_id = $2::text::uuid AND gap.run_id = $3::text::uuid
                      ORDER BY call.decision_position, call.call_position, gap.gap_position",
                    &params,
                )
                .await
                .map_err(agent_run_database_error)?
                .iter()
                .map(|row| {
                    let reason = SourceGapReason::parse(row.get(/*idx*/ 3)).ok_or_else(|| {
                        agent_run_parse_error(std::io::Error::other("The Source Gap is damaged"))
                    })?;
                    Ok(SourceGapInspect {
                        source_gap_id: row.get(/*idx*/ 0),
                        tool_call_id: row.get(/*idx*/ 1),
                        locator: row.get(/*idx*/ 2),
                        reason: match reason {
                            SourceGapReason::Failed => storyos_contracts::SourceGapReason::Failed,
                            SourceGapReason::Empty => storyos_contracts::SourceGapReason::Empty,
                            SourceGapReason::Missing => storyos_contracts::SourceGapReason::Missing,
                            SourceGapReason::Unattributed => {
                                storyos_contracts::SourceGapReason::Unattributed
                            }
                        },
                        recorded_at: row.get(/*idx*/ 4),
                        result_offset: row.get(/*idx*/ 5),
                        result_bytes: row.get(/*idx*/ 6),
                    })
                })
                .collect::<Result<_, CreateAgentRunError>>()?;
            let unknown_calls = client
                .query(
                    "SELECT tool_call_id::text, tool_spec_revision, arguments::text, state,
                            attempt -> 'destination_attempt' ->> 'settlement',
                            attempt -> 'late_observation' ->> 'settlement', decision_id::text
                       FROM storyos.tool_calls
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid AND state = 'unknown'
                      ORDER BY decision_position, call_position",
                    &params,
                )
                .await
                .map_err(agent_run_database_error)?
                .iter()
                .map(|row| {
                    let settlement = |idx| {
                        row.get::<_, Option<&str>>(idx)
                            .map(|value| {
                                ToolAttemptSettlement::parse(value).ok_or_else(|| {
                                    agent_run_parse_error(std::io::Error::other(
                                        "The ToolCall settlement is damaged",
                                    ))
                                })
                            })
                            .transpose()
                    };
                    // Only a terminal answer after the fence makes the call partial.
                    let class = tool_call_class(
                        ToolCallState::Unknown,
                        settlement(/*idx*/ 4)?,
                        settlement(/*idx*/ 5)?,
                    );
                    Ok(UnknownResearchCallInspect {
                        tool_call_id: row.get(/*idx*/ 0),
                        decision_id: row.get(/*idx*/ 6),
                        call: crate::agent_run_tool_call::call_inspect(
                            row.get(/*idx*/ 1),
                            row.get(/*idx*/ 2),
                            row.get(/*idx*/ 3),
                        )?,
                        late_result: class == Some(ToolCallClass::Partial),
                    })
                })
                .collect::<Result<_, CreateAgentRunError>>()?;
            Ok(Some(ResearchSources {
                snapshots,
                gaps,
                unknown_calls,
            }))
        })
        .await
    }

    async fn get_research_source(
        &self,
        scope: &ProjectScope,
        source_snapshot_id: &str,
    ) -> Result<Option<ResearchSourceText>, CreateAgentRunError> {
        self.in_read_transaction(scope, async |client| {
            client
                .query_opt(
                    &format!(
                        "SELECT {SNAPSHOT_COLUMNS}, snapshot.text
                           FROM storyos.source_snapshots AS snapshot
                          WHERE snapshot.owner_user_id = $1::text::uuid
                            AND snapshot.project_id = $2::text::uuid
                            AND snapshot.source_snapshot_id = $3::text::uuid"
                    ),
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &source_snapshot_id,
                    ],
                )
                .await
                .map_err(agent_run_database_error)?
                .map(|row| {
                    Ok(ResearchSourceText {
                        snapshot: snapshot(&row)?,
                        text: row.get(/*idx*/ 10),
                    })
                })
                .transpose()
        })
        .await
    }
}

impl PostgresProjectReader {
    /// Runs `body` in one read-only transaction under the exact Project Scope.
    async fn in_read_transaction<T>(
        &self,
        scope: &ProjectScope,
        body: impl AsyncFnOnce(&tokio_postgres::Client) -> Result<T, CreateAgentRunError>,
    ) -> Result<T, CreateAgentRunError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(|error| CreateAgentRunError::Unavailable(Box::new(error)))?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(agent_run_database_error)?;
        crate::set_challenge_scope_on_client(&client, scope)
            .await
            .map_err(|error| CreateAgentRunError::Unavailable(Box::new(error)))?;
        let result = body(&client).await;
        match &result {
            Ok(_) => client
                .batch_execute("COMMIT")
                .await
                .map_err(agent_run_database_error)?,
            Err(_) => {
                let _rollback = client.batch_execute("ROLLBACK").await;
            }
        }
        result
    }
}

#[cfg(test)]
#[path = "research_source_tests.rs"]
pub(crate) mod tests;
