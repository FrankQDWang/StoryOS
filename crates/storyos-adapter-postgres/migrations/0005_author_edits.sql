ALTER TABLE storyos.project_writer_generations
  ADD CONSTRAINT project_writer_generations_session_generation_key
  UNIQUE (owner_user_id, project_id, current_editor_session_id, writer_generation);
ALTER TABLE storyos.project_command_challenges
  ADD CONSTRAINT project_command_challenges_consumption_key
  UNIQUE (
    owner_user_id, project_id, command_kind, idempotency_key, consumed_at, expires_at
  );
GRANT REFERENCES ON storyos.command_idempotency, storyos.project_command_challenges
  TO storyos_owner;
SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.author_command_admissions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  command_id uuid NOT NULL,
  editor_session_id uuid NOT NULL,
  writer_generation numeric(20, 0) NOT NULL
    CHECK (writer_generation BETWEEN 1 AND 18446744073709551615),
  client_session_binding_ref text NOT NULL,
  client_session_generation numeric(20, 0) NOT NULL
    CHECK (client_session_generation BETWEEN 0 AND 18446744073709551615),
  client_contract_revision text NOT NULL,
  security_policy_revision text NOT NULL,
  action_class text NOT NULL CHECK (action_class = 'direct_editor_action'),
  method text NOT NULL,
  route_template text NOT NULL,
  command_schema text NOT NULL,
  command_kind text NOT NULL CHECK (command_kind = 'applyAuthorEdit'),
  canonical_command_digest text NOT NULL,
  idempotency_key uuid NOT NULL,
  challenge_consumed_at timestamptz NOT NULL,
  challenge_expires_at timestamptz NOT NULL,
  correlation_id uuid NOT NULL,
  chapter_object_id uuid NOT NULL,
  expected_authoritative_revision_id uuid NOT NULL,
  expected_proposal_head_revision_ids uuid[] NOT NULL,
  target_refs text[] NOT NULL,
  observed_ownership_partition text NOT NULL CHECK (observed_ownership_partition = 'authoritative'),
  editor_contract_revision text NOT NULL,
  undo_group_id uuid NOT NULL,
  completed_intent_record_id uuid NOT NULL,
  local_intent_sequence numeric(20, 0) NOT NULL
    CHECK (local_intent_sequence BETWEEN 1 AND 18446744073709551615),
  command_payload jsonb NOT NULL CHECK (jsonb_typeof(command_payload) = 'object'),
  issued_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, author_command_admission_id),
  UNIQUE (owner_user_id, project_id, command_id),
  UNIQUE (owner_user_id, project_id, command_kind, idempotency_key),
  UNIQUE (
    owner_user_id, project_id, author_command_admission_id, command_id,
    command_kind, canonical_command_digest, idempotency_key
  ),
  FOREIGN KEY (owner_user_id, project_id, editor_session_id)
    REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, editor_session_id, writer_generation)
    REFERENCES storyos.project_writer_generations
      (owner_user_id, project_id, current_editor_session_id, writer_generation) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, chapter_object_id, expected_authoritative_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key)
    REFERENCES storyos.command_idempotency
      (owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL,
  FOREIGN KEY (
    owner_user_id, project_id, command_kind, idempotency_key,
    challenge_consumed_at, challenge_expires_at
  ) REFERENCES storyos.project_command_challenges (
    owner_user_id, project_id, command_kind, idempotency_key, consumed_at, expires_at
  ) MATCH FULL
);
CREATE INDEX author_command_admissions_editor_session_fk_idx
  ON storyos.author_command_admissions (owner_user_id, project_id, editor_session_id);
CREATE INDEX author_command_admissions_writer_generation_fk_idx
  ON storyos.author_command_admissions
    (owner_user_id, project_id, editor_session_id, writer_generation);
CREATE INDEX author_command_admissions_revision_fk_idx
  ON storyos.author_command_admissions
    (owner_user_id, project_id, chapter_object_id, expected_authoritative_revision_id);

CREATE TABLE storyos.scope_counters (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  author_action_sequence numeric(20, 0) NOT NULL DEFAULT 0
    CHECK (author_action_sequence BETWEEN 0 AND 18446744073709551614),
  authoritative_commit_sequence numeric(20, 0) NOT NULL DEFAULT 0
    CHECK (authoritative_commit_sequence BETWEEN 0 AND 18446744073709551614),
  project_activity_position numeric(20, 0) NOT NULL DEFAULT 0
    CHECK (project_activity_position BETWEEN 0 AND 18446744073709551614),
  PRIMARY KEY (owner_user_id, project_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

CREATE TABLE storyos.authoritative_revision_envelopes (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  manuscript_object_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  parent_revision_id uuid NOT NULL,
  schema_revision text NOT NULL,
  creator_kind text NOT NULL CHECK (creator_kind = 'author_command_admission'),
  creator_ref uuid NOT NULL,
  receipt_id uuid NOT NULL,
  receipt_result_kind text NOT NULL CHECK (receipt_result_kind = 'authoritative_applied'),
  cause_kind text NOT NULL CHECK (cause_kind = 'direct_author_action'),
  payload_digest text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, manuscript_object_id, revision_id),
  UNIQUE (owner_user_id, project_id, creator_ref),
  UNIQUE (owner_user_id, project_id, receipt_id),
  UNIQUE (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    manuscript_object_id, parent_revision_id, revision_id
  ),
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, parent_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, creator_ref)
    REFERENCES storyos.author_command_admissions
      (owner_user_id, project_id, author_command_admission_id) MATCH FULL
);
CREATE INDEX authoritative_revision_envelopes_parent_revision_fk_idx
  ON storyos.authoritative_revision_envelopes
    (owner_user_id, project_id, manuscript_object_id, parent_revision_id);
CREATE INDEX authoritative_revision_envelopes_admission_fk_idx
  ON storyos.authoritative_revision_envelopes
    (owner_user_id, project_id, creator_ref);

CREATE TABLE storyos.authoritative_commits (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  authoritative_commit_id uuid NOT NULL,
  authoritative_commit_sequence numeric(20, 0) NOT NULL
    CHECK (authoritative_commit_sequence BETWEEN 1 AND 18446744073709551615),
  manuscript_object_id uuid NOT NULL,
  prior_revision_id uuid NOT NULL,
  resulting_revision_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  receipt_id uuid NOT NULL,
  receipt_result_kind text NOT NULL CHECK (receipt_result_kind = 'authoritative_applied'),
  committed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, authoritative_commit_id),
  UNIQUE (owner_user_id, project_id, authoritative_commit_sequence),
  UNIQUE (owner_user_id, project_id, author_command_admission_id),
  UNIQUE (owner_user_id, project_id, receipt_id),
  UNIQUE (
    owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id
  ),
  UNIQUE (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id, resulting_revision_id
  ),
  UNIQUE (owner_user_id, project_id, authoritative_commit_id, resulting_revision_id),
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, prior_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, resulting_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, author_command_admission_id)
    REFERENCES storyos.author_command_admissions
      (owner_user_id, project_id, author_command_admission_id) MATCH FULL
);
CREATE INDEX authoritative_commits_prior_revision_fk_idx
  ON storyos.authoritative_commits
    (owner_user_id, project_id, manuscript_object_id, prior_revision_id);
CREATE INDEX authoritative_commits_resulting_revision_fk_idx
  ON storyos.authoritative_commits
    (owner_user_id, project_id, manuscript_object_id, resulting_revision_id);
CREATE INDEX authoritative_commits_admission_fk_idx
  ON storyos.authoritative_commits
    (owner_user_id, project_id, author_command_admission_id);

CREATE TABLE storyos.domain_receipts (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  receipt_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  command_id uuid NOT NULL,
  command_kind text NOT NULL CHECK (command_kind = 'applyAuthorEdit'),
  command_digest text NOT NULL,
  idempotency_key uuid NOT NULL,
  producer_cause text NOT NULL CHECK (producer_cause = 'author_command_admission'),
  expected_heads uuid[] NOT NULL,
  prior_heads uuid[] NOT NULL,
  resulting_heads uuid[] NOT NULL,
  authoritative_revision_ids uuid[] NOT NULL,
  proposal_revision_ids uuid[] NOT NULL,
  authoritative_commit_ids uuid[] NOT NULL,
  draft_artifact_refs text[] NOT NULL,
  artifact_lifecycle_event_refs text[] NOT NULL,
  condition_refs text[] NOT NULL,
  result_kind text NOT NULL CHECK (
    result_kind IN ('authoritative_applied', 'no_effect', 'conflicted', 'refused')
  ),
  result_payload jsonb NOT NULL CHECK (jsonb_typeof(result_payload) = 'object'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, receipt_id),
  UNIQUE (owner_user_id, project_id, author_command_admission_id),
  UNIQUE (owner_user_id, project_id, author_command_admission_id, receipt_id),
  UNIQUE (
    owner_user_id, project_id, author_command_admission_id, receipt_id, result_kind
  ),
  UNIQUE (owner_user_id, project_id, receipt_id, result_kind),
  CHECK (
    cardinality(expected_heads) = 1
    AND cardinality(prior_heads) = 1
    AND cardinality(resulting_heads) = 1
    AND cardinality(proposal_revision_ids) = 0
    AND cardinality(draft_artifact_refs) = 0
    AND cardinality(artifact_lifecycle_event_refs) = 0
    AND cardinality(condition_refs) = 0
  ),
  CHECK (
    (result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 1
      AND cardinality(authoritative_commit_ids) = 1
      AND resulting_heads = authoritative_revision_ids)
    OR (result_kind = 'no_effect'
      AND result_payload = '{"reason":"content_unchanged"}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload ? 'current_authoritative_revision_id'
      AND result_payload - 'reason' - 'current_authoritative_revision_id' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'stale_authoritative_head', 'proposal_head_present', 'ownership_changed'
      )
      AND result_payload->>'current_authoritative_revision_id' = resulting_heads[1]::text
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)
    OR (result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'unsupported_intent_shape', 'invalid_selection', 'target_mismatch'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
  ),
  FOREIGN KEY (
    owner_user_id, project_id, author_command_admission_id, command_id,
    command_kind, command_digest, idempotency_key
  ) REFERENCES storyos.author_command_admissions (
    owner_user_id, project_id, author_command_admission_id, command_id,
    command_kind, canonical_command_digest, idempotency_key
  ) MATCH FULL
);
CREATE INDEX domain_receipts_admission_fk_idx
  ON storyos.domain_receipts
    (owner_user_id, project_id, author_command_admission_id, command_id,
     command_kind, command_digest, idempotency_key);
ALTER TABLE storyos.authoritative_revision_envelopes
  ADD CONSTRAINT authoritative_revision_envelopes_applied_receipt_fk
  FOREIGN KEY (
    owner_user_id, project_id, creator_ref, receipt_id, receipt_result_kind
  ) REFERENCES storyos.domain_receipts (
    owner_user_id, project_id, author_command_admission_id, receipt_id, result_kind
  ) MATCH FULL DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE storyos.authoritative_commits
  ADD CONSTRAINT authoritative_commits_applied_receipt_fk
  FOREIGN KEY (
    owner_user_id, project_id, author_command_admission_id, receipt_id, receipt_result_kind
  ) REFERENCES storyos.domain_receipts (
    owner_user_id, project_id, author_command_admission_id, receipt_id, result_kind
  ) MATCH FULL DEFERRABLE INITIALLY DEFERRED,
  ADD CONSTRAINT authoritative_commits_envelope_fk
  FOREIGN KEY (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    manuscript_object_id, prior_revision_id, resulting_revision_id
  ) REFERENCES storyos.authoritative_revision_envelopes (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    manuscript_object_id, parent_revision_id, revision_id
  ) MATCH FULL DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE storyos.author_command_admission_settlements (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  settlement_kind text NOT NULL CHECK (settlement_kind = 'receipt_settled'),
  receipt_id uuid NOT NULL,
  settled_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, author_command_admission_id),
  UNIQUE (owner_user_id, project_id, receipt_id),
  FOREIGN KEY (owner_user_id, project_id, author_command_admission_id, receipt_id)
    REFERENCES storyos.domain_receipts
      (owner_user_id, project_id, author_command_admission_id, receipt_id) MATCH FULL
);
CREATE INDEX author_command_admission_settlements_receipt_fk_idx
  ON storyos.author_command_admission_settlements
    (owner_user_id, project_id, author_command_admission_id, receipt_id);

CREATE TABLE storyos.author_action_entries (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  author_action_sequence numeric(20, 0) NOT NULL,
  disposition text NOT NULL CHECK (disposition = 'forward'),
  authoritative_commit_id uuid NOT NULL,
  receipt_id uuid NOT NULL,
  receipt_result_kind text NOT NULL CHECK (receipt_result_kind = 'authoritative_applied'),
  PRIMARY KEY (owner_user_id, project_id, author_action_sequence),
  UNIQUE (owner_user_id, project_id, receipt_id),
  UNIQUE (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id, author_action_sequence
  ),
  FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind)
    REFERENCES storyos.domain_receipts
      (owner_user_id, project_id, receipt_id, result_kind) MATCH FULL,
  FOREIGN KEY (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id
  ) REFERENCES storyos.authoritative_commits (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id
  ) MATCH FULL
);
CREATE INDEX author_action_entries_receipt_commit_fk_idx
  ON storyos.author_action_entries
    (owner_user_id, project_id, receipt_id, receipt_result_kind,
     authoritative_commit_id, author_action_sequence);

CREATE TABLE storyos.project_activity_events (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  project_activity_position numeric(20, 0) NOT NULL,
  project_activity_event_id uuid NOT NULL,
  event_kind text NOT NULL CHECK (event_kind = 'authoritative_author_edit_applied'),
  receipt_id uuid NOT NULL,
  receipt_result_kind text NOT NULL CHECK (receipt_result_kind = 'authoritative_applied'),
  authoritative_commit_id uuid NOT NULL,
  resulting_revision_id uuid NOT NULL,
  author_action_sequence numeric(20, 0) NOT NULL
    CHECK (author_action_sequence BETWEEN 1 AND 18446744073709551615),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, project_activity_position),
  UNIQUE (owner_user_id, project_id, project_activity_event_id),
  UNIQUE (owner_user_id, project_id, receipt_id),
  FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind)
    REFERENCES storyos.domain_receipts
      (owner_user_id, project_id, receipt_id, result_kind) MATCH FULL,
  FOREIGN KEY (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id, resulting_revision_id
  ) REFERENCES storyos.authoritative_commits (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id, resulting_revision_id
  ) MATCH FULL,
  FOREIGN KEY (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id, author_action_sequence
  ) REFERENCES storyos.author_action_entries (
    owner_user_id, project_id, receipt_id, receipt_result_kind,
    authoritative_commit_id, author_action_sequence
  ) MATCH FULL
);
CREATE INDEX project_activity_events_receipt_result_fk_idx
  ON storyos.project_activity_events
    (owner_user_id, project_id, receipt_id, receipt_result_kind,
     authoritative_commit_id, author_action_sequence);

CREATE FUNCTION storyos.require_author_edit_receipt_relation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
DECLARE
  scoped_owner_user_id uuid := COALESCE(NEW.owner_user_id, OLD.owner_user_id);
  scoped_project_id uuid := COALESCE(NEW.project_id, OLD.project_id);
  scoped_receipt_id uuid := COALESCE(NEW.receipt_id, OLD.receipt_id);
  scoped_result_kind text;
  activity_count bigint;
  action_count bigint;
  commit_count bigint;
  revision_envelope_count bigint;
BEGIN
  SELECT receipt.result_kind
    INTO scoped_result_kind
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

  IF scoped_result_kind = 'authoritative_applied' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count) <> (1, 1, 1, 1) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'AuthoritativeApplied requires one complete authority and Activity relation';
    END IF;
  ELSIF (activity_count, action_count, commit_count, revision_envelope_count) <> (0, 0, 0, 0) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'A zero-authority Receipt cannot have an authority or Activity relation';
  END IF;
  RETURN NULL;
END
$function$;

CREATE CONSTRAINT TRIGGER domain_receipts_author_edit_relation_complete
AFTER INSERT OR UPDATE ON storyos.domain_receipts
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION storyos.require_author_edit_receipt_relation();

CREATE CONSTRAINT TRIGGER project_activity_author_edit_relation_complete
AFTER INSERT OR UPDATE OR DELETE ON storyos.project_activity_events
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION storyos.require_author_edit_receipt_relation();

DO $policy$
DECLARE relation_name text;
BEGIN
  FOREACH relation_name IN ARRAY ARRAY[
    'author_command_admissions', 'author_command_admission_settlements', 'scope_counters',
    'authoritative_revision_envelopes', 'authoritative_commits', 'domain_receipts',
    'author_action_entries', 'project_activity_events'
  ] LOOP
    EXECUTE format('ALTER TABLE storyos.%I ENABLE ROW LEVEL SECURITY', relation_name);
    EXECUTE format('ALTER TABLE storyos.%I FORCE ROW LEVEL SECURITY', relation_name);
    EXECUTE format(
      'CREATE POLICY %I_exact_scope ON storyos.%I USING (
        owner_user_id = current_setting(''storyos.owner_user_id'')::uuid
        AND project_id = current_setting(''storyos.project_id'')::uuid
      ) WITH CHECK (
        owner_user_id = current_setting(''storyos.owner_user_id'')::uuid
        AND project_id = current_setting(''storyos.project_id'')::uuid
      )', relation_name, relation_name);
  END LOOP;
END
$policy$;

GRANT SELECT, INSERT ON storyos.author_command_admissions,
  storyos.author_command_admission_settlements,
  storyos.authoritative_revision_envelopes,
  storyos.authoritative_commits, storyos.domain_receipts, storyos.author_action_entries,
  storyos.project_activity_events TO storyos_runtime;
GRANT SELECT, INSERT, UPDATE ON storyos.scope_counters TO storyos_runtime;
GRANT INSERT ON storyos.authoritative_payloads, storyos.authoritative_revisions TO storyos_runtime;
GRANT UPDATE ON storyos.authoritative_heads TO storyos_runtime;
