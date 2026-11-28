-- Each Editor Session writes (ADR 0107 section 1). The Project writer generation goes: the
-- Admission of a command no longer names a writer, and no table holds the writer of a Project. An
-- Admission of the past keeps its other columns, and its command payload keeps the generation that
-- it named. The command shape check is the check of the baseline with no writer clause. The
-- branch takeOverProjectWriter stays for the Admissions of the past. An ApplyAuthorEdit Admission
-- still names an exact Authoritative Revision.

SET LOCAL ROLE storyos_owner;

DROP TRIGGER author_command_admissions_writer_generation ON storyos.author_command_admissions;
DROP FUNCTION storyos.require_writer_generation_admission();

ALTER TABLE storyos.author_command_admissions
    DROP CONSTRAINT author_command_admissions_command_shape,
    DROP COLUMN writer_generation;

DROP TABLE storyos.project_writer_generations;

ALTER TABLE storyos.author_command_admissions
    ADD CONSTRAINT author_command_admissions_command_shape CHECK ((
        (command_kind = 'applyAuthorEdit'::text
            AND action_class = 'direct_editor_action'::text
            AND editor_session_id IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND observed_ownership_partition = ANY (ARRAY['authoritative'::text, 'mixed'::text])
            AND (((observed_ownership_partition = 'authoritative'::text) AND (expected_proposal_head_revision_ids = '{}'::uuid[])) OR ((observed_ownership_partition = 'mixed'::text) AND (cardinality(expected_proposal_head_revision_ids) >= 1)))
            AND undo_group_id IS NOT NULL
            AND completed_intent_record_id IS NOT NULL
            AND local_intent_sequence IS NOT NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'takeOverProjectWriter'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
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
        OR (command_kind = 'createProject'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'updateProject'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'createVolume'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'createChapter'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'updateVolume'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'updateChapter'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = ANY (ARRAY['deleteChapter'::text, 'deleteVolume'::text, 'exportHumanReadableManuscript'::text, 'exportProjectArchive'::text, 'updateProjectAssistance'::text, 'updateProjectInstruction'::text, 'deleteMemoryEntry'::text])
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'setCurrentChapter'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND (((chapter_object_id IS NOT NULL) AND (expected_authoritative_revision_id IS NOT NULL)) OR ((chapter_object_id IS NULL) AND (expected_authoritative_revision_id IS NULL)))
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'undoLatestAuthorAction'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND (((chapter_object_id IS NOT NULL) AND (expected_authoritative_revision_id IS NOT NULL)) OR ((chapter_object_id IS NULL) AND (expected_authoritative_revision_id IS NULL)))
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'createAgentRun'::text
            AND action_class = 'agent_run_start'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = ANY (ARRAY['pauseAgentRun'::text, 'cancelAgentRun'::text])
            AND action_class = 'agent_run_control'::text
            AND editor_session_id IS NULL
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
        OR (command_kind = 'acceptProposal'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
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
        OR (command_kind = 'rejectProposalOperations'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
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
        OR (command_kind = 'withdrawProposal'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
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
        OR (command_kind = 'steerAgentRun'::text
            AND action_class = 'agent_run_control'::text
            AND editor_session_id IS NULL
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
    ) IS TRUE);

CREATE FUNCTION storyos.require_exact_revision_admission() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
      FROM storyos.authoritative_revisions AS revision
     WHERE revision.owner_user_id = NEW.owner_user_id
       AND revision.project_id = NEW.project_id
       AND revision.manuscript_object_id = NEW.chapter_object_id
       AND revision.revision_id = NEW.expected_authoritative_revision_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23503',
      MESSAGE = 'ApplyAuthorEdit Admission requires an exact authoritative revision';
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER author_command_admissions_exact_revision BEFORE INSERT ON storyos.author_command_admissions FOR EACH ROW WHEN ((new.command_kind = 'applyAuthorEdit'::text)) EXECUTE FUNCTION storyos.require_exact_revision_admission();
