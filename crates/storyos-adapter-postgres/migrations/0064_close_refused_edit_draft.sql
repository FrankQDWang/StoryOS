ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_kind_check;
ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_result_kind_check CHECK (result_kind IN ('draft_closure_changed', 
    'authoritative_applied', 'proposal_revised', 'proposal_operations_resolved',
    'proposal_generation_completed', 'proposal_generation_started',
    'no_effect', 'conflicted', 'refused', 'invalid'
  , 'refused_to_draft'));

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_command_kind_check;
ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_command_kind_check CHECK (command_kind IN ('closeEditorFlowDraft', 
    'applyAuthorEdit', 'takeOverProjectWriter', 'createProject', 'updateProject',
    'archiveProject', 'createVolume', 'createChapter', 'updateVolume', 'updateChapter',
    'setCurrentChapter', 'undoLatestAuthorAction', 'deleteChapter', 'deleteVolume',
    'exportHumanReadableManuscript', 'exportProjectArchive',
    'updateProjectAssistance', 'createAgentRun', 'pauseAgentRun',
    'cancelAgentRun', 'acceptProposal',
    'rejectProposalOperations', 'reopenRejectedOperations',
    'completeReadyPartialProposal', 'continueProposalGeneration'
  ));

ALTER TABLE storyos.author_action_entries DROP CONSTRAINT author_action_entries_receipt_result_kind_check;
ALTER TABLE storyos.author_action_entries ADD CONSTRAINT author_action_entries_receipt_result_kind_check CHECK (receipt_result_kind IN ('draft_closure_changed', 
    'authoritative_applied', 'proposal_revised', 'proposal_operations_resolved',
    'proposal_generation_completed', 'proposal_generation_started'
  ));

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_common_shape;
ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_common_shape CHECK ((
(
(
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
        'updateProjectAssistance', 'createAgentRun', 'pauseAgentRun',
        'cancelAgentRun'
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
        'updateProjectAssistance', 'createAgentRun', 'pauseAgentRun',
        'cancelAgentRun'
      )
        AND cardinality(expected_heads) = 0
        AND cardinality(prior_heads) = 0
        AND cardinality(resulting_heads) = 0)
    )
  ) IS TRUE
  ) OR ((
    command_kind = 'applyAuthorEdit' AND result_kind = 'refused_to_draft'
      AND array_dims(expected_heads) = '[1:1]' AND expected_heads[1] IS NOT NULL
      AND prior_heads = expected_heads AND resulting_heads = expected_heads
      AND cardinality(authoritative_revision_ids) = 0 AND cardinality(proposal_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0 AND cardinality(condition_refs) = 0
      AND array_dims(draft_artifact_refs) = '[1:1]' AND draft_artifact_refs[1] IS NOT NULL
      AND array_dims(artifact_lifecycle_event_refs) = '[1:1]'
      AND artifact_lifecycle_event_refs[1] IS NOT NULL
  ) IS TRUE)
) OR ((
command_kind='closeEditorFlowDraft' AND expected_heads='{}' AND prior_heads='{}' AND resulting_heads='{}'
AND authoritative_revision_ids='{}' AND proposal_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
AND array_dims(draft_artifact_refs)='[1:1]' AND draft_artifact_refs[1] IS NOT NULL
AND ((result_kind='draft_closure_changed' AND array_dims(artifact_lifecycle_event_refs)='[1:1]' AND artifact_lifecycle_event_refs[1] IS NOT NULL)
OR (result_kind IN ('refused','conflicted') AND artifact_lifecycle_event_refs='{}'))
) IS TRUE));

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_shape;
ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_result_shape CHECK ((
(
(
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
    OR (command_kind IN ('pauseAgentRun', 'cancelAgentRun')
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'pauseAgentRun'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'already_paused'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind = 'cancelAgentRun'
      AND result_kind = 'no_effect'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'already_cancelled'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0)
    OR (command_kind IN ('pauseAgentRun', 'cancelAgentRun')
      AND result_kind = 'conflicted'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' = 'terminal_run'
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
    OR (command_kind = 'completeReadyPartialProposal'
      AND result_kind = 'proposal_generation_completed'
      AND result_payload = '{"transition":"generation_completed"}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'completeReadyPartialProposal'
      AND result_kind IN ('conflicted', 'refused')
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'continueProposalGeneration'
      AND result_kind = 'proposal_generation_started'
      AND result_payload = '{"transition":"generation_started"}'::jsonb
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)
    OR (command_kind = 'continueProposalGeneration'
      AND result_kind IN ('conflicted', 'refused')
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND jsonb_typeof(result_payload->'reason') = 'string'
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads
      AND expected_heads = resulting_heads)

  ) IS TRUE
  ) OR ((
    command_kind = 'applyAuthorEdit' AND result_kind = 'refused_to_draft'
      AND jsonb_typeof(result_payload) = 'object'
      AND result_payload ?& ARRAY['draft_id', 'draft_revision_id', 'creation_event_id']
      AND result_payload - ARRAY['draft_id', 'draft_revision_id', 'creation_event_id'] = '{}'::jsonb
      AND result_payload->>'draft_id' = draft_artifact_refs[1]
      AND result_payload->>'creation_event_id' = artifact_lifecycle_event_refs[1]
      AND result_payload->>'draft_revision_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
      AND cardinality(authoritative_revision_ids) = 0 AND cardinality(authoritative_commit_ids) = 0
      AND cardinality(proposal_revision_ids) = 0
      AND prior_heads = expected_heads AND resulting_heads = expected_heads
  ) IS TRUE)
) OR ((
command_kind='closeEditorFlowDraft' AND result_payload ?& ARRAY['draft_revision_id','payload_digest','observed_closure','event_id','reason']
AND result_payload - ARRAY['draft_revision_id','payload_digest','observed_closure','event_id','reason']='{}'
AND result_payload->>'payload_digest' ~ '^[0-9a-f]{64}$' AND result_payload->>'observed_closure' IN ('open','closed')
AND ((result_kind='draft_closure_changed' AND result_payload->>'reason'='abandoned' AND result_payload->>'observed_closure'='open'
AND result_payload->>'event_id'=artifact_lifecycle_event_refs[1]) OR
(result_kind='conflicted' AND result_payload->>'reason'='source_binding_changed' AND result_payload->'event_id'='null') OR
(result_kind='refused' AND result_payload->>'reason' IN ('source_draft_not_open','source_unavailable') AND result_payload->'event_id'='null'))
) IS TRUE));

ALTER TABLE storyos.author_command_admissions DROP CONSTRAINT author_command_admissions_command_shape;
ALTER TABLE storyos.author_command_admissions ADD CONSTRAINT author_command_admissions_command_shape CHECK ((
(
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
    OR (command_kind IN ('pauseAgentRun', 'cancelAgentRun')
      AND action_class = 'agent_run_control'
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
    OR (command_kind IN ('completeReadyPartialProposal', 'continueProposalGeneration')
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

  ) IS TRUE
) OR ((
command_kind='closeEditorFlowDraft' AND action_class='explicit_editor_command' AND editor_session_id IS NOT NULL AND writer_generation IS NOT NULL
AND chapter_object_id IS NULL AND expected_authoritative_revision_id IS NULL AND expected_proposal_head_revision_ids='{}' AND target_refs='{}'
AND observed_ownership_partition IS NULL AND undo_group_id IS NULL AND completed_intent_record_id IS NULL AND local_intent_sequence IS NULL
AND challenge_consumed_at IS NOT NULL AND challenge_expires_at IS NOT NULL
) IS TRUE));

CREATE TABLE storyos.draft_close_events (
  owner_user_id uuid NOT NULL, project_id uuid NOT NULL, event_id uuid NOT NULL,
  draft_id uuid NOT NULL, revision_id uuid NOT NULL, payload_digest text NOT NULL CHECK (payload_digest ~ '^[0-9a-f]{64}$'),
  receipt_id uuid NOT NULL, receipt_result_kind text NOT NULL DEFAULT 'draft_closure_changed' CHECK (receipt_result_kind='draft_closure_changed'),
  author_action_sequence numeric(20,0) NOT NULL, created_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
  PRIMARY KEY(owner_user_id,project_id,event_id), UNIQUE(owner_user_id,project_id,receipt_id),
  FOREIGN KEY(owner_user_id,project_id,draft_id,revision_id) REFERENCES storyos.draft_artifact_revisions(owner_user_id,project_id,draft_id,revision_id) MATCH FULL DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_user_id,project_id,receipt_id,receipt_result_kind) REFERENCES storyos.domain_receipts(owner_user_id,project_id,receipt_id,result_kind) MATCH FULL DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_user_id,project_id,author_action_sequence) REFERENCES storyos.author_action_entries(owner_user_id,project_id,author_action_sequence) MATCH FULL DEFERRABLE INITIALLY DEFERRED
);
ALTER TABLE storyos.draft_artifacts ADD COLUMN close_event_id uuid;
ALTER TABLE storyos.draft_artifacts ADD CONSTRAINT draft_exact_close_event_fk FOREIGN KEY(owner_user_id,project_id,close_event_id)
  REFERENCES storyos.draft_close_events(owner_user_id,project_id,event_id) DEFERRABLE INITIALLY DEFERRED;
CREATE TRIGGER draft_close_event_immutable BEFORE UPDATE OR DELETE ON storyos.draft_close_events
  FOR EACH ROW EXECUTE FUNCTION storyos.keep_refused_draft_revision_immutable();
CREATE FUNCTION storyos.require_draft_close_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE target_event uuid; target_draft uuid; target_revision uuid; target_receipt uuid;
BEGIN
  IF TG_TABLE_NAME='draft_artifacts' THEN
    IF TG_OP='UPDATE' AND OLD.closure='closed' AND
      (NEW.closure <> 'closed' OR NEW.close_event_id IS DISTINCT FROM OLD.close_event_id) THEN
      RAISE EXCEPTION 'Draft Discard cannot reopen without compensation' USING ERRCODE='23514';
    END IF;
    target_draft := NEW.draft_id;
    target_revision := NEW.current_revision_id;
    IF NEW.closure='open' AND NEW.close_event_id IS NULL THEN RETURN NULL; END IF;
    IF NEW.closure <> 'closed' OR NEW.close_event_id IS NULL THEN
      RAISE EXCEPTION 'Incomplete Draft Discard settlement' USING ERRCODE='23514';
    END IF;
    target_event := NEW.close_event_id;
  ELSIF TG_TABLE_NAME='domain_receipts' THEN
    IF NEW.command_kind <> 'closeEditorFlowDraft' OR NEW.result_kind <> 'draft_closure_changed' THEN RETURN NULL; END IF;
    target_event := (NEW.result_payload->>'event_id')::uuid;
    target_receipt := NEW.receipt_id;
  ELSE
    target_event := NEW.event_id;
  END IF;
  IF NOT EXISTS(SELECT 1 FROM storyos.draft_close_events AS event
    JOIN storyos.domain_receipts AS receipt USING(owner_user_id,project_id,receipt_id)
    JOIN storyos.author_action_entries AS action USING(owner_user_id,project_id,receipt_id)
    JOIN storyos.draft_artifacts AS draft ON (draft.owner_user_id,draft.project_id,draft.draft_id,draft.current_revision_id,draft.close_event_id)=
      (event.owner_user_id,event.project_id,event.draft_id,event.revision_id,event.event_id)
    JOIN storyos.draft_artifact_revisions AS revision ON (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
      (event.owner_user_id,event.project_id,event.draft_id,event.revision_id)
    JOIN storyos.author_command_admissions AS admission ON (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=
      (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
    WHERE event.owner_user_id=NEW.owner_user_id AND event.project_id=NEW.project_id AND event.event_id=target_event
      AND (target_draft IS NULL OR (event.draft_id=target_draft AND event.revision_id=target_revision))
      AND (target_receipt IS NULL OR event.receipt_id=target_receipt)
      AND draft.closure='closed' AND receipt.command_kind='closeEditorFlowDraft' AND receipt.result_kind='draft_closure_changed'
      AND receipt.draft_artifact_refs=ARRAY[event.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]
      AND receipt.result_payload->>'draft_revision_id'=event.revision_id::text AND receipt.result_payload->>'payload_digest'=event.payload_digest
      AND event.payload_digest=revision.payload_digest AND action.disposition='forward' AND action.author_action_sequence=event.author_action_sequence
      AND admission.command_payload->'close_editor_flow_draft_input'->>'source_current_draft_revision_id'=event.revision_id::text
      AND admission.command_payload->'close_editor_flow_draft_input'->>'source_draft_payload_digest'=event.payload_digest
      AND admission.command_payload->'close_editor_flow_draft_input'->>'expected_closure'='open'
      AND admission.command_payload->'close_editor_flow_draft_input'->>'close_reason'='abandoned'
      AND admission.command_payload->'close_editor_flow_draft_input'->>'draft_kind'='refused_edit'
      AND admission.command_kind=receipt.command_kind AND admission.canonical_command_digest=receipt.command_digest
      AND admission.idempotency_key=receipt.idempotency_key AND admission.command_id=receipt.command_id
      AND event.created_at=receipt.created_at AND receipt.result_payload->>'event_id'=event.event_id::text
      AND receipt.result_payload->>'observed_closure'='open'
      AND EXISTS(SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
        WHERE (settlement.owner_user_id,settlement.project_id,settlement.author_command_admission_id,settlement.receipt_id)=
        (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id,receipt.receipt_id) AND settlement.settlement_kind='receipt_settled')
      AND EXISTS(SELECT 1 FROM storyos.command_idempotency AS replay
        WHERE (replay.owner_user_id,replay.project_id,replay.command_kind,replay.idempotency_key)=
        (receipt.owner_user_id,receipt.project_id,receipt.command_kind,receipt.idempotency_key)
        AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text))
  THEN RAISE EXCEPTION 'Incomplete Draft Discard settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;
CREATE CONSTRAINT TRIGGER draft_close_complete AFTER INSERT ON storyos.draft_close_events DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_close_settlement();
CREATE CONSTRAINT TRIGGER draft_close_projection_complete AFTER INSERT OR UPDATE OF closure,close_event_id ON storyos.draft_artifacts
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_close_settlement();
CREATE CONSTRAINT TRIGGER draft_close_receipt_complete AFTER INSERT ON storyos.domain_receipts
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_close_settlement();
ALTER TABLE storyos.draft_close_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_close_events FORCE ROW LEVEL SECURITY;
CREATE POLICY draft_close_scope ON storyos.draft_close_events USING (
  owner_user_id=current_setting('storyos.owner_user_id',true)::uuid AND project_id=current_setting('storyos.project_id',true)::uuid
) WITH CHECK (
  owner_user_id=current_setting('storyos.owner_user_id',true)::uuid AND project_id=current_setting('storyos.project_id',true)::uuid
);
GRANT SELECT,INSERT ON storyos.draft_close_events TO storyos_runtime;
