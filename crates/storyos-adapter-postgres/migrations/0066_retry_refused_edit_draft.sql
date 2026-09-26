ALTER TABLE storyos.domain_receipts ADD COLUMN source_draft_disposition jsonb;
ALTER TABLE storyos.domain_receipts ADD CONSTRAINT draft_retry_disposition_shape CHECK (
  source_draft_disposition IS NULL OR (command_kind='applyAuthorEdit' AND
    source_draft_disposition->>'kind' IN ('unchanged','closed_superseded')) IS TRUE);
DO $migration$
DECLARE prior_check text;
BEGIN
  SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint
    WHERE conrelid='storyos.domain_receipts'::regclass AND conname='domain_receipts_result_shape' AND contype='c';
  IF prior_check NOT LIKE 'CHECK (%)' THEN RAISE EXCEPTION 'Receipt result shape is unavailable'; END IF;
  ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_shape;
  EXECUTE format($sql$ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_result_shape CHECK ((%s) OR ((
    command_kind='undoLatestAuthorAction' AND result_kind='authoritative_applied'
    AND jsonb_typeof(result_payload)='object' AND result_payload ?& ARRAY['proposal_revision_id','source_proposal_revision_id']
    AND result_payload - ARRAY['proposal_revision_id','source_proposal_revision_id']='{}'::jsonb
    AND result_payload->>'proposal_revision_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    AND result_payload->>'source_proposal_revision_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    AND authoritative_revision_ids='{}' AND authoritative_commit_ids='{}' AND proposal_revision_ids='{}'
    AND prior_heads=expected_heads AND resulting_heads=expected_heads
  ) IS TRUE))$sql$,substring(prior_check FROM 8 FOR length(prior_check)-8));
END $migration$;
ALTER TABLE storyos.draft_close_events ADD COLUMN close_reason text NOT NULL DEFAULT 'abandoned';
ALTER TABLE storyos.draft_close_events ALTER COLUMN author_action_sequence DROP NOT NULL;
ALTER TABLE storyos.draft_close_events DROP CONSTRAINT draft_close_events_receipt_result_kind_check;
DO $migration$
DECLARE constraint_name text;
BEGIN
  SELECT conname INTO STRICT constraint_name FROM pg_constraint WHERE conrelid='storyos.draft_close_events'::regclass
    AND contype='f' AND confrelid='storyos.author_action_entries'::regclass;
  EXECUTE format('ALTER TABLE storyos.draft_close_events DROP CONSTRAINT %I',constraint_name);
END $migration$;
ALTER TABLE storyos.draft_close_events ADD CONSTRAINT draft_close_action_fk FOREIGN KEY(owner_user_id,project_id,author_action_sequence)
  REFERENCES storyos.author_action_entries(owner_user_id,project_id,author_action_sequence) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE storyos.draft_close_events ADD CONSTRAINT draft_close_reason_shape CHECK ((
  (close_reason='abandoned' AND receipt_result_kind='draft_closure_changed' AND author_action_sequence IS NOT NULL)
  OR (close_reason='superseded' AND receipt_result_kind IN ('authoritative_applied','proposal_revised') AND author_action_sequence IS NOT NULL)
  OR (close_reason='superseded' AND receipt_result_kind='refused_to_draft' AND author_action_sequence IS NULL)
) IS TRUE);

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_common_shape;
ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_common_shape CHECK ((
(
(
    (
      (result_kind <> 'proposal_revised'
        AND cardinality(proposal_revision_ids) = 0)
      OR (result_kind = 'proposal_revised'
        AND cardinality(proposal_revision_ids) = 1
        AND array_dims(proposal_revision_ids) = '[1:1]')
    )
    AND cardinality(draft_artifact_refs) = 0
    AND cardinality(artifact_lifecycle_event_refs) = 0
    AND (cardinality(condition_refs) = 0 OR
      (command_kind = 'acceptProposal' AND result_kind = 'conflicted'
       AND cardinality(condition_refs) = 1))
    AND array_position(expected_heads, NULL) IS NULL
    AND array_position(prior_heads, NULL) IS NULL
    AND array_position(resulting_heads, NULL) IS NULL
    AND array_position(authoritative_revision_ids, NULL) IS NULL
    AND array_position(proposal_revision_ids, NULL) IS NULL
    AND array_position(authoritative_commit_ids, NULL) IS NULL
    AND array_position(draft_artifact_refs, NULL) IS NULL
    AND array_position(artifact_lifecycle_event_refs, NULL) IS NULL
    AND array_position(condition_refs, NULL) IS NULL
    AND (
      (command_kind NOT IN (
        'createProject', 'updateProject', 'archiveProject', 'createVolume',
        'createChapter', 'updateVolume', 'updateChapter', 'deleteChapter', 'deleteVolume',
        'exportHumanReadableManuscript', 'exportProjectArchive',
        'updateProjectAssistance', 'createAgentRun', 'pauseAgentRun',
        'cancelAgentRun'
      )
        AND cardinality(expected_heads) = 1
        AND cardinality(prior_heads) = 1
        AND cardinality(resulting_heads) = 1
        AND array_dims(expected_heads) = '[1:1]'
        AND array_dims(prior_heads) = '[1:1]'
        AND array_dims(resulting_heads) = '[1:1]')
      OR (command_kind IN (
        'createProject', 'updateProject', 'archiveProject', 'createVolume',
        'createChapter', 'updateVolume', 'updateChapter', 'deleteChapter', 'deleteVolume',
        'exportHumanReadableManuscript', 'exportProjectArchive',
        'updateProjectAssistance', 'createAgentRun', 'pauseAgentRun',
        'cancelAgentRun'
      )
        AND cardinality(expected_heads) = 0
        AND cardinality(prior_heads) = 0
        AND cardinality(resulting_heads) = 0)
    )
  ) IS TRUE
  ) OR ((
    command_kind = 'applyAuthorEdit' AND result_kind = 'refused_to_draft'
      AND array_dims(expected_heads) = '[1:1]' AND expected_heads[1] IS NOT NULL
      AND prior_heads = expected_heads AND resulting_heads = expected_heads
      AND cardinality(authoritative_revision_ids) = 0 AND cardinality(proposal_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0 AND cardinality(condition_refs) = 0
      AND array_dims(draft_artifact_refs) = '[1:1]' AND draft_artifact_refs[1] IS NOT NULL
      AND array_dims(artifact_lifecycle_event_refs) = '[1:1]'
      AND artifact_lifecycle_event_refs[1] IS NOT NULL
  ) IS TRUE)
) OR ((
command_kind='closeEditorFlowDraft' AND expected_heads='{}' AND prior_heads='{}' AND resulting_heads='{}'
AND authoritative_revision_ids='{}' AND proposal_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
AND array_dims(draft_artifact_refs)='[1:1]' AND draft_artifact_refs[1] IS NOT NULL
AND ((result_kind='draft_closure_changed' AND array_dims(artifact_lifecycle_event_refs)='[1:1]' AND artifact_lifecycle_event_refs[1] IS NOT NULL)
OR (result_kind IN ('refused','conflicted') AND artifact_lifecycle_event_refs='{}'))
) IS TRUE) OR ((
command_kind='undoLatestAuthorAction' AND result_kind='draft_closure_changed'
AND array_dims(expected_heads)='[1:1]' AND expected_heads[1] IS NOT NULL AND prior_heads=expected_heads AND resulting_heads=expected_heads
AND authoritative_revision_ids='{}' AND proposal_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
AND array_dims(draft_artifact_refs)='[1:1]' AND draft_artifact_refs[1] IS NOT NULL
AND array_dims(artifact_lifecycle_event_refs)='[1:1]' AND artifact_lifecycle_event_refs[1] IS NOT NULL
) IS TRUE) OR ((
command_kind='applyAuthorEdit' AND source_draft_disposition->>'kind'='closed_superseded'
AND result_kind IN ('authoritative_applied','proposal_revised','refused_to_draft')
AND array_dims(expected_heads)='[1:1]' AND array_dims(prior_heads)='[1:1]' AND array_dims(resulting_heads)='[1:1]'
AND array_position(expected_heads,NULL) IS NULL AND array_position(prior_heads,NULL) IS NULL AND array_position(resulting_heads,NULL) IS NULL
AND condition_refs='{}' AND array_position(draft_artifact_refs,NULL) IS NULL AND array_position(artifact_lifecycle_event_refs,NULL) IS NULL
AND ((result_kind='refused_to_draft' AND cardinality(draft_artifact_refs)=2 AND cardinality(artifact_lifecycle_event_refs)=2
      AND authoritative_revision_ids='{}' AND proposal_revision_ids='{}' AND authoritative_commit_ids='{}' AND prior_heads=expected_heads AND resulting_heads=expected_heads)
 OR (result_kind='authoritative_applied' AND cardinality(draft_artifact_refs)=1 AND cardinality(artifact_lifecycle_event_refs)=1 AND proposal_revision_ids='{}')
 OR (result_kind='proposal_revised' AND cardinality(draft_artifact_refs)=1 AND cardinality(artifact_lifecycle_event_refs)=1
      AND cardinality(proposal_revision_ids)=1 AND authoritative_revision_ids='{}' AND authoritative_commit_ids='{}' AND prior_heads=expected_heads AND resulting_heads=expected_heads))
) IS TRUE) OR ((
command_kind='undoLatestAuthorAction' AND result_kind='authoritative_applied'
AND array_dims(expected_heads)='[1:1]' AND array_dims(prior_heads)='[1:1]' AND array_dims(resulting_heads)='[1:1]'
AND array_position(expected_heads,NULL) IS NULL AND array_position(prior_heads,NULL) IS NULL AND array_position(resulting_heads,NULL) IS NULL
AND proposal_revision_ids='{}' AND condition_refs='{}'
AND ((cardinality(authoritative_revision_ids)=1 AND cardinality(authoritative_commit_ids)=1)
 OR (authoritative_revision_ids='{}' AND authoritative_commit_ids='{}' AND prior_heads=expected_heads AND resulting_heads=expected_heads))
AND array_dims(draft_artifact_refs)='[1:1]' AND draft_artifact_refs[1] IS NOT NULL
AND array_dims(artifact_lifecycle_event_refs)='[1:1]' AND artifact_lifecycle_event_refs[1] IS NOT NULL
) IS TRUE));

CREATE OR REPLACE FUNCTION storyos.require_refused_draft_settlement() RETURNS trigger
LANGUAGE plpgsql AS $function$
DECLARE
  target_receipt uuid;
BEGIN
  IF TG_TABLE_NAME = 'draft_artifacts' THEN
    SELECT receipt_id INTO target_receipt FROM storyos.draft_lifecycle_events
     WHERE owner_user_id = NEW.owner_user_id AND project_id = NEW.project_id
       AND draft_id = NEW.draft_id AND revision_id = NEW.current_revision_id;
  ELSE
    target_receipt := NEW.receipt_id;
  END IF;
  IF TG_TABLE_NAME = 'domain_receipts' THEN
    IF NEW.result_kind <> 'refused_to_draft' THEN RETURN NULL; END IF;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM storyos.domain_receipts AS receipt
      JOIN storyos.draft_lifecycle_events AS event USING (owner_user_id, project_id, receipt_id)
      JOIN storyos.draft_artifact_revisions AS revision
        ON (revision.owner_user_id, revision.project_id, revision.draft_id, revision.revision_id) =
           (event.owner_user_id, event.project_id, event.draft_id, event.revision_id)
      JOIN storyos.author_command_admissions AS admission
        ON (admission.owner_user_id, admission.project_id, admission.author_command_admission_id) =
           (event.owner_user_id, event.project_id, event.author_command_admission_id)
     WHERE receipt.owner_user_id = NEW.owner_user_id AND receipt.project_id = NEW.project_id
       AND receipt.receipt_id = target_receipt AND receipt.result_kind = 'refused_to_draft'
       AND receipt.draft_artifact_refs = ARRAY[event.draft_id::text] || CASE WHEN receipt.source_draft_disposition->>'kind'='closed_superseded' THEN ARRAY[receipt.source_draft_disposition->>'source_draft_id'] ELSE '{}'::text[] END
       AND receipt.artifact_lifecycle_event_refs = ARRAY[event.creation_event_id::text] || CASE WHEN receipt.source_draft_disposition->>'kind'='closed_superseded' THEN ARRAY[receipt.source_draft_disposition->>'closure_event_ref'] ELSE '{}'::text[] END
       AND receipt.result_payload->>'draft_revision_id' = event.revision_id::text
       AND revision.payload->>'completed_intent_record_id' = admission.completed_intent_record_id::text
       AND revision.payload->>'local_intent_sequence' = admission.local_intent_sequence::text
       AND revision.payload->>'undo_group_id' = admission.undo_group_id::text
       AND revision.payload->>'chapter_id' = admission.chapter_object_id::text
       AND revision.payload->>'expected_authoritative_revision_id' = admission.expected_authoritative_revision_id::text
       AND revision.payload->'expected_proposal_head_revision_ids' = to_jsonb(admission.expected_proposal_head_revision_ids)
       AND revision.payload->'target_refs' = to_jsonb(admission.target_refs)
       AND revision.payload->'author_edit_units' = admission.command_payload->'author_edit_units'
       AND revision.created_at = event.created_at
  ) THEN RAISE EXCEPTION 'Incomplete Refused Edit Draft settlement' USING ERRCODE = '23514'; END IF;
  RETURN NULL;
END
$function$;

CREATE OR REPLACE FUNCTION storyos.require_draft_close_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE target_event uuid; target_draft uuid; target_revision uuid; target_receipt uuid;
  prior_reopen_event text; validate_lifecycle boolean := false;
BEGIN
  IF TG_TABLE_NAME='draft_artifacts' THEN
    target_draft := NEW.draft_id;
    target_revision := NEW.current_revision_id;
    IF TG_OP='UPDATE' THEN
      IF OLD.closure='closed' AND NEW.closure='closed' AND NEW.close_event_id IS DISTINCT FROM OLD.close_event_id THEN
        RAISE EXCEPTION 'Draft Discard requires open source' USING ERRCODE='23514';
      END IF;
      validate_lifecycle := OLD.closure='open' AND NEW.closure='closed';
      prior_reopen_event := OLD.reopen_event_id::text;
    END IF;
    IF NEW.closure='open' THEN
      IF TG_OP='UPDATE' AND OLD.closure='closed' AND
        (NEW.close_event_id IS DISTINCT FROM OLD.close_event_id OR NEW.reopen_event_id IS NULL) THEN
        RAISE EXCEPTION 'Draft Discard requires exact compensation' USING ERRCODE='23514';
      END IF;
      IF NEW.close_event_id IS NULL AND NEW.reopen_event_id IS NULL THEN RETURN NULL; END IF;
      IF NOT EXISTS(SELECT 1 FROM storyos.draft_reopen_events AS reopened WHERE
        (reopened.owner_user_id,reopened.project_id,reopened.event_id,reopened.draft_id,reopened.revision_id,reopened.source_close_event_id)=
        (NEW.owner_user_id,NEW.project_id,NEW.reopen_event_id,NEW.draft_id,NEW.current_revision_id,NEW.close_event_id))
      THEN RAISE EXCEPTION 'Draft reopen requires exact compensation' USING ERRCODE='23514'; END IF;
      RETURN NULL;
    END IF;
    IF NEW.reopen_event_id IS NOT NULL THEN RAISE EXCEPTION 'Closed Draft cannot project reopen' USING ERRCODE='23514'; END IF;
    IF NEW.closure <> 'closed' OR NEW.close_event_id IS NULL THEN
      RAISE EXCEPTION 'Incomplete Draft Discard settlement' USING ERRCODE='23514';
    END IF;
    target_event := NEW.close_event_id;
  ELSIF TG_TABLE_NAME='domain_receipts' THEN
    IF NEW.command_kind <> 'closeEditorFlowDraft' OR NEW.result_kind <> 'draft_closure_changed' THEN RETURN NULL; END IF;
    target_event := (NEW.result_payload->>'event_id')::uuid;
    target_receipt := NEW.receipt_id;
  ELSE
    target_event := NEW.event_id;
  END IF;
  IF EXISTS(SELECT 1 FROM storyos.draft_close_events AS event WHERE
    (event.owner_user_id,event.project_id,event.event_id)=(NEW.owner_user_id,NEW.project_id,target_event) AND event.close_reason='superseded') THEN
    IF NOT EXISTS(SELECT 1 FROM storyos.draft_close_events AS event JOIN storyos.domain_receipts AS receipt USING(owner_user_id,project_id,receipt_id)
      WHERE (event.owner_user_id,event.project_id,event.event_id)=(NEW.owner_user_id,NEW.project_id,target_event)
      AND (target_draft IS NULL OR (event.draft_id,event.revision_id)=(target_draft,target_revision))
      AND receipt.source_draft_disposition->>'closure_event_ref'=event.event_id::text)
    THEN RAISE EXCEPTION 'Incomplete Draft retry closure' USING ERRCODE='23514'; END IF;
    RETURN NULL;
  END IF;
  IF NOT EXISTS(SELECT 1 FROM storyos.draft_close_events AS event
    JOIN storyos.domain_receipts AS receipt USING(owner_user_id,project_id,receipt_id)
    JOIN storyos.author_action_entries AS action USING(owner_user_id,project_id,receipt_id)
    JOIN storyos.draft_artifact_revisions AS revision ON (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
      (event.owner_user_id,event.project_id,event.draft_id,event.revision_id)
    JOIN storyos.author_command_admissions AS admission ON (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=
      (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
    WHERE event.owner_user_id=NEW.owner_user_id AND event.project_id=NEW.project_id AND event.event_id=target_event
      AND (target_draft IS NULL OR (event.draft_id=target_draft AND event.revision_id=target_revision))
      AND (target_receipt IS NULL OR event.receipt_id=target_receipt)
      AND receipt.command_kind='closeEditorFlowDraft' AND receipt.result_kind='draft_closure_changed'
      AND receipt.draft_artifact_refs=ARRAY[event.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]
      AND receipt.result_payload->>'draft_revision_id'=event.revision_id::text AND receipt.result_payload->>'payload_digest'=event.payload_digest
      AND event.payload_digest=revision.payload_digest AND action.disposition='forward' AND action.author_action_sequence=event.author_action_sequence
      AND admission.command_payload->'close_editor_flow_draft_input'->>'draft_id'=event.draft_id::text
      AND admission.command_payload->'close_editor_flow_draft_input'->>'source_current_draft_revision_id'=event.revision_id::text
      AND admission.command_payload->'close_editor_flow_draft_input'->>'source_draft_payload_digest'=event.payload_digest
      AND admission.command_payload->'close_editor_flow_draft_input'->>'expected_closure'='open'
      AND (NOT validate_lifecycle OR (admission.command_payload->'close_editor_flow_draft_input'->>'source_reopen_event_id') IS NOT DISTINCT FROM prior_reopen_event)
      AND admission.command_payload->'close_editor_flow_draft_input'->>'close_reason'='abandoned'
      AND admission.command_payload->'close_editor_flow_draft_input'->>'draft_kind'='refused_edit'
      AND admission.command_kind=receipt.command_kind AND admission.canonical_command_digest=receipt.command_digest
      AND admission.idempotency_key=receipt.idempotency_key AND admission.command_id=receipt.command_id
      AND event.created_at=receipt.created_at AND receipt.result_payload->>'event_id'=event.event_id::text
      AND receipt.result_payload->>'observed_closure'='open'
      AND EXISTS(SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
        WHERE (settlement.owner_user_id,settlement.project_id,settlement.author_command_admission_id,settlement.receipt_id)=
        (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id,receipt.receipt_id) AND settlement.settlement_kind='receipt_settled')
      AND EXISTS(SELECT 1 FROM storyos.command_idempotency AS replay
        WHERE (replay.owner_user_id,replay.project_id,replay.command_kind,replay.idempotency_key)=
        (receipt.owner_user_id,receipt.project_id,receipt.command_kind,receipt.idempotency_key)
        AND replay.canonical_command_digest=receipt.command_digest
        AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text))
  THEN RAISE EXCEPTION 'Incomplete Draft Discard settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;


CREATE FUNCTION storyos.require_draft_retry_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
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
      JOIN storyos.proposal_revisions AS parent ON (parent.owner_user_id,parent.project_id,parent.revision_id::text)=
        (r.owner_user_id,r.project_id,r.proposal_revision_ids[1])
      JOIN storyos.proposal_revisions AS restored ON (restored.owner_user_id,restored.project_id,restored.proposal_id,restored.parent_revision_id)=
        (parent.owner_user_id,parent.project_id,parent.proposal_id,parent.revision_id)
      WHERE (a.owner_user_id,a.project_id,a.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id)
        AND a.disposition='compensation' AND s.disposition='forward' AND r.command_kind='applyAuthorEdit' AND r.result_kind='proposal_revised'
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
CREATE CONSTRAINT TRIGGER draft_retry_receipt_complete AFTER INSERT ON storyos.domain_receipts DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_retry_settlement();
CREATE CONSTRAINT TRIGGER draft_retry_close_complete AFTER INSERT ON storyos.draft_close_events DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_retry_settlement();

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
