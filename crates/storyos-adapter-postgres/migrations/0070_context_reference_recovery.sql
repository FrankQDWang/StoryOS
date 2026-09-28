SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.context_reference_recoveries (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  recovery_id uuid NOT NULL,
  conversation_id uuid NOT NULL,
  predecessor_run_id uuid NOT NULL,
  successor_run_id uuid NOT NULL,
  predecessor_model_attempt_id uuid NOT NULL,
  predecessor_continuation_binding_id uuid,
  disposition text NOT NULL
    CHECK (disposition IN ('rebuilt', 'blocked', 'unknown_create')),
  block_reason text,
  run_step_id uuid,
  model_invocation_id uuid,
  model_attempt_id uuid,
  assembly_manifest_id uuid,
  lossless_provider_reconstruction boolean NOT NULL,
  semantic_erasure boolean NOT NULL,
  opaque_reused boolean NOT NULL,
  covered_content_included boolean NOT NULL,
  predecessor_terminal boolean NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, recovery_id),
  UNIQUE (owner_user_id, project_id, successor_run_id),
  FOREIGN KEY (owner_user_id, project_id, conversation_id)
    REFERENCES storyos.project_conversations (
      owner_user_id, project_id, conversation_id
    ) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, predecessor_run_id)
    REFERENCES storyos.agent_runs (owner_user_id, project_id, run_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, successor_run_id)
    REFERENCES storyos.agent_runs (owner_user_id, project_id, run_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, predecessor_model_attempt_id)
    REFERENCES storyos.model_attempts (owner_user_id, project_id, model_attempt_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, model_attempt_id)
    REFERENCES storyos.model_attempts (owner_user_id, project_id, model_attempt_id),
  FOREIGN KEY (owner_user_id, project_id, assembly_manifest_id)
    REFERENCES storyos.context_assembly_manifests (
      owner_user_id, project_id, context_assembly_manifest_id
    ),
  CHECK (predecessor_run_id <> successor_run_id),
  CHECK ((
    lossless_provider_reconstruction = false
    AND semantic_erasure = false
    AND opaque_reused = false
    AND (
      (disposition = 'rebuilt'
        AND block_reason IS NULL
        AND run_step_id IS NOT NULL
        AND model_invocation_id IS NOT NULL
        AND model_attempt_id IS NOT NULL
        AND assembly_manifest_id IS NOT NULL
        AND predecessor_terminal)
      OR (disposition = 'blocked'
        AND block_reason IS NOT NULL
        AND run_step_id IS NULL
        AND model_invocation_id IS NULL
        AND model_attempt_id IS NULL
        AND assembly_manifest_id IS NULL
        AND covered_content_included = false)
      OR (disposition = 'unknown_create'
        AND block_reason IS NULL
        AND run_step_id IS NULL
        AND model_invocation_id IS NULL
        AND model_attempt_id IS NULL
        AND assembly_manifest_id IS NULL
        AND covered_content_included = false)
    )
  ) IS TRUE)
);

ALTER TABLE storyos.context_reference_recoveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.context_reference_recoveries FORCE ROW LEVEL SECURITY;
CREATE POLICY context_reference_recoveries_exact_scope
  ON storyos.context_reference_recoveries USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.context_reference_recoveries TO storyos_runtime;
