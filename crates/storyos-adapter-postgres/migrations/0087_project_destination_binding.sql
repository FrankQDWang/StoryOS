SET LOCAL ROLE storyos_owner;

-- The attributable evidence of a real destination. The fake destination has none.
ALTER TABLE storyos.processing_destination_identity_evidence_revisions
  ADD COLUMN endpoint text CHECK (char_length(endpoint) BETWEEN 1 AND 500),
  ADD COLUMN account_boundary text CHECK (char_length(account_boundary) BETWEEN 1 AND 500),
  ADD COLUMN eligibility_evidence text
    CHECK (char_length(eligibility_evidence) BETWEEN 1 AND 500),
  ADD CONSTRAINT processing_destination_identity_evidence_shape CHECK (
    (evidence_kind = 'host_fake_boundary'
      AND endpoint IS NULL AND account_boundary IS NULL AND eligibility_evidence IS NULL)
    OR (evidence_kind = 'volcengine_agent_plan_boundary'
      AND endpoint IS NOT NULL AND account_boundary IS NOT NULL
      AND eligibility_evidence IS NOT NULL)
  );

-- Only the opaque Credential Reference is stored, never a secret value or a digest of it.
ALTER TABLE storyos.project_external_use_binding_revisions
  ADD COLUMN credential_source text NOT NULL
    CHECK (credential_source IN ('none', 'operator_quota')),
  ADD COLUMN credential_reference text
    CHECK (char_length(credential_reference) BETWEEN 1 AND 500),
  ADD COLUMN budget_bounds text NOT NULL
    CHECK (budget_bounds IN ('not_required', 'unqualified')),
  ADD CONSTRAINT project_external_use_binding_revisions_credential_shape CHECK (
    (credential_source = 'none') = (credential_reference IS NULL)
  );

ALTER TABLE storyos.external_contract_compatibility_decisions
  ADD COLUMN runtime_qualification text NOT NULL
    CHECK (runtime_qualification IN ('qualified', 'pending'));
