SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.proposals
  DROP CONSTRAINT proposals_kind_check;
ALTER TABLE storyos.proposals
  ADD CONSTRAINT proposals_kind_check
  CHECK (kind IN ('block_edit', 'inline_edit'));

CREATE TABLE storyos.proposal_anchors (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  operation_id uuid NOT NULL,
  anchor_order integer NOT NULL CHECK (anchor_order >= 1),
  manuscript_block_id uuid NOT NULL,
  base_authoritative_revision_id uuid NOT NULL,
  manuscript_schema_version integer NOT NULL,
  coordinate_profile text NOT NULL,
  range_from integer NOT NULL CHECK (range_from >= 0),
  range_to integer NOT NULL CHECK (range_to > range_from),
  boundary_profile text NOT NULL,
  base_slice_digest text NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, proposal_id, operation_id, anchor_order),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, operation_id)
    REFERENCES storyos.proposal_operations
      (owner_user_id, project_id, proposal_id, operation_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, manuscript_block_id)
    REFERENCES storyos.manuscript_blocks
      (owner_user_id, project_id, manuscript_block_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, base_authoritative_revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.proposal_anchors ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_anchors FORCE ROW LEVEL SECURITY;
CREATE POLICY proposal_anchors_exact_scope ON storyos.proposal_anchors USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);

GRANT SELECT, INSERT ON storyos.proposal_anchors TO storyos_runtime;
GRANT UPDATE ON storyos.proposal_revisions TO storyos_runtime;
