SET LOCAL ROLE storyos_owner;

-- Global, immutable Model Capability Profiles. The Host writes each one with its Registration.
CREATE TABLE storyos.model_capability_profiles (
  capability_profile_revision text PRIMARY KEY
    CHECK (char_length(capability_profile_revision) BETWEEN 1 AND 200),
  profile jsonb NOT NULL CHECK (jsonb_typeof(profile) = 'object')
);
GRANT SELECT, INSERT ON storyos.model_capability_profiles TO storyos_runtime;

-- The model_kind of a Registration is its Model Provider Adapter.
ALTER TABLE storyos.model_registration_revisions
  DROP CONSTRAINT model_registration_revisions_model_kind_check,
  ADD CONSTRAINT model_registration_revisions_model_kind_check
    CHECK (model_kind IN ('host_fake', 'volcengine_agent_plan_responses')),
  ADD COLUMN api_surface text NOT NULL
    CHECK (char_length(api_surface) BETWEEN 1 AND 200),
  ADD COLUMN provider_model_id text NOT NULL
    CHECK (char_length(provider_model_id) BETWEEN 1 AND 200 AND provider_model_id <> 'latest'),
  ADD COLUMN capability_profile_revision text NOT NULL
    REFERENCES storyos.model_capability_profiles(capability_profile_revision);
CREATE INDEX model_registration_revisions_profile_idx
  ON storyos.model_registration_revisions (capability_profile_revision);
ALTER TABLE storyos.model_registration_heads
  DROP CONSTRAINT model_registration_heads_model_kind_check,
  ADD CONSTRAINT model_registration_heads_model_kind_check
    CHECK (model_kind IN ('host_fake', 'volcengine_agent_plan_responses'));

-- PostgreSQL shortens long default constraint names, so find each kind CHECK by its column.
DO $$
DECLARE
  kind_check record;
BEGIN
  FOR kind_check IN
    SELECT constraint_row.conrelid::regclass AS table_name, constraint_row.conname
      FROM pg_constraint AS constraint_row
      JOIN pg_attribute AS kind_column
        ON kind_column.attrelid = constraint_row.conrelid
       AND kind_column.attnum = constraint_row.conkey[1]
     WHERE constraint_row.contype = 'c'
       AND cardinality(constraint_row.conkey) = 1
       AND (constraint_row.conrelid, kind_column.attname) IN (
         ('storyos.processing_destination_identities'::regclass, 'destination_kind'),
         ('storyos.processing_destination_identity_evidence_revisions'::regclass,
          'evidence_kind'),
         ('storyos.project_destination_grants'::regclass, 'grant_kind'),
         ('storyos.external_contract_compatibility_decisions'::regclass, 'decision_kind')
       )
  LOOP
    EXECUTE format(
      'ALTER TABLE %s DROP CONSTRAINT %I', kind_check.table_name, kind_check.conname
    );
  END LOOP;
END
$$;
ALTER TABLE storyos.processing_destination_identities
  ADD CONSTRAINT processing_destination_identities_kind_check
    CHECK (destination_kind IN ('host_fake', 'volcengine_agent_plan'));
ALTER TABLE storyos.processing_destination_identity_evidence_revisions
  ADD CONSTRAINT processing_destination_identity_evidence_kind_check
    CHECK (evidence_kind IN ('host_fake_boundary', 'volcengine_agent_plan_boundary'));
ALTER TABLE storyos.project_destination_grants
  ADD CONSTRAINT project_destination_grants_kind_check
    CHECK (grant_kind IN ('host_fake_use', 'volcengine_agent_plan_use'));
ALTER TABLE storyos.external_contract_compatibility_decisions
  ADD CONSTRAINT external_contract_compatibility_decisions_kind_check
    CHECK (decision_kind IN ('host_fake_compatible', 'volcengine_agent_plan_compatible'));

-- A Project can move to a new destination binding. Its policy revision identifies the current one.
DO $$
DECLARE
  unique_constraint record;
BEGIN
  FOR unique_constraint IN
    SELECT constraint_row.conrelid::regclass AS table_name, constraint_row.conname
      FROM pg_constraint AS constraint_row
     WHERE constraint_row.contype = 'u'
       AND cardinality(constraint_row.conkey) = 2
       AND constraint_row.conrelid IN (
         'storyos.processing_destination_identities'::regclass,
         'storyos.project_destination_grants'::regclass,
         'storyos.project_external_use_binding_revisions'::regclass,
         'storyos.external_contract_compatibility_decisions'::regclass
       )
  LOOP
    EXECUTE format(
      'ALTER TABLE %s DROP CONSTRAINT %I',
      unique_constraint.table_name, unique_constraint.conname
    );
  END LOOP;
END
$$;

-- FORCE RLS blocks FK validation as storyos_owner when storyos.owner_user_id is unset.
RESET ROLE;
ALTER TABLE storyos.project_policy_revisions
  ADD COLUMN external_compatibility_decision uuid NOT NULL,
  ADD CONSTRAINT project_policy_revisions_decision_fkey
    FOREIGN KEY (owner_user_id, project_id, external_compatibility_decision)
    REFERENCES storyos.external_contract_compatibility_decisions(
      owner_user_id, project_id, external_compatibility_decision
    ) MATCH FULL;
CREATE INDEX project_policy_revisions_decision_idx
  ON storyos.project_policy_revisions (
    owner_user_id, project_id, external_compatibility_decision
  );
CREATE INDEX external_contract_compatibility_decisions_binding_idx
  ON storyos.external_contract_compatibility_decisions (
    owner_user_id, project_id, project_model_use_binding_revision
  );
CREATE INDEX project_external_use_binding_revisions_grant_idx
  ON storyos.project_external_use_binding_revisions (owner_user_id, project_id, grant_id);

-- An AgentRun pins the Registration of its binding, so the Worker claim can select its adapter.
ALTER TABLE storyos.project_external_use_binding_revisions
  ADD CONSTRAINT project_external_use_binding_revisions_registration_key
    UNIQUE (owner_user_id, project_id, project_model_use_binding_revision,
            model_registration_revision);
ALTER TABLE storyos.agent_runs
  ADD COLUMN model_registration_revision uuid NOT NULL,
  ADD CONSTRAINT agent_runs_binding_registration_fkey
    FOREIGN KEY (
      owner_user_id, project_id, project_model_use_binding_revision,
      model_registration_revision
    )
    REFERENCES storyos.project_external_use_binding_revisions(
      owner_user_id, project_id, project_model_use_binding_revision,
      model_registration_revision
    ) MATCH FULL;
DROP INDEX storyos.agent_runs_binding_idx;
CREATE INDEX agent_runs_binding_idx
  ON storyos.agent_runs (
    owner_user_id, project_id, project_model_use_binding_revision,
    model_registration_revision
  );
CREATE INDEX agent_runs_registration_idx
  ON storyos.agent_runs (model_registration_revision);
