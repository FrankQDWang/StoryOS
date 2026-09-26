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
) IS TRUE) OR ((
command_kind='undoLatestAuthorAction' AND result_kind='draft_closure_changed'
AND array_dims(expected_heads)='[1:1]' AND expected_heads[1] IS NOT NULL AND prior_heads=expected_heads AND resulting_heads=expected_heads
AND authoritative_revision_ids='{}' AND proposal_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
AND array_dims(draft_artifact_refs)='[1:1]' AND draft_artifact_refs[1] IS NOT NULL
AND array_dims(artifact_lifecycle_event_refs)='[1:1]' AND artifact_lifecycle_event_refs[1] IS NOT NULL
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
        'frontier_mismatch', 'wrong_target_head', 'source_binding_changed'
      )
      AND cardinality(authoritative_revision_ids) = 0
      AND cardinality(authoritative_commit_ids) = 0
      AND prior_heads = resulting_heads)
    OR (command_kind = 'undoLatestAuthorAction'
      AND result_kind = 'refused'
      AND result_payload ? 'reason'
      AND result_payload - 'reason' = '{}'::jsonb
      AND result_payload->>'reason' IN (
        'no_frontier', 'barrier', 'source_unavailable'
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
) IS TRUE) OR ((
command_kind='undoLatestAuthorAction' AND result_kind='draft_closure_changed'
AND result_payload ?& ARRAY['event_id','handler_receipt_id','author_undo_frontier_sequence']
AND result_payload - ARRAY['event_id','handler_receipt_id','author_undo_frontier_sequence']='{}'::jsonb
AND result_payload->>'event_id'=artifact_lifecycle_event_refs[1]
AND result_payload->>'handler_receipt_id' ~ '^[0-9a-f-]{36}$'
AND (result_payload->'author_undo_frontier_sequence'='null'::jsonb OR result_payload->>'author_undo_frontier_sequence' ~ '^[1-9][0-9]*$')
) IS TRUE));

CREATE TABLE storyos.draft_reopen_receipts (
  owner_user_id uuid NOT NULL, project_id uuid NOT NULL, receipt_id uuid NOT NULL,
  author_undo_receipt_id uuid NOT NULL, source_close_event_id uuid NOT NULL, event_id uuid NOT NULL,
  payload jsonb NOT NULL,
  PRIMARY KEY(owner_user_id,project_id,receipt_id), UNIQUE(owner_user_id,project_id,author_undo_receipt_id),
  FOREIGN KEY(owner_user_id,project_id,author_undo_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id,project_id,receipt_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_user_id,project_id,source_close_event_id) REFERENCES storyos.draft_close_events(owner_user_id,project_id,event_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE storyos.draft_reopen_events (
  owner_user_id uuid NOT NULL, project_id uuid NOT NULL, event_id uuid NOT NULL,
  draft_id uuid NOT NULL, revision_id uuid NOT NULL, source_close_event_id uuid NOT NULL,
  handler_receipt_id uuid NOT NULL, author_action_sequence numeric(20,0) NOT NULL, payload jsonb NOT NULL,
  PRIMARY KEY(owner_user_id,project_id,event_id), UNIQUE(owner_user_id,project_id,handler_receipt_id),
  UNIQUE(owner_user_id,project_id,source_close_event_id),
  FOREIGN KEY(owner_user_id,project_id,draft_id,revision_id) REFERENCES storyos.draft_artifact_revisions(owner_user_id,project_id,draft_id,revision_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_user_id,project_id,handler_receipt_id) REFERENCES storyos.draft_reopen_receipts(owner_user_id,project_id,receipt_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_user_id,project_id,source_close_event_id) REFERENCES storyos.draft_close_events(owner_user_id,project_id,event_id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY(owner_user_id,project_id,author_action_sequence) REFERENCES storyos.author_action_entries(owner_user_id,project_id,author_action_sequence) DEFERRABLE INITIALLY DEFERRED
);
ALTER TABLE storyos.draft_reopen_receipts ADD FOREIGN KEY(owner_user_id,project_id,event_id)
  REFERENCES storyos.draft_reopen_events(owner_user_id,project_id,event_id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE storyos.draft_artifacts ADD COLUMN reopen_event_id uuid;
ALTER TABLE storyos.draft_artifacts ADD FOREIGN KEY(owner_user_id,project_id,reopen_event_id)
  REFERENCES storyos.draft_reopen_events(owner_user_id,project_id,event_id) DEFERRABLE INITIALLY DEFERRED;
CREATE TRIGGER draft_reopen_event_immutable BEFORE UPDATE OR DELETE ON storyos.draft_reopen_events
  FOR EACH ROW EXECUTE FUNCTION storyos.keep_refused_draft_revision_immutable();
CREATE TRIGGER draft_reopen_receipt_immutable BEFORE UPDATE OR DELETE ON storyos.draft_reopen_receipts
  FOR EACH ROW EXECUTE FUNCTION storyos.keep_refused_draft_revision_immutable();
ALTER TABLE storyos.draft_reopen_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_reopen_events FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_reopen_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.draft_reopen_receipts FORCE ROW LEVEL SECURITY;
CREATE POLICY draft_reopen_event_scope ON storyos.draft_reopen_events USING (
  owner_user_id=current_setting('storyos.owner_user_id',true)::uuid AND project_id=current_setting('storyos.project_id',true)::uuid
) WITH CHECK (
  owner_user_id=current_setting('storyos.owner_user_id',true)::uuid AND project_id=current_setting('storyos.project_id',true)::uuid
);
CREATE POLICY draft_reopen_receipt_scope ON storyos.draft_reopen_receipts USING (
  owner_user_id=current_setting('storyos.owner_user_id',true)::uuid AND project_id=current_setting('storyos.project_id',true)::uuid
) WITH CHECK (
  owner_user_id=current_setting('storyos.owner_user_id',true)::uuid AND project_id=current_setting('storyos.project_id',true)::uuid
);
GRANT SELECT,INSERT ON storyos.draft_reopen_events,storyos.draft_reopen_receipts TO storyos_runtime;
GRANT UPDATE(reopen_event_id) ON storyos.draft_artifacts TO storyos_runtime;

CREATE FUNCTION storyos.require_draft_reopen_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE target_event uuid;
BEGIN
  IF TG_TABLE_NAME='domain_receipts' THEN
    IF NEW.command_kind<>'undoLatestAuthorAction' OR NEW.result_kind<>'draft_closure_changed' THEN RETURN NULL; END IF;
    target_event:=(NEW.result_payload->>'event_id')::uuid;
  ELSE target_event:=NEW.event_id;
  END IF;
  IF NOT EXISTS(SELECT 1 FROM storyos.draft_reopen_events AS event
    JOIN storyos.draft_reopen_receipts AS handler ON (handler.owner_user_id,handler.project_id,handler.receipt_id)=
      (event.owner_user_id,event.project_id,event.handler_receipt_id)
    JOIN storyos.draft_close_events AS closed ON (closed.owner_user_id,closed.project_id,closed.event_id)=
      (event.owner_user_id,event.project_id,event.source_close_event_id)
    JOIN storyos.domain_receipts AS receipt ON (receipt.owner_user_id,receipt.project_id,receipt.receipt_id)=
      (handler.owner_user_id,handler.project_id,handler.author_undo_receipt_id)
    JOIN storyos.author_action_entries AS action ON (action.owner_user_id,action.project_id,action.author_action_sequence)=
      (event.owner_user_id,event.project_id,event.author_action_sequence)
    JOIN storyos.author_command_admissions AS admission ON (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=
      (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
    JOIN storyos.draft_artifacts AS draft ON (draft.owner_user_id,draft.project_id,draft.draft_id)=
      (event.owner_user_id,event.project_id,event.draft_id)
    WHERE event.owner_user_id=NEW.owner_user_id AND event.project_id=NEW.project_id AND event.event_id=target_event
      AND draft.closure='open' AND draft.retention_state='retained' AND draft.current_revision_id=event.revision_id
      AND draft.close_event_id=closed.event_id AND draft.reopen_event_id=event.event_id
      AND admission.command_kind=receipt.command_kind AND admission.command_id=receipt.command_id
      AND admission.canonical_command_digest=receipt.command_digest AND admission.idempotency_key=receipt.idempotency_key
      AND EXISTS(SELECT 1 FROM storyos.author_command_admission_settlements AS settlement WHERE
        (settlement.owner_user_id,settlement.project_id,settlement.author_command_admission_id,settlement.receipt_id)=
        (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id,receipt.receipt_id) AND settlement.settlement_kind='receipt_settled')
      AND receipt.command_kind='undoLatestAuthorAction'  AND receipt.result_kind='draft_closure_changed'
      AND receipt.authoritative_revision_ids='{}' AND receipt.authoritative_commit_ids='{}' AND receipt.proposal_revision_ids='{}'
      AND receipt.draft_artifact_refs=ARRAY[event.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]
      AND receipt.result_payload->>'event_id'=event.event_id::text AND receipt.result_payload->>'handler_receipt_id'=handler.receipt_id::text
      AND action.disposition='compensation' AND action.compensated_source_sequence=closed.author_action_sequence AND action.receipt_id=receipt.receipt_id
      AND (closed.draft_id,closed.revision_id)=(event.draft_id,event.revision_id)
      AND (handler.event_id,handler.source_close_event_id)=(event.event_id,event.source_close_event_id)
      AND admission.command_payload->'undo_latest_author_action_input'->>'expected_author_undo_frontier_sequence'=closed.author_action_sequence::text
      AND closed.author_action_sequence=(SELECT max(source.author_action_sequence) FROM storyos.author_action_entries AS source
        WHERE source.owner_user_id=event.owner_user_id AND source.project_id=event.project_id AND source.disposition='forward'
        AND source.author_action_sequence<action.author_action_sequence AND NOT EXISTS(SELECT 1 FROM storyos.author_action_entries AS prior
          WHERE prior.owner_user_id=source.owner_user_id AND prior.project_id=source.project_id AND prior.disposition='compensation'
          AND prior.compensated_source_sequence=source.author_action_sequence AND prior.author_action_sequence<action.author_action_sequence))
      AND handler.payload=jsonb_build_object('schema_id','storyos.receipt.draft-reopen.v1','receipt_id',handler.receipt_id::text,
        'project_scope',jsonb_build_object('owner_user_id',event.owner_user_id::text,'project_id',event.project_id::text),
        'author_undo_receipt_id',receipt.receipt_id::text,'source_close_event_id',closed.event_id::text,'event_id',event.event_id::text,
        'result','draft_reopened','created_at',to_char(receipt.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'))
      AND event.payload=jsonb_build_object('schema_id','storyos.event.editor-flow-draft-reopened.v1','event_kind','editor_flow_draft_reopened',
        'event_id',event.event_id::text,'project_scope',handler.payload->'project_scope','draft_id',event.draft_id::text,
        'draft_revision_id',event.revision_id::text,'payload_digest',closed.payload_digest,'source_close_event_id',closed.event_id::text,
        'prior_closure','closed','closure','open','handler_receipt',handler.payload,'source_author_action_sequence',closed.author_action_sequence::text,
        'author_action_sequence',action.author_action_sequence::text,'created_at',handler.payload->>'created_at',
        'source',jsonb_build_object('command_id',receipt.command_id::text,'author_command_admission_id',receipt.author_command_admission_id::text,
          'receipt_id',receipt.receipt_id::text,'idempotency_key',receipt.idempotency_key::text,'command_digest',jsonb_build_object(
          'algorithm','sha256','profile','storyos.command.undoLatestAuthorAction.jcs.v1','value_hex_lowercase',split_part(receipt.command_digest,':',3))))
      AND EXISTS(SELECT 1 FROM storyos.command_idempotency AS replay WHERE
        (replay.owner_user_id,replay.project_id,replay.command_kind,replay.idempotency_key)=
        (receipt.owner_user_id,receipt.project_id,receipt.command_kind,receipt.idempotency_key)
        AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text AND replay.canonical_command_digest=receipt.command_digest))
  THEN RAISE EXCEPTION 'Incomplete Draft reopen settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;
CREATE CONSTRAINT TRIGGER draft_reopen_complete AFTER INSERT ON storyos.draft_reopen_events DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_reopen_settlement();
CREATE CONSTRAINT TRIGGER draft_reopen_handler_complete AFTER INSERT ON storyos.draft_reopen_receipts DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_reopen_settlement();
CREATE CONSTRAINT TRIGGER draft_reopen_root_complete AFTER INSERT ON storyos.domain_receipts DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_reopen_settlement();

CREATE OR REPLACE FUNCTION storyos.require_draft_close_settlement() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE target_event uuid; target_draft uuid; target_revision uuid; target_receipt uuid;
  prior_reopen_event text; validate_lifecycle boolean := false;
BEGIN
  IF TG_TABLE_NAME='draft_artifacts' THEN
    target_draft := NEW.draft_id;
    target_revision := NEW.current_revision_id;
    IF TG_OP='UPDATE' THEN
      IF OLD.closure='closed' AND NEW.closure='closed' AND NEW.close_event_id IS DISTINCT FROM OLD.close_event_id THEN
        RAISE EXCEPTION 'Draft Discard requires open source' USING ERRCODE='23514';
      END IF;
      validate_lifecycle := OLD.closure='open' AND NEW.closure='closed';
      prior_reopen_event := OLD.reopen_event_id::text;
    END IF;
    IF NEW.closure='open' THEN
      IF TG_OP='UPDATE' AND OLD.closure='closed' AND
        (NEW.close_event_id IS DISTINCT FROM OLD.close_event_id OR NEW.reopen_event_id IS NULL) THEN
        RAISE EXCEPTION 'Draft Discard requires exact compensation' USING ERRCODE='23514';
      END IF;
      IF NEW.close_event_id IS NULL AND NEW.reopen_event_id IS NULL THEN RETURN NULL; END IF;
      IF NOT EXISTS(SELECT 1 FROM storyos.draft_reopen_events AS reopened WHERE
        (reopened.owner_user_id,reopened.project_id,reopened.event_id,reopened.draft_id,reopened.revision_id,reopened.source_close_event_id)=
        (NEW.owner_user_id,NEW.project_id,NEW.reopen_event_id,NEW.draft_id,NEW.current_revision_id,NEW.close_event_id))
      THEN RAISE EXCEPTION 'Draft reopen requires exact compensation' USING ERRCODE='23514'; END IF;
      RETURN NULL;
    END IF;
    IF NEW.reopen_event_id IS NOT NULL THEN RAISE EXCEPTION 'Closed Draft cannot project reopen' USING ERRCODE='23514'; END IF;
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
    JOIN storyos.draft_artifact_revisions AS revision ON (revision.owner_user_id,revision.project_id,revision.draft_id,revision.revision_id)=
      (event.owner_user_id,event.project_id,event.draft_id,event.revision_id)
    JOIN storyos.author_command_admissions AS admission ON (admission.owner_user_id,admission.project_id,admission.author_command_admission_id)=
      (receipt.owner_user_id,receipt.project_id,receipt.author_command_admission_id)
    WHERE event.owner_user_id=NEW.owner_user_id AND event.project_id=NEW.project_id AND event.event_id=target_event
      AND (target_draft IS NULL OR (event.draft_id=target_draft AND event.revision_id=target_revision))
      AND (target_receipt IS NULL OR event.receipt_id=target_receipt)
      AND receipt.command_kind='closeEditorFlowDraft' AND receipt.result_kind='draft_closure_changed'
      AND receipt.draft_artifact_refs=ARRAY[event.draft_id::text] AND receipt.artifact_lifecycle_event_refs=ARRAY[event.event_id::text]
      AND receipt.result_payload->>'draft_revision_id'=event.revision_id::text AND receipt.result_payload->>'payload_digest'=event.payload_digest
      AND event.payload_digest=revision.payload_digest AND action.disposition='forward' AND action.author_action_sequence=event.author_action_sequence
      AND admission.command_payload->'close_editor_flow_draft_input'->>'draft_id'=event.draft_id::text
      AND admission.command_payload->'close_editor_flow_draft_input'->>'source_current_draft_revision_id'=event.revision_id::text
      AND admission.command_payload->'close_editor_flow_draft_input'->>'source_draft_payload_digest'=event.payload_digest
      AND admission.command_payload->'close_editor_flow_draft_input'->>'expected_closure'='open'
      AND (NOT validate_lifecycle OR (admission.command_payload->'close_editor_flow_draft_input'->>'source_reopen_event_id') IS NOT DISTINCT FROM prior_reopen_event)
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
        AND replay.canonical_command_digest=receipt.command_digest
        AND replay.outcome_kind='settled' AND replay.result_reference=receipt.receipt_id::text))
  THEN RAISE EXCEPTION 'Incomplete Draft Discard settlement' USING ERRCODE='23514'; END IF;
  RETURN NULL;
END $function$;

DROP TRIGGER draft_close_projection_complete ON storyos.draft_artifacts;
CREATE CONSTRAINT TRIGGER draft_close_projection_complete AFTER INSERT OR UPDATE OF closure,close_event_id,reopen_event_id ON storyos.draft_artifacts
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_draft_close_settlement();

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
    IF scoped_result_kind = 'draft_closure_changed' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count, payload_count, archival_count) <> (0, 1, 0, 0, 0, 0) THEN
        RAISE EXCEPTION 'Draft reopen requires one compensation and zero manuscript authority' USING ERRCODE='23514';
      END IF;
    ELSIF scoped_result_kind = 'authoritative_applied' THEN
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

  ELSIF scoped_command_kind = 'pauseAgentRun' THEN
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
         OR payload_event_kind IS DISTINCT FROM 'agent_run_paused' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'pauseAgentRun applied requires one paused Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'pauseAgentRun with zero transition cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'cancelAgentRun' THEN
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
         OR payload_event_kind IS DISTINCT FROM 'agent_run_cancelled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'cancelAgentRun applied requires one cancelled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'cancelAgentRun with zero transition cannot have an authority or Activity relation';
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
  ELSIF scoped_command_kind = 'completeReadyPartialProposal'
        AND scoped_result_kind = 'proposal_generation_completed' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'completeReadyPartialProposal completed requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'completeReadyPartialProposal'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'completeReadyPartialProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'continueProposalGeneration'
        AND scoped_result_kind = 'proposal_generation_started' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'continueProposalGeneration started requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'continueProposalGeneration'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'continueProposalGeneration with zero effect cannot have an authority or Activity relation';
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
  ELSIF scoped_command_kind = 'closeEditorFlowDraft'
        AND scoped_result_kind = 'draft_closure_changed' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count, archival_count)
         <> (0, 1, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'DraftClosureChanged requires one Author Action and zero authority';
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
