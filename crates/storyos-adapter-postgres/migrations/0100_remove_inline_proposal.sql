SET LOCAL ROLE storyos_owner;

-- ADR 0073: Release 1 keeps no inline Proposal. A stored row stops this migration and stays.
ALTER TABLE storyos.proposals DROP CONSTRAINT proposals_kind_check;
ALTER TABLE storyos.proposals
  ADD CONSTRAINT proposals_kind_check CHECK (kind IN ('block_edit', 'reversal'));
ALTER TABLE storyos.proposal_anchors
  ADD CONSTRAINT proposal_anchors_removed CHECK (false);
