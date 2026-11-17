//! The command `deleteMemoryEntry` of the settings page (ADR 0094 sections 6 and 9).

use storyos_application::{DeleteMemoryEntryInput, RefusableCommandError};
use storyos_core::{DeleteMemoryEntryRefusal, TransitionOutcome};

use super::command_admission::{
    BodyValidation, ProblemMapping, ProjectCommandRoute, RevisionMismatch, SchemaMismatch,
    SettledReceipt, TargetValidation, admit, controlled_project, positive,
};
use super::contract_reason::contract_reason;
use super::*;

const DELETE_MEMORY_ENTRY: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Delete Memory Entry",
    command_kind: contracts::CommandKind::DeleteMemoryEntry,
    method: contracts::DELETE_MEMORY_ENTRY_METHOD,
    path: contracts::DELETE_MEMORY_ENTRY_PATH,
    schema_id: contracts::DELETE_MEMORY_ENTRY_REQUEST_SCHEMA_ID,
    digest_profile: contracts::DELETE_MEMORY_ENTRY_DIGEST_PROFILE,
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

pub(super) async fn delete_memory_entry(
    State(state): State<Arc<ServerState>>,
    Path((project_id, memory_entry_id)): Path<(String, String)>,
    request: Request,
) -> Result<Json<contracts::DeleteMemoryEntryResponse>, ApiError> {
    let admitted = admit(
        &state,
        &project_id,
        &[&memory_entry_id],
        request,
        &DELETE_MEMORY_ENTRY,
        |body: &contracts::DeleteMemoryEntryRequest| {
            Ok(DeleteMemoryEntryInput {
                memory_entry_id: memory_entry_id.clone(),
                expected_version: positive(&body.delete_memory_entry_input.expected_version)?,
            })
        },
    )
    .await?;
    let settlement = admitted
        .store
        .delete_memory_entry(&admitted.envelope, &admitted.input)
        .await
        .map_err(|error| match error {
            // An entry of another Project gives the same answer as an entry that does not exist.
            RefusableCommandError::RefusedBeforeAdmission(
                DeleteMemoryEntryRefusal::EntryNotFound,
            ) => resource_unavailable(),
            RefusableCommandError::Command(error) => DELETE_MEMORY_ENTRY.problem(error),
        })?;
    admitted.hold_first_acknowledgement().await;
    let result = settlement.outcome.receipt_result();
    let effect = match settlement.outcome {
        TransitionOutcome::Applied(applied) => {
            contracts::DeleteMemoryEntryEffect::AuthoritativeApplied {
                memory_entry_id: applied.effect.memory_entry_id,
                version: applied.effect.version.to_string(),
                memory_revision: applied.effect.memory_revision.to_string(),
                project_activity_position: applied.project_activity_position.to_string(),
            }
        }
        TransitionOutcome::NoEffect(reason) => contracts::DeleteMemoryEntryEffect::NoEffect {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Conflicted(reason) => contracts::DeleteMemoryEntryEffect::Conflicted {
            reason: contract_reason(&reason)?,
        },
        TransitionOutcome::Refused(reason) => match reason {},
    };
    let ack = admitted.acknowledgement(
        &DELETE_MEMORY_ENTRY,
        contracts::DomainReceiptCommandKind::DeleteMemoryEntry,
        SettledReceipt {
            ids: settlement.ids,
            receipt_created_at: settlement.receipt_created_at,
            result,
            authority: None,
            heads: Vec::new(),
        },
    );
    Ok(Json(contracts::DeleteMemoryEntryResponse {
        schema_id: contracts::DELETE_MEMORY_ENTRY_RESPONSE_SCHEMA_ID.to_owned(),
        correlation_id: ack.correlation_id,
        project_scope: ack.project_scope,
        command_id: ack.command_id,
        author_command_admission_id: ack.author_command_admission_id,
        receipt: ack.receipt,
        project: controlled_project(settlement.response),
        effect,
    }))
}
