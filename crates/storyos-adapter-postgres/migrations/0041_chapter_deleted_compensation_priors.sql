SET LOCAL ROLE storyos_owner;

GRANT DELETE ON storyos.chapter_removal_decisions TO storyos_runtime;

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
        AND payload - 'kind' - 'volume_id' - 'title' - 'tree_revision' - 'order'
          - 'prior_title' - 'prior_order' = '{}'::jsonb
        AND (
          (NOT payload ? 'prior_title' AND NOT payload ? 'prior_order')
          OR (
            payload ? 'prior_title'
            AND payload ? 'prior_order'
            AND jsonb_typeof(payload->'prior_title') = 'string'
            AND jsonb_typeof(payload->'prior_order') = 'string'
            AND payload->>'prior_order' ~ '^[1-9][0-9]*$'
          )
        ))
      OR (event_kind = 'chapter_updated'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'chapter_updated'
        AND jsonb_typeof(payload->'chapter_id') = 'string'
        AND jsonb_typeof(payload->'title') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND jsonb_typeof(payload->'order') = 'string'
        AND payload - 'kind' - 'chapter_id' - 'title' - 'tree_revision' - 'order'
          - 'prior_title' - 'prior_order' = '{}'::jsonb
        AND (
          (NOT payload ? 'prior_title' AND NOT payload ? 'prior_order')
          OR (
            payload ? 'prior_title'
            AND payload ? 'prior_order'
            AND jsonb_typeof(payload->'prior_title') = 'string'
            AND jsonb_typeof(payload->'prior_order') = 'string'
            AND payload->>'prior_order' ~ '^[1-9][0-9]*$'
          )
        ))
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
          - 'current_chapter_id' - 'prior_current_chapter_id' = '{}'::jsonb
        AND (
          (NOT payload ? 'prior_current_chapter_id')
          OR jsonb_typeof(payload->'prior_current_chapter_id') IN ('string', 'null')
        ))
      OR (event_kind = 'volume_deleted'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'volume_deleted'
        AND jsonb_typeof(payload->'volume_id') = 'string'
        AND jsonb_typeof(payload->'tree_revision') = 'string'
        AND payload - 'kind' - 'volume_id' - 'tree_revision' = '{}'::jsonb)

      OR (event_kind = 'human_readable_manuscript_export_settled'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'human_readable_manuscript_export_settled'
        AND jsonb_typeof(payload->'export_id') = 'string'
        AND jsonb_typeof(payload->'content_sha256') = 'string'
        AND payload - 'kind' - 'export_id' - 'content_sha256' = '{}'::jsonb)
      OR (event_kind = 'project_export_settled'
        AND receipt_result_kind = 'authoritative_applied'
        AND payload->>'kind' = 'project_export_settled'
        AND jsonb_typeof(payload->'export_id') = 'string'
        AND jsonb_typeof(payload->'archive_profile') = 'string'
        AND jsonb_typeof(payload->'archive_path_profile') = 'string'
        AND payload - 'kind' - 'export_id' - 'archive_profile'
          - 'archive_path_profile' = '{}'::jsonb)
    )
  ) IS TRUE);
