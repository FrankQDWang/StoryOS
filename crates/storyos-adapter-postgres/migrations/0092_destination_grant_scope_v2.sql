SET LOCAL ROLE storyos_owner;

-- S4-06c: the standing scope v2 adds the active compaction of the current Conversation and the
-- Host-managed summary of the same Conversation. A grant with scope v1 keeps it.
ALTER TABLE storyos.project_destination_grants
  DROP CONSTRAINT project_destination_grants_grant_scope_check,
  ADD CONSTRAINT project_destination_grants_grant_scope_check
    CHECK (grant_scope IN ('project_writing_assistance_v1', 'project_writing_assistance_v2'));

ALTER TABLE storyos.context_assembly_manifests
  DROP CONSTRAINT context_assembly_manifests_disclosure_grant_scope_check,
  ADD CONSTRAINT context_assembly_manifests_disclosure_grant_scope_check
    CHECK (disclosure_grant_scope IN (
      'project_writing_assistance_v1', 'project_writing_assistance_v2'
    ));
