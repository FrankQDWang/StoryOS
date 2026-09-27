SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.operation_requirements
  ADD COLUMN requirement_role text NOT NULL DEFAULT 'primary'
    CHECK (requirement_role IN ('primary', 'compaction', 'later_request'));
ALTER TABLE storyos.operation_requirements
  DROP CONSTRAINT operation_requirements_owner_user_id_project_id_run_id_key;
CREATE UNIQUE INDEX operation_requirements_one_role
  ON storyos.operation_requirements (owner_user_id, project_id, run_id, requirement_role);

ALTER TABLE storyos.context_assembly_manifests
  ADD COLUMN manifest_role text NOT NULL DEFAULT 'decision'
    CHECK (manifest_role IN ('decision', 'compaction', 'later_request'));
ALTER TABLE storyos.context_assembly_manifests
  DROP CONSTRAINT context_assembly_manifests_owner_user_id_project_id_run_id_key;
CREATE UNIQUE INDEX context_assembly_manifests_one_role
  ON storyos.context_assembly_manifests (
    owner_user_id, project_id, run_id, manifest_role
  );

ALTER TABLE storyos.model_attempts
  ADD COLUMN attempt_role text NOT NULL DEFAULT 'decision'
    CHECK (attempt_role IN ('decision', 'compaction', 'later_request'));
ALTER TABLE storyos.model_attempts
  DROP CONSTRAINT model_attempts_owner_user_id_project_id_run_id_key;
CREATE UNIQUE INDEX model_attempts_one_role
  ON storyos.model_attempts (owner_user_id, project_id, run_id, attempt_role);

CREATE TABLE storyos.active_context_compactions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  compaction_id uuid NOT NULL,
  run_id uuid NOT NULL,
  prior_model_attempt_id uuid NOT NULL,
  prior_manifest_id uuid NOT NULL,
  prior_run_step_id uuid NOT NULL,
  producer_model_attempt_id uuid NOT NULL,
  producer_manifest_id uuid NOT NULL,
  producer_invocation_id uuid NOT NULL,
  producer text NOT NULL,
  mapping_kind text NOT NULL CHECK (mapping_kind IN ('host_managed', 'native')),
  mapping_revision text NOT NULL,
  known_inputs jsonb NOT NULL,
  output_text text NOT NULL,
  usage_kind text NOT NULL,
  loss_facts jsonb NOT NULL,
  input_digest text NOT NULL,
  install_state text NOT NULL
    CHECK (install_state IN ('staged', 'installed', 'refused')),
  refusal_reason text,
  installed_run_step_id uuid,
  installed_model_invocation_id uuid,
  installed_model_attempt_id uuid,
  preserved_item_ids jsonb NOT NULL,
  admission jsonb NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, compaction_id),
  UNIQUE (owner_user_id, project_id, run_id),
  FOREIGN KEY (owner_user_id, project_id, run_id)
    REFERENCES storyos.agent_runs (owner_user_id, project_id, run_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, prior_model_attempt_id)
    REFERENCES storyos.model_attempts (owner_user_id, project_id, model_attempt_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, producer_model_attempt_id)
    REFERENCES storyos.model_attempts (owner_user_id, project_id, model_attempt_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, installed_model_attempt_id)
    REFERENCES storyos.model_attempts (owner_user_id, project_id, model_attempt_id),
  CHECK ((
    (install_state = 'installed'
      AND installed_run_step_id IS NOT NULL
      AND installed_model_invocation_id IS NOT NULL
      AND installed_model_attempt_id IS NOT NULL
      AND refusal_reason IS NULL)
    OR (install_state = 'staged'
      AND installed_run_step_id IS NULL
      AND installed_model_invocation_id IS NULL
      AND installed_model_attempt_id IS NULL
      AND refusal_reason IS NULL)
    OR (install_state = 'refused'
      AND installed_run_step_id IS NULL
      AND installed_model_invocation_id IS NULL
      AND installed_model_attempt_id IS NULL
      AND refusal_reason IS NOT NULL)
  ) IS TRUE)
);

ALTER TABLE storyos.active_context_compactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.active_context_compactions FORCE ROW LEVEL SECURITY;
CREATE POLICY active_context_compactions_exact_scope
  ON storyos.active_context_compactions USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT, UPDATE ON storyos.active_context_compactions TO storyos_runtime;
