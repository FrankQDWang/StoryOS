SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.project_export_operations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  export_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  command_id uuid NOT NULL,
  command_kind text NOT NULL
    CHECK (command_kind = 'exportProjectArchive'),
  idempotency_key uuid NOT NULL,
  source_snapshot_id uuid NOT NULL,
  source_activity_position bigint NOT NULL CHECK (source_activity_position >= 0),
  archive_profile text NOT NULL
    CHECK (archive_profile = 'storyos.project-export.v1'),
  archive_path_profile text NOT NULL
    CHECK (archive_path_profile = 'storyos.archive-path.utf8-nfc-unicode-16.0.0.v1'),
  wakeup_pending boolean NOT NULL DEFAULT true,
  claim_generation bigint NOT NULL DEFAULT 0
    CHECK (claim_generation >= 0),
  fence_token bigint NOT NULL DEFAULT 0
    CHECK (fence_token >= 0),
  lease_expires_at timestamptz,
  settled_result text
    CHECK (settled_result IS NULL OR settled_result IN ('failed', 'ready')),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, export_id),
  UNIQUE (owner_user_id, project_id, idempotency_key),
  UNIQUE (owner_user_id, project_id, author_command_admission_id),
  UNIQUE (owner_user_id, project_id, command_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, author_command_admission_id)
    REFERENCES storyos.author_command_admissions
      (owner_user_id, project_id, author_command_admission_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key)
    REFERENCES storyos.command_idempotency
      (owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, source_snapshot_id)
    REFERENCES storyos.project_snapshots
      (owner_user_id, project_id, snapshot_id) MATCH FULL
);

ALTER TABLE storyos.project_export_operations
  ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.project_export_operations
  FORCE ROW LEVEL SECURITY;

CREATE POLICY project_export_operations_exact_scope
  ON storyos.project_export_operations USING (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  ) WITH CHECK (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  );

CREATE POLICY project_export_operations_worker_select
  ON storyos.project_export_operations
  FOR SELECT
  USING (
    current_setting('storyos.scope_mode', true) = 'worker'
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  );

CREATE POLICY project_export_operations_worker_update
  ON storyos.project_export_operations
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

GRANT SELECT, INSERT, UPDATE ON storyos.project_export_operations
  TO storyos_runtime;
