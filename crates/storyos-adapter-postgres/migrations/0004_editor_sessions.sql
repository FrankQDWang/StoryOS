SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.editor_sessions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  editor_session_id uuid NOT NULL,
  client_session_binding_ref text NOT NULL,
  client_session_generation numeric(20, 0) NOT NULL
    CHECK (client_session_generation BETWEEN 0 AND 18446744073709551615),
  client_contract_revision text NOT NULL,
  security_policy_revision text NOT NULL,
  opened_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  disposition text NOT NULL DEFAULT 'open' CHECK (disposition = 'open'),
  PRIMARY KEY (owner_user_id, project_id, editor_session_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

CREATE TABLE storyos.project_writer_generations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  writer_generation numeric(20, 0) NOT NULL
    CHECK (writer_generation BETWEEN 1 AND 18446744073709551615),
  current_editor_session_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id),
  UNIQUE (owner_user_id, project_id, writer_generation),
  FOREIGN KEY (owner_user_id, project_id, current_editor_session_id)
    REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id) MATCH FULL
);

CREATE TABLE storyos.editor_session_base_snapshots (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  snapshot_id uuid NOT NULL,
  editor_session_id uuid NOT NULL,
  chapter_object_id uuid NOT NULL,
  authoritative_revision_id uuid NOT NULL,
  project_activity_position numeric(20, 0) NOT NULL DEFAULT 0
    CHECK (project_activity_position BETWEEN 0 AND 18446744073709551615),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, snapshot_id),
  UNIQUE (owner_user_id, project_id, editor_session_id),
  FOREIGN KEY (owner_user_id, project_id, editor_session_id)
    REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, chapter_object_id, authoritative_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL
);
CREATE INDEX editor_session_base_snapshots_revision_fk_idx
  ON storyos.editor_session_base_snapshots
  (owner_user_id, project_id, chapter_object_id, authoritative_revision_id);

DO $policy$
DECLARE relation_name text;
BEGIN
  FOREACH relation_name IN ARRAY ARRAY[
    'editor_sessions', 'project_writer_generations', 'editor_session_base_snapshots'
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

GRANT SELECT, INSERT ON storyos.editor_sessions,
  storyos.project_writer_generations, storyos.editor_session_base_snapshots TO storyos_runtime;
