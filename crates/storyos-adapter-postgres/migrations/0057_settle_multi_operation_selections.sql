SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.proposals
  ADD COLUMN bundle_policy text NOT NULL DEFAULT 'none'
  CHECK (bundle_policy IN ('none', 'atomic'));

ALTER TABLE storyos.proposal_operations
  ADD COLUMN candidate_text text NOT NULL DEFAULT '',
  ADD COLUMN predecessor_operation_ids uuid[] NOT NULL DEFAULT '{}';

UPDATE storyos.proposal_operations AS operation
   SET candidate_text = revision.candidate_text
  FROM storyos.proposal_heads AS head
  JOIN storyos.proposal_revisions AS revision
    ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
        revision.revision_id) =
       (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
 WHERE (operation.owner_user_id, operation.project_id, operation.proposal_id) =
       (head.owner_user_id, head.project_id, head.proposal_id)
   AND operation.candidate_text = '';

ALTER TABLE storyos.proposal_operations
  ALTER COLUMN candidate_text DROP DEFAULT;

ALTER TABLE storyos.acceptance_receipts
  ADD COLUMN selected_operation_ids uuid[] NOT NULL DEFAULT '{}';

UPDATE storyos.acceptance_receipts
   SET selected_operation_ids = ARRAY[selected_operation_id];

ALTER TABLE storyos.acceptance_receipts
  ALTER COLUMN selected_operation_ids DROP DEFAULT;

ALTER TABLE storyos.acceptance_receipts
  ADD CONSTRAINT acceptance_receipts_selected_operation_ids_present
  CHECK (cardinality(selected_operation_ids) >= 1);

CREATE FUNCTION storyos.normalize_acceptance_receipt_selection()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
BEGIN
  IF NEW.selected_operation_ids IS NULL
     OR cardinality(NEW.selected_operation_ids) = 0 THEN
    NEW.selected_operation_ids := ARRAY[NEW.selected_operation_id];
  ELSE
    NEW.selected_operation_id := NEW.selected_operation_ids[1];
  END IF;
  RETURN NEW;
END
$function$;
GRANT EXECUTE ON FUNCTION storyos.normalize_acceptance_receipt_selection()
  TO storyos_runtime;

CREATE TRIGGER acceptance_receipts_normalize_selection
BEFORE INSERT OR UPDATE ON storyos.acceptance_receipts
FOR EACH ROW EXECUTE FUNCTION storyos.normalize_acceptance_receipt_selection();
