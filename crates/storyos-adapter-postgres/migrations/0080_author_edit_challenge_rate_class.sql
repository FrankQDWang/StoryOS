SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.project_command_challenge_rate_windows
  DROP CONSTRAINT project_command_challenge_rate_windows_issued_count_check,
  ADD CONSTRAINT project_command_challenge_rate_windows_issued_count_check CHECK (
    (policy_revision = 'storyos.project-command-challenge-rate.fixed-window.v1'
      AND issued_count BETWEEN 0 AND 10)
    OR (policy_revision = 'storyos.project-command-challenge-rate.author-edit.fixed-window.v1'
      AND issued_count BETWEEN 0 AND 120)
  );
