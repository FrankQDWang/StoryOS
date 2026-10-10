SET LOCAL ROLE storyos_owner;

-- The Operation whose candidate a Proposal edit changed, and that candidate before the edit.
-- Author Undo restores this candidate. NULL values are a record from before this migration.
ALTER TABLE storyos.proposal_revisions
  ADD COLUMN edited_operation_id uuid,
  ADD COLUMN prior_operation_candidate_text text;

ALTER TABLE storyos.proposal_revisions
  ADD CONSTRAINT proposal_revisions_edited_operation_evidence CHECK (
    (edited_operation_id IS NULL) = (prior_operation_candidate_text IS NULL)
  );

ALTER TABLE storyos.proposal_revisions
  ADD CONSTRAINT proposal_revisions_edited_operation_fkey
    FOREIGN KEY (owner_user_id, project_id, proposal_id, edited_operation_id)
    REFERENCES storyos.proposal_operations (owner_user_id, project_id, proposal_id, operation_id);
