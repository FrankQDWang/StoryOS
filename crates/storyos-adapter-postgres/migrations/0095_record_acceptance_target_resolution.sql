SET LOCAL ROLE storyos_owner;

-- The owner reads and records the rows of all Projects, so RLS stops for this transaction only.
ALTER TABLE storyos.acceptance_receipts NO FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.domain_receipts NO FORCE ROW LEVEL SECURITY;

-- An applied Acceptance records the Chapter Head that it applied to and how it found its target
-- on that Head (ADR 0064).
ALTER TABLE storyos.acceptance_receipts
  ADD COLUMN applied_authoritative_revision_id uuid,
  ADD COLUMN target_resolution text
    CHECK (target_resolution IN
      ('exact_base', 'block_replacement', 'proven_range', 'base_block_replacement'));

-- Before this migration, an applied Acceptance needed a base equal to the Chapter Head.
UPDATE storyos.acceptance_receipts AS acceptance
   SET applied_authoritative_revision_id = receipt.prior_heads[1],
       target_resolution = 'exact_base'
  FROM storyos.domain_receipts AS receipt
 WHERE (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
       (acceptance.owner_user_id, acceptance.project_id, acceptance.acceptance_receipt_id)
   AND acceptance.result = 'authoritative_applied';

ALTER TABLE storyos.acceptance_receipts
  ADD CONSTRAINT acceptance_receipts_applied_target_recorded
  CHECK ((result = 'authoritative_applied') =
         (applied_authoritative_revision_id IS NOT NULL AND target_resolution IS NOT NULL));

ALTER TABLE storyos.domain_receipts FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.acceptance_receipts FORCE ROW LEVEL SECURITY;
