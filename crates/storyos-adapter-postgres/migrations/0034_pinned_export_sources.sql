SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.pinned_export_sources (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  export_id uuid NOT NULL,
  source_snapshot_id uuid NOT NULL,
  completeness_profile text NOT NULL
    CHECK (completeness_profile IN (
      'human_readable_manuscript',
      'project_export_archive'
    )),
  facts jsonb NOT NULL,
  facts_sha256 text NOT NULL
    CHECK (facts_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, export_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, source_snapshot_id)
    REFERENCES storyos.project_snapshots
      (owner_user_id, project_id, snapshot_id) MATCH FULL
);

ALTER TABLE storyos.pinned_export_sources
  ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.pinned_export_sources
  FORCE ROW LEVEL SECURITY;

CREATE POLICY pinned_export_sources_exact_scope
  ON storyos.pinned_export_sources USING (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  ) WITH CHECK (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  );

CREATE POLICY pinned_export_sources_worker_select
  ON storyos.pinned_export_sources
  FOR SELECT
  USING (
    current_setting('storyos.scope_mode', true) = 'worker'
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  );

CREATE POLICY pinned_export_sources_worker_update
  ON storyos.pinned_export_sources
  FOR UPDATE
  USING (
    current_setting('storyos.scope_mode', true) = 'worker'
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  )
  WITH CHECK (
    current_setting('storyos.scope_mode', true) = 'worker'
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  );

GRANT SELECT, INSERT, UPDATE ON storyos.pinned_export_sources
  TO storyos_runtime;
