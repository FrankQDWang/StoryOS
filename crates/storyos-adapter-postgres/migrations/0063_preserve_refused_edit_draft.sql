SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_kind_check;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_result_kind_check
  CHECK (result_kind IN (
    'authoritative_applied', 'proposal_revised', 'proposal_operations_resolved',
    'proposal_generation_completed', 'proposal_generation_started',
    'no_effect', 'conflicted', 'refused', 'invalid'
  , 'refused_to_draft'));

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_common_shape;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_common_shape CHECK ((
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
  ) IS TRUE));

ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_shape;
ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_result_shape CHECK ((
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
  ) IS TRUE));

ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_draft_source_key
  UNIQUE (owner_user_id, project_id, receipt_id, author_command_admission_id, command_id, result_kind);

CREATE TABLE storyos.draft_artifacts (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  draft_id uuid NOT NULL,
  draft_kind text NOT NULL DEFAULT 'refused_edit' CHECK (draft_kind = 'refused_edit'),
  current_revision_id uuid NOT NULL,
  closure text NOT NULL DEFAULT 'open' CHECK (closure IN ('open', 'closed')),
  retention_state text NOT NULL DEFAULT 'retained' CHECK (retention_state IN ('retained', 'archived', 'tombstoned')),
  PRIMARY KEY (owner_user_id, project_id, draft_id),
  FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects (owner_user_id, project_id) MATCH FULL
);

CREATE TABLE storyos.draft_artifact_revisions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  draft_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  payload jsonb NOT NULL CHECK ((jsonb_typeof(payload) = 'object'
    AND payload->>'schema_revision' = 'storyos.refused-edit-payload.v1'
    AND payload ?& ARRAY['schema_revision', 'chapter_id', 'expected_authoritative_revision_id', 'expected_proposal_head_revision_ids', 'target_refs', 'author_edit_units', 'undo_group_id', 'completed_intent_record_id', 'local_intent_sequence']
    AND payload - ARRAY['schema_revision', 'chapter_id', 'expected_authoritative_revision_id', 'expected_proposal_head_revision_ids', 'target_refs', 'author_edit_units', 'undo_group_id', 'completed_intent_record_id', 'local_intent_sequence'] = '{}'::jsonb) IS TRUE),
  payload_digest text NOT NULL CHECK (payload_digest ~ '^[0-9a-f]{64}$'),
  payload_digest_profile text NOT NULL DEFAULT 'storyos.refused-edit-payload.jcs.v1'
    CHECK (payload_digest_profile = 'storyos.refused-edit-payload.jcs.v1'),
  created_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, draft_id, revision_id),
  UNIQUE (owner_user_id, project_id, draft_id),
  FOREIGN KEY (owner_user_id, project_id, draft_id)
    REFERENCES storyos.draft_artifacts (owner_user_id, project_id, draft_id) MATCH FULL
    DEFERRABLE INITIALLY DEFERRED
);
ALTER TABLE storyos.draft_artifacts ADD CONSTRAINT draft_artifacts_exact_revision_fk
  FOREIGN KEY (owner_user_id, project_id, draft_id, current_revision_id)
    REFERENCES storyos.draft_artifact_revisions (owner_user_id, project_id, draft_id, revision_id) MATCH FULL
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE storyos.draft_lifecycle_events (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  creation_event_id uuid NOT NULL,
  event_kind text NOT NULL DEFAULT 'refused_edit_draft_created' CHECK (event_kind = 'refused_edit_draft_created'),
  draft_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  receipt_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  command_id uuid NOT NULL,
  receipt_result_kind text NOT NULL DEFAULT 'refused_to_draft' CHECK (receipt_result_kind = 'refused_to_draft'),
  created_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, creation_event_id),
  UNIQUE (owner_user_id, project_id, receipt_id),
  UNIQUE (owner_user_id, project_id, draft_id, revision_id),
  FOREIGN KEY (owner_user_id, project_id, draft_id, revision_id)
    REFERENCES storyos.draft_artifact_revisions (owner_user_id, project_id, draft_id, revision_id) MATCH FULL
    DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (owner_user_id, project_id, receipt_id, author_command_admission_id, command_id, receipt_result_kind)
    REFERENCES storyos.domain_receipts
      (owner_user_id, project_id, receipt_id, author_command_admission_id, command_id, result_kind) MATCH FULL
    DEFERRABLE INITIALLY DEFERRED
);

CREATE FUNCTION storyos.require_refused_draft_settlement() RETURNS trigger
LANGUAGE plpgsql AS $function$
DECLARE
  target_receipt uuid;
BEGIN
  IF TG_TABLE_NAME = 'draft_artifacts' THEN
    SELECT receipt_id INTO target_receipt FROM storyos.draft_lifecycle_events
     WHERE owner_user_id = NEW.owner_user_id AND project_id = NEW.project_id
       AND draft_id = NEW.draft_id AND revision_id = NEW.current_revision_id;
  ELSE
    target_receipt := NEW.receipt_id;
  END IF;
  IF TG_TABLE_NAME = 'domain_receipts' THEN
    IF NEW.result_kind <> 'refused_to_draft' THEN RETURN NULL; END IF;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM storyos.domain_receipts AS receipt
      JOIN storyos.draft_lifecycle_events AS event USING (owner_user_id, project_id, receipt_id)
      JOIN storyos.draft_artifact_revisions AS revision
        ON (revision.owner_user_id, revision.project_id, revision.draft_id, revision.revision_id) =
           (event.owner_user_id, event.project_id, event.draft_id, event.revision_id)
      JOIN storyos.author_command_admissions AS admission
        ON (admission.owner_user_id, admission.project_id, admission.author_command_admission_id) =
           (event.owner_user_id, event.project_id, event.author_command_admission_id)
     WHERE receipt.owner_user_id = NEW.owner_user_id AND receipt.project_id = NEW.project_id
       AND receipt.receipt_id = target_receipt AND receipt.result_kind = 'refused_to_draft'
       AND receipt.draft_artifact_refs = ARRAY[event.draft_id::text]
       AND receipt.artifact_lifecycle_event_refs = ARRAY[event.creation_event_id::text]
       AND receipt.result_payload->>'draft_revision_id' = event.revision_id::text
       AND revision.payload->>'completed_intent_record_id' = admission.completed_intent_record_id::text
       AND revision.payload->>'local_intent_sequence' = admission.local_intent_sequence::text
       AND revision.payload->>'undo_group_id' = admission.undo_group_id::text
       AND revision.payload->>'chapter_id' = admission.chapter_object_id::text
       AND revision.payload->>'expected_authoritative_revision_id' = admission.expected_authoritative_revision_id::text
       AND revision.payload->'expected_proposal_head_revision_ids' = to_jsonb(admission.expected_proposal_head_revision_ids)
       AND revision.payload->'target_refs' = to_jsonb(admission.target_refs)
       AND revision.payload->'author_edit_units' = admission.command_payload->'author_edit_units'
       AND revision.created_at = event.created_at
  ) THEN RAISE EXCEPTION 'Incomplete Refused Edit Draft settlement' USING ERRCODE = '23514'; END IF;
  RETURN NULL;
END
$function$;
CREATE CONSTRAINT TRIGGER refused_draft_artifact_complete AFTER INSERT ON storyos.draft_artifacts
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_refused_draft_settlement();
CREATE CONSTRAINT TRIGGER refused_draft_receipt_complete AFTER INSERT ON storyos.domain_receipts
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_refused_draft_settlement();
CREATE CONSTRAINT TRIGGER refused_draft_creation_complete AFTER INSERT ON storyos.draft_lifecycle_events
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_refused_draft_settlement();

CREATE FUNCTION storyos.keep_refused_draft_revision_immutable() RETURNS trigger
LANGUAGE plpgsql AS $function$
BEGIN
  RAISE EXCEPTION 'Refused Edit Draft Revision and creation are immutable' USING ERRCODE = '23514';
END
$function$;
CREATE TRIGGER refused_draft_revision_immutable BEFORE UPDATE OR DELETE ON storyos.draft_artifact_revisions
  FOR EACH ROW EXECUTE FUNCTION storyos.keep_refused_draft_revision_immutable();
CREATE TRIGGER refused_draft_creation_immutable BEFORE UPDATE OR DELETE ON storyos.draft_lifecycle_events
  FOR EACH ROW EXECUTE FUNCTION storyos.keep_refused_draft_revision_immutable();

ALTER TABLE storyos.draft_artifacts ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_artifacts FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_artifact_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_artifact_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_lifecycle_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_lifecycle_events FORCE ROW LEVEL SECURITY;
CREATE POLICY draft_artifacts_exact_scope ON storyos.draft_artifacts USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY draft_artifact_revisions_exact_scope ON storyos.draft_artifact_revisions USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
CREATE POLICY draft_lifecycle_events_exact_scope ON storyos.draft_lifecycle_events USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.draft_artifacts, storyos.draft_artifact_revisions,
  storyos.draft_lifecycle_events TO storyos_runtime;
