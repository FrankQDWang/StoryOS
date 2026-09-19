SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.proposal_generations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  generation_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  last_applied_stream_seq bigint NOT NULL CHECK (last_applied_stream_seq >= 0),
  PRIMARY KEY (owner_user_id, project_id, generation_id),
  UNIQUE (owner_user_id, project_id, proposal_id, generation_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id)
    REFERENCES storyos.proposals (owner_user_id, project_id, proposal_id) MATCH FULL
);

CREATE TABLE storyos.proposal_stream_events (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  generation_id uuid NOT NULL,
  stream_seq bigint NOT NULL CHECK (stream_seq >= 1),
  proposal_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  batch_digest text NOT NULL,
  candidate_text text NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, generation_id, stream_seq),
  FOREIGN KEY (owner_user_id, project_id, generation_id)
    REFERENCES storyos.proposal_generations
      (owner_user_id, project_id, generation_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

CREATE TABLE storyos.editor_input_fences (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  editor_input_fence_id uuid NOT NULL,
  editor_session_id uuid NOT NULL,
  writer_generation bigint NOT NULL CHECK (writer_generation >= 1),
  local_intent_sequence bigint NOT NULL CHECK (local_intent_sequence >= 1),
  proposal_id uuid NOT NULL,
  generation_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, editor_input_fence_id),
  UNIQUE (owner_user_id, project_id, generation_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id)
    REFERENCES storyos.proposals (owner_user_id, project_id, proposal_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, generation_id)
    REFERENCES storyos.proposal_generations
      (owner_user_id, project_id, generation_id) MATCH FULL
);

CREATE TABLE storyos.proposal_pause_fences (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_pause_fence_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  generation_id uuid NOT NULL,
  proposal_revision_id uuid NOT NULL,
  admitted_through_stream_seq bigint NOT NULL CHECK (admitted_through_stream_seq >= 0),
  editor_input_fence_id uuid NOT NULL,
  projection_checkpoint_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, proposal_pause_fence_id),
  UNIQUE (owner_user_id, project_id, generation_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, editor_input_fence_id)
    REFERENCES storyos.editor_input_fences
      (owner_user_id, project_id, editor_input_fence_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, generation_id)
    REFERENCES storyos.proposal_generations
      (owner_user_id, project_id, generation_id) MATCH FULL
);

ALTER TABLE storyos.proposal_generations ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_generations FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_stream_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_stream_events FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.editor_input_fences ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.editor_input_fences FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_pause_fences ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_pause_fences FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_generations_exact_scope ON storyos.proposal_generations USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY proposal_stream_events_exact_scope ON storyos.proposal_stream_events USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY editor_input_fences_exact_scope ON storyos.editor_input_fences USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY proposal_pause_fences_exact_scope ON storyos.proposal_pause_fences USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);

GRANT SELECT, INSERT, UPDATE ON storyos.proposal_generations TO storyos_runtime;
GRANT SELECT, INSERT ON storyos.proposal_stream_events, storyos.editor_input_fences,
  storyos.proposal_pause_fences TO storyos_runtime;
