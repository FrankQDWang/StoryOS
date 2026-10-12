SET LOCAL ROLE storyos_owner;

-- ADR 0051: the standing scope of a Project Destination Grant. A row from before this migration
-- has no scope and authorizes no dispatch.
ALTER TABLE storyos.project_destination_grants
  ADD COLUMN grant_scope text CHECK (grant_scope IN ('project_writing_assistance_v1'));

-- The outbound disclosure manifest names the grant and scope that admitted its dispatch claim.
ALTER TABLE storyos.context_assembly_manifests
  ADD COLUMN disclosure_grant_id uuid,
  ADD COLUMN disclosure_grant_scope text
    CHECK (disclosure_grant_scope IN ('project_writing_assistance_v1')),
  ADD CONSTRAINT context_assembly_manifests_disclosure_grant_shape CHECK (
    (disclosure_grant_id IS NULL) = (disclosure_grant_scope IS NULL)
  ),
  ADD CONSTRAINT context_assembly_manifests_disclosure_grant_fk
    FOREIGN KEY (owner_user_id, project_id, disclosure_grant_id)
    REFERENCES storyos.project_destination_grants(owner_user_id, project_id, grant_id);
