SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.proposal_rejection_receipts
  ADD COLUMN preserved_generation text,
  ADD COLUMN preserved_validation text,
  ADD COLUMN preserved_closure text;
ALTER TABLE storyos.proposal_rejection_receipts
  ADD CHECK ((preserved_generation IS NULL AND preserved_validation IS NULL
              AND preserved_closure IS NULL)
    OR (result = 'proposal_operations_resolved'
        AND preserved_generation IN ('generating', 'ready_partial', 'ready')
        AND preserved_validation IN ('pending', 'valid', 'invalid', 'conflicted')
        AND preserved_closure IN ('open', 'withdrawn', 'superseded')));

ALTER TABLE storyos.proposal_withdrawals
  ADD COLUMN preserved_generation text,
  ADD COLUMN preserved_validation text;
ALTER TABLE storyos.proposal_withdrawals
  ADD CHECK ((preserved_generation IS NULL AND preserved_validation IS NULL)
    OR (preserved_generation IN ('generating', 'ready_partial', 'ready')
        AND preserved_validation IN ('pending', 'valid', 'invalid', 'conflicted')));

ALTER TABLE storyos.proposal_replans
  ADD COLUMN preserved_generation text,
  ADD COLUMN preserved_closure text;
ALTER TABLE storyos.proposal_replans
  ADD CHECK ((preserved_generation IS NULL AND preserved_closure IS NULL)
    OR (preserved_generation IN ('generating', 'ready_partial', 'ready')
        AND preserved_closure IN ('open', 'withdrawn', 'superseded')));

ALTER TABLE storyos.proposal_operation_reopenings
  ADD COLUMN preserved_generation text,
  ADD COLUMN preserved_closure text;
ALTER TABLE storyos.proposal_operation_reopenings
  ADD CHECK ((preserved_generation IS NULL AND preserved_closure IS NULL)
    OR (preserved_generation IN ('generating', 'ready_partial', 'ready')
        AND preserved_closure IN ('open', 'withdrawn', 'superseded')));

CREATE TABLE storyos.proposal_withdrawal_reopenings (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  reopen_event_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  source_proposal_revision_id uuid NOT NULL,
  resulting_proposal_revision_id uuid NOT NULL,
  withdrawal_event_id uuid NOT NULL,
  preserved_generation text,
  preserved_operation_resolution text,
  reopen_receipt_id uuid NOT NULL,
  author_action_sequence numeric(20, 0) NOT NULL
    CHECK (author_action_sequence BETWEEN 1 AND 18446744073709551615),
  PRIMARY KEY (owner_user_id, project_id, reopen_event_id),
  UNIQUE (owner_user_id, project_id, withdrawal_event_id),
  UNIQUE (owner_user_id, project_id, reopen_receipt_id),
  CHECK ((preserved_generation IS NULL AND preserved_operation_resolution IS NULL)
    OR (preserved_generation IN ('generating', 'ready_partial', 'ready')
        AND preserved_operation_resolution IN ('pending', 'applied', 'rejected'))),
  FOREIGN KEY (owner_user_id, project_id, withdrawal_event_id)
    REFERENCES storyos.proposal_withdrawals
      (owner_user_id, project_id, withdrawal_event_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, source_proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, resulting_proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, reopen_receipt_id)
    REFERENCES storyos.domain_receipts (owner_user_id, project_id, receipt_id) MATCH FULL
);
ALTER TABLE storyos.proposal_withdrawal_reopenings ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_withdrawal_reopenings FORCE ROW LEVEL SECURITY;
CREATE POLICY proposal_withdrawal_reopenings_exact_scope
  ON storyos.proposal_withdrawal_reopenings USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.proposal_withdrawal_reopenings TO storyos_runtime;
