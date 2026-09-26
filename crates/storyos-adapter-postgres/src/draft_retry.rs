use storyos_application::{ApplyAuthorEditCommand, AuthorEditError};
use storyos_contracts::{
    ObservedDraftClosure, RetryDraftClosure, RetryDraftKind, SourceDraftDisposition,
};
use storyos_core::{AuthorEditPrimitive, RefusedEditPayload, canonical_json, hex_sha256};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::author_edit::author_edit_database_error;

pub(super) struct RetrySourceFacts {
    pub disposition: SourceDraftDisposition,
    pub input_matches: bool,
}

pub(super) async fn load_source(
    client: &Client,
    command: &ApplyAuthorEditCommand,
) -> Result<Option<RetrySourceFacts>, AuthorEditError> {
    let Some(retry) = &command.retry_source else {
        return Ok(None);
    };
    let scope = &command.project_scope;
    let row = client
        .query_opt(
            "SELECT draft.current_revision_id::text, revision.payload_digest, draft.closure,
                closed.close_reason, closed.event_id::text,
                CASE WHEN draft.retention_state='retained' THEN revision.payload::text END
           FROM storyos.draft_artifacts AS draft
           JOIN storyos.draft_artifact_revisions AS revision ON
                (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
                (draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id)
           LEFT JOIN storyos.draft_close_events AS closed ON
                (closed.owner_user_id,closed.project_id,closed.event_id)=
                (draft.owner_user_id,draft.project_id,draft.close_event_id)
          WHERE draft.owner_user_id=$1::text::uuid AND draft.project_id=$2::text::uuid
            AND draft.draft_id=$3::text::uuid FOR UPDATE OF draft",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &retry.source_draft_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?
        .ok_or(AuthorEditError::BindingConflict)?;
    let current_revision: String = row.get(0);
    let current_digest: String = row.get(1);
    let closure: String = row.get(2);
    let current_closure = match closure.as_str() {
        "open" => ObservedDraftClosure::Open,
        "closed" => ObservedDraftClosure::Closed {
            close_reason: row.get(3),
            closure_event_ref: row.get(4),
        },
        _ => return Err(AuthorEditError::BindingConflict),
    };
    let mut input_matches = current_revision == retry.source_current_draft_revision_id
        && current_digest == retry.source_draft_payload_digest
        && closure == "open";
    let payload = row
        .get::<_, Option<String>>(5)
        .ok_or(AuthorEditError::BindingConflict)?;
    let payload: RefusedEditPayload = serde_json::from_str(&payload)
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    let range = &retry.selected_payload_range;
    let selected = storyos_core::select_draft_replacement(
        &payload,
        (range.from.block_index, range.from.offset),
        (range.to.block_index, range.to.offset),
    );
    let [unit] = command.author_edit_units.as_slice() else {
        return Err(AuthorEditError::BindingConflict);
    };
    let submitted = match unit.normalized_primitives.as_slice() {
        [AuthorEditPrimitive::ReplaceStructuredSelection { replacement }] => {
            Some(replacement.clone())
        }
        [
            AuthorEditPrimitive::ReplaceSelection { text, .. }
            | AuthorEditPrimitive::ReplaceBlockSelection { text, .. },
        ] => Some(vec![storyos_core::ReplacementBlock {
            block_kind: storyos_core::ManuscriptBlockKind::Paragraph,
            text: text.clone(),
        }]),
        _ => None,
    };
    input_matches &= range.coordinate_profile == "storyos.draft-replacement.block-utf16.v1"
        && payload.chapter_id == command.chapter_id
        && hex_sha256(
            canonical_json(
                &serde_json::to_value(&payload)
                    .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?,
            )
            .as_bytes(),
        ) == current_digest
        && selected.as_ref().is_some_and(|blocks| {
            Some(blocks) == submitted.as_ref()
                && serde_json::to_value(blocks).is_ok_and(|value| {
                    hex_sha256(canonical_json(&value).as_bytes()) == range.slice_digest
                })
        });
    Ok(Some(RetrySourceFacts {
        input_matches,
        disposition: SourceDraftDisposition::Unchanged {
            source_draft_kind: RetryDraftKind::RefusedEdit,
            source_draft_id: retry.source_draft_id.clone(),
            requested_source_draft_revision_id: retry.source_current_draft_revision_id.clone(),
            current_source_draft_revision_id: current_revision,
            current_source_draft_payload_digest: current_digest,
            current_closure,
        },
    }))
}

pub(super) async fn supersede_source(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    disposition: Option<SourceDraftDisposition>,
    result_kind: &str,
    action_sequence: Option<u64>,
) -> Result<Option<SourceDraftDisposition>, AuthorEditError> {
    let Some(disposition) = disposition else {
        return Ok(None);
    };
    if !matches!(
        result_kind,
        "authoritative_applied" | "proposal_revised" | "refused_to_draft"
    ) {
        return Ok(Some(disposition));
    }
    let SourceDraftDisposition::Unchanged {
        source_draft_id,
        current_source_draft_revision_id,
        current_source_draft_payload_digest,
        current_closure: ObservedDraftClosure::Open,
        ..
    } = disposition
    else {
        return Err(AuthorEditError::BindingConflict);
    };
    let event = Uuid::now_v7().to_string();
    let scope = &command.project_scope;
    client.execute("INSERT INTO storyos.draft_close_events
        (owner_user_id,project_id,event_id,draft_id,revision_id,payload_digest,receipt_id,receipt_result_kind,author_action_sequence,close_reason)
        VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6,$7::text::uuid,$8,$9::text::numeric,'superseded')",
        &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref(),&event,&source_draft_id,
          &current_source_draft_revision_id,&current_source_draft_payload_digest,&command.ids.receipt_id,
          &result_kind,&action_sequence.map(|value| value.to_string())]).await.map_err(author_edit_database_error)?;
    let updated = client.execute("UPDATE storyos.draft_artifacts SET closure='closed',close_event_id=$4::text::uuid,reopen_event_id=NULL
        WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND draft_id=$3::text::uuid
        AND closure='open' AND retention_state='retained' AND current_revision_id=$5::text::uuid",
        &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref(),&source_draft_id,&event,&current_source_draft_revision_id])
        .await.map_err(author_edit_database_error)?;
    if updated != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
    Ok(Some(SourceDraftDisposition::ClosedSuperseded {
        source_draft_kind: RetryDraftKind::RefusedEdit,
        source_draft_id,
        source_draft_revision_id: current_source_draft_revision_id,
        source_draft_payload_digest: current_source_draft_payload_digest,
        prior_closure: RetryDraftClosure::Open,
        resulting_closure: "closed".to_owned(),
        close_reason: "superseded".to_owned(),
        closure_event_ref: event,
    }))
}

pub(super) fn replacement_provenance(
    disposition: Option<&SourceDraftDisposition>,
    retry: Option<&storyos_contracts::DraftRetry>,
) -> Option<storyos_contracts::DraftRetryReplacement> {
    match (disposition, retry) {
        (
            Some(SourceDraftDisposition::ClosedSuperseded {
                source_draft_id,
                source_draft_revision_id,
                source_draft_payload_digest,
                closure_event_ref,
                ..
            }),
            Some(retry),
        ) => Some(storyos_contracts::DraftRetryReplacement {
            source_draft_id: source_draft_id.clone(),
            source_draft_revision_id: source_draft_revision_id.clone(),
            source_draft_payload_digest: source_draft_payload_digest.clone(),
            closure_event_ref: closure_event_ref.clone(),
            selected_payload_range: retry.selected_payload_range.clone(),
        }),
        _ => None,
    }
}
