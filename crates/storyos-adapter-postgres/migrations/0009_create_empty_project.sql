SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.projects
  ALTER COLUMN current_chapter_id DROP NOT NULL;

DO $drop_current_chapter_fk$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.projects'::regclass
       AND contype = 'f'
       AND pg_get_constraintdef(oid) LIKE '%current_chapter_id%'
  LOOP
    EXECUTE format('ALTER TABLE storyos.projects DROP CONSTRAINT %I', constraint_name);
  END LOOP;
END
$drop_current_chapter_fk$;

-- FORCE RLS blocks FK validation as storyos_owner when GUC values are unset.
RESET ROLE;
ALTER TABLE storyos.projects
  ADD CONSTRAINT projects_current_chapter_fk
  FOREIGN KEY (owner_user_id, project_id, current_chapter_id)
  REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id)
  MATCH SIMPLE
  DEFERRABLE INITIALLY DEFERRED;

SET LOCAL ROLE storyos_owner;
GRANT INSERT ON storyos.projects TO storyos_runtime;

ALTER TABLE storyos.create_project_idempotency
  ADD COLUMN outcome_kind text NOT NULL DEFAULT 'pending'
    CHECK (outcome_kind IN ('pending', 'in_progress', 'settled')),
  ADD COLUMN result_reference text;

ALTER TABLE storyos.create_project_challenges
  ADD COLUMN consumed_at timestamptz;

ALTER TABLE storyos.author_command_admissions
  ALTER COLUMN editor_session_id DROP NOT NULL,
  ALTER COLUMN writer_generation DROP NOT NULL,
  ALTER COLUMN challenge_consumed_at DROP NOT NULL,
  ALTER COLUMN challenge_expires_at DROP NOT NULL;

DO $drop_admission_optional_fks$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.author_command_admissions'::regclass
       AND contype = 'f'
       AND (
         pg_get_constraintdef(oid) LIKE '%editor_sessions%'
         OR pg_get_constraintdef(oid) LIKE '%project_writer_generations%'
         OR pg_get_constraintdef(oid) LIKE '%project_command_challenges%'
       )
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.author_command_admissions DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_admission_optional_fks$;

RESET ROLE;
ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_editor_session_fkey
  FOREIGN KEY (owner_user_id, project_id, editor_session_id)
  REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id)
  MATCH SIMPLE;

ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_writer_generation_fkey
  FOREIGN KEY (owner_user_id, project_id, writer_generation)
  REFERENCES storyos.project_writer_generations(owner_user_id, project_id, writer_generation)
  MATCH SIMPLE;

ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_project_command_challenge_fkey
  FOREIGN KEY (
    owner_user_id, project_id, command_kind, idempotency_key,
    challenge_consumed_at, challenge_expires_at
  ) REFERENCES storyos.project_command_challenges (
    owner_user_id, project_id, command_kind, idempotency_key, consumed_at, expires_at
  ) MATCH SIMPLE;

SET LOCAL ROLE storyos_owner;

DO $drop_admission_shape$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.author_command_admissions'::regclass
       AND contype = 'c'
       AND pg_get_constraintdef(oid) LIKE '%command_kind%'
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.author_command_admissions DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_admission_shape$;

ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_command_shape CHECK ((
    (command_kind = 'applyAuthorEdit'
      AND action_class = 'direct_editor_action'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL
      AND expected_authoritative_revision_id IS NOT NULL
      AND observed_ownership_partition = 'authoritative'
      AND undo_group_id IS NOT NULL
      AND completed_intent_record_id IS NOT NULL
      AND local_intent_sequence IS NOT NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'takeOverProjectWriter'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'createProject'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NULL
      AND challenge_expires_at IS NULL)
  ) IS TRUE);

CREATE FUNCTION storyos.require_create_project_challenge_admission()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
BEGIN
  IF NEW.command_kind <> 'createProject' THEN
    RETURN NEW;
  END IF;
  IF NOT EXISTS (
    SELECT 1
      FROM storyos.create_project_challenges AS challenge
     WHERE challenge.user_id = NEW.owner_user_id
       AND challenge.idempotency_key = NEW.idempotency_key
       AND challenge.prospective_project_id = NEW.project_id
       AND challenge.canonical_command_digest = NEW.canonical_command_digest
       AND challenge.consumed_at IS NOT NULL
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'createProject Admission requires a consumed User-level challenge';
  END IF;
  RETURN NEW;
END
$function$;

CREATE TRIGGER author_command_admissions_create_project_challenge
BEFORE INSERT ON storyos.author_command_admissions
FOR EACH ROW EXECUTE FUNCTION storyos.require_create_project_challenge_admission();

DROP TRIGGER author_command_admissions_writer_generation ON storyos.author_command_admissions;
CREATE TRIGGER author_command_admissions_writer_generation
BEFORE INSERT ON storyos.author_command_admissions
FOR EACH ROW
WHEN (NEW.command_kind <> 'createProject')
EXECUTE FUNCTION storyos.require_writer_generation_admission();

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_command_kind_check;

ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_command_kind_check
  CHECK (command_kind IN ('applyAuthorEdit', 'takeOverProjectWriter', 'createProject'));

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_common_shape;

ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_common_shape CHECK ((
    cardinality(proposal_revision_ids) = 0
    AND cardinality(draft_artifact_refs) = 0
    AND cardinality(artifact_lifecycle_event_refs) = 0
    AND cardinality(condition_refs) = 0
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
      (command_kind <> 'createProject'
        AND cardinality(expected_heads) = 1
        AND cardinality(prior_heads) = 1
        AND cardinality(resulting_heads) = 1
        AND array_dims(expected_heads) = '[1:1]'
        AND array_dims(prior_heads) = '[1:1]'
        AND array_dims(resulting_heads) = '[1:1]')
      OR (command_kind = 'createProject'
        AND cardinality(expected_heads) = 0
        AND cardinality(prior_heads) = 0
        AND cardinality(resulting_heads) = 0)
    )
  ) IS TRUE);

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_result_shape;

ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_result_shape CHECK ((
    (command_kind = 'applyAuthorEdit'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 1
      AND cardinality(authoritative_commit_ids) = 1
      AND array_dims(authoritative_revision_ids) = '[1:1]'
      AND array_dims(authoritative_commit_ids) = '[1:1]'
      AND resulting_heads = authoritative_revision_ids)
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'content_unchanged'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'takeOverProjectWriter'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'writer_takeover_applied',
        'writer_takeover_compare_failed'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'createProject'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload ? 'current_authoritative_revision_id'
      AND result_payload - 'reason' - 'current_authoritative_revision_id' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND jsonb_typeof(result_payload->'current_authoritative_revision_id') = 'string'
      AND result_payload->>'reason' IN (
        'stale_authoritative_head', 'proposal_head_present', 'ownership_changed'
      )
      AND result_payload->>'current_authoritative_revision_id' = resulting_heads[1]::text
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND (result_payload->>'reason' <> 'stale_authoritative_head'
        OR expected_heads <> resulting_heads))
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND result_payload->>'reason' IN (
        'unsupported_intent_shape', 'invalid_selection', 'target_mismatch'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
  ) IS TRUE);

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  DROP CONSTRAINT author_command_admission_outcome_unknown_command_kind_check;

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  ADD CONSTRAINT author_command_admission_outcome_unknown_command_kind_check
  CHECK (command_kind IN ('applyAuthorEdit', 'takeOverProjectWriter', 'createProject'));

ALTER TABLE storyos.project_activity_event_payloads
  DROP CONSTRAINT project_activity_event_payloads_event_kind_check;

ALTER TABLE storyos.project_activity_event_payloads
  ADD CONSTRAINT project_activity_event_payloads_event_kind_check
  CHECK (event_kind IN (
    'writer_takeover_applied', 'writer_takeover_compare_failed', 'project_created'
  ));

ALTER TABLE storyos.project_activity_event_payloads
  DROP CONSTRAINT project_activity_event_payloads_receipt_result_kind_check;

ALTER TABLE storyos.project_activity_event_payloads
  ADD CONSTRAINT project_activity_event_payloads_receipt_result_kind_check
  CHECK (receipt_result_kind IN ('no_effect', 'authoritative_applied'));

ALTER TABLE storyos.project_activity_event_payloads
  DROP CONSTRAINT project_activity_event_payloads_shape;

ALTER TABLE storyos.project_activity_event_payloads
  ADD CONSTRAINT project_activity_event_payloads_shape CHECK ((
    jsonb_typeof(payload) = 'object'
    AND (
      (event_kind = 'writer_takeover_applied'
        AND receipt_result_kind = 'no_effect'
        AND payload->>'kind' = 'takeover_applied'
        AND jsonb_typeof(payload->'prior_editor_session_id') = 'string'
        AND jsonb_typeof(payload->'prior_writer_generation') = 'string'
        AND jsonb_typeof(payload->'resulting_editor_session_id') = 'string'
        AND jsonb_typeof(payload->'resulting_writer_generation') = 'string'
        AND jsonb_typeof(payload->'resulting_snapshot_id') = 'string'
        AND jsonb_typeof(payload->'resulting_snapshot_activity_position') = 'string'
        AND jsonb_typeof(payload->'resulting_heads') = 'array'
        AND payload - 'kind' - 'prior_editor_session_id' - 'prior_writer_generation'
          - 'resulting_editor_session_id' - 'resulting_writer_generation'
          - 'resulting_snapshot_id' - 'resulting_snapshot_activity_position'
          - 'resulting_heads' = '{}'::jsonb)
      OR (event_kind = 'writer_takeover_compare_failed'
        AND receipt_result_kind = 'no_effect'
        AND payload->>'kind' = 'takeover_compare_failed'
        AND jsonb_typeof(payload->'observed_writer_generation') = 'string'
        AND jsonb_typeof(payload->'current_writer_generation') = 'string'
        AND jsonb_typeof(payload->'current_writer_projection') = 'object'
        AND jsonb_typeof(payload->'current_snapshot_id') = 'string'
        AND jsonb_typeof(payload->'current_snapshot_activity_position') = 'string'
        AND jsonb_typeof(payload->'current_heads') = 'array'
        AND jsonb_typeof(payload->'reason') = 'string'
        AND payload->>'reason' IN (
          'writer_generation_advanced_after_admission',
          'requester_became_current_after_admission'
        )
        AND payload - 'kind' - 'observed_writer_generation' - 'current_writer_generation'
          - 'current_writer_projection' - 'current_snapshot_id'
          - 'current_snapshot_activity_position' - 'current_heads' - 'reason' = '{}'::jsonb)
      OR (event_kind = 'project_created'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'project_created'
        AND payload->>'open_kind' = 'empty'
        AND jsonb_typeof(payload->'title') = 'string'
        AND payload - 'kind' - 'open_kind' - 'title' = '{}'::jsonb)
    )
  ) IS TRUE);

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

  IF scoped_command_kind = 'takeOverProjectWriter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'no_effect'
       OR (activity_count, action_count, commit_count, revision_envelope_count, payload_count)
          <> (0, 0, 0, 0, 1)
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
       OR (activity_count, action_count, commit_count, revision_envelope_count, payload_count)
          <> (0, 0, 0, 0, 1)
       OR payload_event_kind IS DISTINCT FROM 'project_created' THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createProject requires one project-created Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_result_kind = 'authoritative_applied' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count, payload_count)
         <> (1, 1, 1, 1, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'AuthoritativeApplied requires one complete authority and Activity relation';
    END IF;
  ELSIF (activity_count, action_count, commit_count, revision_envelope_count, payload_count)
          <> (0, 0, 0, 0, 0) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'A zero-authority Receipt cannot have an authority or Activity relation';
  END IF;
  RETURN NULL;
END
$function$;
