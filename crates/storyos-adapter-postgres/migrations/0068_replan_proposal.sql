SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.proposal_validation_conditions
  ADD COLUMN condition_kind text;
UPDATE storyos.proposal_validation_conditions
   SET condition_kind = 'proposal_conflict'
 WHERE validation = 'conflicted' AND condition_kind IS NULL;
ALTER TABLE storyos.proposal_validation_conditions
  ADD CHECK ((validation = 'conflicted') = (condition_kind IS NOT NULL));
ALTER TABLE storyos.proposal_validation_conditions
  ADD CHECK (condition_kind IS NULL OR condition_kind IN (
    'proposal_conflict', 'proposal_recovery_conflict'
  ));

CREATE TABLE storyos.proposal_replans (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  replan_event_id uuid NOT NULL,
  proposal_id uuid NOT NULL,
  source_proposal_revision_id uuid NOT NULL,
  resulting_proposal_revision_id uuid NOT NULL,
  source_condition_kind text NOT NULL CHECK (source_condition_kind IN (
    'proposal_conflict', 'proposal_recovery_conflict'
  )),
  source_condition_ref uuid NOT NULL,
  replan_receipt_id uuid NOT NULL,
  author_action_sequence numeric(20, 0) NOT NULL
    CHECK (author_action_sequence BETWEEN 1 AND 18446744073709551615),
  PRIMARY KEY (owner_user_id, project_id, replan_event_id),
  UNIQUE (owner_user_id, project_id, source_condition_ref),
  UNIQUE (owner_user_id, project_id, replan_receipt_id),
  FOREIGN KEY (owner_user_id, project_id, proposal_id, source_proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, proposal_id, resulting_proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, replan_receipt_id)
    REFERENCES storyos.domain_receipts (owner_user_id, project_id, receipt_id) MATCH FULL
);
ALTER TABLE storyos.proposal_replans ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_replans FORCE ROW LEVEL SECURITY;
CREATE POLICY proposal_replans_exact_scope ON storyos.proposal_replans USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);
GRANT SELECT, INSERT ON storyos.proposal_replans TO storyos_runtime;

DO $migration$
DECLARE item record; prior_check text;
BEGIN
  FOR item IN SELECT * FROM (VALUES
    ('domain_receipts','domain_receipts_command_kind_check',$p$command_kind='replanProposal'$p$),
    ('author_command_admission_outcome_unknown_observations','author_command_admission_outcome_unknown_command_kind_check',$p$command_kind='replanProposal'$p$),
    ('author_command_admissions','author_command_admissions_command_shape',$p$command_kind='replanProposal'
      AND action_class='explicit_editor_command' AND editor_session_id IS NOT NULL AND writer_generation IS NOT NULL
      AND chapter_object_id IS NOT NULL AND expected_authoritative_revision_id IS NOT NULL
      AND expected_proposal_head_revision_ids='{}' AND target_refs='{}'
      AND observed_ownership_partition IS NULL AND undo_group_id IS NULL
      AND completed_intent_record_id IS NULL AND local_intent_sequence IS NULL
      AND challenge_consumed_at IS NOT NULL AND challenge_expires_at IS NOT NULL$p$),
    ('domain_receipts','domain_receipts_common_shape',$p$command_kind='replanProposal'
      AND cardinality(expected_heads)=1 AND cardinality(prior_heads)<=1 AND resulting_heads=prior_heads
      AND authoritative_revision_ids='{}' AND authoritative_commit_ids='{}' AND condition_refs='{}'
      AND draft_artifact_refs='{}' AND artifact_lifecycle_event_refs='{}'
      AND ((result_kind='proposal_revised' AND expected_heads=prior_heads AND cardinality(proposal_revision_ids)=1
        AND array_position(proposal_revision_ids,NULL) IS NULL)
      OR (result_kind IN ('refused','conflicted') AND proposal_revision_ids='{}'))$p$),
    ('domain_receipts','domain_receipts_result_shape',$p$command_kind='replanProposal'
      AND jsonb_typeof(result_payload)='object'
      AND ((result_kind='proposal_revised' AND result_payload->>'transition'='replan'
        AND result_payload-ARRAY['transition']='{}')
      OR (result_kind='conflicted' AND result_payload->>'reason'='changed_head'
        AND result_payload-ARRAY['reason']='{}')
      OR (result_kind='refused' AND result_payload->>'reason' IN (
        'wrong_scope','wrong_admission','stale_proposal_revision','not_eligible','unavailable_proof')
        AND result_payload-ARRAY['reason']='{}'))$p$)
  ) AS entries(table_name,constraint_name,predicate) LOOP
    SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint WHERE
      conrelid=format('storyos.%I',item.table_name)::regclass AND conname=item.constraint_name AND contype='c';
    IF prior_check NOT LIKE 'CHECK (%)' THEN RAISE EXCEPTION 'Required settlement constraint is unavailable'; END IF;
    EXECUTE format('ALTER TABLE storyos.%I DROP CONSTRAINT %I',item.table_name,item.constraint_name);
    EXECUTE format('ALTER TABLE storyos.%I ADD CONSTRAINT %I CHECK ((%s) OR ((%s) IS TRUE))',
      item.table_name,item.constraint_name,substring(prior_check FROM 8 FOR length(prior_check)-8),item.predicate);
  END LOOP;
END $migration$;
