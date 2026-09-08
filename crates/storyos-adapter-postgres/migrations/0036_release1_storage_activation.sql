SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.schema_migrations (
  schema_version text PRIMARY KEY,
  migration_id text NOT NULL,
  checksum text NOT NULL
    CHECK (checksum ~ '^sha256:[0-9a-f]{64}$'),
  release_identity text NOT NULL,
  runner_revision text NOT NULL,
  started_at timestamptz NOT NULL,
  finished_at timestamptz,
  status text NOT NULL
    CHECK (status = 'applied')
);

CREATE TABLE storyos.migration_phases (
  attempt_id bigint NOT NULL,
  phase_id text NOT NULL
    CHECK (phase_id IN (
      'preflight',
      'initial_schema_bootstrap',
      'validate_transaction',
      'activate'
    )),
  class text NOT NULL,
  postcondition text NOT NULL,
  disposition text NOT NULL
    CHECK (disposition = 'passed'),
  recorded_at timestamptz NOT NULL,
  PRIMARY KEY (attempt_id, phase_id)
);

CREATE TABLE storyos.migration_phase_checksums (
  attempt_id bigint NOT NULL,
  source_path text NOT NULL,
  checksum text NOT NULL
    CHECK (checksum ~ '^sha256:[0-9a-f]{64}$'),
  PRIMARY KEY (attempt_id, source_path)
);

CREATE TABLE storyos.storage_activation_proofs (
  proof_id text PRIMARY KEY
    CHECK (proof_id = 'release-1'),
  phase text NOT NULL
    CHECK (phase = 'active'),
  catalog_id text NOT NULL,
  catalog_checksum text NOT NULL
    CHECK (catalog_checksum ~ '^sha256:[0-9a-f]{64}$'),
  migration_chain_id text NOT NULL,
  migration_chain_digest text NOT NULL
    CHECK (migration_chain_digest ~ '^sha256:[0-9a-f]{64}$'),
  database_schema_identity text NOT NULL,
  active_schema_version text NOT NULL,
  public_release text NOT NULL,
  route_catalog_id text NOT NULL,
  route_catalog_sha256 text NOT NULL
    CHECK (route_catalog_sha256 ~ '^sha256:[0-9a-f]{64}$'),
  activated_at timestamptz NOT NULL
);

GRANT SELECT ON storyos.storage_activation_proofs TO storyos_runtime;
