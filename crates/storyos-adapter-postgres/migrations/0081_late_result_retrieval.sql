SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.model_attempts
  DROP CONSTRAINT model_attempts_attempt_role_check;
ALTER TABLE storyos.model_attempts
  ADD CONSTRAINT model_attempts_attempt_role_check
  CHECK (attempt_role IN (
    'decision', 'compaction', 'later_request', 'retrieval', 'successor', 'late_retrieval'
  ));
