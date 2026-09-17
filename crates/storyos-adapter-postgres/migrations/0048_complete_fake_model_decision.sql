SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.agent_runs
  ADD COLUMN claim_generation bigint NOT NULL DEFAULT 0
    CHECK (claim_generation >= 0),
  ADD COLUMN fence_token bigint NOT NULL DEFAULT 0
    CHECK (fence_token >= 0),
  ADD COLUMN lease_expires_at timestamptz,
  ADD COLUMN wakeup_pending boolean NOT NULL DEFAULT true,
  ADD COLUMN settlement jsonb;

ALTER TABLE storyos.agent_runs
  DROP CONSTRAINT agent_runs_status_check;
ALTER TABLE storyos.agent_runs
  ADD CONSTRAINT agent_runs_status_check
  CHECK (status IN ('queued', 'claimed', 'waiting', 'completed', 'refused'));

DROP INDEX storyos.agent_runs_one_queued_conversation;
CREATE UNIQUE INDEX agent_runs_one_live_conversation
  ON storyos.agent_runs (owner_user_id, project_id, conversation_id)
  WHERE status IN ('queued', 'claimed', 'waiting');

ALTER TABLE storyos.context_assembly_manifests
  DROP CONSTRAINT context_assembly_manifests_check;
ALTER TABLE storyos.context_assembly_manifests
  ADD CONSTRAINT context_assembly_manifests_destination_pair CHECK ((
    (destination_context_manifest_id IS NULL
      AND outbound_disclosure_manifest_id IS NULL)
    OR (destination_context_manifest_id IS NOT NULL
      AND outbound_disclosure_manifest_id IS NOT NULL)
  ) IS TRUE);

CREATE TABLE storyos.model_attempts (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  run_id uuid NOT NULL,
  model_attempt_id uuid NOT NULL,
  destination_attempt_id uuid NOT NULL,
  outbound_disclosure_event_id uuid NOT NULL,
  destination_context_manifest_id uuid NOT NULL,
  outbound_disclosure_manifest_id uuid NOT NULL,
  wire_payload_projection_id uuid NOT NULL,
  model_invocation_id uuid NOT NULL,
  conversation_id uuid NOT NULL,
  decision_id uuid,
  continuation_binding_id uuid,
  dispatch_state text NOT NULL
    CHECK (dispatch_state IN ('uncertain', 'settled')),
  payload jsonb NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, model_attempt_id),
  UNIQUE (owner_user_id, project_id, run_id),
  FOREIGN KEY (owner_user_id, project_id, run_id)
    REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, conversation_id)
    REFERENCES storyos.project_conversations(
      owner_user_id, project_id, conversation_id
    ) MATCH FULL
);

DROP POLICY agent_runs_exact_scope ON storyos.agent_runs;
CREATE POLICY agent_runs_exact_scope
  ON storyos.agent_runs USING (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  ) WITH CHECK (
    owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
    AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
  );
CREATE POLICY agent_runs_worker_select
  ON storyos.agent_runs
  FOR SELECT
  USING (
    current_setting('storyos.scope_mode', true) = 'worker'
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  );
CREATE POLICY agent_runs_worker_update
  ON storyos.agent_runs
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

ALTER TABLE storyos.model_attempts ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.model_attempts FORCE ROW LEVEL SECURITY;
CREATE POLICY model_attempts_exact_scope
  ON storyos.model_attempts USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT, UPDATE ON storyos.agent_runs TO storyos_runtime;
GRANT SELECT, INSERT, UPDATE ON storyos.model_attempts TO storyos_runtime;
GRANT UPDATE ON storyos.context_assembly_manifests TO storyos_runtime;
