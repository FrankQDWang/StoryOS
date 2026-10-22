SET LOCAL ROLE storyos_owner;

-- The Draft Compensation of a Refused Edit Draft expansion withdraws the current Proposal head
-- (ADR 0068). That head is the Revision of the expansion or a later Revision that a recorded
-- Compensation wrote: a Proposal Compensation records the appended Revision and its parent in
-- its Undo Receipt payload, and an Acceptance Compensation records its reopen Revision in its
-- Undo Acceptance Receipt. The adapter proves that the head restores the Revision of the
-- expansion. Only the expansion branch changes. All other branches are as migration 0067 wrote
-- them.
CREATE OR REPLACE FUNCTION storyos.require_draft_reopen_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE target_event uuid;
BEGIN
  IF TG_TABLE_NAME='domain_receipts' THEN
    IF NEW.command_kind<>'undoLatestAuthorAction' OR cardinality(NEW.artifact_lifecycle_event_refs)=0 THEN RETURN NULL; END IF;
    target_event:=NEW.artifact_lifecycle_event_refs[1]::uuid;
  ELSE target_event:=NEW.event_id;
  END IF;
  IF NOT EXISTS(SELECT 1 FROM storyos.draft_reopen_events AS event
    JOIN storyos.draft_reopen_receipts AS handler ON (handler.owner_user_id,handler.project_id,handler.receipt_id)=
      (event.owner_user_id,event.project_id,event.handler_receipt_id)
    JOIN storyos.draft_close_events AS closed ON (closed.owner_user_id,closed.project_id,closed.event_id)=
      (event.owner_user_id,event.project_id,event.source_close_event_id)
    JOIN storyos.domain_receipts AS receipt ON (receipt.owner_user_id,receipt.project_id,receipt.receipt_id)=
      (handler.owner_user_id,handler.project_id,handler.author_undo_receipt_id)
    JOIN storyos.author_action_entries AS action ON (action.owner_user_id,action.project_id,action.author_action_sequence)=
      (event.owner_user_id,event.project_id,event.author_action_sequence)
    JOIN storyos.author_command_admissions AS admission ON (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=
      (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
    JOIN storyos.draft_artifacts AS draft ON (draft.owner_user_id,draft.project_id,draft.draft_id)=
      (event.owner_user_id,event.project_id,event.draft_id)
    WHERE event.owner_user_id=NEW.owner_user_id AND event.project_id=NEW.project_id AND event.event_id=target_event
      AND draft.closure='open' AND draft.retention_state='retained' AND draft.current_revision_id=event.revision_id
      AND draft.close_event_id=closed.event_id AND draft.reopen_event_id=event.event_id
      AND admission.command_kind=receipt.command_kind AND admission.command_id=receipt.command_id
      AND admission.canonical_command_digest=receipt.command_digest AND admission.idempotency_key=receipt.idempotency_key
      AND EXISTS(SELECT 1 FROM storyos.author_command_admission_settlements AS settlement WHERE
        (settlement.owner_user_id,settlement.project_id,settlement.author_command_admission_id,settlement.receipt_id)=
        (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id,receipt.receipt_id) AND settlement.settlement_kind='receipt_settled')
      AND receipt.command_kind='undoLatestAuthorAction'
      AND ((receipt.result_kind='draft_closure_changed' AND closed.close_reason='abandoned'
        AND receipt.authoritative_revision_ids='{}' AND receipt.authoritative_commit_ids='{}' AND receipt.proposal_revision_ids='{}'
        AND receipt.result_payload->>'event_id'=event.event_id::text AND receipt.result_payload->>'handler_receipt_id'=handler.receipt_id::text)
       OR (receipt.result_kind='draft_closure_changed' AND closed.close_reason='superseded'
        AND receipt.authoritative_revision_ids='{}' AND receipt.authoritative_commit_ids='{}' AND receipt.proposal_revision_ids='{}'
        AND receipt.result_payload->>'event_id'=event.event_id::text AND receipt.result_payload->>'handler_receipt_id'=handler.receipt_id::text
        AND EXISTS(SELECT 1 FROM storyos.domain_receipts AS source
          JOIN storyos.proposals AS proposal ON (proposal.owner_user_id,proposal.project_id,proposal.proposal_id::text)=
            (source.owner_user_id,source.project_id,source.result_payload->>'proposal_id')
          JOIN storyos.proposal_heads AS head ON (head.owner_user_id,head.project_id,head.proposal_id)=
            (proposal.owner_user_id,proposal.project_id,proposal.proposal_id)
          JOIN storyos.proposal_revisions AS candidate ON (candidate.owner_user_id,candidate.project_id,candidate.proposal_id,candidate.revision_id)=
            (head.owner_user_id,head.project_id,head.proposal_id,head.current_revision_id)
          WHERE (source.owner_user_id,source.project_id,source.receipt_id)=(closed.owner_user_id,closed.project_id,closed.receipt_id)
            AND source.command_kind='expandRefusedEditDraftToProposal' AND source.result_kind='proposal_created_from_draft'
            AND cardinality(source.proposal_revision_ids)=1 AND candidate.closure='withdrawn'
            AND (candidate.revision_id=source.proposal_revision_ids[1]
              OR EXISTS(SELECT 1 FROM storyos.author_action_entries AS appended
                JOIN storyos.domain_receipts AS appended_receipt ON (appended_receipt.owner_user_id,appended_receipt.project_id,appended_receipt.receipt_id)=
                  (appended.owner_user_id,appended.project_id,appended.receipt_id)
                WHERE (appended.owner_user_id,appended.project_id)=(candidate.owner_user_id,candidate.project_id) AND appended.disposition='compensation'
                  AND appended_receipt.result_payload->>'proposal_revision_id'=candidate.revision_id::text
                  AND appended_receipt.result_payload->>'source_proposal_revision_id'=candidate.parent_revision_id::text)
              OR EXISTS(SELECT 1 FROM storyos.undo_acceptance_receipts AS reopened WHERE
                (reopened.owner_user_id,reopened.project_id,reopened.proposal_id,reopened.proposal_revision_id)=
                (candidate.owner_user_id,candidate.project_id,candidate.proposal_id,candidate.revision_id) AND reopened.outcome='compensated'))
            AND (proposal.source_draft_id,proposal.source_draft_revision_id,proposal.source_draft_payload_digest)=
              (event.draft_id,event.revision_id,closed.payload_digest)
            AND source.source_draft_disposition->>'closure_event_ref'=closed.event_id::text
            AND NOT EXISTS(SELECT 1 FROM storyos.proposal_operations AS operation WHERE
              (operation.owner_user_id,operation.project_id,operation.proposal_id)=(proposal.owner_user_id,proposal.project_id,proposal.proposal_id)
              AND (operation.resolution<>'pending' OR operation.reservation_state<>'resolved'))))
       OR (receipt.result_kind='authoritative_applied' AND closed.close_reason='superseded'
        AND EXISTS(SELECT 1 FROM storyos.domain_receipts AS source WHERE
          (source.owner_user_id,source.project_id,source.receipt_id)=(closed.owner_user_id,closed.project_id,closed.receipt_id)
          AND source.command_kind='applyAuthorEdit' AND source.result_kind IN ('authoritative_applied','proposal_revised')
          AND source.source_draft_disposition->>'closure_event_ref'=closed.event_id::text
          AND source.source_draft_disposition->>'source_draft_id'=event.draft_id::text
          AND source.source_draft_disposition->>'source_draft_revision_id'=event.revision_id::text)))
      AND receipt.draft_artifact_refs=ARRAY[event.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]
      AND action.disposition='compensation' AND action.compensated_source_sequence=closed.author_action_sequence AND action.receipt_id=receipt.receipt_id
      AND (closed.draft_id,closed.revision_id)=(event.draft_id,event.revision_id)
      AND (handler.event_id,handler.source_close_event_id)=(event.event_id,event.source_close_event_id)
      AND admission.command_payload->'undo_latest_author_action_input'->>'expected_author_undo_frontier_sequence'=closed.author_action_sequence::text
      AND closed.author_action_sequence=(SELECT max(source.author_action_sequence) FROM storyos.author_action_entries AS source
        WHERE source.owner_user_id=event.owner_user_id AND source.project_id=event.project_id AND source.disposition='forward'
        AND source.author_action_sequence<action.author_action_sequence AND NOT EXISTS(SELECT 1 FROM storyos.author_action_entries AS prior
          WHERE prior.owner_user_id=source.owner_user_id AND prior.project_id=source.project_id AND prior.disposition='compensation'
          AND prior.compensated_source_sequence=source.author_action_sequence AND prior.author_action_sequence<action.author_action_sequence))
      AND handler.payload=jsonb_build_object('schema_id','storyos.receipt.draft-reopen.v1','receipt_id',handler.receipt_id::text,
        'project_scope',jsonb_build_object('owner_user_id',event.owner_user_id::text,'project_id',event.project_id::text),
        'author_undo_receipt_id',receipt.receipt_id::text,'source_close_event_id',closed.event_id::text,'event_id',event.event_id::text,
        'result','draft_reopened','created_at',to_char(receipt.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'))
      AND event.payload=jsonb_build_object('schema_id','storyos.event.editor-flow-draft-reopened.v1','event_kind','editor_flow_draft_reopened',
        'event_id',event.event_id::text,'project_scope',handler.payload->'project_scope','draft_id',event.draft_id::text,
        'draft_revision_id',event.revision_id::text,'payload_digest',closed.payload_digest,'source_close_event_id',closed.event_id::text,
        'prior_closure','closed','closure','open','handler_receipt',handler.payload,'source_author_action_sequence',closed.author_action_sequence::text,
        'author_action_sequence',action.author_action_sequence::text,'created_at',handler.payload->>'created_at',
        'source',jsonb_build_object('command_id',receipt.command_id::text,'author_command_admission_id',receipt.author_command_admission_id::text,
          'receipt_id',receipt.receipt_id::text,'idempotency_key',receipt.idempotency_key::text,'command_digest',jsonb_build_object(
          'algorithm','sha256','profile','storyos.command.undoLatestAuthorAction.jcs.v1','value_hex_lowercase',split_part(receipt.command_digest,':',3))))
      AND EXISTS(SELECT 1 FROM storyos.command_idempotency AS replay WHERE
        (replay.owner_user_id,replay.project_id,replay.command_kind,replay.idempotency_key)=
        (receipt.owner_user_id,receipt.project_id,receipt.command_kind,receipt.idempotency_key)
        AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text AND replay.canonical_command_digest=receipt.command_digest))
  THEN RAISE EXCEPTION 'Incomplete Draft reopen settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;
