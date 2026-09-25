SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.command_idempotency
  ADD COLUMN response_assistance jsonb;

ALTER TABLE storyos.command_idempotency
  ADD CONSTRAINT command_idempotency_response_assistance_evidence CHECK (
    (acknowledgement_format IS NOT DISTINCT FROM 'command_response_project_assistance.v1') =
    (response_assistance IS NOT NULL)
  );
