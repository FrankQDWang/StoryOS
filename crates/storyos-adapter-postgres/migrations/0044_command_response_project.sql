SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.command_idempotency
  ADD COLUMN acknowledgement_format text,
  ADD COLUMN response_project jsonb;

ALTER TABLE storyos.command_idempotency
  ADD CONSTRAINT command_idempotency_acknowledgement_evidence CHECK ((
    (acknowledgement_format IS NULL AND response_project IS NULL)
    OR (acknowledgement_format IS NOT NULL AND response_project IS NOT NULL)
  ) IS TRUE);
