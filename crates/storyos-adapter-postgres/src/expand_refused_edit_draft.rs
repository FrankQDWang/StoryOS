use storyos_application::{
    DraftCloseError, DraftExpansionSettlement, ExpandRefusedEditDraftCommand,
    ExpandRefusedEditDraftStore, ProjectCommandChallengeError, ProjectCommandChallengeTransaction,
    ProjectCommandChallengeUse,
};
use storyos_core::{
    CloseEditorFlowDraftRefusal, DraftCloseSource, ExpandRefusedEditDraftResult,
    OpenInlineProposal, TransitionOutcome,
};
use uuid::Uuid;

use crate::PostgresProjectReader;

impl ExpandRefusedEditDraftStore for PostgresProjectReader {
    async fn expand_refused_edit_draft(
        &self,
        command: &ExpandRefusedEditDraftCommand,
    ) -> Result<DraftExpansionSettlement, DraftCloseError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(challenge_error)?;
        let usage = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(challenge_error)?;
        let result = match usage {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                read_settlement(&transaction.client, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                Err(DraftCloseError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => persist(&transaction.client, command).await,
        };
        match result {
            Ok(result) => {
                transaction.commit().await.map_err(challenge_error)?;
                Ok(result)
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
    command: &ExpandRefusedEditDraftCommand,
) -> Result<DraftExpansionSettlement, DraftCloseError> {
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    let input = &command.input;
    let source = client.query_opt("SELECT draft.current_revision_id::text, revision.payload_digest,
        draft.closure, draft.retention_state, draft.reopen_event_id::text,
        CASE WHEN draft.retention_state='retained' THEN revision.payload::text END
        FROM storyos.draft_artifacts AS draft JOIN storyos.projects AS project USING(owner_user_id,project_id)
        JOIN storyos.draft_artifact_revisions AS revision ON
        (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
        (draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id)
        WHERE draft.owner_user_id=$1::text::uuid AND draft.project_id=$2::text::uuid
        AND draft.draft_id=$3::text::uuid AND project.lifecycle_state='active' FOR UPDATE OF project,draft",
        &[&owner,&project,&command.draft_id]).await.map_err(unavailable)?.ok_or(DraftCloseError::MissingDraft)?;
    let revision: String = source.get(0);
    let digest: String = source.get(1);
    let closure: String = source.get(2);
    let reopen: Option<String> = source.get(4);
    insert_admission(client, command).await?;
    let target = client.query_opt("SELECT head.current_revision_id::text, convert_from(payload.canonical_bytes,'UTF8'),
        ARRAY(SELECT member.manuscript_block_id::text FROM storyos.manuscript_revision_members AS member
          WHERE (member.owner_user_id,member.project_id,member.manuscript_object_id,member.revision_id)=
          (head.owner_user_id,head.project_id,head.manuscript_object_id,head.current_revision_id) ORDER BY member.block_order),
        EXISTS(SELECT 1 FROM storyos.proposal_operations AS operation WHERE operation.owner_user_id=head.owner_user_id
          AND operation.project_id=head.project_id AND operation.manuscript_block_id=$4::text::uuid AND operation.reservation_state='unresolved')
        FROM storyos.authoritative_heads AS head JOIN storyos.authoritative_revisions AS revision ON
        (revision.owner_user_id,revision.project_id,revision.manuscript_object_id,revision.revision_id)=
        (head.owner_user_id,head.project_id,head.manuscript_object_id,head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload ON
        (payload.owner_user_id,payload.project_id,payload.payload_id)=(revision.owner_user_id,revision.project_id,revision.payload_id)
        WHERE head.owner_user_id=$1::text::uuid AND head.project_id=$2::text::uuid AND head.manuscript_object_id=$3::text::uuid
        AND NOT EXISTS(SELECT 1 FROM storyos.chapter_removal_decisions AS removed WHERE
          (removed.owner_user_id,removed.project_id,removed.chapter_id)=(head.owner_user_id,head.project_id,head.manuscript_object_id)) FOR UPDATE OF head",
        &[&owner,&project,&input.chapter_id,&input.target_refs[0]]).await.map_err(unavailable)?;
    let head = target.as_ref().map(|row| row.get::<_, String>(0));
    let blocks = target
        .as_ref()
        .map(|row| {
            crate::manuscript_block::blocks_from_stored_payload(
                &row.get::<_, String>(1),
                &row.get::<_, Vec<String>>(2),
            )
        })
        .unwrap_or_default();
    let target = OpenInlineProposal {
        scope_matches: true,
        target_block_present: blocks
            .iter()
            .any(|block| block.manuscript_block_id == input.target_refs[0]),
        expected_base_revision_id: input.expected_target_revisions[0].clone(),
        current_base_revision_id: head.clone(),
        conflicting_reservation: target.is_some_and(|row| row.get(3)),
        current_schema_version: 1,
        current_coordinate_profile: storyos_core::PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
        blocks: blocks
            .into_iter()
            .map(|block| storyos_core::InlineTargetBlock {
                manuscript_block_id: block.manuscript_block_id,
                block_kind: match block.block_kind {
                    storyos_core::ManuscriptBlockKind::Paragraph => "paragraph",
                    storyos_core::ManuscriptBlockKind::Heading => "heading",
                }
                .to_owned(),
                text: block.text,
            })
            .collect(),
        anchors: input
            .anchors
            .iter()
            .map(|anchor| storyos_core::OpenInlineProposalAnchor {
                manuscript_block_id: anchor.manuscript_block_id.clone(),
                base_authoritative_revision_id: anchor.base_authoritative_revision_id.clone(),
                manuscript_schema_version: anchor.manuscript_schema_version,
                coordinate_profile: anchor.coordinate_profile.clone(),
                from: anchor.from,
                to: anchor.to,
                boundary_profile: anchor.boundary_profile.clone(),
                base_slice_digest: anchor.base_slice_digest.clone(),
            })
            .collect(),
    };
    let expected = DraftCloseSource {
        revision: &input.source_current_draft_revision_id,
        digest: &input.source_draft_payload_digest,
        reopen_event_id: input.source_reopen_event_id.as_deref(),
    };
    let current = DraftCloseSource {
        revision: &revision,
        digest: &digest,
        reopen_event_id: reopen.as_deref(),
    };
    let result = match storyos_core::close_editor_flow_draft(
        &expected,
        &current,
        &closure,
        &source.get::<_, String>(3),
    ) {
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(_) => ExpandRefusedEditDraftResult::Conflicted,
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceDraftNotOpen) => {
            ExpandRefusedEditDraftResult::SourceDraftNotOpen
        }
        TransitionOutcome::Refused(CloseEditorFlowDraftRefusal::SourceUnavailable) => {
            ExpandRefusedEditDraftResult::SourceUnavailable
        }
        TransitionOutcome::Applied(()) => {
            let payload: serde_json::Value =
                serde_json::from_str(&source.get::<_, String>(5)).map_err(unavailable)?;
            if storyos_core::hex_sha256(storyos_core::canonical_json(&payload).as_bytes()) != digest
            {
                return Err(DraftCloseError::BindingConflict);
            }
            storyos_core::expand_refused_edit_draft(
                &expected,
                &current,
                &closure,
                "retained",
                &serde_json::from_value(payload).map_err(unavailable)?,
                &target,
            )
        }
    };
    let proposal_id = Uuid::now_v7().to_string();
    let proposal_revision_id = Uuid::now_v7().to_string();
    let event_id = Uuid::now_v7().to_string();
    let (result_kind, reason, replacement) = match &result {
        ExpandRefusedEditDraftResult::ProposalCreated { replacement } => (
            "proposal_created_from_draft",
            "superseded",
            Some(replacement),
        ),
        ExpandRefusedEditDraftResult::Conflicted => {
            ("conflicted", "source_or_target_changed", None)
        }
        ExpandRefusedEditDraftResult::SourceDraftNotOpen => {
            ("refused", "source_draft_not_open", None)
        }
        ExpandRefusedEditDraftResult::SourceUnavailable => ("refused", "source_unavailable", None),
        ExpandRefusedEditDraftResult::UnsupportedPayload => {
            ("refused", "unsupported_payload", None)
        }
        ExpandRefusedEditDraftResult::TargetUnavailable => ("refused", "target_unavailable", None),
    };
    let effect = serde_json::json!({"draft_revision_id":revision,"payload_digest":digest,"observed_closure":closure,
        "current_target_revision_id":head,"reason":reason,"event_id":replacement.map(|_| &event_id),
        "proposal_id":replacement.map(|_| &proposal_id),"proposal_revision_id":replacement.map(|_| &proposal_revision_id)});
    persist_effect(client, command, result_kind, &effect, replacement).await?;
    read_settlement(client, command, &command.ids.receipt_id).await
}

async fn insert_admission(
    client: &tokio_postgres::Client,
    command: &ExpandRefusedEditDraftCommand,
) -> Result<(), DraftCloseError> {
    let binding = &command.client_binding;
    let count = client.execute("WITH command AS (SELECT convert_from($10::bytea,'UTF8')::jsonb AS payload)
        INSERT INTO storyos.author_command_admissions(owner_user_id,project_id,author_command_admission_id,command_id,
          editor_session_id,writer_generation,client_session_binding_ref,client_session_generation,client_contract_revision,
          security_policy_revision,action_class,method,route_template,command_schema,command_kind,canonical_command_digest,
          idempotency_key,challenge_consumed_at,challenge_expires_at,correlation_id,chapter_object_id,
          expected_authoritative_revision_id,expected_proposal_head_revision_ids,target_refs,editor_contract_revision,command_payload)
        SELECT $1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,session.editor_session_id,writer.writer_generation,
          session.client_session_binding_ref,session.client_session_generation,session.client_contract_revision,session.security_policy_revision,
          'explicit_editor_command','POST','/api/v1/projects/{project_id}/drafts/{draft_id}/proposal-expansions',
          command.payload->>'command_schema','expandRefusedEditDraftToProposal',$11,$9::text::uuid,
          challenge.consumed_at,challenge.expires_at,(input->>'correlation_id')::uuid,(input->>'chapter_id')::uuid,
          (input->'expected_target_revisions'->>0)::uuid,'{}',ARRAY(SELECT jsonb_array_elements_text(input->'target_refs')),
          session.client_contract_revision,command.payload
        FROM command CROSS JOIN LATERAL (SELECT payload->'expand_refused_edit_draft_to_proposal_input' AS input) AS request
        JOIN storyos.editor_sessions AS session ON session.editor_session_id=(input->>'editor_session_id')::uuid
        JOIN storyos.project_writer_generations AS writer ON
          (writer.owner_user_id,writer.project_id,writer.current_editor_session_id)=(session.owner_user_id,session.project_id,session.editor_session_id)
          AND writer.writer_generation=(input->>'writer_generation')::numeric
          AND writer.writer_generation=(SELECT max(w.writer_generation) FROM storyos.project_writer_generations AS w
            WHERE (w.owner_user_id,w.project_id)=(session.owner_user_id,session.project_id))
        JOIN storyos.project_command_challenges AS challenge ON
          (challenge.owner_user_id,challenge.project_id,challenge.idempotency_key,challenge.command_kind)=
          (session.owner_user_id,session.project_id,$9::text::uuid,'expandRefusedEditDraftToProposal')
        WHERE session.owner_user_id=$1::text::uuid AND session.project_id=$2::text::uuid
          AND session.client_session_binding_ref=$5 AND session.client_session_generation=$6::text::numeric
          AND session.client_contract_revision=$7 AND session.security_policy_revision=$8
          AND challenge.consumed_at IS NOT NULL AND challenge.canonical_command_digest=$11",
        &[&command.project_scope.owner_user_id.as_ref(),&command.project_scope.project_id.as_ref(),
          &command.ids.author_command_admission_id,&command.ids.command_id,&binding.binding_ref,
          &binding.session_generation.to_string(),&binding.client_contract_revision,&binding.security_policy_revision,
          &command.challenge_binding.idempotency_key,&command.canonical_command_bytes,
          &command.challenge_binding.canonical_command_digest]).await.map_err(unavailable)?;
    if count != 1 {
        return Err(DraftCloseError::InvalidWriter);
    }
    Ok(())
}

async fn persist_effect(
    client: &tokio_postgres::Client,
    command: &ExpandRefusedEditDraftCommand,
    result_kind: &str,
    effect: &serde_json::Value,
    replacement: Option<&Vec<storyos_core::ReplacementBlock>>,
) -> Result<(), DraftCloseError> {
    let context = serde_json::json!({"owner":command.project_scope.owner_user_id.as_ref(),
        "project":command.project_scope.project_id.as_ref(),"receipt":command.ids.receipt_id,
        "admission":command.ids.author_command_admission_id,"command":command.ids.command_id,
        "digest":command.challenge_binding.canonical_command_digest,"key":command.challenge_binding.idempotency_key,
        "draft":command.draft_id,"input":command.input,"effect":effect,"result":result_kind,
        "blocks":replacement,"candidate":replacement.map(|blocks| blocks.iter().map(|block| block.text.as_str()).collect::<Vec<_>>().join("\n")),
        "operation":Uuid::now_v7().to_string()}).to_string();
    client.execute("WITH context AS (SELECT $1::text::jsonb AS c), ids AS (
        SELECT c,(c->>'owner')::uuid AS owner,(c->>'project')::uuid AS project,(c->>'receipt')::uuid AS receipt,
          (c->'effect'->>'proposal_id')::uuid AS proposal,(c->'effect'->>'proposal_revision_id')::uuid AS revision,
          (c->'effect'->>'event_id')::uuid AS event,(c->>'draft')::uuid AS draft,(c->>'operation')::uuid AS operation FROM context),
        counter AS (UPDATE storyos.scope_counters SET author_action_sequence=author_action_sequence+1
          FROM ids WHERE owner_user_id=ids.owner AND project_id=ids.project AND c->>'result'='proposal_created_from_draft'
          RETURNING author_action_sequence),
        proposal AS (INSERT INTO storyos.proposals(owner_user_id,project_id,proposal_id,kind,chapter_id,manuscript_block_id,
          source_draft_id,source_draft_revision_id,source_draft_payload_digest)
          SELECT owner,project,proposal,'inline_edit',(c->'input'->>'chapter_id')::uuid,(c->'input'->'target_refs'->>0)::uuid,
          draft,(c->'effect'->>'draft_revision_id')::uuid,c->'effect'->>'payload_digest' FROM ids WHERE proposal IS NOT NULL),
        revision AS (INSERT INTO storyos.proposal_revisions(owner_user_id,project_id,proposal_id,revision_id,generation,validation,
          closure,candidate_text,candidate_blocks,base_authoritative_revision_id)
          SELECT owner,project,proposal,revision,'ready','pending','open',c->>'candidate',c->'blocks',
          (c->'effect'->>'current_target_revision_id')::uuid FROM ids WHERE proposal IS NOT NULL),
        head AS (INSERT INTO storyos.proposal_heads SELECT owner,project,proposal,revision FROM ids WHERE proposal IS NOT NULL),
        operation AS (INSERT INTO storyos.proposal_operations(owner_user_id,project_id,proposal_id,operation_id,
          manuscript_block_id,resolution,reservation_state,candidate_text,candidate_blocks)
          SELECT owner,project,proposal,operation,(c->'input'->'target_refs'->>0)::uuid,'pending','unresolved',c->>'candidate',c->'blocks'
          FROM ids WHERE proposal IS NOT NULL),
        anchor AS (INSERT INTO storyos.proposal_anchors(owner_user_id,project_id,proposal_id,operation_id,anchor_order,
          manuscript_block_id,base_authoritative_revision_id,manuscript_schema_version,coordinate_profile,range_from,range_to,boundary_profile,base_slice_digest)
          SELECT owner,project,proposal,operation,1,(a->>'manuscript_block_id')::uuid,(a->>'base_authoritative_revision_id')::uuid,
          (a->>'manuscript_schema_version')::integer,a->>'coordinate_profile',(a->>'from')::integer,(a->>'to')::integer,
          a->>'boundary_profile',a->>'base_slice_digest' FROM ids CROSS JOIN LATERAL
          (SELECT c->'input'->'anchors'->0 AS a) AS source WHERE proposal IS NOT NULL),
        receipt AS (INSERT INTO storyos.domain_receipts(owner_user_id,project_id,receipt_id,author_command_admission_id,command_id,
          command_kind,command_digest,idempotency_key,producer_cause,expected_heads,prior_heads,resulting_heads,
          authoritative_revision_ids,proposal_revision_ids,authoritative_commit_ids,draft_artifact_refs,artifact_lifecycle_event_refs,
          condition_refs,result_kind,result_payload,source_draft_disposition)
          SELECT owner,project,receipt,(c->>'admission')::uuid,(c->>'command')::uuid,'expandRefusedEditDraftToProposal',c->>'digest',
          (c->>'key')::uuid,'author_command_admission',ARRAY(SELECT jsonb_array_elements_text(c->'input'->'expected_target_revisions'))::uuid[],
          CASE WHEN c->'effect'->>'current_target_revision_id' IS NULL THEN '{}'::uuid[] ELSE ARRAY[(c->'effect'->>'current_target_revision_id')::uuid] END,
          CASE WHEN c->'effect'->>'current_target_revision_id' IS NULL THEN '{}'::uuid[] ELSE ARRAY[(c->'effect'->>'current_target_revision_id')::uuid] END,
          '{}',CASE WHEN revision IS NULL THEN '{}'::uuid[] ELSE ARRAY[revision] END,'{}',ARRAY[draft::text],
          CASE WHEN event IS NULL THEN '{}'::text[] ELSE ARRAY[event::text] END,'{}',c->>'result',c->'effect',
          CASE WHEN event IS NULL THEN NULL ELSE jsonb_build_object('kind','closed_superseded','source_draft_kind','refused_edit',
          'source_draft_id',draft::text,'source_draft_revision_id',c->'effect'->>'draft_revision_id',
          'source_draft_payload_digest',c->'effect'->>'payload_digest','prior_closure','open','resulting_closure','closed',
          'close_reason','superseded','closure_event_ref',event::text) END FROM ids RETURNING created_at),
        action AS (INSERT INTO storyos.author_action_entries(owner_user_id,project_id,author_action_sequence,disposition,receipt_id,receipt_result_kind)
          SELECT owner,project,author_action_sequence,'forward',ids.receipt,'proposal_created_from_draft' FROM ids CROSS JOIN counter),
        closed AS (INSERT INTO storyos.draft_close_events(owner_user_id,project_id,event_id,draft_id,revision_id,payload_digest,
          receipt_id,receipt_result_kind,author_action_sequence,created_at,close_reason)
          SELECT owner,project,event,draft,(c->'effect'->>'draft_revision_id')::uuid,c->'effect'->>'payload_digest',ids.receipt,
          'proposal_created_from_draft',author_action_sequence,created_at,'superseded' FROM ids CROSS JOIN counter CROSS JOIN receipt),
        draft AS (UPDATE storyos.draft_artifacts SET closure='closed',close_event_id=event,reopen_event_id=NULL FROM ids
          WHERE owner_user_id=owner AND project_id=project AND draft_id=draft AND event IS NOT NULL),
        settled AS (INSERT INTO storyos.author_command_admission_settlements(owner_user_id,project_id,author_command_admission_id,settlement_kind,receipt_id)
          SELECT owner,project,(c->>'admission')::uuid,'receipt_settled',receipt FROM ids)
        UPDATE storyos.command_idempotency SET outcome_kind='settled',result_reference=receipt::text FROM ids
          WHERE owner_user_id=owner AND project_id=project AND command_kind='expandRefusedEditDraftToProposal'
          AND idempotency_key=(c->>'key')::uuid", &[&context]).await.map_err(unavailable)?;
    Ok(())
}

fn challenge_error(error: ProjectCommandChallengeError) -> DraftCloseError {
    match error {
        ProjectCommandChallengeError::BindingConflict => DraftCloseError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => DraftCloseError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(error) => DraftCloseError::Unavailable(error),
    }
}

async fn read_settlement(
    client: &tokio_postgres::Client,
    command: &ExpandRefusedEditDraftCommand,
    receipt_id: &str,
) -> Result<DraftExpansionSettlement, DraftCloseError> {
    let row = client.query_opt("SELECT receipt.command_id::text,receipt.author_command_admission_id::text,
        admission.correlation_id::text,receipt.result_kind,receipt.result_payload::text,action.author_action_sequence::text,
        to_char(receipt.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),revision.candidate_blocks::text
        FROM storyos.domain_receipts AS receipt JOIN storyos.author_command_admissions AS admission
          USING(owner_user_id,project_id,author_command_admission_id,command_id)
        JOIN storyos.author_command_admission_settlements AS settled USING(owner_user_id,project_id,author_command_admission_id,receipt_id)
        LEFT JOIN storyos.author_action_entries AS action USING(owner_user_id,project_id,receipt_id)
        LEFT JOIN storyos.proposal_revisions AS revision ON
          (revision.owner_user_id,revision.project_id,revision.proposal_id::text,revision.revision_id::text)=
          (receipt.owner_user_id,receipt.project_id,receipt.result_payload->>'proposal_id',receipt.result_payload->>'proposal_revision_id')
        WHERE receipt.owner_user_id=$1::text::uuid AND receipt.project_id=$2::text::uuid AND receipt.receipt_id=$3::text::uuid
          AND receipt.command_kind='expandRefusedEditDraftToProposal' AND receipt.command_digest=$4 AND receipt.idempotency_key=$5::text::uuid
          AND admission.command_payload=$6::text::jsonb AND admission.command_kind=receipt.command_kind
          AND admission.canonical_command_digest=receipt.command_digest AND admission.idempotency_key=receipt.idempotency_key
          AND settled.settlement_kind='receipt_settled' AND receipt.draft_artifact_refs=ARRAY[$7::text]
          AND ((receipt.result_kind='proposal_created_from_draft' AND action.disposition='forward' AND revision.revision_id IS NOT NULL
            AND EXISTS(SELECT 1 FROM storyos.draft_close_events AS event WHERE
              (event.owner_user_id,event.project_id,event.receipt_id,event.author_action_sequence)=
              (receipt.owner_user_id,receipt.project_id,receipt.receipt_id,action.author_action_sequence)
              AND event.event_id::text=receipt.result_payload->>'event_id' AND event.draft_id::text=$7 AND event.close_reason='superseded'))
            OR (receipt.result_kind IN ('refused','conflicted') AND action.receipt_id IS NULL))",
        &[&command.project_scope.owner_user_id.as_ref(),&command.project_scope.project_id.as_ref(),&receipt_id,
          &command.challenge_binding.canonical_command_digest,&command.challenge_binding.idempotency_key,
          &String::from_utf8_lossy(&command.canonical_command_bytes).as_ref(),&command.draft_id])
        .await.map_err(unavailable)?.ok_or(DraftCloseError::BindingConflict)?;
    let payload: serde_json::Value =
        serde_json::from_str(&row.get::<_, String>(4)).map_err(unavailable)?;
    let text = |field: &str| payload[field].as_str().map(str::to_owned);
    let result = match (row.get::<_, String>(3).as_str(), payload["reason"].as_str()) {
        ("proposal_created_from_draft", Some("superseded")) => {
            ExpandRefusedEditDraftResult::ProposalCreated {
                replacement: serde_json::from_str(
                    &row.get::<_, Option<String>>(7)
                        .ok_or(DraftCloseError::BindingConflict)?,
                )
                .map_err(unavailable)?,
            }
        }
        ("conflicted", Some("source_or_target_changed")) => {
            ExpandRefusedEditDraftResult::Conflicted
        }
        ("refused", Some("source_draft_not_open")) => {
            ExpandRefusedEditDraftResult::SourceDraftNotOpen
        }
        ("refused", Some("source_unavailable")) => ExpandRefusedEditDraftResult::SourceUnavailable,
        ("refused", Some("unsupported_payload")) => {
            ExpandRefusedEditDraftResult::UnsupportedPayload
        }
        ("refused", Some("target_unavailable")) => ExpandRefusedEditDraftResult::TargetUnavailable,
        _ => return Err(DraftCloseError::BindingConflict),
    };
    Ok(DraftExpansionSettlement {
        ids: storyos_application::AuthorCommandAdmissionIds {
            command_id: row.get(0),
            author_command_admission_id: row.get(1),
            receipt_id: receipt_id.to_owned(),
        },
        correlation_id: row.get(2),
        draft_revision_id: text("draft_revision_id").ok_or(DraftCloseError::BindingConflict)?,
        payload_digest: text("payload_digest").ok_or(DraftCloseError::BindingConflict)?,
        observed_closure: text("observed_closure").ok_or(DraftCloseError::BindingConflict)?,
        event_id: text("event_id"),
        author_action_sequence: row.get(5),
        created_at: row.get(6),
        result,
        proposal_id: text("proposal_id"),
        proposal_revision_id: text("proposal_revision_id"),
        current_target_revision_id: text("current_target_revision_id"),
    })
}
fn unavailable(error: impl std::error::Error + Send + Sync + 'static) -> DraftCloseError {
    DraftCloseError::Unavailable(Box::new(error))
}
