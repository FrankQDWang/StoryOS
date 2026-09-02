SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.author_command_admission_reconfirmations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  author_command_admission_id uuid NOT NULL,
  command_id uuid NOT NULL,
  reconfirmation_reason text NOT NULL CHECK (reconfirmation_reason IN (
    'admission_expired',
    'binding_changed',
    'direct_edit_intent_unrecoverable'
  )),
  recovery_draft_ref uuid,
  settled_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (owner_user_id, project_id, author_command_admission_id),
  UNIQUE (owner_user_id, project_id, command_id),
  FOREIGN KEY (owner_user_id, project_id, author_command_admission_id)
    REFERENCES storyos.author_command_admissions (
      owner_user_id, project_id, author_command_admission_id
    ) MATCH FULL
);


CREATE FUNCTION storyos.reject_reconfirmation_after_receipt()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
BEGIN
  IF EXISTS (
    SELECT 1
      FROM storyos.author_command_admission_settlements AS settlement
     WHERE settlement.owner_user_id = NEW.owner_user_id
       AND settlement.project_id = NEW.project_id
       AND settlement.author_command_admission_id = NEW.author_command_admission_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'a Receipt-settled Admission cannot append RequiresReconfirmation';
  END IF;
  RETURN NEW;
END
$function$;

CREATE FUNCTION storyos.reject_receipt_after_reconfirmation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
BEGIN
  IF EXISTS (
    SELECT 1
      FROM storyos.author_command_admission_reconfirmations AS reconfirmation
     WHERE reconfirmation.owner_user_id = NEW.owner_user_id
       AND reconfirmation.project_id = NEW.project_id
       AND reconfirmation.author_command_admission_id = NEW.author_command_admission_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'a RequiresReconfirmation Admission cannot append a Receipt settlement';
  END IF;
  RETURN NEW;
END
$function$;

CREATE TRIGGER author_command_admission_reconfirmation_before_receipt
BEFORE INSERT ON storyos.author_command_admission_reconfirmations
FOR EACH ROW EXECUTE FUNCTION storyos.reject_reconfirmation_after_receipt();

CREATE TRIGGER author_command_admission_settlement_before_reconfirmation
BEFORE INSERT ON storyos.author_command_admission_settlements
FOR EACH ROW EXECUTE FUNCTION storyos.reject_receipt_after_reconfirmation();

ALTER TABLE storyos.author_command_admission_reconfirmations
  ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.author_command_admission_reconfirmations
  FORCE ROW LEVEL SECURITY;
CREATE POLICY author_command_admission_reconfirmation_exact_scope
  ON storyos.author_command_admission_reconfirmations
  USING (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  )
  WITH CHECK (
    owner_user_id = current_setting('storyos.owner_user_id')::uuid
    AND project_id = current_setting('storyos.project_id')::uuid
  );

GRANT SELECT, INSERT ON storyos.author_command_admission_reconfirmations
  TO storyos_runtime;
