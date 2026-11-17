//! A consolidation pass in the store (ADR 0094 sections 6 and 8): the start in one transaction,
//! the settled `apply_memory_operations` call in the Tool Gateway transaction, and the failure of a
//! pass whose Run ends with no settled call. A pass is a Run of the kind `memory_consolidation`
//! with one RunStep. No diagnostic records an entry, an observation, or an operation (ADR 0047).

use serde_json::{Value, json};
use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, MemoryConsolidationStore, ProjectReadError,
    ProjectScope, StartedMemoryConsolidation,
};
use storyos_core::{
    AgentRunKind, AssistanceAvailability, CaptureRefusal, CaptureRunEnd, CurrentMemoryEntry,
    MemoryConsolidationAssembly, MemoryEntryOrigin, MemoryEntryType, MemoryOperationCandidate,
    MemorySourceReference, PassMemoryEntry, PassObservation, ToolCallState,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};
use crate::agent_run_work::complete_database_error;
use crate::memory_entries::{TIME_FORM, VersionSource, lock_memory_head, write_memory_version};
use crate::update_project_assistance::read_assistance_record;
use crate::{PostgresProjectReader, read_error};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

impl MemoryConsolidationStore for PostgresProjectReader {
    async fn start_memory_consolidation(
        &self,
        scope: &ProjectScope,
    ) -> Result<Result<StartedMemoryConsolidation, CaptureRefusal>, ProjectReadError> {
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(ProjectReadError::unavailable)?;
        let started = start(&transaction.client, scope).await;
        match &started {
            Ok(Ok(_)) => transaction
                .commit()
                .await
                .map_err(ProjectReadError::unavailable)?,
            Ok(Err(_)) | Err(_) => {
                let _rollback = transaction.rollback().await;
            }
        }
        started
    }
}

/// Pins the current Model Use Binding, its grant, its Model Registration, and the Memory revision.
/// Then it writes the queued Run, the pass row, and the Context Assembly of its one RunStep. The
/// input has the oldest observations that fit the request. A refused start writes no row. The
/// claim of a schedule row calls it in the transaction of the claim.
pub(crate) async fn start(
    client: &Client,
    scope: &ProjectScope,
) -> Result<Result<StartedMemoryConsolidation, CaptureRefusal>, ProjectReadError> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let assistance = read_assistance_record(client, scope).await?;
    let grant_scope: Option<String> = match &assistance {
        Some(record) => client
            .query_opt(
                "SELECT grant_scope FROM storyos.project_destination_grants
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND grant_id = $3::text::uuid",
                &[&keys[0], &keys[1], &record.grant_id],
            )
            .await
            .map_err(read_error)?
            .and_then(|row| row.get(/*idx*/ 0)),
        None => None,
    };
    let available = assistance
        .as_ref()
        .is_some_and(|record| record.availability == AssistanceAvailability::Available);
    let admission = storyos_core::admit_memory_consolidation(
        assistance.is_some(),
        available,
        grant_scope.as_deref(),
    );
    let assistance = match (admission, assistance) {
        (Ok(()), Some(assistance)) => assistance,
        (Err(refusal), _) => return Ok(Err(refusal)),
        (Ok(()), None) => return Ok(Err(CaptureRefusal::NoModelUseBinding)),
    };
    let unavailable = |error: BoxError| ProjectReadError::unavailable(std::io::Error::other(error));
    let revision = crate::memory_entries::memory_head(client, scope)
        .await
        .map_err(unavailable)?;
    let entries: Vec<(PassMemoryEntry, String)> = pass_entries(client, scope)
        .await
        .map_err(unavailable)?
        .into_iter()
        .map(|entry| (entry.entry, entry.changed_at))
        .collect();
    let (keys_of, observations): (Vec<[String; 2]>, Vec<PassObservation>) =
        waiting_observations(client, scope)
            .await
            .map_err(unavailable)?
            .into_iter()
            .unzip();
    let budget_bounds: String = client
        .query_one(
            "SELECT budget_bounds FROM storyos.project_external_use_binding_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND project_model_use_binding_revision = $3::text::uuid",
            &[
                &keys[0],
                &keys[1],
                &assistance.project_model_use_binding_revision,
            ],
        )
        .await
        .map_err(read_error)?
        .get(/*idx*/ 0);
    let unused = unused_entries(client, scope, &entries)
        .await
        .map_err(unavailable)?;
    let marks = crate::memory_entries::read_delete_marks(client, scope)
        .await
        .map_err(unavailable)?;
    let (run_id, pass_id) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    let mut assembly = MemoryConsolidationAssembly {
        operation_requirement_id: Uuid::now_v7().to_string(),
        run_id: run_id.clone(),
        owner_user_id: keys[0].to_owned(),
        project_id: keys[1].to_owned(),
        destination_identity: assistance.processing_destination_identity.clone(),
        memory_revision: revision,
        entries: &entries,
        unused: &unused,
        observations: &observations,
        marks: &marks,
    };
    let taken = storyos_core::pass_observation_count(
        &assembly,
        &assistance.model_registration_revision,
        &budget_bounds,
    );
    // Background work writes no grant and no binding: the Run pins the current ones. The first
    // Run of the author made the Agent of the Project.
    client
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, grant_id,
                project_model_use_binding_revision, status, model_registration_revision, run_kind)
             SELECT agent.owner_user_id, agent.project_id, $3::text::uuid, agent.project_agent_id,
                    $4::text::uuid, $5::text::uuid, 'queued', $6::text::uuid, $7
               FROM storyos.project_agents AS agent
              WHERE agent.owner_user_id = $1::text::uuid AND agent.project_id = $2::text::uuid",
            &[
                &keys[0],
                &keys[1],
                &run_id,
                &assistance.grant_id,
                &assistance.project_model_use_binding_revision,
                &assistance.model_registration_revision,
                &AgentRunKind::MemoryConsolidation.name(),
            ],
        )
        .await
        .map_err(read_error)?;
    let input: Vec<Value> = keys_of[..taken].iter().map(|key| json!(key)).collect();
    client
        .execute(
            "INSERT INTO storyos.memory_consolidation_passes
               (owner_user_id, project_id, pass_id, run_id, memory_revision, observations)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::numeric, $6::text::jsonb)",
            &[
                &keys[0],
                &keys[1],
                &pass_id,
                &run_id,
                &revision.to_string(),
                &Value::from(input).to_string(),
            ],
        )
        .await
        .map_err(read_error)?;
    assembly.observations = &observations[..taken];
    let record = storyos_core::assemble_memory_consolidation_context(&assembly);
    crate::create_agent_run::context::insert_assembly(
        client, scope, &run_id, /*receipt_id*/ None, "0", &record,
    )
    .await
    .map_err(ProjectReadError::unavailable)?;
    Ok(Ok(StartedMemoryConsolidation { pass_id, run_id }))
}

/// The numbers of the entries of `entries` that Core gives the mark `unused`, from the time of the
/// last read of each entry by a Run of the author, the time of its last change, and the time now.
async fn unused_entries(
    client: &Client,
    scope: &ProjectScope,
    entries: &[(PassMemoryEntry, String)],
) -> Result<Vec<u64>, BoxError> {
    let reads = crate::memory_entries::last_memory_reads(client, scope).await?;
    let now: String = client
        .query_one(
            "SELECT to_char(statement_timestamp() AT TIME ZONE 'UTC', $1)",
            &[&TIME_FORM],
        )
        .await?
        .get(/*idx*/ 0);
    let unused = entries.iter().filter(|(pass_entry, changed_at)| {
        let number = pass_entry.entry.number;
        let read = reads
            .iter()
            .find(|(_, read_number, _)| *read_number == number);
        let last_read = read.and_then(|(_, _, time)| time.as_deref());
        storyos_core::memory_entry_unused(pass_entry.origin, last_read, changed_at, &now)
    });
    Ok(unused
        .map(|(pass_entry, _)| pass_entry.entry.number)
        .collect())
}

/// One current entry of the Project now, with the time of its last change and its source
/// references.
struct StoredEntry {
    entry: PassMemoryEntry,
    changed_at: String,
    sources: Vec<Value>,
}

/// The last version of each current entry of the Project now, in the order of the numbers.
async fn pass_entries(client: &Client, scope: &ProjectScope) -> Result<Vec<StoredEntry>, BoxError> {
    let rows = client
        .query(
            &format!(
                "SELECT memory_entry_id::text, entry_number::text, version::text, entry_type,
                        title, description, text, origin, to_char(recorded_at AT TIME ZONE 'UTC',
                        '{TIME_FORM}'), source_references::text
                   FROM (SELECT DISTINCT ON (memory_entry_id) *
                           FROM storyos.memory_entry_versions
                          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                          ORDER BY memory_entry_id, memory_revision DESC) AS latest
                  WHERE state = 'current'
                  ORDER BY entry_number"
            ),
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    let invalid = || Box::new(std::io::Error::other("A Memory Entry row is invalid"));
    rows.iter()
        .map(|row| {
            let number = |index| row.get::<_, &str>(index).parse().map_err(|_| invalid());
            Ok(StoredEntry {
                entry: PassMemoryEntry {
                    entry: CurrentMemoryEntry {
                        memory_entry_id: row.get(0),
                        number: number(1)?,
                        version: number(2)?,
                        entry_type: MemoryEntryType::parse(row.get(3)).ok_or_else(invalid)?,
                        title: row.get(4),
                        description: row.get(5),
                        text: row.get(6),
                    },
                    origin: MemoryEntryOrigin::parse(row.get(7)).ok_or_else(invalid)?,
                },
                changed_at: row.get(8),
                sources: serde_json::from_str(row.get(9))?,
            })
        })
        .collect()
}

/// Each observation that waits in the Project, the oldest capture first and in list order, with
/// its key `[capture, position]`.
async fn waiting_observations(
    client: &Client,
    scope: &ProjectScope,
) -> Result<Vec<([String; 2], PassObservation)>, BoxError> {
    let rows = client
        .query(
            "SELECT item.capture_id::text, item.position::text, item.entry_type, item.title,
                    item.text, capture.conversation_id::text, capture.chapter_id::text
               FROM storyos.memory_observations AS item
               JOIN storyos.memory_captures AS capture
                 USING (owner_user_id, project_id, capture_id)
              WHERE item.owner_user_id = $1::text::uuid AND item.project_id = $2::text::uuid
                AND item.state = 'waiting'
              ORDER BY capture.started_at, capture.capture_id, item.position",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    rows.iter().map(observation).collect()
}

fn observation(row: &tokio_postgres::Row) -> Result<([String; 2], PassObservation), BoxError> {
    let source = match (row.get(5), row.get(6)) {
        (Some(conversation_id), _) => MemorySourceReference::Conversation {
            conversation_id,
            run_id: None,
            tool_call_id: None,
        },
        (None, Some(chapter_id)) => MemorySourceReference::Chapter { chapter_id },
        (None, None) => return Err("A capture has no source".into()),
    };
    Ok((
        [row.get(0), row.get(1)],
        PassObservation {
            entry_type: MemoryEntryType::parse(row.get(2))
                .ok_or("A Memory Observation type is invalid")?,
            title: row.get(3),
            text: row.get(4),
            source,
        },
    ))
}

/// The running pass of the Run: its identity, its pinned Memory revision, and the observations of
/// its input in input order.
async fn running_pass(
    client: &Client,
    claim: &ClaimedAgentRun,
) -> Result<(String, u64, Vec<([String; 2], PassObservation)>), BoxError> {
    let scope = &claim.project_scope;
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let row = client
        .query_opt(
            "SELECT pass_id::text, memory_revision::text FROM storyos.memory_consolidation_passes
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND outcome IS NULL",
            &[&keys[0], &keys[1], &claim.run_id],
        )
        .await?
        .ok_or("The running pass of the Run is missing")?;
    let rows = client
        .query(
            "SELECT item.capture_id::text, item.position::text, item.entry_type, item.title,
                    item.text, capture.conversation_id::text, capture.chapter_id::text
               FROM storyos.memory_consolidation_passes AS pass,
                    jsonb_array_elements(pass.observations) WITH ORDINALITY AS input(key, number)
               JOIN storyos.memory_observations AS item
                 ON (item.capture_id, item.position) =
                    ((input.key ->> 0)::uuid, (input.key ->> 1)::numeric)
               JOIN storyos.memory_captures AS capture
                 USING (owner_user_id, project_id, capture_id)
              WHERE (pass.owner_user_id, pass.project_id, pass.pass_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND (item.owner_user_id, item.project_id) = (pass.owner_user_id, pass.project_id)
              ORDER BY input.number",
            &[&keys[0], &keys[1], &row.get::<_, &str>(0)],
        )
        .await?;
    let observations = rows.iter().map(observation).collect::<Result<_, _>>()?;
    Ok((row.get(0), row.get::<_, &str>(1).parse()?, observations))
}

/// Executes the `apply_memory_operations` call `row` of a pass in the Tool Gateway transaction
/// that checked the Run Lease fence. The transaction takes the lock of the Memory head before it
/// reads the entries, so a write of the author and the pass do not change the entries at the same
/// time. Core decides the outcome and each operation, the store writes each admitted version, and
/// the settled call completes the Run.
pub(crate) async fn execute(
    client: &Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    candidates: &[MemoryOperationCandidate],
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let unavailable = CompleteAgentRunError::Unavailable;
    let revision = lock_memory_head(client, scope).await.map_err(unavailable)?;
    let (pass_id, pinned, observations) = running_pass(client, claim)
        .await
        .map_err(|_| damaged("The running pass of the Run is missing"))?;
    let (_, next_entry_number, _) = crate::memory_entries::current_memory(client, scope)
        .await
        .map_err(unavailable)?;
    let stored = pass_entries(client, scope).await.map_err(unavailable)?;
    let marks = crate::memory_entries::read_delete_marks(client, scope)
        .await
        .map_err(unavailable)?;
    let entries: Vec<PassMemoryEntry> = stored.iter().map(|item| item.entry.clone()).collect();
    let input: Vec<PassObservation> = observations.iter().map(|(_, o)| o.clone()).collect();
    let settlement = storyos_core::settle_operations_call(
        candidates,
        &storyos_core::PassFacts {
            pinned_revision: pinned,
            memory_revision: revision,
            entries: &entries,
            next_entry_number,
            observations: &input,
            marks: &marks,
        },
    );
    let stored_of = |number: u64| stored.iter().find(|item| item.entry.entry.number == number);
    let cause = json!({ "kind": "consolidation", "run_id": claim.run_id, "pass_id": pass_id });
    let mut written = revision;
    let mut results = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let decision = settlement.decisions.get(index);
        let (result, refusal, entry) = match decision {
            None => ("not_applied", None, None),
            Some(Err(refusal)) => ("refused", Some(refusal.label()), None),
            Some(Ok(admitted)) => {
                // The result holds the sources of its entry, of each merged entry, and of the
                // observations. A merged entry keeps its own sources in its deleted version.
                let numbers = [&admitted.version].into_iter().chain(&admitted.merged);
                let all = numbers
                    .filter_map(|version| stored_of(version.number))
                    .flat_map(|item| item.sources.iter().cloned())
                    .chain(admitted.sources.iter().map(|source| json!(source)));
                let mut unique: Vec<Value> = Vec::new();
                for source in all {
                    if !unique.contains(&source) {
                        unique.push(source);
                    }
                }
                let source = VersionSource {
                    origin: admitted.origin.as_str(),
                    cause: cause.clone(),
                    source_references: Value::from(unique),
                };
                let new_entry_id = Uuid::now_v7().to_string();
                let version = (written, &admitted.version, new_entry_id.as_str());
                write_memory_version(client, scope, version, &source)
                    .await
                    .map_err(unavailable)?;
                written += 1;
                for merged in &admitted.merged {
                    let item = stored_of(merged.number)
                        .ok_or_else(|| damaged("A merged entry is missing"))?;
                    let source = VersionSource {
                        origin: item.entry.origin.as_str(),
                        cause: cause.clone(),
                        source_references: Value::from(item.sources.clone()),
                    };
                    write_memory_version(client, scope, (written, merged, ""), &source)
                        .await
                        .map_err(unavailable)?;
                    written += 1;
                }
                ("applied", None, Some(admitted.version.number))
            }
        };
        results.push(json!({
            "operation": candidate.arguments(),
            "result": result,
            "refusal": refusal,
            "entry": entry,
        }));
    }
    let consumed = match settlement.outcome {
        storyos_core::ConsolidationOutcome::Applied
        | storyos_core::ConsolidationOutcome::NoOperation => observations.as_slice(),
        storyos_core::ConsolidationOutcome::Stale | storyos_core::ConsolidationOutcome::Failed => {
            &[]
        }
    };
    let keys: Vec<Value> = consumed.iter().map(|(key, _)| json!(key)).collect();
    client
        .execute(
            "WITH consumed AS (
                 UPDATE storyos.memory_observations AS item SET state = 'consumed'
                   FROM jsonb_array_elements($4::text::jsonb) AS input(key)
                  WHERE (item.owner_user_id, item.project_id) = ($1::text::uuid, $2::text::uuid)
                    AND (item.capture_id, item.position) =
                        ((input.key ->> 0)::uuid, (input.key ->> 1)::numeric)
                    AND item.state = 'waiting')
             UPDATE storyos.memory_consolidation_passes SET outcome = $5, operations = $6::text::jsonb
              WHERE (owner_user_id, project_id, run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
                &Value::from(keys).to_string(),
                &settlement.outcome.label(),
                &Value::from(results).to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    crate::memory_work_schedule::mark_consolidation_due(client, scope).await?;
    resolve(
        client,
        claim,
        row,
        Resolution {
            state: ToolCallState::Settled,
            refusal: None,
            attempt: None,
            outcome: None,
            result: Some(&settlement.result),
        },
    )
    .await?;
    crate::agent_run_work::complete(client, claim).await?;
    Ok(())
}

/// Fails the running pass of the Run, when the Run has one, with the reason that Core gives from
/// `end`. It applies nothing. A Run with no running pass is not changed. The settlement of a pass
/// with each outcome can make the consolidation row of the Project due.
pub(crate) async fn settle_end(
    client: &Client,
    claim: &ClaimedAgentRun,
    end: CaptureRunEnd<'_>,
) -> Result<(), CompleteAgentRunError> {
    let Some(reason) = storyos_core::consolidation_failure(end) else {
        return Ok(());
    };
    let scope = &claim.project_scope;
    let failed = client
        .execute(
            "UPDATE storyos.memory_consolidation_passes SET outcome = 'failed', failure_reason = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND outcome IS NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
                &reason,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    if failed > 0 {
        crate::memory_work_schedule::mark_consolidation_due(client, scope).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "memory_consolidation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "memory_consolidation_recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
#[path = "memory_consolidation_merge_tests.rs"]
mod merge_tests;

#[cfg(test)]
#[path = "memory_consolidation_schedule_tests.rs"]
mod schedule_tests;
