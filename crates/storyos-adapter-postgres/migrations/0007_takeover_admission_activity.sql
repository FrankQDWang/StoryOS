SET LOCAL ROLE storyos_owner;

DO $drop_writer_pk$
DECLARE constraint_name text;
BEGIN
  SELECT conname INTO constraint_name
    FROM pg_constraint
   WHERE conrelid = 'storyos.project_writer_generations'::regclass
     AND contype = 'p';
  EXECUTE format(
    'ALTER TABLE storyos.project_writer_generations DROP CONSTRAINT %I',
    constraint_name
  );
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.project_writer_generations'::regclass
       AND contype = 'u'
       AND pg_get_constraintdef(oid) = 'UNIQUE (owner_user_id, project_id, writer_generation)'
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.project_writer_generations DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_writer_pk$;

ALTER TABLE storyos.project_writer_generations
  ADD CONSTRAINT project_writer_generations_pkey
  PRIMARY KEY (owner_user_id, project_id, writer_generation);

DO $drop_admission_writer_fk$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.author_command_admissions'::regclass
       AND contype = 'f'
       AND pg_get_constraintdef(oid) LIKE '%current_editor_session_id%'
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.author_command_admissions DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_admission_writer_fk$;

-- FORCE RLS blocks FK validation as storyos_owner when storyos.owner_user_id is unset.
RESET ROLE;
ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_writer_generation_fkey
  FOREIGN KEY (owner_user_id, project_id, writer_generation)
  REFERENCES storyos.project_writer_generations
    (owner_user_id, project_id, writer_generation) MATCH FULL;

DO $drop_admission_checks$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.author_command_admissions'::regclass
       AND contype = 'c'
       AND (
         pg_get_constraintdef(oid) LIKE '%applyAuthorEdit%'
         OR pg_get_constraintdef(oid) LIKE '%direct_editor_action%'
       )
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.author_command_admissions DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_admission_checks$;

ALTER TABLE storyos.author_command_admissions
  ALTER COLUMN chapter_object_id DROP NOT NULL,
  ALTER COLUMN expected_authoritative_revision_id DROP NOT NULL,
  ALTER COLUMN observed_ownership_partition DROP NOT NULL,
  ALTER COLUMN undo_group_id DROP NOT NULL,
  ALTER COLUMN completed_intent_record_id DROP NOT NULL,
  ALTER COLUMN local_intent_sequence DROP NOT NULL;

ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_command_shape CHECK ((
    (command_kind = 'applyAuthorEdit'
      AND action_class = 'direct_editor_action'
      AND chapter_object_id IS NOT NULL
      AND expected_authoritative_revision_id IS NOT NULL
      AND observed_ownership_partition = 'authoritative'
      AND undo_group_id IS NOT NULL
      AND completed_intent_record_id IS NOT NULL
      AND local_intent_sequence IS NOT NULL)
    OR (command_kind = 'takeOverProjectWriter'
      AND action_class = 'explicit_editor_command'
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL)
  ) IS TRUE);

-- MATCH FULL cannot mix required Scope with omitted takeover manuscript columns.
DO $drop_admission_chapter_fk$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.author_command_admissions'::regclass
       AND contype = 'f'
       AND pg_get_constraintdef(oid) LIKE '%chapter_object_id%'
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.author_command_admissions DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_admission_chapter_fk$;

SET LOCAL ROLE storyos_owner;
CREATE FUNCTION storyos.require_writer_generation_admission()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
DECLARE
  current_generation numeric(20, 0);
  current_session uuid;
BEGIN
  SELECT writer.writer_generation, writer.current_editor_session_id
    INTO current_generation, current_session
    FROM storyos.project_writer_generations AS writer
   WHERE writer.owner_user_id = NEW.owner_user_id
     AND writer.project_id = NEW.project_id
   ORDER BY writer.writer_generation DESC
   LIMIT 1;
  IF NEW.command_kind = 'applyAuthorEdit' THEN
    IF NEW.writer_generation IS DISTINCT FROM current_generation
       OR NEW.editor_session_id IS DISTINCT FROM current_session THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'ApplyAuthorEdit Admission requires the current writer session and generation';
    END IF;
    IF NOT EXISTS (
      SELECT 1
        FROM storyos.authoritative_revisions AS revision
       WHERE revision.owner_user_id = NEW.owner_user_id
         AND revision.project_id = NEW.project_id
         AND revision.manuscript_object_id = NEW.chapter_object_id
         AND revision.revision_id = NEW.expected_authoritative_revision_id
    ) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23503',
        MESSAGE = 'ApplyAuthorEdit Admission requires an exact authoritative revision';
    END IF;
  ELSIF NEW.command_kind = 'takeOverProjectWriter' THEN
    IF NEW.writer_generation IS DISTINCT FROM current_generation
       OR NEW.editor_session_id IS NOT DISTINCT FROM current_session THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'takeOverProjectWriter Admission requires an observer of the current generation';
    END IF;
  END IF;
  RETURN NEW;
END
$function$;

CREATE TRIGGER author_command_admissions_writer_generation
BEFORE INSERT ON storyos.author_command_admissions
FOR EACH ROW EXECUTE FUNCTION storyos.require_writer_generation_admission();

CREATE INDEX author_command_admissions_writer_generation_history_fk_idx
  ON storyos.author_command_admissions
    (owner_user_id, project_id, writer_generation);

RESET ROLE;
DO $drop_receipt_command_kind$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.domain_receipts'::regclass
       AND contype = 'c'
       AND pg_get_constraintdef(oid) LIKE '%applyAuthorEdit%'
  LOOP
    EXECUTE format('ALTER TABLE storyos.domain_receipts DROP CONSTRAINT %I', constraint_name);
  END LOOP;
END
$drop_receipt_command_kind$;

ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_command_kind_check
  CHECK (command_kind IN ('applyAuthorEdit', 'takeOverProjectWriter'));

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

DO $drop_unknown_command_kind$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'storyos.author_command_admission_outcome_unknown_observations'::regclass
       AND contype = 'c'
       AND pg_get_constraintdef(oid) LIKE '%applyAuthorEdit%'
  LOOP
    EXECUTE format(
      'ALTER TABLE storyos.author_command_admission_outcome_unknown_observations DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END
$drop_unknown_command_kind$;

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  ADD CONSTRAINT author_command_admission_outcome_unknown_command_kind_check
  CHECK (command_kind IN ('applyAuthorEdit', 'takeOverProjectWriter'));

SET LOCAL ROLE storyos_owner;
CREATE TABLE storyos.project_activity_event_payloads (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  project_activity_position numeric(20, 0) NOT NULL
    CHECK (project_activity_position BETWEEN 1 AND 18446744073709551615),
  project_activity_event_id uuid NOT NULL,
  event_kind text NOT NULL CHECK (event_kind IN (
    'writer_takeover_applied', 'writer_takeover_compare_failed'
  )),
  receipt_id uuid NOT NULL,
  receipt_result_kind text NOT NULL CHECK (receipt_result_kind = 'no_effect'),
  payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, project_activity_position),
  UNIQUE (owner_user_id, project_id, project_activity_event_id),
  UNIQUE (owner_user_id, project_id, receipt_id),
  FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind)
    REFERENCES storyos.domain_receipts
      (owner_user_id, project_id, receipt_id, result_kind) MATCH FULL,
  CONSTRAINT project_activity_event_payloads_shape CHECK ((
    jsonb_typeof(payload) = 'object'
    AND (
      (event_kind = 'writer_takeover_applied'
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
    )
  ) IS TRUE)
);

CREATE FUNCTION storyos.require_activity_position_exclusive()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
DECLARE
  scoped_owner_user_id uuid := COALESCE(NEW.owner_user_id, OLD.owner_user_id);
  scoped_project_id uuid := COALESCE(NEW.project_id, OLD.project_id);
  scoped_position numeric(20, 0) := COALESCE(
    NEW.project_activity_position, OLD.project_activity_position
  );
  overlap_count bigint;
BEGIN
  SELECT count(*) INTO overlap_count
    FROM storyos.project_activity_events AS activity
    JOIN storyos.project_activity_event_payloads AS payload
      ON (payload.owner_user_id, payload.project_id, payload.project_activity_position) =
         (activity.owner_user_id, activity.project_id, activity.project_activity_position)
   WHERE activity.owner_user_id = scoped_owner_user_id
     AND activity.project_id = scoped_project_id
     AND activity.project_activity_position = scoped_position;
  IF overlap_count <> 0 THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'Author Edit Activity and takeover Activity cannot share a position';
  END IF;
  RETURN NULL;
END
$function$;

CREATE CONSTRAINT TRIGGER project_activity_events_position_exclusive
AFTER INSERT OR UPDATE OR DELETE ON storyos.project_activity_events
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION storyos.require_activity_position_exclusive();

CREATE CONSTRAINT TRIGGER project_activity_event_payloads_position_exclusive
AFTER INSERT OR UPDATE OR DELETE ON storyos.project_activity_event_payloads
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION storyos.require_activity_position_exclusive();

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

CREATE CONSTRAINT TRIGGER project_activity_event_payloads_receipt_relation_complete
AFTER INSERT OR UPDATE OR DELETE ON storyos.project_activity_event_payloads
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION storyos.require_author_edit_receipt_relation();

ALTER TABLE storyos.project_activity_event_payloads ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.project_activity_event_payloads FORCE ROW LEVEL SECURITY;
CREATE POLICY project_activity_event_payloads_exact_scope
  ON storyos.project_activity_event_payloads USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.project_activity_event_payloads TO storyos_runtime;
