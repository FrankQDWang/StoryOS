SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.proposals (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  kind text NOT NULL CHECK (kind = 'block_edit'),
  chapter_id uuid NOT NULL,
  manuscript_block_id uuid NOT NULL,
  source_run_id uuid NOT NULL,
  source_decision_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, proposal_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects (owner_user_id, project_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, chapter_id)
    REFERENCES storyos.manuscript_objects
      (owner_user_id, project_id, manuscript_object_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, manuscript_block_id)
    REFERENCES storyos.manuscript_blocks
      (owner_user_id, project_id, manuscript_block_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, source_run_id)
    REFERENCES storyos.agent_runs (owner_user_id, project_id, run_id) MATCH FULL
);

CREATE TABLE storyos.proposal_revisions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  generation text NOT NULL
    CHECK (generation IN ('generating', 'ready_partial', 'ready')),
  validation text NOT NULL
    CHECK (validation IN ('pending', 'valid', 'invalid', 'conflicted')),
  closure text NOT NULL
    CHECK (closure IN ('open', 'withdrawn', 'superseded')),
  candidate_text text NOT NULL,
  base_authoritative_revision_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, proposal_id, revision_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id)
    REFERENCES storyos.proposals (owner_user_id, project_id, proposal_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, base_authoritative_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, revision_id) MATCH FULL
);

CREATE TABLE storyos.proposal_heads (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  current_revision_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, proposal_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, current_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

CREATE TABLE storyos.proposal_operations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  operation_id uuid NOT NULL,
  manuscript_block_id uuid NOT NULL,
  resolution text NOT NULL
    CHECK (resolution IN ('pending', 'applied', 'rejected')),
  reservation_state text NOT NULL
    CHECK (reservation_state IN ('unresolved', 'resolved')),
  PRIMARY KEY (owner_user_id, project_id, proposal_id, operation_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id)
    REFERENCES storyos.proposals (owner_user_id, project_id, proposal_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, manuscript_block_id)
    REFERENCES storyos.manuscript_blocks
      (owner_user_id, project_id, manuscript_block_id) MATCH FULL
);
CREATE UNIQUE INDEX proposal_operations_one_unresolved_block
  ON storyos.proposal_operations (owner_user_id, project_id, manuscript_block_id)
  WHERE reservation_state = 'unresolved';

CREATE TABLE storyos.validation_receipts (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  validation_receipt_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  proposal_revision_id uuid NOT NULL,
  result text NOT NULL
    CHECK (result IN ('valid', 'invalid', 'conflicted')),
  base_authoritative_revision_id uuid NOT NULL,
  manuscript_block_id uuid NOT NULL,
  candidate_text text NOT NULL,
  reservation_state text NOT NULL
    CHECK (reservation_state IN ('unresolved', 'resolved')),
  PRIMARY KEY (owner_user_id, project_id, validation_receipt_id),
  UNIQUE (owner_user_id, project_id, proposal_id, proposal_revision_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.proposals ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposals FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_heads ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_heads FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_operations ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_operations FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.validation_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.validation_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY proposals_exact_scope ON storyos.proposals USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY proposal_revisions_exact_scope ON storyos.proposal_revisions USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY proposal_heads_exact_scope ON storyos.proposal_heads USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY proposal_operations_exact_scope ON storyos.proposal_operations USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY validation_receipts_exact_scope ON storyos.validation_receipts USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);

GRANT SELECT, INSERT ON storyos.proposals, storyos.proposal_revisions,
  storyos.proposal_heads, storyos.proposal_operations, storyos.validation_receipts
  TO storyos_runtime;
