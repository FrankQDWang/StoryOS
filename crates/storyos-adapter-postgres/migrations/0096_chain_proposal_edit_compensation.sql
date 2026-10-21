SET LOCAL ROLE storyos_owner;

-- A Proposal edit Compensation appends its Revision to the current Proposal head (ADR 0068).
-- That head is the edited Revision or a later Revision of the same Proposal that later
-- Compensations wrote. The Undo Receipt records the appended Revision and its parent. The
-- adapter proves that the parent is in the Compensation lineage of the edited Revision. Only the
-- Proposal edit Compensation branch changes. All other branches are as migration 0066 wrote them.
CREATE OR REPLACE FUNCTION storyos.require_draft_retry_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE receipt storyos.domain_receipts; admission storyos.author_command_admissions;
  source jsonb; disposition jsonb; draft storyos.draft_artifacts; revision storyos.draft_artifact_revisions;
  event storyos.draft_close_events; creation storyos.draft_lifecycle_events;
BEGIN
  IF TG_TABLE_NAME='domain_receipts' THEN receipt:=NEW;
  ELSE SELECT * INTO STRICT receipt FROM storyos.domain_receipts AS r WHERE
    (r.owner_user_id,r.project_id,r.receipt_id)=(NEW.owner_user_id,NEW.project_id,NEW.receipt_id); END IF;
  IF receipt.command_kind='undoLatestAuthorAction' AND receipt.result_kind='authoritative_applied'
    AND EXISTS(SELECT 1 FROM storyos.author_action_entries AS a JOIN storyos.author_action_entries AS s
      ON (s.owner_user_id,s.project_id,s.author_action_sequence)=(a.owner_user_id,a.project_id,a.compensated_source_sequence)
      JOIN storyos.domain_receipts AS r ON (r.owner_user_id,r.project_id,r.receipt_id)=(s.owner_user_id,s.project_id,s.receipt_id)
      WHERE (a.owner_user_id,a.project_id,a.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id) AND r.result_kind='proposal_revised') THEN
    IF NOT EXISTS(SELECT 1 FROM storyos.author_action_entries AS a JOIN storyos.author_action_entries AS s
      ON (s.owner_user_id,s.project_id,s.author_action_sequence)=(a.owner_user_id,a.project_id,a.compensated_source_sequence)
      JOIN storyos.domain_receipts AS r ON (r.owner_user_id,r.project_id,r.receipt_id)=(s.owner_user_id,s.project_id,s.receipt_id)
      JOIN storyos.proposal_revisions AS edited ON (edited.owner_user_id,edited.project_id,edited.revision_id)=
        (r.owner_user_id,r.project_id,r.proposal_revision_ids[1])
      JOIN storyos.proposal_revisions AS parent ON (parent.owner_user_id,parent.project_id,parent.proposal_id)=
        (edited.owner_user_id,edited.project_id,edited.proposal_id)
      JOIN storyos.proposal_revisions AS restored ON (restored.owner_user_id,restored.project_id,restored.proposal_id,restored.parent_revision_id)=
        (parent.owner_user_id,parent.project_id,parent.proposal_id,parent.revision_id)
      WHERE (a.owner_user_id,a.project_id,a.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id)
        AND a.disposition='compensation' AND s.disposition='forward' AND r.result_kind='proposal_revised'
        AND receipt.result_payload->>'proposal_revision_id'=restored.revision_id::text
        AND receipt.result_payload->>'source_proposal_revision_id'=parent.revision_id::text)
    THEN RAISE EXCEPTION 'Incomplete Proposal compensation identity' USING ERRCODE='23514'; END IF;
    RETURN NULL;
  END IF;
  IF receipt.command_kind<>'applyAuthorEdit' THEN RETURN NULL; END IF;
  SELECT * INTO STRICT admission FROM storyos.author_command_admissions AS a WHERE
    (a.owner_user_id,a.project_id,a.author_command_admission_id)=(receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id);
  source:=admission.command_payload->'retry_source'; disposition:=receipt.source_draft_disposition;
  IF source IS NULL THEN
    IF disposition IS NOT NULL THEN RAISE EXCEPTION 'Fresh intent cannot close a retry source' USING ERRCODE='23514'; END IF;
    RETURN NULL;
  END IF;
  SELECT * INTO STRICT draft FROM storyos.draft_artifacts AS d WHERE
    (d.owner_user_id,d.project_id,d.draft_id::text)=(receipt.owner_user_id,receipt.project_id,source->>'source_draft_id');
  SELECT * INTO STRICT revision FROM storyos.draft_artifact_revisions AS r WHERE
    (r.owner_user_id,r.project_id,r.draft_id,r.revision_id)=(draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id);
  IF NOT (source->>'kind'='draft_retry' AND source->>'source_draft_kind'='refused_edit'
    AND source->>'expected_source_draft_closure'='open' AND draft.retention_state='retained'
    AND disposition->>'source_draft_kind'='refused_edit' AND disposition->>'source_draft_id'=draft.draft_id::text
    AND admission.command_kind=receipt.command_kind AND admission.command_id=receipt.command_id
    AND admission.canonical_command_digest=receipt.command_digest AND admission.idempotency_key=receipt.idempotency_key
    AND EXISTS(SELECT 1 FROM storyos.author_command_admission_settlements AS s WHERE
      (s.owner_user_id,s.project_id,s.author_command_admission_id,s.receipt_id)=
      (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id,receipt.receipt_id) AND s.settlement_kind='receipt_settled')
    AND EXISTS(SELECT 1 FROM storyos.command_idempotency AS replay WHERE
      (replay.owner_user_id,replay.project_id,replay.command_kind,replay.idempotency_key)=
      (receipt.owner_user_id,receipt.project_id,receipt.command_kind,receipt.idempotency_key)
      AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text AND replay.canonical_command_digest=receipt.command_digest)
  ) IS TRUE THEN RAISE EXCEPTION 'Incomplete Draft retry source binding' USING ERRCODE='23514'; END IF;
  IF disposition->>'kind'='unchanged' THEN
    IF NOT (receipt.result_kind IN ('no_effect','conflicted','refused')
      AND disposition->>'requested_source_draft_revision_id'=source->>'source_current_draft_revision_id'
      AND disposition->>'current_source_draft_revision_id'=draft.current_revision_id::text
      AND disposition->>'current_source_draft_payload_digest'=revision.payload_digest
      AND disposition->'current_closure'->>'kind'=draft.closure
      AND (draft.closure='open' OR EXISTS(SELECT 1 FROM storyos.draft_close_events AS c WHERE
        (c.owner_user_id,c.project_id,c.event_id)=(draft.owner_user_id,draft.project_id,draft.close_event_id)
        AND disposition->'current_closure'->>'closure_event_ref'=c.event_id::text AND disposition->'current_closure'->>'close_reason'=c.close_reason))
      AND NOT EXISTS(SELECT 1 FROM storyos.draft_close_events AS c WHERE
        (c.owner_user_id,c.project_id,c.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id))
    ) IS TRUE THEN RAISE EXCEPTION 'Draft retry must keep its source unchanged' USING ERRCODE='23514'; END IF;
    RETURN NULL;
  END IF;
  SELECT * INTO STRICT event FROM storyos.draft_close_events AS c WHERE
    (c.owner_user_id,c.project_id,c.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id);
  IF receipt.result_kind='refused_to_draft' THEN
    SELECT * INTO STRICT creation FROM storyos.draft_lifecycle_events AS c WHERE
      (c.owner_user_id,c.project_id,c.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id);
  END IF;
  IF NOT (disposition=jsonb_build_object('kind','closed_superseded','source_draft_kind','refused_edit',
      'source_draft_id',draft.draft_id::text,'source_draft_revision_id',revision.revision_id::text,
      'source_draft_payload_digest',revision.payload_digest,'prior_closure','open','resulting_closure','closed',
      'close_reason','superseded','closure_event_ref',event.event_id::text)
    AND source->>'source_current_draft_revision_id'=revision.revision_id::text AND source->>'source_draft_payload_digest'=revision.payload_digest
    AND draft.closure='closed' AND draft.close_event_id=event.event_id AND draft.reopen_event_id IS NULL
    AND (event.draft_id,event.revision_id,event.payload_digest,event.close_reason,event.receipt_result_kind)=
      (draft.draft_id,revision.revision_id,revision.payload_digest,'superseded',receipt.result_kind)
    AND event.created_at=receipt.created_at AND receipt.result_kind IN ('authoritative_applied','proposal_revised','refused_to_draft')
    AND ((receipt.result_kind='refused_to_draft' AND event.author_action_sequence IS NULL AND creation.draft_id<>draft.draft_id
        AND receipt.draft_artifact_refs=ARRAY[creation.draft_id::text,draft.draft_id::text]
        AND receipt.artifact_lifecycle_event_refs=ARRAY[creation.creation_event_id::text,event.event_id::text])
      OR (receipt.result_kind IN ('authoritative_applied','proposal_revised')
        AND receipt.draft_artifact_refs=ARRAY[draft.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]
        AND EXISTS(SELECT 1 FROM storyos.author_action_entries AS a WHERE
          (a.owner_user_id,a.project_id,a.receipt_id,a.author_action_sequence)=
          (event.owner_user_id,event.project_id,event.receipt_id,event.author_action_sequence) AND a.disposition='forward')))
  ) IS TRUE THEN RAISE EXCEPTION 'Incomplete atomic Draft retry settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;
