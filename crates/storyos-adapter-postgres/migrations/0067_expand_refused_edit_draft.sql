SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.proposals ALTER COLUMN source_run_id DROP NOT NULL;
ALTER TABLE storyos.proposals ALTER COLUMN source_decision_id DROP NOT NULL;
ALTER TABLE storyos.proposals ADD COLUMN source_draft_id uuid;
ALTER TABLE storyos.proposals ADD COLUMN source_draft_revision_id uuid;
ALTER TABLE storyos.proposals ADD COLUMN source_draft_payload_digest text;
ALTER TABLE storyos.proposal_revisions ADD COLUMN candidate_blocks jsonb;
ALTER TABLE storyos.proposal_operations ADD COLUMN candidate_blocks jsonb;
ALTER TABLE storyos.draft_artifact_revisions ADD UNIQUE(owner_user_id,project_id,draft_id,revision_id,payload_digest);
DO $migration$
DECLARE constraint_name text;
BEGIN
  SELECT conname INTO STRICT constraint_name FROM pg_constraint WHERE conrelid='storyos.proposals'::regclass
    AND contype='f' AND confrelid='storyos.agent_runs'::regclass;
  EXECUTE format('ALTER TABLE storyos.proposals DROP CONSTRAINT %I',constraint_name);
END $migration$;
ALTER TABLE storyos.proposals ADD FOREIGN KEY(owner_user_id,project_id,source_run_id)
  REFERENCES storyos.agent_runs(owner_user_id,project_id,run_id);
ALTER TABLE storyos.proposals ADD FOREIGN KEY(owner_user_id,project_id,source_draft_id,source_draft_revision_id,source_draft_payload_digest)
  REFERENCES storyos.draft_artifact_revisions(owner_user_id,project_id,draft_id,revision_id,payload_digest);
ALTER TABLE storyos.proposals ADD CONSTRAINT proposal_source_shape CHECK ((
  (source_run_id IS NOT NULL AND source_decision_id IS NOT NULL AND source_draft_id IS NULL
    AND source_draft_revision_id IS NULL AND source_draft_payload_digest IS NULL)
  OR (source_run_id IS NULL AND source_decision_id IS NULL AND source_draft_id IS NOT NULL
    AND source_draft_revision_id IS NOT NULL AND source_draft_payload_digest ~ '^[0-9a-f]{64}$')
) IS TRUE);
ALTER TABLE storyos.proposal_revisions ADD CHECK (candidate_blocks IS NULL OR
  (jsonb_typeof(candidate_blocks)='array' AND jsonb_array_length(candidate_blocks)>0));
DO $migration$
DECLARE item record; prior_check text;
BEGIN
  FOR item IN SELECT * FROM (VALUES
    ('domain_receipts','domain_receipts_command_kind_check',$p$command_kind='expandRefusedEditDraftToProposal'$p$),
    ('domain_receipts','domain_receipts_result_kind_check',$p$result_kind='proposal_created_from_draft'$p$),
    ('author_action_entries','author_action_entries_receipt_result_kind_check',$p$receipt_result_kind='proposal_created_from_draft'$p$),
    ('domain_receipts','draft_retry_disposition_shape',$p$command_kind='expandRefusedEditDraftToProposal' AND source_draft_disposition->>'kind'='closed_superseded'$p$),
    ('draft_close_events','draft_close_reason_shape',$p$close_reason='superseded' AND receipt_result_kind='proposal_created_from_draft' AND author_action_sequence IS NOT NULL$p$),
    ('author_command_admissions','author_command_admissions_command_shape',$p$command_kind='expandRefusedEditDraftToProposal'
      AND action_class='explicit_editor_command' AND editor_session_id IS NOT NULL AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL AND expected_authoritative_revision_id IS NOT NULL AND cardinality(target_refs)=1
      AND expected_proposal_head_revision_ids='{}' AND observed_ownership_partition IS NULL AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL AND local_intent_sequence IS NULL AND challenge_consumed_at IS NOT NULL AND challenge_expires_at IS NOT NULL$p$),
    ('domain_receipts','domain_receipts_common_shape',$p$command_kind='expandRefusedEditDraftToProposal'
      AND cardinality(expected_heads)=1 AND cardinality(prior_heads)<=1 AND resulting_heads=prior_heads
      AND authoritative_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
      AND cardinality(draft_artifact_refs)=1 AND array_position(draft_artifact_refs,NULL) IS NULL
      AND ((result_kind='proposal_created_from_draft' AND expected_heads=prior_heads AND cardinality(proposal_revision_ids)=1
        AND cardinality(artifact_lifecycle_event_refs)=1 AND array_position(proposal_revision_ids,NULL) IS NULL
        AND array_position(artifact_lifecycle_event_refs,NULL) IS NULL)
      OR (result_kind IN ('refused','conflicted') AND proposal_revision_ids='{}' AND artifact_lifecycle_event_refs='{}'))$p$),
    ('domain_receipts','domain_receipts_result_shape',$p$command_kind='expandRefusedEditDraftToProposal'
      AND jsonb_typeof(result_payload)='object' AND result_payload ?& ARRAY['draft_revision_id','payload_digest','observed_closure',
        'current_target_revision_id','reason','event_id','proposal_id','proposal_revision_id']
      AND result_payload-ARRAY['draft_revision_id','payload_digest','observed_closure','current_target_revision_id','reason','event_id','proposal_id','proposal_revision_id']='{}'
      AND result_payload->>'observed_closure' IN ('open','closed') AND result_payload->>'payload_digest' ~ '^[0-9a-f]{64}$'
      AND ((result_kind='proposal_created_from_draft' AND result_payload->>'reason'='superseded')
        OR (result_kind='conflicted' AND result_payload->>'reason'='source_or_target_changed')
        OR (result_kind='refused' AND result_payload->>'reason' IN ('source_draft_not_open','source_unavailable','unsupported_payload','target_unavailable')))$p$)
  ) AS entries(table_name,constraint_name,predicate) LOOP
    SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint WHERE
      conrelid=format('storyos.%I',item.table_name)::regclass AND conname=item.constraint_name AND contype='c';
    IF prior_check NOT LIKE 'CHECK (%)' THEN RAISE EXCEPTION 'Required settlement constraint is unavailable'; END IF;
    EXECUTE format('ALTER TABLE storyos.%I DROP CONSTRAINT %I',item.table_name,item.constraint_name);
    EXECUTE format('ALTER TABLE storyos.%I ADD CONSTRAINT %I CHECK ((%s) OR ((%s) IS TRUE))',
      item.table_name,item.constraint_name,substring(prior_check FROM 8 FOR length(prior_check)-8),item.predicate);
  END LOOP;
END $migration$;

CREATE FUNCTION storyos.require_draft_expansion_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE receipt storyos.domain_receipts;
BEGIN
  IF TG_TABLE_NAME='proposals' THEN
    IF NEW.source_draft_id IS NULL THEN RETURN NULL; END IF;
    SELECT * INTO STRICT receipt FROM storyos.domain_receipts AS r WHERE
      (r.owner_user_id,r.project_id,r.result_payload->>'proposal_id')=(NEW.owner_user_id,NEW.project_id,NEW.proposal_id::text)
      AND r.command_kind='expandRefusedEditDraftToProposal' AND r.result_kind='proposal_created_from_draft';
  ELSE receipt:=NEW;
    IF receipt.command_kind<>'expandRefusedEditDraftToProposal' OR receipt.result_kind<>'proposal_created_from_draft' THEN RETURN NULL; END IF;
  END IF;
  IF NOT EXISTS(SELECT 1 FROM storyos.proposals AS proposal
    JOIN storyos.proposal_revisions AS candidate USING(owner_user_id,project_id,proposal_id)
    JOIN storyos.proposal_heads AS head USING(owner_user_id,project_id,proposal_id)
    JOIN storyos.proposal_operations AS operation USING(owner_user_id,project_id,proposal_id)
    JOIN storyos.draft_artifact_revisions AS source ON
      (source.owner_user_id,source.project_id,source.draft_id,source.revision_id,source.payload_digest)=
      (proposal.owner_user_id,proposal.project_id,proposal.source_draft_id,proposal.source_draft_revision_id,proposal.source_draft_payload_digest)
    JOIN storyos.draft_artifacts AS draft ON
      (draft.owner_user_id,draft.project_id,draft.draft_id)=(source.owner_user_id,source.project_id,source.draft_id)
    JOIN storyos.draft_close_events AS closed ON
      (closed.owner_user_id,closed.project_id,closed.draft_id)=(draft.owner_user_id,draft.project_id,draft.draft_id)
    JOIN storyos.author_action_entries AS action ON
      (action.owner_user_id,action.project_id,action.receipt_id)=(closed.owner_user_id,closed.project_id,closed.receipt_id)
    JOIN storyos.author_command_admissions AS admission ON
      (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=(receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
    WHERE proposal.owner_user_id=receipt.owner_user_id AND proposal.project_id=receipt.project_id
      AND proposal.proposal_id::text=receipt.result_payload->>'proposal_id' AND candidate.revision_id=head.current_revision_id
      AND receipt.proposal_revision_ids=ARRAY[candidate.revision_id] AND candidate.validation='pending' AND candidate.closure='open'
      AND candidate.candidate_blocks=jsonb_path_query_array(source.payload,'$.author_edit_units[*].normalized_primitives[*].replacement[*]')
      AND operation.candidate_blocks=candidate.candidate_blocks AND operation.candidate_text=candidate.candidate_text
      AND candidate.candidate_text=(SELECT string_agg(block->>'text',E'\n' ORDER BY position)
        FROM jsonb_array_elements(candidate.candidate_blocks) WITH ORDINALITY AS blocks(block,position))
      AND draft.closure='closed' AND draft.retention_state='retained' AND draft.current_revision_id=source.revision_id
      AND draft.close_event_id=closed.event_id AND closed.revision_id=source.revision_id AND closed.receipt_id=receipt.receipt_id
      AND receipt.artifact_lifecycle_event_refs=ARRAY[closed.event_id::text] AND receipt.draft_artifact_refs=ARRAY[draft.draft_id::text]
      AND closed.close_reason='superseded' AND action.disposition='forward' AND action.author_action_sequence=closed.author_action_sequence
      AND admission.command_payload->'expand_refused_edit_draft_to_proposal_input'->>'source_current_draft_revision_id'=source.revision_id::text
      AND admission.command_payload->'expand_refused_edit_draft_to_proposal_input'->>'source_draft_payload_digest'=source.payload_digest
      AND EXISTS(SELECT 1 FROM storyos.author_command_admission_settlements AS settled WHERE
        (settled.owner_user_id,settled.project_id,settled.author_command_admission_id,settled.receipt_id)=
        (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id,receipt.receipt_id) AND settled.settlement_kind='receipt_settled')
      AND EXISTS(SELECT 1 FROM storyos.command_idempotency AS replay WHERE
        (replay.owner_user_id,replay.project_id,replay.command_kind,replay.idempotency_key)=
        (receipt.owner_user_id,receipt.project_id,receipt.command_kind,receipt.idempotency_key)
        AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text))
  THEN RAISE EXCEPTION 'Incomplete whole Draft expansion settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;
CREATE CONSTRAINT TRIGGER draft_expansion_receipt_complete AFTER INSERT ON storyos.domain_receipts DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_expansion_settlement();
CREATE CONSTRAINT TRIGGER draft_expansion_proposal_complete AFTER INSERT ON storyos.proposals DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_expansion_settlement();

CREATE OR REPLACE FUNCTION storyos.require_author_edit_receipt_relation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
DECLARE
  scoped_owner_user_id uuid := COALESCE(NEW.owner_user_id, OLD.owner_user_id);
  scoped_project_id uuid := COALESCE(NEW.project_id, OLD.project_id);
  scoped_receipt_id uuid := COALESCE(NEW.receipt_id, OLD.receipt_id);
  scoped_result_kind text;
  scoped_command_kind text;
  scoped_reason text;
  activity_count bigint;
  payload_count bigint;
  action_count bigint;
  commit_count bigint;
  revision_envelope_count bigint;
  archival_count bigint;
  payload_event_kind text;
BEGIN
  SELECT receipt.result_kind, receipt.command_kind, receipt.result_payload->>'reason'
    INTO scoped_result_kind, scoped_command_kind, scoped_reason
    FROM storyos.domain_receipts AS receipt
   WHERE receipt.owner_user_id = scoped_owner_user_id
     AND receipt.project_id = scoped_project_id
     AND receipt.receipt_id = scoped_receipt_id;
  IF NOT FOUND THEN
    RETURN NULL;
  END IF;

  SELECT count(*) INTO activity_count
    FROM storyos.project_activity_events AS activity
   WHERE activity.owner_user_id = scoped_owner_user_id
     AND activity.project_id = scoped_project_id
     AND activity.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO payload_count
    FROM storyos.project_activity_event_payloads AS payload
   WHERE payload.owner_user_id = scoped_owner_user_id
     AND payload.project_id = scoped_project_id
     AND payload.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO action_count
    FROM storyos.author_action_entries AS action
   WHERE action.owner_user_id = scoped_owner_user_id
     AND action.project_id = scoped_project_id
     AND action.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO commit_count
    FROM storyos.authoritative_commits AS authoritative_commit
   WHERE authoritative_commit.owner_user_id = scoped_owner_user_id
     AND authoritative_commit.project_id = scoped_project_id
     AND authoritative_commit.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO revision_envelope_count
    FROM storyos.authoritative_revision_envelopes AS envelope
   WHERE envelope.owner_user_id = scoped_owner_user_id
     AND envelope.project_id = scoped_project_id
     AND envelope.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO archival_count
    FROM storyos.project_archival_decisions AS archival
   WHERE archival.owner_user_id = scoped_owner_user_id
     AND archival.project_id = scoped_project_id
     AND archival.receipt_id = scoped_receipt_id;

  IF scoped_command_kind = 'takeOverProjectWriter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'no_effect'
       OR (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
          <> (0, 0, 0, 0, 1, 0)
       OR payload_event_kind IS DISTINCT FROM scoped_reason THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'takeOverProjectWriter requires one takeover Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_command_kind = 'createProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'authoritative_applied'
       OR (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
          <> (0, 0, 0, 0, 1, 0)
       OR payload_event_kind IS DISTINCT FROM 'project_created' THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createProject requires one project-created Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_command_kind = 'updateProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'project_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateProject applied requires one project-updated Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateProject with zero title effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'archiveProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 1)
         OR payload_event_kind IS DISTINCT FROM 'project_archival_changed' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'archiveProject applied requires one archival decision, Activity, and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'archiveProject with zero lifecycle effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'createVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createVolume applied requires one volume-created Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'updateVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateVolume applied requires one volume-updated Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'updateChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateChapter applied requires one chapter-updated Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'setCurrentChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR commit_count <> 0
         OR revision_envelope_count <> 0
         OR action_count NOT IN (0, 1)
         OR payload_event_kind IS DISTINCT FROM 'current_chapter_set' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'setCurrentChapter applied requires one current-chapter-set Activity, no Commit, and at most one Author Action';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'setCurrentChapter with zero current-chapter effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'createChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createChapter applied requires one chapter-created Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'deleteChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteChapter applied requires one chapter-deleted Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'deleteVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteVolume applied requires one volume-deleted Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'exportHumanReadableManuscript' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'human_readable_manuscript_export_settled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'exportHumanReadableManuscript applied requires one export-settled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'exportHumanReadableManuscript with zero export effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'exportProjectArchive' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'project_export_settled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'exportProjectArchive applied requires one export-settled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'exportProjectArchive with zero export effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'undoLatestAuthorAction' THEN
    IF scoped_result_kind = 'draft_closure_changed' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count, payload_count, archival_count) <> (0, 1, 0, 0, 0, 0) THEN
        RAISE EXCEPTION 'Draft reopen requires one compensation and zero manuscript authority' USING ERRCODE='23514';
      END IF;
    ELSIF scoped_result_kind = 'authoritative_applied' THEN
      IF NOT (
        (activity_count, action_count, commit_count, revision_envelope_count,
         payload_count, archival_count) = (1, 1, 1, 1, 0, 0)
        OR (activity_count, action_count, commit_count, revision_envelope_count,
            payload_count, archival_count) = (0, 1, 1, 0, 0, 0)
        OR (activity_count, action_count, commit_count, revision_envelope_count,
            payload_count, archival_count) = (0, 1, 0, 0, 0, 0)
      ) THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'undoLatestAuthorAction applied requires prose, structure, or Current Chapter compensation authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'undoLatestAuthorAction with zero authority cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'updateProjectAssistance' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'project_assistance_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateProjectAssistance applied requires one assistance Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateProjectAssistance with zero availability effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'createAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createAgentRun applied requires one AgentRun Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createAgentRun with zero admission effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'pauseAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_paused' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'pauseAgentRun applied requires one paused Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'pauseAgentRun with zero transition cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'cancelAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_cancelled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'cancelAgentRun applied requires one cancelled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'cancelAgentRun with zero transition cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'rejectProposalOperations'
        AND scoped_result_kind = 'proposal_operations_resolved' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'rejectProposalOperations resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'rejectProposalOperations'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'rejectProposalOperations with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'reopenRejectedOperations'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenRejectedOperations resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'reopenRejectedOperations'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenRejectedOperations with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'completeReadyPartialProposal'
        AND scoped_result_kind = 'proposal_generation_completed' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'completeReadyPartialProposal completed requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'completeReadyPartialProposal'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'completeReadyPartialProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'continueProposalGeneration'
        AND scoped_result_kind = 'proposal_generation_started' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'continueProposalGeneration started requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'continueProposalGeneration'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'continueProposalGeneration with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'applyAuthorEdit'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'applyAuthorEdit ProposalRevised requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_result_kind = 'authoritative_applied' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (1, 1, 1, 1, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'AuthoritativeApplied requires one complete authority and Activity relation';
    END IF;
  ELSIF (scoped_command_kind = 'closeEditorFlowDraft' AND scoped_result_kind = 'draft_closure_changed')
     OR (scoped_command_kind = 'expandRefusedEditDraftToProposal' AND scoped_result_kind = 'proposal_created_from_draft') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'DraftClosureChanged requires one Author Action and zero authority';
    END IF;
  ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
         payload_count, archival_count)
          <> (0, 0, 0, 0, 0, 0) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'A zero-authority Receipt cannot have an authority or Activity relation';
  END IF;
  RETURN NULL;
END
$function$;


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
          JOIN storyos.proposal_heads AS head USING(owner_user_id,project_id,proposal_id)
          JOIN storyos.proposal_revisions AS candidate ON (candidate.owner_user_id,candidate.project_id,candidate.proposal_id,candidate.revision_id)=
            (head.owner_user_id,head.project_id,head.proposal_id,head.current_revision_id)
          WHERE (source.owner_user_id,source.project_id,source.receipt_id)=(closed.owner_user_id,closed.project_id,closed.receipt_id)
            AND source.command_kind='expandRefusedEditDraftToProposal' AND source.result_kind='proposal_created_from_draft'
            AND source.proposal_revision_ids=ARRAY[candidate.revision_id] AND candidate.closure='withdrawn' AND candidate.validation='pending'
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
