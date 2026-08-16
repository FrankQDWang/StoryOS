BEGIN;

ALTER TABLE storyos.project_command_challenges
  DROP CONSTRAINT project_command_challenges_owner_user_id_project_id_client_fkey;
ALTER TABLE storyos.project_command_challenge_rate_windows
  DROP CONSTRAINT project_command_challenge_rat_owner_user_id_project_id_cli_fkey;

ALTER TABLE storyos.project_command_challenge_rate_guards
  DROP CONSTRAINT project_command_challenge_rate__client_session_generation_check,
  ALTER COLUMN client_session_generation TYPE numeric(20, 0)
    USING client_session_generation::numeric,
  ADD CONSTRAINT project_command_challenge_rate_guards_generation_u64
    CHECK (client_session_generation BETWEEN 0 AND 18446744073709551615);

ALTER TABLE storyos.project_command_challenge_rate_windows
  DROP CONSTRAINT project_command_challenge_rate_client_session_generation_check1,
  ALTER COLUMN client_session_generation TYPE numeric(20, 0)
    USING client_session_generation::numeric,
  ADD CONSTRAINT project_command_challenge_rate_windows_generation_u64
    CHECK (client_session_generation BETWEEN 0 AND 18446744073709551615);

ALTER TABLE storyos.project_command_challenges
  DROP CONSTRAINT project_command_challenges_client_session_generation_check,
  ALTER COLUMN client_session_generation TYPE numeric(20, 0)
    USING client_session_generation::numeric,
  ADD CONSTRAINT project_command_challenges_generation_u64
    CHECK (client_session_generation BETWEEN 0 AND 18446744073709551615);

ALTER TABLE storyos.project_command_challenge_rate_windows
  ADD CONSTRAINT project_command_challenge_rat_owner_user_id_project_id_cli_fkey
  FOREIGN KEY (owner_user_id, project_id, client_session_generation, policy_revision)
  REFERENCES storyos.project_command_challenge_rate_guards
    (owner_user_id, project_id, client_session_generation, policy_revision);

ALTER TABLE storyos.project_command_challenges
  ADD CONSTRAINT project_command_challenges_owner_user_id_project_id_client_fkey
  FOREIGN KEY (
    owner_user_id, project_id, client_session_generation,
    challenge_rate_policy_revision, challenge_rate_window_started_at
  ) REFERENCES storyos.project_command_challenge_rate_windows (
    owner_user_id, project_id, client_session_generation,
    policy_revision, window_started_at
  );

COMMIT;
