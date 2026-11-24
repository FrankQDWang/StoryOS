//! The Delete Memory Entry command of the settings page (ADR 0094 sections 6 and 9). An applied
//! delete writes the next version of the entry with the state `deleted` and moves the Memory
//! revision of the Project. Each earlier version stays. No diagnostic records a title, a
//! description, or a text of an entry (ADR 0047).

use crate::statement_cache::Client;
use serde_json::json;
use storyos_application::{
    DeleteMemoryEntryError, DeleteMemoryEntryInput, DeleteMemoryEntrySettlement,
    MemoryEntryDeleted, ProjectCommandEnvelope, RefusableCommandError,
};
use storyos_contracts::CommandKind;
use storyos_core::{
    DeleteMemoryEntry as CoreDeleteMemoryEntry, MemoryVersionWrite, TransitionOutcome,
    delete_memory_entry,
};

use crate::PostgresProjectReader;
use crate::agent_run_control::CONTROL_SETTLEMENTS;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, AppliedVariant, Classification,
    CommandResponse, CommandSpec, CommandWrite, LockedProject, ProjectCommand, ReplayEffect,
    settle_project_command_after_contention, unavailable,
};
use crate::memory_entries::{
    VersionSource, last_entry_version, lock_memory_head, write_memory_version,
};

impl PostgresProjectReader {
    /// Settles one delete of a Memory Entry by the author. A delete that waited on the lock of the
    /// Memory head while a consolidation pass settled loses a serialization race. It then settles
    /// again in a new transaction, so the author does not wait for the pass (ADR 0094 section 6).
    pub async fn delete_memory_entry(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &DeleteMemoryEntryInput,
    ) -> Result<DeleteMemoryEntrySettlement, DeleteMemoryEntryError> {
        settle_project_command_after_contention(self, envelope, input, CONTROL_SETTLEMENTS).await
    }
}

impl ProjectCommand for DeleteMemoryEntryInput {
    const SPEC: CommandSpec<DeleteMemoryEntryError> = CommandSpec {
        kind: CommandKind::DeleteMemoryEntry,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "memory_entry_deleted",
        replay_effect: ReplayEffect::NoQuery,
        response: CommandResponse::Project,
        contention_refusal: None,
        receipt_payload: "{}",
    };
    type Error = DeleteMemoryEntryError;
    type Profile = ActivityOnly;
    type Applied = MemoryEntryDeleted;
    /// The Memory revision of the Project before the delete, and the version to write.
    type Plan = (u64, MemoryVersionWrite);
    type Settlement = DeleteMemoryEntrySettlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, DeleteMemoryEntryError> {
        let scope = &envelope.project_scope;
        let revision = lock_memory_head(client, scope).await.map_err(unavailable)?;
        let last = last_entry_version(client, scope, &self.memory_entry_id)
            .await
            .map_err(unavailable)?;
        let outcome = match delete_memory_entry(&CoreDeleteMemoryEntry {
            last,
            expected_version: self.expected_version,
        }) {
            TransitionOutcome::Applied(version) => {
                let deleted = MemoryEntryDeleted {
                    memory_entry_id: self.memory_entry_id.clone(),
                    version: version.version,
                    memory_revision: revision + 1,
                };
                TransitionOutcome::Applied((deleted, (revision, version)))
            }
            TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
            TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
            TransitionOutcome::Refused(refusal) => {
                return Err(RefusableCommandError::RefusedBeforeAdmission(refusal));
            }
        };
        Ok(Classification::project_command(outcome))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        (revision, version): (u64, MemoryVersionWrite),
        applied: MemoryEntryDeleted,
    ) -> Result<CommandWrite<'_, Self>, storyos_application::ProjectCommandError> {
        // The cause names the Domain Receipt of the command. The mark of the delete is the title,
        // the description, and the cause of this version (ADR 0094 section 6).
        let source = VersionSource {
            origin: "author_request",
            cause: json!({ "kind": "author_delete", "receipt_id": envelope.ids.receipt_id }),
            source_references: json!([]),
        };
        let written = (revision, &version, self.memory_entry_id.as_str());
        write_memory_version(client, &envelope.project_scope, written, &source)
            .await
            .map_err(unavailable)?;
        // A delete of a character entry can need an intent pass (ADR 0101 section 4).
        if storyos_core::write_can_change_character(&version, &[]) {
            crate::intent_pass::mark_due_when_needed(client, &envelope.project_scope)
                .await
                .map_err(unavailable)?;
        }
        Ok(CommandWrite {
            profile: ActivityWrite {
                activity: json!({
                    "memory_entry_id": applied.memory_entry_id,
                    "version": applied.version.to_string(),
                    "memory_revision": applied.memory_revision.to_string(),
                }),
                effect: applied,
            },
            after_authority: None,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<MemoryEntryDeleted, ReplayFault> {
        Ok(MemoryEntryDeleted {
            memory_entry_id: replay.activity_uuid("memory_entry_id")?,
            version: replay.activity_u64("version")?,
            memory_revision: replay.activity_u64("memory_revision")?,
        })
    }
}

#[cfg(test)]
#[path = "delete_memory_entry_tests.rs"]
pub(crate) mod tests;
