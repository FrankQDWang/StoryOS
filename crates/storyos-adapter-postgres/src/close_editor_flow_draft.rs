use crate::PostgresProjectReader;
use storyos_application::{
    AuthorCommandAdmissionIds, CloseEditorFlowDraftCommand, CloseEditorFlowDraftStore,
    DraftCloseError, DraftCloseSettlement, ProjectCommandChallengeError,
    ProjectCommandChallengeTransaction, ProjectCommandChallengeUse,
};
use storyos_core::{CloseEditorFlowDraftResult, canonical_json, hex_sha256};
use uuid::Uuid;

impl CloseEditorFlowDraftStore for PostgresProjectReader {
    async fn close_editor_flow_draft(
        &self,
        command: &CloseEditorFlowDraftCommand,
    ) -> Result<DraftCloseSettlement, DraftCloseError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(challenge_error)?;
        let usage = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(challenge_error)?;
        let response = match usage {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                read_settlement(&transaction.client, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                Err(DraftCloseError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => persist(&transaction.client, command).await,
        };
        match response {
            Ok(response) => {
                transaction.commit().await.map_err(challenge_error)?;
                Ok(response)
            }
            Err(error) => {
                transaction.rollback().await.map_err(challenge_error)?;
                Err(error)
            }
        }
    }
}

async fn persist(
    client: &tokio_postgres::Client,
    command: &CloseEditorFlowDraftCommand,
) -> Result<DraftCloseSettlement, DraftCloseError> {
    let scope = &command.project_scope;
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    let row = client.query_opt("SELECT draft.current_revision_id::text, revision.payload_digest,
        draft.closure, draft.retention_state, CASE WHEN draft.retention_state='retained' THEN revision.payload::text END
        FROM storyos.draft_artifacts AS draft JOIN storyos.projects AS project USING(owner_user_id,project_id)
        JOIN storyos.draft_artifact_revisions AS revision ON
        (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
        (draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id)
        WHERE draft.owner_user_id=$1::text::uuid AND draft.project_id=$2::text::uuid
        AND draft.draft_id=$3::text::uuid AND project.lifecycle_state='active' FOR UPDATE OF project,draft",
        &[&owner,&project,&command.draft_id]).await.map_err(database_error)?.ok_or(DraftCloseError::MissingDraft)?;
    let revision: String = row.get(0);
    let digest: String = row.get(1);
    let closure: String = row.get(2);
    let retention: String = row.get(3);
    if retention == "retained" {
        let payload: serde_json::Value = serde_json::from_str(
            &row.get::<_, Option<String>>(4)
                .ok_or(DraftCloseError::BindingConflict)?,
        )
        .map_err(unavailable)?;
        if hex_sha256(canonical_json(&payload).as_bytes()) != digest {
            return Err(DraftCloseError::BindingConflict);
        }
    }
    insert_admission(client, command).await?;
    let classified = storyos_core::close_editor_flow_draft(
        &command.input.source_current_draft_revision_id,
        &command.input.source_draft_payload_digest,
        &revision,
        &digest,
        &closure,
        &retention,
    );
    let mut sequence = None;
    let mut event_id = None;
    let result_kind = match classified {
        CloseEditorFlowDraftResult::DraftClosureChanged => {
            sequence = Some(client.query_one("UPDATE storyos.scope_counters SET author_action_sequence=author_action_sequence+1
                WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid RETURNING author_action_sequence::text",
                &[&owner,&project]).await.map_err(database_error)?.get::<_,String>(0));
            event_id = Some(Uuid::now_v7().to_string());
            "draft_closure_changed"
        }
        CloseEditorFlowDraftResult::Conflicted => "conflicted",
        CloseEditorFlowDraftResult::SourceDraftNotOpen
        | CloseEditorFlowDraftResult::SourceUnavailable => "refused",
    };
    let payload = serde_json::json!({"draft_revision_id":revision,"payload_digest":digest,
    "observed_closure":closure,"event_id":event_id,"reason":match classified {
        CloseEditorFlowDraftResult::DraftClosureChanged => "abandoned",
        CloseEditorFlowDraftResult::Conflicted => "source_binding_changed",
        CloseEditorFlowDraftResult::SourceDraftNotOpen => "source_draft_not_open",
        CloseEditorFlowDraftResult::SourceUnavailable => "source_unavailable",
    }});
    let effect_json = payload.to_string();
    client.execute("INSERT INTO storyos.domain_receipts(owner_user_id,project_id,receipt_id,author_command_admission_id,command_id,
        command_kind,command_digest,idempotency_key,producer_cause,expected_heads,prior_heads,resulting_heads,
        authoritative_revision_ids,proposal_revision_ids,authoritative_commit_ids,draft_artifact_refs,artifact_lifecycle_event_refs,
        condition_refs,result_kind,result_payload)
        VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,'closeEditorFlowDraft',$6,$7::text::uuid,
        'author_command_admission','{}','{}','{}','{}','{}','{}',ARRAY[$8::text],$9::text[],'{}',$10,$11::text::jsonb)",
        &[&owner,&project,&command.ids.receipt_id,&command.ids.author_command_admission_id,&command.ids.command_id,
          &command.challenge_binding.canonical_command_digest,&command.challenge_binding.idempotency_key,&command.draft_id,
          &event_id.iter().cloned().collect::<Vec<_>>(),&result_kind,&effect_json]).await.map_err(database_error)?;
    if let (Some(event_id), Some(sequence)) = (&event_id, &sequence) {
        client.execute("INSERT INTO storyos.author_action_entries(owner_user_id,project_id,author_action_sequence,disposition,receipt_id,receipt_result_kind)
            VALUES($1::text::uuid,$2::text::uuid,$3::text::numeric,'forward',$4::text::uuid,'draft_closure_changed')",
            &[&owner,&project,&sequence,&command.ids.receipt_id]).await.map_err(database_error)?;
        client.execute("INSERT INTO storyos.draft_close_events(owner_user_id,project_id,event_id,draft_id,revision_id,payload_digest,receipt_id,author_action_sequence,created_at)
            VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6,$7::text::uuid,$8::text::numeric,(SELECT created_at FROM storyos.domain_receipts
              WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND receipt_id=$7::text::uuid))",
            &[&owner,&project,&event_id,&command.draft_id,&revision,&digest,&command.ids.receipt_id,&sequence]).await.map_err(database_error)?;
        client.execute("UPDATE storyos.draft_artifacts SET closure='closed',close_event_id=$4::text::uuid
            WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND draft_id=$3::text::uuid",
            &[&owner,&project,&command.draft_id,&event_id]).await.map_err(database_error)?;
    }
    client.execute("INSERT INTO storyos.author_command_admission_settlements(owner_user_id,project_id,author_command_admission_id,settlement_kind,receipt_id)
        VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,'receipt_settled',$4::text::uuid)",
        &[&owner,&project,&command.ids.author_command_admission_id,&command.ids.receipt_id]).await.map_err(database_error)?;
    client.execute("UPDATE storyos.command_idempotency SET outcome_kind='settled',result_reference=$4
        WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND command_kind='closeEditorFlowDraft' AND idempotency_key=$3::text::uuid",
        &[&owner,&project,&command.challenge_binding.idempotency_key,&command.ids.receipt_id]).await.map_err(database_error)?;
    read_settlement(client, command, &command.ids.receipt_id).await
}

async fn insert_admission(
    client: &tokio_postgres::Client,
    command: &CloseEditorFlowDraftCommand,
) -> Result<(), DraftCloseError> {
    let binding = &command.challenge_binding;
    let input = &command.input;
    let client_binding = &command.client_binding;
    let count=client.execute("INSERT INTO storyos.author_command_admissions
        (owner_user_id,project_id,author_command_admission_id,command_id,editor_session_id,writer_generation,
        client_session_binding_ref,client_session_generation,client_contract_revision,security_policy_revision,
        action_class,method,route_template,command_schema,command_kind,canonical_command_digest,idempotency_key,
        challenge_consumed_at,challenge_expires_at,correlation_id,expected_proposal_head_revision_ids,target_refs,
        editor_contract_revision,command_payload)
        SELECT $1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,session.editor_session_id,writer.writer_generation,
        session.client_session_binding_ref,session.client_session_generation,session.client_contract_revision,session.security_policy_revision,
        'explicit_editor_command','POST',$5,$6,'closeEditorFlowDraft',$7,$8::text::uuid,challenge.consumed_at,challenge.expires_at,
        $9::text::uuid,'{}','{}',session.client_contract_revision,convert_from($10::bytea,'UTF8')::jsonb
        FROM storyos.editor_sessions AS session JOIN storyos.project_writer_generations AS writer
        ON (writer.owner_user_id,writer.project_id,writer.current_editor_session_id)=(session.owner_user_id,session.project_id,session.editor_session_id)
        AND writer.writer_generation=(SELECT max(w.writer_generation) FROM storyos.project_writer_generations AS w
        WHERE w.owner_user_id=session.owner_user_id AND w.project_id=session.project_id)
        JOIN storyos.project_command_challenges AS challenge ON (challenge.owner_user_id,challenge.project_id,challenge.idempotency_key)=
        (session.owner_user_id,session.project_id,$8::text::uuid) AND challenge.command_kind='closeEditorFlowDraft'
        WHERE session.owner_user_id=$1::text::uuid AND session.project_id=$2::text::uuid AND session.editor_session_id=$11::text::uuid
        AND session.client_session_binding_ref=$12 AND session.client_session_generation=$13::text::numeric
        AND session.client_contract_revision=$14 AND session.security_policy_revision=$15 AND challenge.consumed_at IS NOT NULL",
        &[&command.project_scope.owner_user_id.as_ref(),&command.project_scope.project_id.as_ref(),&command.ids.author_command_admission_id,&command.ids.command_id,
          &binding.route_template,&binding.command_schema,&binding.canonical_command_digest,&binding.idempotency_key,&input.correlation_id,
          &command.canonical_command_bytes,&input.editor_session_id,&client_binding.binding_ref,&client_binding.session_generation.to_string(),
          &client_binding.client_contract_revision,&client_binding.security_policy_revision]).await.map_err(database_error)?;
    if count != 1 {
        return Err(DraftCloseError::InvalidWriter);
    }
    Ok(())
}

async fn read_settlement(
    client: &tokio_postgres::Client,
    command: &CloseEditorFlowDraftCommand,
    receipt_id: &str,
) -> Result<DraftCloseSettlement, DraftCloseError> {
    let row=client.query_opt("SELECT receipt.command_id::text,receipt.author_command_admission_id::text,receipt.result_payload::text,
        action.author_action_sequence::text,to_char(receipt.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),receipt.result_kind,
        admission.correlation_id::text FROM storyos.domain_receipts AS receipt
        JOIN storyos.author_command_admissions AS admission USING(owner_user_id,project_id,author_command_admission_id,command_id)
        JOIN storyos.author_command_admission_settlements AS settlement USING(owner_user_id,project_id,author_command_admission_id,receipt_id)
        LEFT JOIN storyos.author_action_entries AS action USING(owner_user_id,project_id,receipt_id)
        WHERE receipt.owner_user_id=$1::text::uuid AND receipt.project_id=$2::text::uuid AND receipt.receipt_id=$3::text::uuid
        AND receipt.command_kind='closeEditorFlowDraft' AND receipt.command_digest=$4 AND receipt.idempotency_key=$5::text::uuid
        AND settlement.settlement_kind='receipt_settled' AND receipt.draft_artifact_refs=ARRAY[$6::text]
        AND admission.command_kind=receipt.command_kind AND admission.canonical_command_digest=receipt.command_digest
        AND admission.idempotency_key=receipt.idempotency_key AND admission.command_payload=$7::text::jsonb
        AND ((receipt.result_kind='draft_closure_changed' AND action.disposition='forward' AND EXISTS(
          SELECT 1 FROM storyos.draft_close_events AS event WHERE (event.owner_user_id,event.project_id,event.receipt_id,event.author_action_sequence)=
          (receipt.owner_user_id,receipt.project_id,receipt.receipt_id,action.author_action_sequence)
          AND event.event_id::text=receipt.result_payload->>'event_id' AND event.draft_id::text=$6))
          OR (receipt.result_kind IN ('conflicted','refused') AND action.receipt_id IS NULL))",
        &[&command.project_scope.owner_user_id.as_ref(),&command.project_scope.project_id.as_ref(),&receipt_id,
          &command.challenge_binding.canonical_command_digest,&command.challenge_binding.idempotency_key,&command.draft_id,&String::from_utf8_lossy(&command.canonical_command_bytes).as_ref()])
        .await.map_err(database_error)?.ok_or(DraftCloseError::BindingConflict)?;
    let payload: serde_json::Value =
        serde_json::from_str(&row.get::<_, String>(2)).map_err(unavailable)?;
    let text = |field: &str| {
        payload[field]
            .as_str()
            .map(str::to_owned)
            .ok_or(DraftCloseError::BindingConflict)
    };
    let result = match (row.get::<_, String>(5).as_str(), payload["reason"].as_str()) {
        ("draft_closure_changed", Some("abandoned")) => {
            CloseEditorFlowDraftResult::DraftClosureChanged
        }
        ("conflicted", Some("source_binding_changed")) => CloseEditorFlowDraftResult::Conflicted,
        ("refused", Some("source_draft_not_open")) => {
            CloseEditorFlowDraftResult::SourceDraftNotOpen
        }
        ("refused", Some("source_unavailable")) => CloseEditorFlowDraftResult::SourceUnavailable,
        _ => return Err(DraftCloseError::BindingConflict),
    };
    Ok(DraftCloseSettlement {
        ids: AuthorCommandAdmissionIds {
            command_id: row.get(0),
            author_command_admission_id: row.get(1),
            receipt_id: receipt_id.to_owned(),
        },
        correlation_id: row.get(6),
        result,
        draft_revision_id: text("draft_revision_id")?,
        payload_digest: text("payload_digest")?,
        observed_closure: text("observed_closure")?,
        event_id: payload["event_id"].as_str().map(str::to_owned),
        author_action_sequence: row.get(3),
        created_at: row.get(4),
    })
}

fn challenge_error(error: ProjectCommandChallengeError) -> DraftCloseError {
    match error {
        ProjectCommandChallengeError::BindingConflict => DraftCloseError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => DraftCloseError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(error) => DraftCloseError::Unavailable(error),
    }
}
fn database_error(error: tokio_postgres::Error) -> DraftCloseError {
    unavailable(error)
}
fn unavailable(error: impl std::error::Error + Send + Sync + 'static) -> DraftCloseError {
    DraftCloseError::Unavailable(Box::new(error))
}
