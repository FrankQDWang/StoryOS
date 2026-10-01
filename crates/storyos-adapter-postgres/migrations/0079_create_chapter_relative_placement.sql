SET LOCAL ROLE storyos_owner;

DO $migration$
DECLARE
  prior_check text;
BEGIN
  SELECT pg_get_constraintdef(oid) INTO STRICT prior_check FROM pg_constraint
    WHERE conrelid='storyos.domain_receipts'::regclass
      AND conname='domain_receipts_result_shape' AND contype='c';
  IF prior_check NOT LIKE 'CHECK (%)' THEN
    RAISE EXCEPTION 'Required settlement constraint is unavailable';
  END IF;
  ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_shape;
  EXECUTE format('ALTER TABLE storyos.domain_receipts ADD CONSTRAINT domain_receipts_result_shape
    CHECK ((%s) OR ((command_kind=''createChapter'' AND result_kind=''refused''
      AND result_payload=''{"reason":"invalid_placement"}''::jsonb
      AND cardinality(authoritative_revision_ids)=0
      AND cardinality(authoritative_commit_ids)=0) IS TRUE))',
    substring(prior_check FROM 8 FOR length(prior_check)-8));
END $migration$;
