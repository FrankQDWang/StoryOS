SET LOCAL ROLE storyos_owner;

-- ADR 0069: after the Undo of its reopen, the author can reopen the same Withdrawal again.
-- Each reopen keeps its own record, so a Withdrawal event can have more than one record.
DO $migration$
DECLARE unique_key name;
BEGIN
  SELECT reopen_key.conname INTO STRICT unique_key
    FROM pg_constraint AS reopen_key
   WHERE reopen_key.conrelid = 'storyos.proposal_withdrawal_reopenings'::regclass
     AND reopen_key.contype = 'u'
     AND reopen_key.conkey = ARRAY(
       SELECT attribute.attnum
         FROM pg_attribute AS attribute
        WHERE attribute.attrelid = 'storyos.proposal_withdrawal_reopenings'::regclass
          AND attribute.attname IN ('owner_user_id', 'project_id', 'withdrawal_event_id')
        ORDER BY attribute.attnum
     )::int2[];
  EXECUTE format('ALTER TABLE storyos.proposal_withdrawal_reopenings DROP CONSTRAINT %I', unique_key);
END $migration$;
