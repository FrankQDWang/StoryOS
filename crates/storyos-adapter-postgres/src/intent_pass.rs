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
    IntentCallSettlement, IntentPassAssembly, IntentPassOutcome, ToolResultCandidate,
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

/// The Character File of one character entry of the background work that needs a pass: the entry
/// and the version of its last change, and the entry of the author with the same title and the
/// version of its last change, when the Project has one.
pub(crate) struct NeededEntry {
    memory_entry_id: String,
    version: String,
    title: String,
    text: String,
    author: Option<(String, String, String)>,
}

/// The current character entry of the background work with the lowest number that has no pass for
/// the version of its last change and the version of the last change of the entry of the author.
/// A character with only the entry of the author needs no pass.
pub(crate) async fn needed_entry(
    client: &Client,
    scope: &ProjectScope,
) -> Result<Option<NeededEntry>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            &format!(
                "WITH latest AS (SELECT DISTINCT ON (memory_entry_id) *
                                   FROM storyos.memory_entry_versions
                                  WHERE (owner_user_id, project_id) =
                                        ($1::text::uuid, $2::text::uuid)
                                  ORDER BY memory_entry_id, memory_revision DESC),
                      changed AS ({CHANGES}),
                      part AS (SELECT changed.*, changed.origin = 'author_request' AS author
                                 FROM latest JOIN changed USING (memory_entry_id)
                                WHERE latest.state = 'current' AND latest.entry_type = 'character')
                 SELECT background.memory_entry_id::text, background.version::text,
                        background.title, background.text, author.memory_entry_id::text,
                        author.version::text, author.text
                   FROM part AS background
                   LEFT JOIN part AS author ON author.author AND author.title = background.title
                  WHERE NOT background.author
                    AND NOT EXISTS (
                          SELECT 1 FROM storyos.intent_passes AS pass
                           WHERE (pass.owner_user_id, pass.project_id, pass.memory_entry_id,
                                  pass.entry_version) =
                                 (background.owner_user_id, background.project_id,
                                  background.memory_entry_id, background.version)
                             AND pass.author_entry_id IS NOT DISTINCT FROM author.memory_entry_id
                             AND pass.author_version IS NOT DISTINCT FROM author.version)
                  ORDER BY background.entry_number
                  LIMIT 1"
            ),
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await?;
    Ok(row.map(|row| NeededEntry {
        memory_entry_id: row.get(0),
        version: row.get(1),
        title: row.get(2),
        text: row.get(3),
        author: match (row.get(4), row.get(5), row.get(6)) {
            (Some(id), Some(version), Some(text)) => Some((id, version, text)),
            _ => None,
        },
    }))
}

/// Pins the current Model Use Binding, its grant, and its Model Registration for the pass of
/// `entry`. Then it writes the queued Run, the pass row, and the Context Assembly of its first
/// RunStep. The Chapter of the pass is the last live Chapter in Chapter order that has text. A
/// refused start writes no row.
pub(crate) async fn start(
    client: &Client,
    scope: &ProjectScope,
    entry: &NeededEntry,
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
    let author = entry.author.as_ref();
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
                &author.map(|(id, _, _)| id),
                &author.map(|(_, version, _)| version),
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
            author_text: author.map(|(_, _, text)| text.as_str()),
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
/// the pass with the result. The settled call completes the Run.
pub(crate) async fn execute(
    client: &Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    intent: &str,
    questions: &[String],
) -> Result<(), CompleteAgentRunError> {
    let settlement = storyos_core::settle_intent_call(intent, questions);
    let (result, valid) = match &settlement {
        IntentCallSettlement::Refused { refusal, result } => {
            let state = settlement.call_state();
            let resolution = Resolution::host(state, Some(refusal.label()), Some(result));
            return resolve(client, claim, row, resolution).await;
        }
        IntentCallSettlement::Settled { intent, result } => (result, intent),
    };
    let scope = &claim.project_scope;
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    crate::memory_entries::lock_memory_head(client, scope)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    let pass_id: String = client
        .query_opt(
            "SELECT pass_id::text FROM storyos.intent_passes
              WHERE (owner_user_id, project_id, run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND outcome IS NULL",
            &[&keys[0], &keys[1], &claim.run_id],
        )
        .await
        .map_err(complete_unavailable)?
        .ok_or_else(|| damaged("The running intent pass of the Run is missing"))?
        .get(/*idx*/ 0);
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
    let recorded = IntentPassOutcome::Recorded.label();
    settle_pass(client, scope, &claim.run_id, recorded, None).await?;
    resolve(client, claim, row, Resolution::settled(result)).await?;
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
/// pass row of the Project due again when another entry needs a pass.
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
    let needed = needed_entry(client, scope)
        .await
        .map_err(complete_unavailable)?;
    if needed.is_some() {
        crate::memory_work_schedule::mark_intent_pass_due(client, scope).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "intent_pass_tests.rs"]
mod tests;
