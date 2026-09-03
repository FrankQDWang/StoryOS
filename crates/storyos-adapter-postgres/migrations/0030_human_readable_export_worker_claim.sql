SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.human_readable_manuscript_export_operations
  ADD COLUMN claim_generation bigint NOT NULL DEFAULT 0
    CHECK (claim_generation >= 0),
  ADD COLUMN fence_token bigint NOT NULL DEFAULT 0
    CHECK (fence_token >= 0),
  ADD COLUMN lease_expires_at timestamptz,
  ADD COLUMN settled_result text
    CHECK (settled_result IS NULL OR settled_result IN ('failed', 'ready'));

DROP POLICY human_readable_manuscript_export_operations_exact_scope
  ON storyos.human_readable_manuscript_export_operations;

CREATE POLICY human_readable_manuscript_export_operations_exact_scope
  ON storyos.human_readable_manuscript_export_operations USING (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  ) WITH CHECK (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  );

CREATE POLICY human_readable_manuscript_export_operations_worker_select
  ON storyos.human_readable_manuscript_export_operations
  FOR SELECT
  USING (
    current_setting('storyos.scope_mode', true) = 'worker'
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  );

CREATE POLICY human_readable_manuscript_export_operations_worker_update
  ON storyos.human_readable_manuscript_export_operations
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

GRANT UPDATE ON storyos.human_readable_manuscript_export_operations
  TO storyos_runtime;
