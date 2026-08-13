BEGIN;

INSERT INTO storyos.command_idempotency (
  owner_user_id, project_id, command_kind, idempotency_key,
  canonical_command_digest
) VALUES (
  '018f0000-0000-7001-8000-000000000001',
  '018f0000-0000-7001-8000-000000000002',
  'migration-proof',
  '018f0000-0000-7001-8000-000000000901',
  'sha256:migration-proof'
);

INSERT INTO storyos.project_command_challenge_rate_guards (
  owner_user_id, project_id, client_session_generation, policy_revision
) VALUES (
  '018f0000-0000-7001-8000-000000000001',
  '018f0000-0000-7001-8000-000000000002',
  9223372036854775807,
  'migration-proof'
);

INSERT INTO storyos.project_command_challenge_rate_windows (
  owner_user_id, project_id, client_session_generation, policy_revision,
  window_started_at, issued_count
) VALUES (
  '018f0000-0000-7001-8000-000000000001',
  '018f0000-0000-7001-8000-000000000002',
  9223372036854775807,
  'migration-proof',
  '2026-01-01 00:00:00+00',
  1
);

INSERT INTO storyos.project_command_challenges (
  owner_user_id, project_id, command_kind, idempotency_key,
  client_session_binding_digest, client_session_generation,
  client_contract_revision, security_policy_revision, limit_profile_revision,
  challenge_rate_policy_revision, challenge_rate_window_started_at,
  method, route_template, command_schema, canonical_command_digest,
  nonce_digest
) VALUES (
  '018f0000-0000-7001-8000-000000000001',
  '018f0000-0000-7001-8000-000000000002',
  'migration-proof',
  '018f0000-0000-7001-8000-000000000901',
  'sha256:migration-binding',
  9223372036854775807,
  'migration-client-contract',
  'migration-security-policy',
  'migration-limit-profile',
  'migration-proof',
  '2026-01-01 00:00:00+00',
  'POST',
  '/migration-proof',
  'migration-proof',
  'sha256:migration-proof',
  'sha256:migration-nonce'
);

COMMIT;
