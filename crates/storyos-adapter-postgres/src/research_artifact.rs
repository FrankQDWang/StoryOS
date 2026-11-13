//! The Research Artifacts of research synthesis Decisions (S5-07, ADR 0087). A valid synthesis
//! writes one immutable row in the transaction of its Decision. The store has no update or delete
//! path, and the runtime role has only SELECT and INSERT on the table.

use std::collections::BTreeMap;

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, CreateAgentRunError, ProjectScope,
    ResearchArtifactReadStore,
};
use storyos_contracts::{
    ResearchArtifactInspect, ResearchArtifactLineage, ResearchCitationInspect, ResearchClaimInspect,
};
use storyos_core::{
    CitableSource, CitableSources, CitedSource, DecisionCandidate, ModelOutput, ResearchSynthesis,
    SourceGapReason,
};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::{RunPhaseRow, complete_database_error};
use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};
use crate::research_source::{GAP_COLUMNS, SNAPSHOT_COLUMNS, gap, snapshot};

/// Each Source Snapshot and Source Gap of the claimed Run with its ToolCall identity: by ToolCall
/// in item order, then the Snapshots in result order, then the Gaps in request order.
pub(crate) async fn run_sources(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Vec<(String, CitableSource)>, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT source.tool_call_id::text, source.id::text, source.reason, source.locator,
                    source.owner_user_id::text, source.project_id::text, source.run_id::text
               FROM (SELECT owner_user_id, project_id, run_id, tool_call_id, 0 AS class,
                            entry_position AS position, source_snapshot_id AS id, locator,
                            NULL::text AS reason
                       FROM storyos.source_snapshots
                     UNION ALL
                     SELECT owner_user_id, project_id, run_id, tool_call_id, 1, gap_position,
                            source_gap_id, locator, reason
                       FROM storyos.source_gaps) AS source
               JOIN storyos.tool_calls AS call
                 USING (owner_user_id, project_id, tool_call_id)
              WHERE source.owner_user_id = $1::text::uuid AND source.project_id = $2::text::uuid
                AND source.run_id = $3::text::uuid
              ORDER BY call.decision_position, call.call_position, source.class, source.position",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    rows.iter()
        .map(|row| {
            let cited = match row.get::<_, Option<&str>>(/*idx*/ 2) {
                None => CitedSource::Snapshot,
                Some(reason) => CitedSource::Gap(
                    SourceGapReason::parse(reason)
                        .ok_or_else(|| damaged("The Source Gap is damaged"))?,
                ),
            };
            Ok((
                row.get(/*idx*/ 0),
                CitableSource {
                    id: row.get(/*idx*/ 1),
                    cited,
                    locator: row.get(/*idx*/ 3),
                    owner_user_id: row.get(/*idx*/ 4),
                    project_id: row.get(/*idx*/ 5),
                    run_id: row.get(/*idx*/ 6),
                },
            ))
        })
        .collect()
}

/// The sources that a synthesis in `output` can cite. Only a synthesis reads them.
pub(crate) async fn decision_citable(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    output: Option<&ModelOutput>,
) -> Result<CitableSources, CompleteAgentRunError> {
    let sources = match output.map(|output| &output.candidate) {
        Some(DecisionCandidate::ResearchSynthesis { .. }) => run_sources(client, claim)
            .await?
            .into_iter()
            .map(|(_, source)| source)
            .collect(),
        Some(
            DecisionCandidate::Advisory { .. }
            | DecisionCandidate::ProseChange { .. }
            | DecisionCandidate::Clarification { .. },
        )
        | None => Vec::new(),
    };
    Ok(CitableSources {
        owner_user_id: claim.project_scope.owner_user_id.as_ref().to_owned(),
        project_id: claim.project_scope.project_id.as_ref().to_owned(),
        run_id: claim.run_id.clone(),
        sources,
    })
}

/// The Core refusal of a synthesis that made no Decision, as evidence of its Model Attempt.
pub(crate) fn refusal_evidence(
    output: Option<&ModelOutput>,
    citable: &CitableSources,
) -> Option<serde_json::Value> {
    let Some(DecisionCandidate::ResearchSynthesis { summary, claims }) =
        output.map(|output| &output.candidate)
    else {
        return None;
    };
    storyos_core::validate_research_synthesis(summary, claims, citable)
        .err()
        .map(|refusal| serde_json::json!(refusal.as_str()))
}

/// Writes the one Research Artifact of the selected synthesis Decision `decision_id`.
pub(crate) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    attempt_id: &str,
    decision_id: &str,
    synthesis: &ResearchSynthesis,
) -> Result<String, CompleteAgentRunError> {
    let requirement = &run
        .disclosure_request
        .as_ref()
        .ok_or_else(|| damaged("The Context Assembly of the synthesis is damaged"))?
        .operation_requirement;
    let tool_call_ids: Vec<&str> = requirement
        .tool_results
        .iter()
        .flatten()
        .map(|source| source.tool_call_id.as_str())
        .collect();
    let research_artifact_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.research_artifacts
               (owner_user_id, project_id, research_artifact_id, run_id, decision_position,
                operation_requirement_id, model_attempt_id, decision_id, tool_call_ids, content,
                content_digest)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::numeric, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text[]::uuid[], $10::text::jsonb, $11)",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &research_artifact_id,
                &claim.run_id,
                &run.decision_position,
                &requirement.operation_requirement_id,
                &attempt_id,
                &decision_id,
                &tool_call_ids,
                &synthesis.content().to_string(),
                &synthesis.content_digest(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(research_artifact_id)
}

/// The lineage of a Proposal that the active RunStep opens when its Model Attempt continues the
/// response of a research synthesis: that response is the prior response of the request, so the
/// model read the Artifact. The lineage names the Model Attempt that produced the Proposal: the
/// successor of an unknown create, or else the decision attempt. A request with a full input reads
/// no earlier response, and its Proposal names no Artifact.
pub(crate) async fn proposal_lineage(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<String>, tokio_postgres::Error> {
    client
        .query_opt(
            "SELECT jsonb_build_object(
                      'kind', 'research_artifact',
                      'run_id', run.run_id::text,
                      'decision_position', run.active_decision_position,
                      'operation_requirement_id', requirement.operation_requirement_id::text,
                      'model_attempt_id', producer.model_attempt_id::text,
                      'research_artifact_id', artifact.research_artifact_id::text)::text
               FROM storyos.agent_runs AS run
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.owner_user_id, requirement.project_id, requirement.run_id,
                     requirement.decision_position) =
                    (run.owner_user_id, run.project_id, run.run_id, run.active_decision_position)
                AND requirement.requirement_role = 'primary'
               JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.run_id,
                     attempt.decision_position) =
                    (run.owner_user_id, run.project_id, run.run_id, run.active_decision_position)
                AND attempt.attempt_role = 'decision'
               -- The successor of an unknown create sends the same request again, so the decision
               -- attempt names the prior response, and the last attempt produced the Proposal.
               CROSS JOIN LATERAL (
                 SELECT producer.model_attempt_id FROM storyos.model_attempts AS producer
                  WHERE (producer.owner_user_id, producer.project_id, producer.run_id,
                         producer.decision_position) =
                        (attempt.owner_user_id, attempt.project_id, attempt.run_id,
                         attempt.decision_position)
                    AND producer.attempt_role IN ('decision', 'successor')
                  ORDER BY producer.model_attempt_id DESC LIMIT 1) AS producer
               JOIN storyos.model_attempts AS prior
                 ON (prior.owner_user_id, prior.project_id) = (run.owner_user_id, run.project_id)
                AND prior.continuation_binding_id::text =
                    attempt.payload #>> '{continuation,prior_continuation_binding_id}'
               -- In the tool loop profile, the closing response of the Run continues the response
               -- that recorded the Artifact (ADR 0091). A Run records at most one Artifact.
               CROSS JOIN LATERAL (
                 SELECT artifact.research_artifact_id FROM storyos.research_artifacts AS artifact
                  WHERE (artifact.owner_user_id, artifact.project_id, artifact.run_id) =
                        (prior.owner_user_id, prior.project_id, prior.run_id)
                    AND artifact.decision_position <= prior.decision_position
                  ORDER BY artifact.decision_position DESC LIMIT 1) AS artifact
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map(|row| row.map(|row| row.get(/*idx*/ 0)))
}

impl ResearchArtifactReadStore for PostgresProjectReader {
    async fn get_research_artifact(
        &self,
        scope: &ProjectScope,
        research_artifact_id: &str,
    ) -> Result<Option<ResearchArtifactInspect>, CreateAgentRunError> {
        self.in_read_transaction(scope, async |client| {
            let params: [&(dyn tokio_postgres::types::ToSql + Sync); 3] = [
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &research_artifact_id,
            ];
            let Some(row) = client
                .query_opt(
                    "SELECT run_id::text, decision_position::text,
                            operation_requirement_id::text, model_attempt_id::text,
                            decision_id::text, tool_call_ids::text[], content::text,
                            content_digest,
                            to_char(created_at AT TIME ZONE 'UTC',
                                    'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                       FROM storyos.research_artifacts
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND research_artifact_id = $3::text::uuid",
                    &params,
                )
                .await
                .map_err(agent_run_database_error)?
            else {
                return Ok(None);
            };
            let content: serde_json::Value =
                serde_json::from_str(row.get(/*idx*/ 6)).map_err(agent_run_parse_error)?;
            let scope_params = |ids| -> [&(dyn tokio_postgres::types::ToSql + Sync); 3] {
                [params[0], params[1], ids]
            };
            let snapshot_ids = cited_ids(&content, "snapshot", "source_snapshot_id");
            let snapshots: BTreeMap<String, _> = client
                .query(
                    &format!(
                        "SELECT {SNAPSHOT_COLUMNS} FROM storyos.source_snapshots AS snapshot
                          WHERE snapshot.owner_user_id = $1::text::uuid
                            AND snapshot.project_id = $2::text::uuid
                            AND snapshot.source_snapshot_id = ANY($3::text[]::uuid[])"
                    ),
                    &scope_params(&snapshot_ids),
                )
                .await
                .map_err(agent_run_database_error)?
                .iter()
                .map(|row| Ok((row.get(/*idx*/ 0), snapshot(row)?)))
                .collect::<Result<_, CreateAgentRunError>>()?;
            let gap_ids = cited_ids(&content, "gap", "source_gap_id");
            let gaps: BTreeMap<String, _> = client
                .query(
                    &format!(
                        "SELECT {GAP_COLUMNS} FROM storyos.source_gaps AS gap
                          WHERE gap.owner_user_id = $1::text::uuid
                            AND gap.project_id = $2::text::uuid
                            AND gap.source_gap_id = ANY($3::text[]::uuid[])"
                    ),
                    &scope_params(&gap_ids),
                )
                .await
                .map_err(agent_run_database_error)?
                .iter()
                .map(|row| Ok((row.get(/*idx*/ 0), gap(row)?)))
                .collect::<Result<_, CreateAgentRunError>>()?;
            let mut citations = Vec::new();
            let mut numbers = BTreeMap::new();
            let mut claims = Vec::new();
            for claim in content["claims"].as_array().into_iter().flatten() {
                let mut cited_numbers = Vec::new();
                for citation in claim["citations"].as_array().into_iter().flatten() {
                    let resolved = match citation["kind"].as_str() {
                        Some("snapshot") => citation["source_snapshot_id"]
                            .as_str()
                            .and_then(|id| snapshots.get(id))
                            .cloned()
                            .map(|snapshot| ResearchCitationInspect::Snapshot { snapshot }),
                        Some("gap") => citation["source_gap_id"]
                            .as_str()
                            .and_then(|id| gaps.get(id))
                            .cloned()
                            .map(|gap| ResearchCitationInspect::Gap { gap }),
                        _ => None,
                    }
                    .ok_or_else(|| {
                        agent_run_parse_error(std::io::Error::other(
                            "The citation of a Research Artifact is damaged",
                        ))
                    })?;
                    let next = u32::try_from(citations.len() + 1).map_err(agent_run_parse_error)?;
                    let number = *numbers.entry(citation.to_string()).or_insert_with(|| {
                        citations.push(resolved);
                        next
                    });
                    cited_numbers.push(number);
                }
                claims.push(ResearchClaimInspect {
                    text: claim["text"].as_str().unwrap_or_default().to_owned(),
                    citations: cited_numbers,
                });
            }
            Ok(Some(ResearchArtifactInspect {
                research_artifact_id: research_artifact_id.to_owned(),
                created_at: row.get(/*idx*/ 8),
                content_digest: row.get(/*idx*/ 7),
                lineage: ResearchArtifactLineage {
                    run_id: row.get(/*idx*/ 0),
                    decision_position: row.get(/*idx*/ 1),
                    operation_requirement_id: row.get(/*idx*/ 2),
                    model_attempt_id: row.get(/*idx*/ 3),
                    decision_id: row.get(/*idx*/ 4),
                    tool_call_ids: row.get(/*idx*/ 5),
                },
                summary: content["summary"].as_str().unwrap_or_default().to_owned(),
                claims,
                citations,
            }))
        })
        .await
    }
}

/// The identities that the claims of `content` cite as `kind`, in citation order.
fn cited_ids<'a>(content: &'a serde_json::Value, kind: &str, field: &str) -> Vec<&'a str> {
    content["claims"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|claim| claim["citations"].as_array().into_iter().flatten())
        .filter(|citation| citation["kind"] == kind)
        .filter_map(|citation| citation[field].as_str())
        .collect()
}

#[cfg(test)]
#[path = "research_artifact_tests.rs"]
pub(crate) mod tests;
