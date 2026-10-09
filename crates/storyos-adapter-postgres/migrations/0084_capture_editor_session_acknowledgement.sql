SET LOCAL ROLE storyos_owner;

-- The first Create Editor Session acknowledgement. A NULL value is a pre-capture record.
ALTER TABLE storyos.command_idempotency
  ADD COLUMN response_editor_session jsonb;

ALTER TABLE storyos.command_idempotency
  ADD CONSTRAINT command_idempotency_response_editor_session_evidence CHECK (
    response_editor_session IS NULL
    OR (command_kind = 'createEditorSession' AND outcome_kind = 'settled')
  );
