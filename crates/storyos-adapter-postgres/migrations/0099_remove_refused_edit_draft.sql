SET LOCAL ROLE storyos_owner;

-- ADR 0073: Release 1 keeps no Refused Edit Draft. A stored row stops this migration and stays.
ALTER TABLE storyos.draft_artifacts
  ADD CONSTRAINT draft_artifacts_removed CHECK (false);
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_no_refused_edit_draft CHECK (
    command_kind NOT IN ('closeEditorFlowDraft', 'expandRefusedEditDraftToProposal')
    AND result_kind NOT IN ('refused_to_draft', 'draft_closure_changed', 'proposal_created_from_draft')
  );
