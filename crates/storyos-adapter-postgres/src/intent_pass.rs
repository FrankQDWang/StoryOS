//! An intent pass in the store (ADR 0101 section 4): the character entry that needs a pass, the
//! start in the transaction of the claim, the second RunStep after a refused call, the settled
//! `record_character_intent` call in the Tool Gateway transaction, and the failure of a pass whose
//! Run ends with no settled call. A pass is a Run of the kind `intent_pass` with at most two
//! RunSteps. No diagnostic records the file, the intent, or a question (ADR 0047).

use serde_json::json;
use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, ProjectReadError, ProjectScope, StartedIntentPass,
};
use storyos_core::{
    AgentRunKind, CaptureRefusal, CaptureRunEnd, CharacterFile, CurrentPassageAssemblyRecord,
    IntentCallSettlement, IntentPassAssembly, IntentPassEntryFacts, IntentPassOutcome,
    IntentPassRead, NeededCharacter, ToolResultCandidate,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};
use crate::agent_run_work::complete_unavailable;
use crate::memory_capture::{admitted_assistance, insert_background_run};
use crate::read_error;

/// The version of the last change of each entry of the Project: its last version that is not a
/// flag version and not an intent version (ADR 0094 section 7, ADR 0101 section 4).
const CHANGES: &str = "SELECT DISTINCT ON (memory_entry_id) *
       FROM storyos.memory_entry_versions
      WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
        AND flag_reason IS NULL AND intent IS NULL
      ORDER BY memory_entry_id, memory_revision DESC";

/// The facts of the start rule of an intent pass: each Memory Entry of the Project with its last
/// version and its last change, and the versions that each pass read.
pub(crate) struct EntryFacts {
    entries: Vec<IntentPassEntryFacts>,
    passes: Vec<IntentPassRead>,
}

impl EntryFacts {
    /// The character that Core selects for the next pass.
    pub(crate) fn needed(&self) -> Option<NeededCharacter<'_>> {
        storyos_core::select_intent_pass_entry(&self.entries, &self.passes)
    }
}

/// Loads the facts of the start rule of an intent pass of the Project of `scope`.
pub(crate) async fn entry_facts(
    client: &Client,
    scope: &ProjectScope,
) -> Result<EntryFacts, tokio_postgres::Error> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let entries = client
        .query(
            &format!(
                "WITH latest AS (SELECT DISTINCT ON (memory_entry_id) *
                                   FROM storyos.memory_entry_versions
                                  WHERE (owner_user_id, project_id) =
                                        ($1::text::uuid, $2::text::uuid)
                                  ORDER BY memory_entry_id, memory_revision DESC),
                      changed AS ({CHANGES})
                 SELECT changed.memory_entry_id::text, changed.entry_number::text, latest.state,
                        latest.entry_type, changed.origin = 'author_request', changed.title,
                        changed.version::text, changed.text
                   FROM latest JOIN changed USING (memory_entry_id)"
            ),
            &[&keys[0], &keys[1]],
        )
        .await?;
    let entries = (entries.iter())
        .map(|row| IntentPassEntryFacts {
            memory_entry_id: row.get(0),
            entry_number: row.get::<_, String>(1).parse().unwrap_or(u64::MAX),
            state: row.get(2),
            entry_type: row.get(3),
            author: row.get(4),
            title: row.get(5),
            version: row.get(6),
            text: row.get(7),
        })
        .collect::<Vec<_>>();
    let passes = client
        .query(
            "SELECT memory_entry_id::text, entry_version::text, author_entry_id::text,
                    author_version::text
               FROM storyos.intent_passes
              WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)",
            &[&keys[0], &keys[1]],
        )
        .await?;
    let passes = (passes.iter())
        .map(|row| IntentPassRead {
            entry: (row.get(0), row.get(1)),
            author: row.get::<_, Option<String>>(2).zip(row.get(3)),
        })
        .collect::<Vec<_>>();
    Ok(EntryFacts { entries, passes })
}

/// Pins the current Model Use Binding, its grant, and its Model Registration for the pass of
/// `character`. Then it writes the queued Run, the pass row, and the Context Assembly of its first
/// RunStep. The Chapter of the pass is the last live Chapter in Chapter order that has text. A
/// refused start writes no row.
pub(crate) async fn start(
    client: &Client,
    scope: &ProjectScope,
    character: NeededCharacter<'_>,
) -> Result<Result<StartedIntentPass, CaptureRefusal>, ProjectReadError> {
    let admission = admitted_assistance(client, scope, |facts| {
        storyos_core::admit_intent_pass(
            facts.binding,
            facts.assistance_available,
            facts.grant_scope,
        )
    })
    .await?;
    let assistance = match admission {
        Ok(assistance) => assistance,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let chapters = crate::manuscript_search::read_live_chapter_blocks(
        client,
        scope,
        crate::manuscript_search::LiveChapterReadExtent::AllChapters,
    )
    .await?;
    let texts = (chapters.iter())
        .map(|chapter| {
            let blocks = chapter.blocks.iter().map(|block| block.text.as_str());
            (chapter.chapter_id.as_ref(), blocks.collect())
        })
        .collect::<Vec<_>>();
    let chapter_id = storyos_core::intent_pass_chapter(&texts);
    let (run_id, pass_id) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    insert_background_run(
        client,
        scope,
        &run_id,
        &assistance,
        AgentRunKind::IntentPass,
    )
    .await?;
    let (entry, author) = (character.entry, character.author);
    client
        .execute(
            "INSERT INTO storyos.intent_passes
               (owner_user_id, project_id, pass_id, run_id, memory_entry_id, entry_version,
                author_entry_id, author_version, chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::numeric, $7::text::uuid, $8::text::numeric,
                     $9::text::uuid)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &pass_id,
                &run_id,
                &entry.memory_entry_id,
                &entry.version,
                &author.map(|author| &author.memory_entry_id),
                &author.map(|author| &author.version),
                &chapter_id,
            ],
        )
        .await
        .map_err(read_error)?;
    let record = storyos_core::assemble_intent_pass_context(&IntentPassAssembly {
        operation_requirement_id: Uuid::now_v7().to_string(),
        run_id: run_id.clone(),
        owner_user_id: scope.owner_user_id.as_ref().to_owned(),
        project_id: scope.project_id.as_ref().to_owned(),
        destination_identity: assistance.processing_destination_identity.clone(),
        file: CharacterFile {
            name: &entry.title,
            author_text: author.map(|author| author.text.as_str()),
            manuscript_text: &entry.text,
        },
    });
    crate::create_agent_run::context::insert_assembly(
        client, scope, &run_id, /*receipt_id*/ None, "0", &record,
    )
    .await
    .map_err(ProjectReadError::unavailable)?;
    Ok(Ok(StartedIntentPass { pass_id, run_id }))
}

/// Writes the Context Assembly of the second RunStep of the pass of the Run: the file item of the
/// RunStep before it and the result of the refused call.
pub(crate) async fn persist_next_step(
    client: &Client,
    claim: &ClaimedAgentRun,
    retained: &CurrentPassageAssemblyRecord,
    next_position: &str,
    candidates: &[ToolResultCandidate<'_>],
) -> Result<(), CompleteAgentRunError> {
    let record =
        storyos_core::next_intent_pass_context(retained, Uuid::now_v7().to_string(), candidates);
    crate::create_agent_run::context::insert_assembly(
        client,
        &claim.project_scope,
        &claim.run_id,
        /*receipt_id*/ None,
        next_position,
        &record,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    Ok(())
}

/// Executes the `record_character_intent` call `row` of a pass in the Tool Gateway transaction that
/// checked the Run Lease fence. A refused call records nothing, and the Run continues to its next
/// RunStep. A valid call takes the lock of the Memory head and writes one version of the entry of
/// the pass with the result, when Core keeps the result for the versions that the pass read. Else
/// the outcome is `not_kept`, and the Host writes no version. The settled call completes the Run.
pub(crate) async fn execute(
    client: &Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    intent: &str,
    questions: &[String],
) -> Result<(), CompleteAgentRunError> {
    let settlement = storyos_core::settle_intent_call(intent, questions);
    let valid = match &settlement {
        IntentCallSettlement::Refused { refusal, result } => {
            let state = settlement.call_state();
            let resolution = Resolution::host(state, Some(refusal.label()), Some(result));
            return resolve(client, claim, row, resolution).await;
        }
        IntentCallSettlement::Settled { intent, .. } => intent,
    };
    let scope = &claim.project_scope;
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    crate::memory_entries::lock_memory_head(client, scope)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    // The versions that the pass read, and the facts of the entries after the lock of the Memory
    // head.
    let pass = client
        .query_opt(
            "SELECT pass_id::text, memory_entry_id::text, entry_version::text,
                    author_entry_id::text, author_version::text
               FROM storyos.intent_passes
              WHERE (owner_user_id, project_id, run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND outcome IS NULL",
            &[&keys[0], &keys[1], &claim.run_id],
        )
        .await
        .map_err(complete_unavailable)?
        .ok_or_else(|| damaged("The running intent pass of the Run is missing"))?;
    let pass_id: String = pass.get(/*idx*/ 0);
    let read = IntentPassRead {
        entry: (pass.get(/*idx*/ 1), pass.get(/*idx*/ 2)),
        author: pass
            .get::<_, Option<String>>(/*idx*/ 3)
            .zip(pass.get(/*idx*/ 4)),
    };
    let facts = entry_facts(client, scope)
        .await
        .map_err(complete_unavailable)?;
    let (outcome, result) = storyos_core::settle_intent_result(&read, &facts.entries);
    if outcome == IntentPassOutcome::NotKept {
        settle_pass(client, scope, &claim.run_id, outcome.label(), None).await?;
        resolve(client, claim, row, Resolution::settled(&result)).await?;
        return crate::agent_run_work::complete(client, claim)
            .await
            .map(drop);
    }
    let cause = json!({ "kind": "intent_pass", "run_id": claim.run_id, "pass_id": pass_id });
    client
        .execute(
            "WITH head AS (
                     UPDATE storyos.project_memory_heads SET memory_revision = memory_revision + 1
                      WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                  RETURNING memory_revision),
                 pass AS (SELECT * FROM storyos.intent_passes
                           WHERE (owner_user_id, project_id, pass_id) =
                                 ($1::text::uuid, $2::text::uuid, $3::text::uuid)),
                 latest AS (SELECT version.* FROM storyos.memory_entry_versions AS version, pass
                             WHERE (version.owner_user_id, version.project_id,
                                    version.memory_entry_id) =
                                   (pass.owner_user_id, pass.project_id, pass.memory_entry_id)
                             ORDER BY version.memory_revision DESC
                             LIMIT 1)
                 INSERT INTO storyos.memory_entry_versions
                     (owner_user_id, project_id, memory_entry_id, entry_number, version,
                      memory_revision, state, entry_type, origin, title, description, text, cause,
                      source_references, intent, intent_questions, intent_chapter_id,
                      intent_author_entry_id, intent_author_version)
                 SELECT latest.owner_user_id, latest.project_id, latest.memory_entry_id,
                        latest.entry_number, latest.version + 1, head.memory_revision,
                        latest.state, latest.entry_type, latest.origin, latest.title,
                        latest.description, latest.text, $4::text::jsonb,
                        latest.source_references, $5, $6::text::jsonb, pass.chapter_id,
                        pass.author_entry_id, pass.author_version
                   FROM latest, head, pass",
            &[
                &keys[0],
                &keys[1],
                &pass_id,
                &cause.to_string(),
                &valid.intent,
                &json!(valid.questions).to_string(),
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    settle_pass(client, scope, &claim.run_id, outcome.label(), None).await?;
    resolve(client, claim, row, Resolution::settled(&result)).await?;
    crate::agent_run_work::complete(client, claim).await?;
    Ok(())
}

/// Fails the running pass of the Run, when the Run has one, with the reason that Core gives from
/// `end`. It records no result. A Run with no running pass is not changed.
pub(crate) async fn settle_end(
    client: &Client,
    claim: &ClaimedAgentRun,
    end: CaptureRunEnd<'_>,
) -> Result<(), CompleteAgentRunError> {
    let Some(reason) = storyos_core::intent_pass_failure(end) else {
        return Ok(());
    };
    let scope = &claim.project_scope;
    settle_pass(client, scope, &claim.run_id, "failed", Some(reason)).await
}

/// Writes the outcome of the running pass of the Run `run_id`. The settlement makes the intent
/// pass row of the Project due again when an entry needs a pass.
async fn settle_pass(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
    outcome: &str,
    failure_reason: Option<&str>,
) -> Result<(), CompleteAgentRunError> {
    let settled = client
        .execute(
            "UPDATE storyos.intent_passes SET outcome = $4, failure_reason = $5
              WHERE (owner_user_id, project_id, run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND outcome IS NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &outcome,
                &failure_reason,
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    if settled == 0 {
        return Ok(());
    }
    mark_due_when_needed(client, scope)
        .await
        .map_err(complete_unavailable)
}

/// Makes the intent pass row of the Project of `scope` due now when a character entry needs a
/// pass. The settlement of an intent pass and of a consolidation pass with each outcome, and each
/// applied write of the author on a Memory Entry, use it (ADR 0101 section 4).
pub(crate) async fn mark_due_when_needed(
    client: &Client,
    scope: &ProjectScope,
) -> Result<(), tokio_postgres::Error> {
    if entry_facts(client, scope).await?.needed().is_some() {
        crate::memory_work_schedule::mark_intent_pass_due(client, scope, /*due_at*/ None).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "intent_pass_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "intent_pass_start_tests.rs"]
mod start_tests;
