SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.agent_runs ADD COLUMN active_decision_position numeric(20,0) NOT NULL DEFAULT 0 CHECK (active_decision_position BETWEEN 0 AND 18446744073709551615);
DO $migration$
DECLARE item record; prior_check text; definition text;
BEGIN
  FOR item IN SELECT * FROM (VALUES
    ('operation_requirements','requirement_role'), ('context_assembly_manifests','manifest_role'), ('model_attempts','attempt_role')
  ) AS roles(table_name,role_name) LOOP
    EXECUTE format('ALTER TABLE storyos.%I ADD COLUMN decision_position numeric(20,0) NOT NULL DEFAULT 0 CHECK (decision_position BETWEEN 0 AND 18446744073709551615)', item.table_name);
    EXECUTE format('DROP INDEX storyos.%I', item.table_name || '_one_role');
    EXECUTE format('CREATE UNIQUE INDEX %I ON storyos.%I (owner_user_id,project_id,run_id,%I,decision_position)', item.table_name || '_one_role', item.table_name, item.role_name);
  END LOOP;
  FOR item IN SELECT * FROM (VALUES
    ('domain_receipts','domain_receipts_command_kind_check',$p$command_kind='steerAgentRun'$p$),
    ('author_command_admission_outcome_unknown_observations','author_command_admission_outcome_unknown_command_kind_check',$p$command_kind='steerAgentRun'$p$),
    ('author_command_admissions','author_command_admissions_command_shape',$p$command_kind='steerAgentRun' AND action_class='agent_run_control'
      AND editor_session_id IS NULL AND writer_generation IS NULL AND chapter_object_id IS NULL
      AND expected_authoritative_revision_id IS NULL AND expected_proposal_head_revision_ids='{}' AND target_refs='{}'
      AND observed_ownership_partition IS NULL AND undo_group_id IS NULL AND completed_intent_record_id IS NULL
      AND local_intent_sequence IS NULL AND challenge_consumed_at IS NOT NULL AND challenge_expires_at IS NOT NULL$p$),
    ('domain_receipts','domain_receipts_common_shape',$p$command_kind='steerAgentRun' AND expected_heads='{}' AND prior_heads='{}' AND resulting_heads='{}'
      AND authoritative_revision_ids='{}' AND proposal_revision_ids='{}' AND authoritative_commit_ids='{}'
      AND draft_artifact_refs='{}' AND artifact_lifecycle_event_refs='{}' AND condition_refs='{}'$p$),
    ('domain_receipts','domain_receipts_result_shape',$p$command_kind='steerAgentRun' AND jsonb_typeof(result_payload)='object'
      AND result_payload-ARRAY['reason']='{}' AND ((result_kind='no_effect' AND result_payload->>'reason'='steering_retained')
        OR (result_kind='conflicted' AND result_payload->>'reason'='terminal_run'))$p$),
    ('project_activity_event_payloads','project_activity_event_payloads_event_kind_check',$p$event_kind='agent_run_steering_retained'$p$),
    ('project_activity_event_payloads','project_activity_event_payloads_shape',$p$event_kind='agent_run_steering_retained' AND receipt_result_kind='no_effect'
      AND payload->>'kind'=event_kind AND jsonb_typeof(payload->'run_id')='string'
      AND jsonb_typeof(payload->'conversation_id')='string' AND jsonb_typeof(payload->'steering_input_id')='string'
      AND (payload->>'input_position') ~ '^[1-9][0-9]*$' AND (payload->>'input_position')::numeric <= 18446744073709551615
      AND char_length(payload->>'author_message') BETWEEN 1 AND 8000
      AND payload-ARRAY['kind','run_id','conversation_id','steering_input_id','input_position','author_message']='{}'$p$)
  ) AS entries(table_name,constraint_name,predicate) LOOP
    SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint WHERE
      conrelid=format('storyos.%I',item.table_name)::regclass AND conname=item.constraint_name AND contype='c';
    IF prior_check NOT LIKE 'CHECK (%)' THEN RAISE EXCEPTION 'Required settlement constraint is unavailable'; END IF;
    EXECUTE format('ALTER TABLE storyos.%I DROP CONSTRAINT %I',item.table_name,item.constraint_name);
    EXECUTE format('ALTER TABLE storyos.%I ADD CONSTRAINT %I CHECK ((%s) OR ((%s) IS TRUE))',
      item.table_name,item.constraint_name,substring(prior_check FROM 8 FOR length(prior_check)-8),item.predicate);
  END LOOP;
  SELECT pg_get_functiondef('storyos.require_author_edit_receipt_relation()'::regprocedure) INTO definition;
  IF strpos(definition, 'ELSIF scoped_command_kind = ''pauseAgentRun'' THEN') = 0 THEN RAISE EXCEPTION 'Required receipt relation is unavailable'; END IF;
  definition := replace(definition, 'ELSIF scoped_command_kind = ''pauseAgentRun'' THEN', $branch$
  ELSIF scoped_command_kind = 'steerAgentRun' THEN
    SELECT payload.event_kind INTO payload_event_kind FROM storyos.project_activity_event_payloads AS payload
      WHERE payload.owner_user_id=scoped_owner_user_id AND payload.project_id=scoped_project_id AND payload.receipt_id=scoped_receipt_id;
    IF (activity_count,action_count,commit_count,revision_envelope_count,archival_count) <> (0,0,0,0,0)
      OR (scoped_result_kind='no_effect' AND (payload_count<>1 OR payload_event_kind IS DISTINCT FROM 'agent_run_steering_retained'))
      OR (scoped_result_kind='conflicted' AND payload_count<>0) THEN
      RAISE EXCEPTION 'Steering retention requires exact Activity and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'pauseAgentRun' THEN$branch$);
  EXECUTE definition;
END $migration$;
DROP TRIGGER author_command_admissions_writer_generation ON storyos.author_command_admissions;
CREATE TRIGGER author_command_admissions_writer_generation BEFORE INSERT ON storyos.author_command_admissions FOR EACH ROW
WHEN (NEW.action_class <> 'agent_run_control' AND NEW.command_kind NOT IN (
  'createProject','updateProject','archiveProject','createVolume','createChapter','updateVolume','updateChapter','deleteChapter','deleteVolume',
  'exportHumanReadableManuscript','exportProjectArchive','updateProjectAssistance','createAgentRun'
)) EXECUTE FUNCTION storyos.require_writer_generation_admission();
