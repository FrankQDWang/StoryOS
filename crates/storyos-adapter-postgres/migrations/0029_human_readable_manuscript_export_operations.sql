SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.human_readable_manuscript_export_operations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  export_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  command_id uuid NOT NULL,
  command_kind text NOT NULL
    CHECK (command_kind = 'exportHumanReadableManuscript'),
  idempotency_key uuid NOT NULL,
  source_snapshot_id uuid NOT NULL,
  source_activity_position bigint NOT NULL CHECK (source_activity_position >= 0),
  export_profile text NOT NULL
    CHECK (export_profile = 'storyos.readable-export.utf8-lf.v1'),
  wakeup_pending boolean NOT NULL DEFAULT true,
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

ALTER TABLE storyos.human_readable_manuscript_export_operations
  ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.human_readable_manuscript_export_operations
  FORCE ROW LEVEL SECURITY;
CREATE POLICY human_readable_manuscript_export_operations_exact_scope
  ON storyos.human_readable_manuscript_export_operations USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.human_readable_manuscript_export_operations
  TO storyos_runtime;
