SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.operation_requirements
  DROP CONSTRAINT operation_requirements_requirement_role_check;
ALTER TABLE storyos.operation_requirements
  ADD CONSTRAINT operation_requirements_requirement_role_check
  CHECK (requirement_role IN ('primary', 'compaction', 'later_request', 'retrieval'));

ALTER TABLE storyos.context_assembly_manifests
  DROP CONSTRAINT context_assembly_manifests_manifest_role_check;
ALTER TABLE storyos.context_assembly_manifests
  ADD CONSTRAINT context_assembly_manifests_manifest_role_check
  CHECK (manifest_role IN ('decision', 'compaction', 'later_request', 'retrieval'));

ALTER TABLE storyos.model_attempts
  DROP CONSTRAINT model_attempts_attempt_role_check;
ALTER TABLE storyos.model_attempts
  ADD CONSTRAINT model_attempts_attempt_role_check
  CHECK (attempt_role IN ('decision', 'compaction', 'later_request', 'retrieval'));
