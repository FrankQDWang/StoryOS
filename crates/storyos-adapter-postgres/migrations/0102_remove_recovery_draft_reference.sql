SET LOCAL ROLE storyos_owner;

-- ADR 0077: StoryOS has no Host-created Recovery Draft. A stored reference stops this migration and stays.
ALTER TABLE storyos.author_command_admission_reconfirmations
  ADD CONSTRAINT author_command_admission_reconfirmations_no_recovery_draft
  CHECK (recovery_draft_ref IS NULL);
ALTER TABLE storyos.author_command_admission_reconfirmations
  DROP COLUMN recovery_draft_ref;
