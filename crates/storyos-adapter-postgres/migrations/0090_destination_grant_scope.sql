SET LOCAL ROLE storyos_owner;

-- ADR 0051: the standing scope of a Project Destination Grant. A row from before this migration
-- has no scope and authorizes no dispatch.
ALTER TABLE storyos.project_destination_grants
  ADD COLUMN grant_scope text CHECK (grant_scope IN ('project_writing_assistance_v1'));
