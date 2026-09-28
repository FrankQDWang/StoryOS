SET LOCAL ROLE storyos_owner;

DO $migration$
DECLARE item record; prior_check text;
BEGIN
  FOR item IN SELECT * FROM (VALUES
    ('domain_receipts','domain_receipts_command_kind_check',$p$command_kind='reopenWithdrawnProposal'$p$),
    ('author_command_admission_outcome_unknown_observations','author_command_admission_outcome_unknown_command_kind_check',$p$command_kind='reopenWithdrawnProposal'$p$),
    ('author_command_admissions','author_command_admissions_command_shape',$p$command_kind='reopenWithdrawnProposal'
      AND action_class='explicit_editor_command' AND editor_session_id IS NOT NULL AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL AND expected_authoritative_revision_id IS NOT NULL
      AND expected_proposal_head_revision_ids='{}' AND target_refs='{}'
      AND observed_ownership_partition IS NULL AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL AND challenge_expires_at IS NOT NULL$p$),
    ('domain_receipts','domain_receipts_common_shape',$p$command_kind='reopenWithdrawnProposal'
      AND cardinality(expected_heads)=1 AND cardinality(prior_heads)<=1 AND resulting_heads=prior_heads
      AND authoritative_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
      AND draft_artifact_refs='{}' AND artifact_lifecycle_event_refs='{}'
      AND ((result_kind='proposal_revised' AND expected_heads=prior_heads AND cardinality(proposal_revision_ids)=1
        AND array_position(proposal_revision_ids,NULL) IS NULL)
      OR (result_kind IN ('refused','conflicted','no_effect') AND proposal_revision_ids='{}'))$p$),
    ('domain_receipts','domain_receipts_result_shape',$p$command_kind='reopenWithdrawnProposal'
      AND jsonb_typeof(result_payload)='object'
      AND ((result_kind='proposal_revised' AND result_payload->>'transition'='reopen_withdrawn'
        AND result_payload-ARRAY['transition']='{}')
      OR (result_kind='conflicted' AND result_payload->>'reason'='changed_head'
        AND result_payload-ARRAY['reason']='{}')
      OR (result_kind='refused' AND result_payload->>'reason' IN (
        'wrong_scope','wrong_admission','stale_proposal_revision')
        AND result_payload-ARRAY['reason']='{}')
      OR (result_kind='no_effect' AND result_payload->>'reason' IN (
        'terminal_supersession','closure_not_withdrawn','withdrawal_event_mismatch')
        AND result_payload-ARRAY['reason']='{}'))$p$)
  ) AS entries(table_name,constraint_name,predicate) LOOP
    SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint WHERE
      conrelid=format('storyos.%I',item.table_name)::regclass AND conname=item.constraint_name AND contype='c';
    IF prior_check NOT LIKE 'CHECK (%)' THEN RAISE EXCEPTION 'Required settlement constraint is unavailable'; END IF;
    EXECUTE format('ALTER TABLE storyos.%I DROP CONSTRAINT %I',item.table_name,item.constraint_name);
    EXECUTE format('ALTER TABLE storyos.%I ADD CONSTRAINT %I CHECK ((%s) OR ((%s) IS TRUE))',
      item.table_name,item.constraint_name,substring(prior_check FROM 8 FOR length(prior_check)-8),item.predicate);
  END LOOP;
END $migration$;

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
  scoped_producer_cause text;
  activity_count bigint;
  payload_count bigint;
  action_count bigint;
  commit_count bigint;
  revision_envelope_count bigint;
  archival_count bigint;
  payload_event_kind text;
BEGIN
  SELECT receipt.result_kind, receipt.command_kind, receipt.result_payload->>'reason',
         receipt.producer_cause
    INTO scoped_result_kind, scoped_command_kind, scoped_reason, scoped_producer_cause
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
  ELSIF scoped_command_kind = 'replanProposal'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'replanProposal resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'replanProposal'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'replanProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'withdrawProposal'
        AND scoped_result_kind = 'proposal_closure_changed'
        AND scoped_producer_cause = 'author_command_admission' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'withdrawProposal resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'withdrawProposal'
        AND scoped_result_kind = 'proposal_closure_changed'
        AND scoped_producer_cause = 'agent_run_decision' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'withdrawProposal current-producer resolved requires zero Author Actions and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'withdrawProposal'
        AND scoped_result_kind IN ('conflicted', 'refused', 'no_effect') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'withdrawProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'reopenWithdrawnProposal'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenWithdrawnProposal resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'reopenWithdrawnProposal'
        AND scoped_result_kind IN ('conflicted', 'refused', 'no_effect') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenWithdrawnProposal with zero effect cannot have an authority or Activity relation';
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
