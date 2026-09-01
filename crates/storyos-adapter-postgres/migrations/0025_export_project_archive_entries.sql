SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.project_export_entries (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  export_id uuid NOT NULL,
  path text NOT NULL
    CHECK (octet_length(path) BETWEEN 1 AND 512 AND path !~ '[\\]' AND path !~ '//'),
  media_type text NOT NULL,
  payload_schema text NOT NULL,
  byte_length bigint NOT NULL CHECK (byte_length >= 0),
  digest text NOT NULL CHECK (digest ~ '^sha256:[0-9a-f]{64}$'),
  payload bytea NOT NULL CHECK (octet_length(payload) = byte_length),
  PRIMARY KEY (owner_user_id, project_id, export_id, path),
  FOREIGN KEY (owner_user_id, project_id, export_id)
    REFERENCES storyos.project_export_manifests(owner_user_id, project_id, export_id) MATCH FULL
);

ALTER TABLE storyos.project_export_entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.project_export_entries FORCE ROW LEVEL SECURITY;
CREATE POLICY project_export_entries_exact_scope
  ON storyos.project_export_entries USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.project_export_entries TO storyos_runtime;
GRANT UPDATE ON storyos.project_export_manifests TO storyos_runtime;
