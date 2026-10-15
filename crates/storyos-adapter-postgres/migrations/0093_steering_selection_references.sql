SET LOCAL ROLE storyos_owner;

-- ADR 0062: a steering input can add 1 to 8 selection references to its Run. An input with no
-- references keeps the prior shape.
DO $migration$
DECLARE prior_check text;
BEGIN
  SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint WHERE
    conrelid='storyos.project_activity_event_payloads'::regclass
    AND conname='project_activity_event_payloads_shape' AND contype='c';
  IF prior_check NOT LIKE 'CHECK (%)' THEN RAISE EXCEPTION 'Required activity payload constraint is unavailable'; END IF;
  ALTER TABLE storyos.project_activity_event_payloads DROP CONSTRAINT project_activity_event_payloads_shape;
  EXECUTE format('ALTER TABLE storyos.project_activity_event_payloads ADD CONSTRAINT project_activity_event_payloads_shape CHECK ((%s) OR ((%s) IS TRUE))',
    substring(prior_check FROM 8 FOR length(prior_check)-8),
    $p$event_kind='agent_run_steering_retained' AND receipt_result_kind='no_effect'
      AND payload->>'kind'=event_kind AND jsonb_typeof(payload->'run_id')='string'
      AND jsonb_typeof(payload->'conversation_id')='string' AND jsonb_typeof(payload->'steering_input_id')='string'
      AND (payload->>'input_position') ~ '^[1-9][0-9]*$' AND (payload->>'input_position')::numeric <= 18446744073709551615
      AND char_length(payload->>'author_message') BETWEEN 1 AND 8000
      AND jsonb_typeof(payload->'selection_references')='array'
      AND jsonb_array_length(payload->'selection_references') BETWEEN 1 AND 8
      AND payload-ARRAY['kind','run_id','conversation_id','steering_input_id','input_position','author_message','selection_references']='{}'$p$);
END
$migration$;
