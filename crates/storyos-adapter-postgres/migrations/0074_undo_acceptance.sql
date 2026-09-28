SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.proposals
  DROP CONSTRAINT proposals_kind_check;
ALTER TABLE storyos.proposals
  ADD CONSTRAINT proposals_kind_check
  CHECK (kind IN ('block_edit', 'inline_edit', 'reversal'));

CREATE TABLE storyos.undo_acceptance_receipts (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  undo_acceptance_receipt_id uuid NOT NULL,
  author_undo_receipt_id uuid NOT NULL,
  acceptance_receipt_id uuid NOT NULL,
  source_author_action_sequence numeric(20, 0) NOT NULL,
  outcome text NOT NULL
    CHECK (outcome IN ('compensated', 'reversal_required', 'unavailable')),
  authoritative_commit_id uuid,
  proposal_id uuid,
  proposal_revision_id uuid,
  reason text,
  reported_activity_position numeric(20, 0) NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, undo_acceptance_receipt_id),
  UNIQUE (owner_user_id, project_id, author_undo_receipt_id),
  FOREIGN KEY (owner_user_id, project_id, author_undo_receipt_id)
    REFERENCES storyos.domain_receipts (owner_user_id, project_id, receipt_id)
    DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (owner_user_id, project_id, acceptance_receipt_id)
    REFERENCES storyos.acceptance_receipts (owner_user_id, project_id, acceptance_receipt_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id)
    REFERENCES storyos.proposal_revisions (owner_user_id, project_id, proposal_id, revision_id)
    DEFERRABLE INITIALLY DEFERRED,
  CHECK (
    (outcome = 'compensated'
      AND authoritative_commit_id IS NOT NULL
      AND reason IS NULL
      AND (proposal_id IS NULL) = (proposal_revision_id IS NULL))
    OR (outcome = 'reversal_required'
      AND authoritative_commit_id IS NULL
      AND reason IS NULL
      AND proposal_id IS NOT NULL
      AND proposal_revision_id IS NOT NULL)
    OR (outcome = 'unavailable'
      AND authoritative_commit_id IS NULL
      AND proposal_id IS NULL
      AND proposal_revision_id IS NULL
      AND reason IS NOT NULL)
  )
);

CREATE FUNCTION storyos.keep_undo_acceptance_receipt_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'Undo Acceptance Receipts are immutable' USING ERRCODE = '23514';
END;
$$;

CREATE TRIGGER undo_acceptance_receipt_immutable
  BEFORE UPDATE OR DELETE ON storyos.undo_acceptance_receipts
  FOR EACH ROW EXECUTE FUNCTION storyos.keep_undo_acceptance_receipt_immutable();

ALTER TABLE storyos.undo_acceptance_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.undo_acceptance_receipts FORCE ROW LEVEL SECURITY;
CREATE POLICY undo_acceptance_receipts_exact_scope ON storyos.undo_acceptance_receipts USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.undo_acceptance_receipts TO storyos_runtime;
