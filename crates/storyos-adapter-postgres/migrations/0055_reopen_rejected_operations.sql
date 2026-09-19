SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.proposal_operation_reopenings (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  reopen_event_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  source_proposal_revision_id uuid NOT NULL,
  resulting_proposal_revision_id uuid NOT NULL,
  operation_id uuid NOT NULL,
  rejection_event_id uuid NOT NULL,
  prior_resolution text NOT NULL CHECK (prior_resolution = 'rejected'),
  resulting_resolution text NOT NULL CHECK (resulting_resolution = 'pending'),
  reopen_receipt_id uuid NOT NULL,
  author_action_sequence numeric(20, 0) NOT NULL
    CHECK (author_action_sequence BETWEEN 1 AND 18446744073709551615),
  PRIMARY KEY (owner_user_id, project_id, reopen_event_id),
  UNIQUE (owner_user_id, project_id, rejection_event_id),
  UNIQUE (owner_user_id, project_id, reopen_receipt_id, operation_id),
  FOREIGN KEY (owner_user_id, project_id, rejection_event_id)
    REFERENCES storyos.proposal_operation_resolutions
      (owner_user_id, project_id, resolution_event_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, operation_id)
    REFERENCES storyos.proposal_operations
      (owner_user_id, project_id, proposal_id, operation_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, source_proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, resulting_proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, reopen_receipt_id)
    REFERENCES storyos.domain_receipts (owner_user_id, project_id, receipt_id) MATCH FULL
);
ALTER TABLE storyos.proposal_operation_reopenings ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_operation_reopenings FORCE ROW LEVEL SECURITY;
CREATE POLICY proposal_operation_reopenings_exact_scope ON storyos.proposal_operation_reopenings USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.proposal_operation_reopenings TO storyos_runtime;

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_command_kind_check;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_command_kind_check
  CHECK (command_kind IN (
    'applyAuthorEdit', 'takeOverProjectWriter', 'createProject', 'updateProject',
    'archiveProject', 'createVolume', 'createChapter', 'updateVolume', 'updateChapter',
    'setCurrentChapter', 'undoLatestAuthorAction', 'deleteChapter', 'deleteVolume',
    'exportHumanReadableManuscript', 'exportProjectArchive',
    'updateProjectAssistance', 'createAgentRun', 'acceptProposal',
    'rejectProposalOperations', 'reopenRejectedOperations'
  ));

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  DROP CONSTRAINT author_command_admission_outcome_unknown_command_kind_check;
ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  ADD CONSTRAINT author_command_admission_outcome_unknown_command_kind_check
  CHECK (command_kind IN (
    'applyAuthorEdit', 'takeOverProjectWriter', 'createProject', 'updateProject',
    'archiveProject', 'createVolume', 'createChapter', 'updateVolume', 'updateChapter',
    'setCurrentChapter', 'undoLatestAuthorAction', 'deleteChapter', 'deleteVolume',
    'exportHumanReadableManuscript', 'exportProjectArchive',
    'updateProjectAssistance', 'createAgentRun', 'acceptProposal',
    'rejectProposalOperations', 'reopenRejectedOperations'
  ));

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_result_kind_check;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_result_kind_check
  CHECK (result_kind IN (
    'authoritative_applied', 'proposal_revised', 'proposal_operations_resolved',
    'no_effect', 'conflicted', 'refused', 'invalid'
  ));

ALTER TABLE storyos.author_action_entries
  DROP CONSTRAINT author_action_entries_receipt_result_kind_check;
ALTER TABLE storyos.author_action_entries
  ADD CONSTRAINT author_action_entries_receipt_result_kind_check
  CHECK (receipt_result_kind IN (
    'authoritative_applied', 'proposal_revised', 'proposal_operations_resolved'
  ));

ALTER TABLE storyos.author_command_admissions
  DROP CONSTRAINT author_command_admissions_command_shape;
ALTER TABLE storyos.author_command_admissions
  ADD CONSTRAINT author_command_admissions_command_shape CHECK ((
    (command_kind = 'applyAuthorEdit'
      AND action_class = 'direct_editor_action'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL
      AND expected_authoritative_revision_id IS NOT NULL
      AND observed_ownership_partition IN ('authoritative', 'mixed')
      AND (
        (observed_ownership_partition = 'authoritative'
          AND expected_proposal_head_revision_ids = '{}'::uuid[])
        OR (observed_ownership_partition = 'mixed'
          AND cardinality(expected_proposal_head_revision_ids) >= 1)
      )
      AND undo_group_id IS NOT NULL
      AND completed_intent_record_id IS NOT NULL
      AND local_intent_sequence IS NOT NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'takeOverProjectWriter'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'createProject'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NULL
      AND challenge_expires_at IS NULL)
    OR (command_kind = 'updateProject'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'archiveProject'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'createVolume'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'createChapter'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'updateVolume'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'updateChapter'
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)

    OR (command_kind IN ('deleteChapter', 'deleteVolume', 'exportHumanReadableManuscript', 'exportProjectArchive', 'updateProjectAssistance')
      AND action_class = 'explicit_project_command'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'setCurrentChapter'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND (
        (chapter_object_id IS NOT NULL AND expected_authoritative_revision_id IS NOT NULL)
        OR (chapter_object_id IS NULL AND expected_authoritative_revision_id IS NULL)
      )
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'undoLatestAuthorAction'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND (
        (chapter_object_id IS NOT NULL AND expected_authoritative_revision_id IS NOT NULL)
        OR (chapter_object_id IS NULL AND expected_authoritative_revision_id IS NULL)
      )
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'createAgentRun'
      AND action_class = 'agent_run_start'
      AND editor_session_id IS NULL
      AND writer_generation IS NULL
      AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'acceptProposal'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL
      AND expected_authoritative_revision_id IS NOT NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'rejectProposalOperations'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL
      AND expected_authoritative_revision_id IS NOT NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)
    OR (command_kind = 'reopenRejectedOperations'
      AND action_class = 'explicit_editor_command'
      AND editor_session_id IS NOT NULL
      AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL
      AND expected_authoritative_revision_id IS NOT NULL
      AND expected_proposal_head_revision_ids = '{}'::uuid[]
      AND target_refs = '{}'::text[]
      AND observed_ownership_partition IS NULL
      AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL
      AND challenge_expires_at IS NOT NULL)

  ) IS TRUE);

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_result_shape;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_result_shape CHECK ((
    (command_kind = 'applyAuthorEdit'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 1
      AND cardinality(authoritative_commit_ids) = 1
      AND array_dims(authoritative_revision_ids) = '[1:1]'
      AND array_dims(authoritative_commit_ids) = '[1:1]'
      AND resulting_heads = authoritative_revision_ids)
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'content_unchanged'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'takeOverProjectWriter'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'writer_takeover_applied',
        'writer_takeover_compare_failed'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'createProject'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateProject'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateProject'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'title_unchanged'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateProject'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_project_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'archiveProject'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'archiveProject'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'already_archived'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'archiveProject'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_project_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'createVolume'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'createVolume'
      AND result_kind = 'authoritative_applied'
      AND result_payload ? 'order'
      AND result_payload - 'order' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'order') = 'string'
      AND result_payload->>'order' ~ '^[1-9][0-9]*$'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'createVolume'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_tree_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'createVolume'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN ('archived_project', 'invalid_title')
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'createChapter'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'createChapter'
      AND result_kind = 'authoritative_applied'
      AND result_payload ? 'order'
      AND result_payload - 'order' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'order') = 'string'
      AND result_payload->>'order' ~ '^[1-9][0-9]*$'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'createChapter'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_tree_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'createChapter'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'invalid_title', 'invalid_volume_join'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateVolume'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'updateVolume'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'unchanged'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateVolume'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_tree_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateVolume'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'invalid_title', 'invalid_order', 'invalid_volume_join'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateChapter'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'updateChapter'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'unchanged'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateChapter'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_tree_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateChapter'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'invalid_title', 'invalid_order', 'invalid_chapter_join'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)

    OR (command_kind = 'deleteChapter'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'deleteChapter'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'already_removed'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'deleteChapter'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_tree_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'deleteChapter'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'invalid_chapter_join'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'deleteVolume'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (cardinality(authoritative_commit_ids) = 0
        OR array_dims(authoritative_commit_ids) = '[1:1]'))
    OR (command_kind = 'deleteVolume'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'already_removed'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'deleteVolume'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_tree_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'deleteVolume'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'invalid_volume_join', 'nonempty_volume'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'setCurrentChapter'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)
    OR (command_kind = 'setCurrentChapter'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'already_current'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'setCurrentChapter'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'stale_current_chapter', 'wrong_target_head'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)
    OR (command_kind = 'setCurrentChapter'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'invalid_chapter_join', 'empty_project'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload ? 'current_authoritative_revision_id'
      AND result_payload - 'reason' - 'current_authoritative_revision_id' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND jsonb_typeof(result_payload->'current_authoritative_revision_id') = 'string'
      AND result_payload->>'reason' IN (
        'stale_authoritative_head', 'proposal_head_present', 'ownership_changed'
      )
      AND result_payload->>'current_authoritative_revision_id' = resulting_heads[1]::text
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND (result_payload->>'reason' <> 'stale_authoritative_head'
        OR expected_heads <> resulting_heads))
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'proposal_revised'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND cardinality(proposal_revision_ids) = 1
      AND array_dims(proposal_revision_ids) = '[1:1]'
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'applyAuthorEdit'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND result_payload->>'reason' IN (
        'unsupported_intent_shape', 'invalid_selection', 'target_mismatch'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)

    OR (command_kind = 'exportHumanReadableManuscript'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'exportHumanReadableManuscript'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'pinned_export_source_unavailable'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'exportProjectArchive'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'exportProjectArchive'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'archived_project', 'pinned_export_source_unavailable'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'undoLatestAuthorAction'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_commit_ids) IN (0, 1)
      AND (
        (cardinality(authoritative_commit_ids) = 1
          AND array_dims(authoritative_commit_ids) = '[1:1]'
          AND (
            (cardinality(authoritative_revision_ids) = 1
              AND array_dims(authoritative_revision_ids) = '[1:1]'
              AND resulting_heads = authoritative_revision_ids)
            OR (cardinality(authoritative_revision_ids) = 0
              AND prior_heads = resulting_heads)
          ))
        OR (cardinality(authoritative_commit_ids) = 0
          AND cardinality(authoritative_revision_ids) = 0
          AND prior_heads = resulting_heads)
      ))
    OR (command_kind = 'undoLatestAuthorAction'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'frontier_mismatch', 'wrong_target_head'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)
    OR (command_kind = 'undoLatestAuthorAction'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'no_frontier', 'barrier'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)

    OR (command_kind = 'updateProjectAssistance'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateProjectAssistance'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'availability_unchanged'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'updateProjectAssistance'
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'stale_assistance_revision'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)

    OR (command_kind = 'createAgentRun'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'acceptProposal'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 1
      AND cardinality(authoritative_commit_ids) = 1
      AND array_dims(authoritative_revision_ids) = '[1:1]'
      AND array_dims(authoritative_commit_ids) = '[1:1]'
      AND resulting_heads = authoritative_revision_ids)
    OR (command_kind = 'acceptProposal'
      AND result_kind IN ('invalid', 'conflicted', 'refused')
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'rejectProposalOperations'
      AND result_kind = 'proposal_operations_resolved'
      AND result_payload ? 'rejection_reason'
      AND result_payload - 'rejection_reason' = '{}'::jsonb
      AND result_payload->>'rejection_reason' = 'author_declined'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'rejectProposalOperations'
      AND result_kind IN ('conflicted', 'refused')
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'reopenRejectedOperations'
      AND result_kind = 'proposal_revised'
      AND result_payload = '{"transition":"reopen_rejected"}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND cardinality(proposal_revision_ids) = 1
      AND array_dims(proposal_revision_ids) = '[1:1]'
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'reopenRejectedOperations'
      AND result_kind IN ('conflicted', 'refused')
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)

  ) IS TRUE);

CREATE OR REPLACE FUNCTION storyos.require_author_edit_receipt_relation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
DECLARE
  scoped_owner_user_id uuid := COALESCE(NEW.owner_user_id, OLD.owner_user_id);
  scoped_project_id uuid := COALESCE(NEW.project_id, OLD.project_id);
  scoped_receipt_id uuid := COALESCE(NEW.receipt_id, OLD.receipt_id);
  scoped_result_kind text;
  scoped_command_kind text;
  scoped_reason text;
  activity_count bigint;
  payload_count bigint;
  action_count bigint;
  commit_count bigint;
  revision_envelope_count bigint;
  archival_count bigint;
  payload_event_kind text;
BEGIN
  SELECT receipt.result_kind, receipt.command_kind, receipt.result_payload->>'reason'
    INTO scoped_result_kind, scoped_command_kind, scoped_reason
    FROM storyos.domain_receipts AS receipt
   WHERE receipt.owner_user_id = scoped_owner_user_id
     AND receipt.project_id = scoped_project_id
     AND receipt.receipt_id = scoped_receipt_id;
  IF NOT FOUND THEN
    RETURN NULL;
  END IF;

  SELECT count(*) INTO activity_count
    FROM storyos.project_activity_events AS activity
   WHERE activity.owner_user_id = scoped_owner_user_id
     AND activity.project_id = scoped_project_id
     AND activity.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO payload_count
    FROM storyos.project_activity_event_payloads AS payload
   WHERE payload.owner_user_id = scoped_owner_user_id
     AND payload.project_id = scoped_project_id
     AND payload.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO action_count
    FROM storyos.author_action_entries AS action
   WHERE action.owner_user_id = scoped_owner_user_id
     AND action.project_id = scoped_project_id
     AND action.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO commit_count
    FROM storyos.authoritative_commits AS authoritative_commit
   WHERE authoritative_commit.owner_user_id = scoped_owner_user_id
     AND authoritative_commit.project_id = scoped_project_id
     AND authoritative_commit.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO revision_envelope_count
    FROM storyos.authoritative_revision_envelopes AS envelope
   WHERE envelope.owner_user_id = scoped_owner_user_id
     AND envelope.project_id = scoped_project_id
     AND envelope.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO archival_count
    FROM storyos.project_archival_decisions AS archival
   WHERE archival.owner_user_id = scoped_owner_user_id
     AND archival.project_id = scoped_project_id
     AND archival.receipt_id = scoped_receipt_id;

  IF scoped_command_kind = 'takeOverProjectWriter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'no_effect'
       OR (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
          <> (0, 0, 0, 0, 1, 0)
       OR payload_event_kind IS DISTINCT FROM scoped_reason THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'takeOverProjectWriter requires one takeover Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_command_kind = 'createProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'authoritative_applied'
       OR (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
          <> (0, 0, 0, 0, 1, 0)
       OR payload_event_kind IS DISTINCT FROM 'project_created' THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createProject requires one project-created Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_command_kind = 'updateProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'project_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateProject applied requires one project-updated Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateProject with zero title effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'archiveProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 1)
         OR payload_event_kind IS DISTINCT FROM 'project_archival_changed' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'archiveProject applied requires one archival decision, Activity, and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'archiveProject with zero lifecycle effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'createVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createVolume applied requires one volume-created Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'updateVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateVolume applied requires one volume-updated Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'updateChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateChapter applied requires one chapter-updated Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'setCurrentChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR commit_count <> 0
         OR revision_envelope_count <> 0
         OR action_count NOT IN (0, 1)
         OR payload_event_kind IS DISTINCT FROM 'current_chapter_set' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'setCurrentChapter applied requires one current-chapter-set Activity, no Commit, and at most one Author Action';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'setCurrentChapter with zero current-chapter effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'createChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createChapter applied requires one chapter-created Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'deleteChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteChapter applied requires one chapter-deleted Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'deleteVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR archival_count <> 0
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteVolume applied requires one volume-deleted Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'exportHumanReadableManuscript' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'human_readable_manuscript_export_settled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'exportHumanReadableManuscript applied requires one export-settled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'exportHumanReadableManuscript with zero export effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'exportProjectArchive' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'project_export_settled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'exportProjectArchive applied requires one export-settled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'exportProjectArchive with zero export effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'undoLatestAuthorAction' THEN
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF NOT (
        (activity_count, action_count, commit_count, revision_envelope_count,
         payload_count, archival_count) = (1, 1, 1, 1, 0, 0)
        OR (activity_count, action_count, commit_count, revision_envelope_count,
            payload_count, archival_count) = (0, 1, 1, 0, 0, 0)
        OR (activity_count, action_count, commit_count, revision_envelope_count,
            payload_count, archival_count) = (0, 1, 0, 0, 0, 0)
      ) THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'undoLatestAuthorAction applied requires prose, structure, or Current Chapter compensation authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'undoLatestAuthorAction with zero authority cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'updateProjectAssistance' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'project_assistance_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateProjectAssistance applied requires one assistance Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateProjectAssistance with zero availability effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'createAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createAgentRun applied requires one AgentRun Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createAgentRun with zero admission effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'rejectProposalOperations'
        AND scoped_result_kind = 'proposal_operations_resolved' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'rejectProposalOperations resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'rejectProposalOperations'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'rejectProposalOperations with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'reopenRejectedOperations'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenRejectedOperations resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'reopenRejectedOperations'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenRejectedOperations with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'applyAuthorEdit'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'applyAuthorEdit ProposalRevised requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_result_kind = 'authoritative_applied' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (1, 1, 1, 1, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'AuthoritativeApplied requires one complete authority and Activity relation';
    END IF;
  ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
         payload_count, archival_count)
          <> (0, 0, 0, 0, 0, 0) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'A zero-authority Receipt cannot have an authority or Activity relation';
  END IF;
  RETURN NULL;
END
$function$;
