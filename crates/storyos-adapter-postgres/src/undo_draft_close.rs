use storyos_application::{
    ProjectScope, UndoLatestAuthorActionCommand, UndoLatestAuthorActionError,
    UndoLatestAuthorActionSettlement, UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::{AuthorUndoFrontierKind, canonical_json, hex_sha256};
use uuid::Uuid;

use crate::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
};

pub(super) struct ObservedDraftClose {
    pub sequence: u64,
    pub draft_id: String,
    pub revision_id: String,
    pub digest: String,
    pub close_event_id: String,
    pub kind: AuthorUndoFrontierKind,
    pub current_head_revision_id: String,
    pub derived_proposal: Option<(String, String)>,
}

pub(super) async fn load_frontier(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    sequence: u64,
) -> Result<Option<ObservedDraftClose>, UndoLatestAuthorActionError> {
    let scope = &command.project_scope;
    let row = client.query_opt("SELECT closed.draft_id::text, closed.revision_id::text, closed.payload_digest,
        closed.event_id::text, draft.current_revision_id::text, revision.payload_digest, draft.closure,
        draft.retention_state, draft.close_event_id::text,
        CASE WHEN draft.retention_state='retained' THEN revision.payload::text END,
        head.current_revision_id::text, receipt.command_kind
        FROM storyos.author_action_entries AS action JOIN storyos.domain_receipts AS receipt USING(owner_user_id,project_id,receipt_id)
        JOIN storyos.draft_close_events AS closed USING(owner_user_id,project_id,receipt_id,author_action_sequence)
        JOIN storyos.draft_artifacts AS draft USING(owner_user_id,project_id,draft_id)
        JOIN storyos.draft_artifact_revisions AS revision ON
        (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
        (draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id)
        JOIN storyos.editor_session_base_snapshots AS snapshot ON
        (snapshot.owner_user_id,snapshot.project_id,snapshot.editor_session_id)=(action.owner_user_id,action.project_id,$4::text::uuid)
        JOIN storyos.authoritative_heads AS head ON (head.owner_user_id,head.project_id,head.manuscript_object_id)=
        (snapshot.owner_user_id,snapshot.project_id,snapshot.chapter_object_id)
        WHERE action.owner_user_id=$1::text::uuid AND action.project_id=$2::text::uuid
        AND action.author_action_sequence=$3::text::numeric AND action.disposition='forward'
        AND ((receipt.command_kind='closeEditorFlowDraft' AND receipt.result_kind='draft_closure_changed')
          OR (receipt.command_kind='expandRefusedEditDraftToProposal' AND receipt.result_kind='proposal_created_from_draft' AND closed.close_reason='superseded')
          OR (receipt.command_kind='applyAuthorEdit' AND closed.close_reason='superseded' AND receipt.result_kind IN ('authoritative_applied','proposal_revised')))
        FOR UPDATE OF draft", &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref(),&sequence.to_string(),&command.editor_session_id.as_ref()])
        .await.map_err(undo_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let revision_id: String = row.get(1);
    let digest: String = row.get(2);
    let close_event_id: String = row.get(3);
    let mut kind = if row.get::<_, String>(7) != "retained" {
        AuthorUndoFrontierKind::DraftSourceUnavailable
    } else {
        let payload: serde_json::Value = serde_json::from_str(&row.get::<_, String>(9))
            .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?;
        if revision_id == row.get::<_, String>(4)
            && digest == row.get::<_, String>(5)
            && row.get::<_, String>(6) == "closed"
            && Some(&close_event_id) == row.get::<_, Option<String>>(8).as_ref()
            && hex_sha256(canonical_json(&payload).as_bytes()) == digest
        {
            AuthorUndoFrontierKind::ReversibleDraftClose
        } else {
            AuthorUndoFrontierKind::DraftBindingChanged
        }
    };
    let derived_proposal = if row.get::<_, String>(11) == "expandRefusedEditDraftToProposal" {
        let derived = client.query_opt("SELECT proposal.proposal_id::text, candidate.revision_id::text
          FROM storyos.draft_close_events AS closed JOIN storyos.domain_receipts AS receipt USING(owner_user_id,project_id,receipt_id)
          JOIN storyos.proposals AS proposal ON (proposal.owner_user_id,proposal.project_id,proposal.proposal_id::text)=
            (receipt.owner_user_id,receipt.project_id,receipt.result_payload->>'proposal_id')
          JOIN storyos.proposal_heads AS head ON (head.owner_user_id,head.project_id,head.proposal_id)=
            (proposal.owner_user_id,proposal.project_id,proposal.proposal_id)
          JOIN storyos.proposal_revisions AS candidate ON (candidate.owner_user_id,candidate.project_id,candidate.proposal_id,candidate.revision_id)=
            (head.owner_user_id,head.project_id,head.proposal_id,head.current_revision_id)
          WHERE closed.owner_user_id=$1::text::uuid AND closed.project_id=$2::text::uuid AND closed.event_id=$3::text::uuid
            AND receipt.proposal_revision_ids=ARRAY[candidate.revision_id] AND candidate.closure='open' AND candidate.validation='pending'
            AND candidate.base_authoritative_revision_id=$4::text::uuid
            AND (proposal.source_draft_id,proposal.source_draft_revision_id,proposal.source_draft_payload_digest)=
              (closed.draft_id,closed.revision_id,closed.payload_digest)
            AND EXISTS(SELECT 1 FROM storyos.proposal_operations AS operation WHERE
              (operation.owner_user_id,operation.project_id,operation.proposal_id)=(proposal.owner_user_id,proposal.project_id,proposal.proposal_id)
              AND operation.resolution='pending' AND operation.reservation_state='unresolved') FOR UPDATE OF head,candidate",
          &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref(),&close_event_id,&row.get::<_,String>(10)])
          .await.map_err(undo_database_error)?;
        if derived.is_none() && kind == AuthorUndoFrontierKind::ReversibleDraftClose {
            kind = AuthorUndoFrontierKind::DraftBindingChanged;
        }
        derived.map(|row| (row.get(0), row.get(1)))
    } else {
        None
    };
    Ok(Some(ObservedDraftClose {
        sequence,
        draft_id: row.get(0),
        revision_id,
        digest,
        close_event_id,
        kind,
        current_head_revision_id: row.get(10),
        derived_proposal,
    }))
}

pub(super) async fn persist_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedDraftClose,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let scope = &command.project_scope;
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    if let Some((proposal, revision)) = &frontier.derived_proposal {
        let count = client.execute("WITH withdrawn AS (UPDATE storyos.proposal_revisions SET closure='withdrawn'
          WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND proposal_id=$3::text::uuid
            AND revision_id=$4::text::uuid AND closure='open' RETURNING proposal_id)
          UPDATE storyos.proposal_operations SET reservation_state='resolved' WHERE owner_user_id=$1::text::uuid
            AND project_id=$2::text::uuid AND proposal_id IN (SELECT proposal_id FROM withdrawn)",
          &[&owner,&project,&proposal,&revision]).await.map_err(undo_database_error)?;
        if count != 1 {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        }
    }
    let row = client
        .query_one(
            "UPDATE storyos.scope_counters SET author_action_sequence=author_action_sequence+1
        WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
        RETURNING author_action_sequence::text,project_activity_position::text",
            &[&owner, &project],
        )
        .await
        .map_err(undo_database_error)?;
    let sequence: String = row.get(0);
    let activity: String = row.get(1);
    let next: Option<String> = client.query_one("SELECT max(action.author_action_sequence)::text FROM storyos.author_action_entries AS action
        WHERE action.owner_user_id=$1::text::uuid AND action.project_id=$2::text::uuid AND action.disposition='forward'
        AND action.author_action_sequence<>$3::text::numeric AND NOT EXISTS(SELECT 1 FROM storyos.author_action_entries AS compensation
        WHERE compensation.owner_user_id=action.owner_user_id AND compensation.project_id=action.project_id
        AND compensation.disposition='compensation' AND compensation.compensated_source_sequence=action.author_action_sequence)",
        &[&owner,&project,&frontier.sequence.to_string()]).await.map_err(undo_database_error)?.get(0);
    let event_id = Uuid::now_v7().to_string();
    let handler_receipt_id = Uuid::now_v7().to_string();
    let payload = serde_json::json!({"event_id":event_id,"handler_receipt_id":handler_receipt_id,
        "author_undo_frontier_sequence":next})
    .to_string();
    let created_at = insert_undo_receipt(
        client,
        command,
        "draft_closure_changed",
        &payload,
        &command.expected_authoritative_revision_id,
        &command.expected_authoritative_revision_id,
        UndoReceiptAuthority::Draft {
            draft_id: frontier.draft_id.clone(),
            event_id: event_id.clone(),
        },
    )
    .await?;
    client.execute("INSERT INTO storyos.author_action_entries(owner_user_id,project_id,author_action_sequence,disposition,compensated_source_sequence,receipt_id,receipt_result_kind)
        VALUES($1::text::uuid,$2::text::uuid,$3::text::numeric,'compensation',$4::text::numeric,$5::text::uuid,'draft_closure_changed')",
        &[&owner,&project,&sequence,&frontier.sequence.to_string(),&command.ids.receipt_id]).await.map_err(undo_database_error)?;
    let event = persist_reopen(
        client,
        command,
        frontier,
        DraftReopenWrite {
            event_id,
            handler_receipt_id,
            sequence,
            created_at: created_at.clone(),
        },
    )
    .await?;
    let response_project = settle_idempotency(client, command).await?;
    Ok(UndoLatestAuthorActionSettlement {
        source_reopen_event: None,
        ids: command.ids.clone(),
        effect: UndoLatestAuthorActionSettlementEffect::CompensatedDraft {
            event: Box::new(event),
            author_undo_frontier_sequence: next
                .map(|value| value.parse())
                .transpose()
                .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?,
        },
        receipt_created_at: created_at,
        project_activity_position: activity
            .parse()
            .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?,
        response_project,
    })
}

pub(super) struct DraftReopenWrite {
    pub event_id: String,
    pub handler_receipt_id: String,
    pub sequence: String,
    pub created_at: String,
}

pub(super) async fn persist_reopen(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedDraftClose,
    write: DraftReopenWrite,
) -> Result<storyos_contracts::EditorFlowDraftReopened, UndoLatestAuthorActionError> {
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    let contract_scope = storyos_contracts::ProjectScope {
        owner_user_id: owner.to_owned(),
        project_id: project.to_owned(),
    };
    let event = storyos_contracts::EditorFlowDraftReopened {
        schema_id: "storyos.event.editor-flow-draft-reopened.v1".to_owned(),
        event_kind: "editor_flow_draft_reopened".to_owned(),
        event_id: write.event_id.clone(),
        project_scope: contract_scope.clone(),
        draft_id: frontier.draft_id.clone(),
        draft_revision_id: frontier.revision_id.clone(),
        payload_digest: frontier.digest.clone(),
        source_close_event_id: frontier.close_event_id.clone(),
        prior_closure: "closed".to_owned(),
        closure: "open".to_owned(),
        source: storyos_contracts::RefusedEditDraftSource {
            command_id: command.ids.command_id.clone(),
            author_command_admission_id: command.ids.author_command_admission_id.clone(),
            receipt_id: command.ids.receipt_id.clone(),
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            command_digest: storyos_contracts::DigestValue {
                algorithm: storyos_contracts::DigestAlgorithm::Sha256,
                profile: storyos_contracts::UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE.to_owned(),
                value_hex_lowercase: hex_sha256(&command.canonical_command_bytes),
            },
        },
        handler_receipt: storyos_contracts::DraftReopenReceipt {
            schema_id: "storyos.receipt.draft-reopen.v1".to_owned(),
            receipt_id: write.handler_receipt_id.clone(),
            project_scope: contract_scope,
            author_undo_receipt_id: command.ids.receipt_id.clone(),
            source_close_event_id: frontier.close_event_id.clone(),
            event_id: write.event_id.clone(),
            result: "draft_reopened".to_owned(),
            created_at: write.created_at.clone(),
        },
        source_author_action_sequence: frontier.sequence.to_string(),
        author_action_sequence: write.sequence.clone(),
        created_at: write.created_at.clone(),
    };
    client.execute("INSERT INTO storyos.draft_reopen_receipts(owner_user_id,project_id,receipt_id,author_undo_receipt_id,source_close_event_id,event_id,payload)
        VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text::uuid,$7::text::jsonb)",
        &[&owner,&project,&write.handler_receipt_id,&command.ids.receipt_id,&frontier.close_event_id,&write.event_id,
          &serde_json::to_string(&event.handler_receipt).map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?])
        .await.map_err(undo_database_error)?;
    client.execute("INSERT INTO storyos.draft_reopen_events(owner_user_id,project_id,event_id,draft_id,revision_id,source_close_event_id,handler_receipt_id,author_action_sequence,payload)
        VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text::uuid,$7::text::uuid,$8::text::numeric,$9::text::jsonb)",
        &[&owner,&project,&write.event_id,&frontier.draft_id,&frontier.revision_id,&frontier.close_event_id,&write.handler_receipt_id,&write.sequence,
          &serde_json::to_string(&event).map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?]).await.map_err(undo_database_error)?;
    let updated = client.execute("UPDATE storyos.draft_artifacts SET closure='open',reopen_event_id=$4::text::uuid
        WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND draft_id=$3::text::uuid
        AND current_revision_id=$5::text::uuid AND close_event_id=$6::text::uuid AND closure='closed' AND retention_state='retained'",
        &[&owner,&project,&frontier.draft_id,&write.event_id,&frontier.revision_id,&frontier.close_event_id]).await.map_err(undo_database_error)?;
    if updated != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    Ok(event)
}

pub(super) async fn read_event(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    event_id: &str,
) -> Result<storyos_contracts::EditorFlowDraftReopened, UndoLatestAuthorActionError> {
    let payload: String = client.query_one("SELECT event.payload::text FROM storyos.draft_reopen_events AS event
        JOIN storyos.draft_reopen_receipts AS receipt ON (receipt.owner_user_id,receipt.project_id,receipt.receipt_id)=
        (event.owner_user_id,event.project_id,event.handler_receipt_id)
        WHERE event.owner_user_id=$1::text::uuid AND event.project_id=$2::text::uuid AND event.event_id=$3::text::uuid",
        &[&scope.owner_user_id.as_ref(),&scope.project_id.as_ref(),&event_id]).await.map_err(undo_database_error)?.get(0);
    serde_json::from_str(&payload)
        .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))
}
