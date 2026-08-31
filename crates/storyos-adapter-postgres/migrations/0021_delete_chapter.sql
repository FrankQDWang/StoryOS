SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.chapter_removal_decisions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  chapter_removal_decision_id uuid NOT NULL,
  receipt_id uuid NOT NULL,
  chapter_id uuid NOT NULL,
  volume_id uuid NOT NULL,
  tree_revision bigint NOT NULL CHECK (tree_revision >= 1),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, chapter_removal_decision_id),
  UNIQUE (owner_user_id, project_id, receipt_id),
  UNIQUE (owner_user_id, project_id, chapter_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, receipt_id)
    REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, chapter_id)
    REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL
);

ALTER TABLE storyos.chapter_removal_decisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.chapter_removal_decisions FORCE ROW LEVEL SECURITY;
CREATE POLICY chapter_removal_decisions_exact_scope
  ON storyos.chapter_removal_decisions USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  ) WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.chapter_removal_decisions TO storyos_runtime;

DROP TRIGGER author_command_admissions_writer_generation ON storyos.author_command_admissions;
CREATE TRIGGER author_command_admissions_writer_generation
BEFORE INSERT ON storyos.author_command_admissions
FOR EACH ROW
WHEN (NEW.command_kind NOT IN (
  'createProject', 'updateProject', 'archiveProject', 'createVolume',
  'createChapter', 'updateVolume', 'updateChapter', 'deleteChapter'
))
EXECUTE FUNCTION storyos.require_writer_generation_admission();

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
      AND observed_ownership_partition = 'authoritative'
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

    OR (command_kind = 'deleteChapter'
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
  ) IS TRUE);

ALTER TABLE storyos.domain_receipts
  DROP CONSTRAINT domain_receipts_command_kind_check;

ALTER TABLE storyos.domain_receipts
  ADD CONSTRAINT domain_receipts_command_kind_check
  CHECK (command_kind IN (
    'applyAuthorEdit', 'takeOverProjectWriter', 'createProject', 'updateProject',
    'archiveProject', 'createVolume', 'createChapter', 'updateVolume', 'updateChapter',
    'setCurrentChapter', 'undoLatestAuthorAction', 'deleteChapter'
  ));

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
      AND cardinality(authoritative_commit_ids) = 0)
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
      AND cardinality(authoritative_commit_ids) = 0)
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
      AND cardinality(authoritative_commit_ids) = 0)
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
      AND result_payload->>'reason' = 'stale_volume_revision'
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
      AND cardinality(authoritative_commit_ids) = 0)
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
      AND result_payload->>'reason' = 'stale_chapter_revision'
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
      AND cardinality(authoritative_commit_ids) = 0)
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
      AND result_payload->>'reason' = 'stale_chapter_revision'
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
    OR (command_kind = 'undoLatestAuthorAction'
      AND result_kind = 'authoritative_applied'
      AND result_payload = '{}'::jsonb
      AND cardinality(authoritative_revision_ids) = 1
      AND cardinality(authoritative_commit_ids) = 1
      AND array_dims(authoritative_revision_ids) = '[1:1]'
      AND array_dims(authoritative_commit_ids) = '[1:1]'
      AND resulting_heads = authoritative_revision_ids)
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
  ) IS TRUE);

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  DROP CONSTRAINT author_command_admission_outcome_unknown_command_kind_check;

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations
  ADD CONSTRAINT author_command_admission_outcome_unknown_command_kind_check
  CHECK (command_kind IN (
    'applyAuthorEdit', 'takeOverProjectWriter', 'createProject', 'updateProject',
    'archiveProject', 'createVolume', 'createChapter', 'updateVolume', 'updateChapter',
    'setCurrentChapter', 'undoLatestAuthorAction', 'deleteChapter'
  ));

ALTER TABLE storyos.project_activity_event_payloads
  DROP CONSTRAINT project_activity_event_payloads_event_kind_check;

ALTER TABLE storyos.project_activity_event_payloads
  ADD CONSTRAINT project_activity_event_payloads_event_kind_check
  CHECK (event_kind IN (
    'writer_takeover_applied', 'writer_takeover_compare_failed',
    'project_created', 'project_updated', 'project_archival_changed',
    'volume_created', 'volume_updated', 'chapter_created', 'chapter_updated',
    'current_chapter_set', 'chapter_deleted'
  ));

ALTER TABLE storyos.project_activity_event_payloads
  DROP CONSTRAINT project_activity_event_payloads_shape;

ALTER TABLE storyos.project_activity_event_payloads
  ADD CONSTRAINT project_activity_event_payloads_shape CHECK ((
    jsonb_typeof(payload) = 'object'
    AND (
      (event_kind = 'writer_takeover_applied'
        AND receipt_result_kind = 'no_effect'
        AND payload->>'kind' = 'takeover_applied'
        AND jsonb_typeof(payload->'prior_editor_session_id') = 'string'
        AND jsonb_typeof(payload->'prior_writer_generation') = 'string'
        AND jsonb_typeof(payload->'resulting_editor_session_id') = 'string'
        AND jsonb_typeof(payload->'resulting_writer_generation') = 'string'
        AND jsonb_typeof(payload->'resulting_snapshot_id') = 'string'
        AND jsonb_typeof(payload->'resulting_snapshot_activity_position') = 'string'
        AND jsonb_typeof(payload->'resulting_heads') = 'array'
        AND payload - 'kind' - 'prior_editor_session_id' - 'prior_writer_generation'
          - 'resulting_editor_session_id' - 'resulting_writer_generation'
          - 'resulting_snapshot_id' - 'resulting_snapshot_activity_position'
          - 'resulting_heads' = '{}'::jsonb)
      OR (event_kind = 'writer_takeover_compare_failed'
        AND receipt_result_kind = 'no_effect'
        AND payload->>'kind' = 'takeover_compare_failed'
        AND jsonb_typeof(payload->'observed_writer_generation') = 'string'
        AND jsonb_typeof(payload->'current_writer_generation') = 'string'
        AND jsonb_typeof(payload->'current_writer_projection') = 'object'
        AND jsonb_typeof(payload->'current_snapshot_id') = 'string'
        AND jsonb_typeof(payload->'current_snapshot_activity_position') = 'string'
        AND jsonb_typeof(payload->'current_heads') = 'array'
        AND jsonb_typeof(payload->'reason') = 'string'
        AND payload->>'reason' IN (
          'writer_generation_advanced_after_admission',
          'requester_became_current_after_admission'
        )
        AND payload - 'kind' - 'observed_writer_generation' - 'current_writer_generation'
          - 'current_writer_projection' - 'current_snapshot_id'
          - 'current_snapshot_activity_position' - 'current_heads' - 'reason' = '{}'::jsonb)
      OR (event_kind = 'project_created'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'project_created'
        AND payload->>'open_kind' = 'empty'
        AND jsonb_typeof(payload->'title') = 'string'
        AND payload - 'kind' - 'open_kind' - 'title' = '{}'::jsonb)
      OR (event_kind = 'project_updated'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'project_updated'
        AND jsonb_typeof(payload->'title') = 'string'
        AND jsonb_typeof(payload->'revision') = 'string'
        AND payload - 'kind' - 'title' - 'revision' = '{}'::jsonb)
      OR (event_kind = 'project_archival_changed'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'project_archival_changed'
        AND payload->>'lifecycle' = 'archived'
        AND jsonb_typeof(payload->'revision') = 'string'
        AND payload - 'kind' - 'lifecycle' - 'revision' = '{}'::jsonb)
      OR (event_kind = 'volume_created'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'volume_created'
        AND jsonb_typeof(payload->'volume_id') = 'string'
        AND jsonb_typeof(payload->'title') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND jsonb_typeof(payload->'order') = 'string'
        AND payload - 'kind' - 'volume_id' - 'title' - 'tree_revision' - 'order' = '{}'::jsonb)
      OR (event_kind = 'volume_updated'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'volume_updated'
        AND jsonb_typeof(payload->'volume_id') = 'string'
        AND jsonb_typeof(payload->'title') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND jsonb_typeof(payload->'order') = 'string'
        AND payload - 'kind' - 'volume_id' - 'title' - 'tree_revision' - 'order' = '{}'::jsonb)
      OR (event_kind = 'chapter_updated'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'chapter_updated'
        AND jsonb_typeof(payload->'chapter_id') = 'string'
        AND jsonb_typeof(payload->'title') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND jsonb_typeof(payload->'order') = 'string'
        AND payload - 'kind' - 'chapter_id' - 'title' - 'tree_revision' - 'order' = '{}'::jsonb)
      OR (event_kind = 'chapter_created'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'chapter_created'
        AND jsonb_typeof(payload->'volume_id') = 'string'
        AND jsonb_typeof(payload->'chapter_id') = 'string'
        AND jsonb_typeof(payload->'title') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND jsonb_typeof(payload->'order') = 'string'
        AND jsonb_typeof(payload->'current_chapter_id') = 'string'
        AND payload - 'kind' - 'volume_id' - 'chapter_id' - 'title' - 'tree_revision'
          - 'order' - 'current_chapter_id' = '{}'::jsonb)
      OR (event_kind = 'current_chapter_set'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'current_chapter_set'
        AND jsonb_typeof(payload->'prior_chapter_id') = 'string'
        AND jsonb_typeof(payload->'current_chapter_id') = 'string'
        AND jsonb_typeof(payload->'base_snapshot_id') = 'string'
        AND payload - 'kind' - 'prior_chapter_id' - 'current_chapter_id'
          - 'base_snapshot_id' = '{}'::jsonb)
      OR (event_kind = 'chapter_deleted'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'chapter_deleted'
        AND jsonb_typeof(payload->'chapter_id') = 'string'
        AND jsonb_typeof(payload->'volume_id') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND payload ? 'current_chapter_id'
        AND jsonb_typeof(payload->'current_chapter_id') IN ('string', 'null')
        AND payload - 'kind' - 'chapter_id' - 'volume_id' - 'tree_revision'
          - 'current_chapter_id' = '{}'::jsonb)
    )
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
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'volume_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createVolume applied requires one volume-created Activity and zero chapter authority';
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
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'volume_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateVolume applied requires one volume-updated Activity and zero chapter authority';
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
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'chapter_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateChapter applied requires one chapter-updated Activity and zero Author Edit authority';
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
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'current_chapter_set' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'setCurrentChapter applied requires one current-chapter-set Activity and zero manuscript authority';
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
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'chapter_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createChapter applied requires one chapter-created Activity and zero Author Edit authority';
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
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count, archival_count)
           <> (0, 0, 0, 0, 1, 0)
         OR payload_event_kind IS DISTINCT FROM 'chapter_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteChapter applied requires one chapter-deleted Activity and zero Author Edit authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count, archival_count)
            <> (0, 0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteChapter with zero tree effect cannot have an authority or Activity relation';
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
