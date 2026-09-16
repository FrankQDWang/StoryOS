SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.operation_requirements (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  operation_requirement_id uuid NOT NULL,
  run_id uuid NOT NULL,
  input_snapshot_id uuid NOT NULL,
  receipt_id uuid NOT NULL,
  payload jsonb NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, operation_requirement_id),
  UNIQUE (owner_user_id, project_id, run_id),
  FOREIGN KEY (owner_user_id, project_id, run_id)
    REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, receipt_id)
    REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);
CREATE TABLE storyos.context_assembly_manifests (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  context_assembly_manifest_id uuid NOT NULL,
  operation_requirement_id uuid NOT NULL,
  run_id uuid NOT NULL,
  sufficiency text NOT NULL CHECK (sufficiency IN ('complete', 'blocked')),
  destination_context_manifest_id uuid,
  outbound_disclosure_manifest_id uuid,
  payload jsonb NOT NULL,
  receipt_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, context_assembly_manifest_id),
  UNIQUE (owner_user_id, project_id, run_id),
  UNIQUE (owner_user_id, project_id, operation_requirement_id),
  FOREIGN KEY (owner_user_id, project_id, operation_requirement_id)
    REFERENCES storyos.operation_requirements(
      owner_user_id, project_id, operation_requirement_id
    ) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, run_id)
    REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, receipt_id)
    REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
  CHECK ((
    destination_context_manifest_id IS NULL
    AND outbound_disclosure_manifest_id IS NULL
  ) IS TRUE)
);
CREATE INDEX operation_requirements_receipt_idx
  ON storyos.operation_requirements (owner_user_id, project_id, receipt_id);
CREATE INDEX context_assembly_manifests_receipt_idx
  ON storyos.context_assembly_manifests (owner_user_id, project_id, receipt_id);

ALTER TABLE storyos.operation_requirements ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.operation_requirements FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.context_assembly_manifests ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.context_assembly_manifests FORCE ROW LEVEL SECURITY;

CREATE POLICY operation_requirements_exact_scope
  ON storyos.operation_requirements USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );
CREATE POLICY context_assembly_manifests_exact_scope
  ON storyos.context_assembly_manifests USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.operation_requirements,
  storyos.context_assembly_manifests TO storyos_runtime;
