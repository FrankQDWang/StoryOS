SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.acceptance_receipts ADD UNIQUE
  (owner_user_id, project_id, acceptance_receipt_id, proposal_id, proposal_revision_id, result);

CREATE TABLE storyos.proposal_validation_conditions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  proposal_revision_id uuid NOT NULL,
  acceptance_receipt_id uuid NOT NULL,
  validation text NOT NULL CHECK (validation IN ('invalid', 'conflicted')),
  conflict_id uuid,
  PRIMARY KEY (owner_user_id, project_id, proposal_id, proposal_revision_id),
  UNIQUE (owner_user_id, project_id, conflict_id),
  CHECK ((validation = 'conflicted') = (conflict_id IS NOT NULL)),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, acceptance_receipt_id, proposal_id,
               proposal_revision_id, validation)
    REFERENCES storyos.acceptance_receipts
      (owner_user_id, project_id, acceptance_receipt_id, proposal_id, proposal_revision_id, result) MATCH FULL
);
ALTER TABLE storyos.proposal_validation_conditions ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_validation_conditions FORCE ROW LEVEL SECURITY;
CREATE POLICY proposal_validation_conditions_exact_scope ON storyos.proposal_validation_conditions
USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.proposal_validation_conditions TO storyos_runtime;

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_common_shape;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_common_shape CHECK ((
    (
      (result_kind <> 'proposal_revised'
        AND cardinality(proposal_revision_ids) = 0)
      OR (result_kind = 'proposal_revised'
        AND cardinality(proposal_revision_ids) = 1
        AND array_dims(proposal_revision_ids) = '[1:1]')
    )
    AND cardinality(draft_artifact_refs) = 0
    AND cardinality(artifact_lifecycle_event_refs) = 0
    AND (cardinality(condition_refs) = 0 OR
      (command_kind = 'acceptProposal' AND result_kind = 'conflicted'
       AND cardinality(condition_refs) = 1))
    AND array_position(expected_heads, NULL) IS NULL
    AND array_position(prior_heads, NULL) IS NULL
    AND array_position(resulting_heads, NULL) IS NULL
    AND array_position(authoritative_revision_ids, NULL) IS NULL
    AND array_position(proposal_revision_ids, NULL) IS NULL
    AND array_position(authoritative_commit_ids, NULL) IS NULL
    AND array_position(draft_artifact_refs, NULL) IS NULL
    AND array_position(artifact_lifecycle_event_refs, NULL) IS NULL
    AND array_position(condition_refs, NULL) IS NULL
    AND (
      (command_kind NOT IN (
        'createProject', 'updateProject', 'archiveProject', 'createVolume',
        'createChapter', 'updateVolume', 'updateChapter', 'deleteChapter', 'deleteVolume',
        'exportHumanReadableManuscript', 'exportProjectArchive',
        'updateProjectAssistance', 'createAgentRun'
      )
        AND cardinality(expected_heads) = 1
        AND cardinality(prior_heads) = 1
        AND cardinality(resulting_heads) = 1
        AND array_dims(expected_heads) = '[1:1]'
        AND array_dims(prior_heads) = '[1:1]'
        AND array_dims(resulting_heads) = '[1:1]')
      OR (command_kind IN (
        'createProject', 'updateProject', 'archiveProject', 'createVolume',
        'createChapter', 'updateVolume', 'updateChapter', 'deleteChapter', 'deleteVolume',
        'exportHumanReadableManuscript', 'exportProjectArchive',
        'updateProjectAssistance', 'createAgentRun'
      )
        AND cardinality(expected_heads) = 0
        AND cardinality(prior_heads) = 0
        AND cardinality(resulting_heads) = 0)
    )
  ) IS TRUE);
