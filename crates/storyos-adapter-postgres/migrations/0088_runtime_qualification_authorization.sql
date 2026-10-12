SET LOCAL ROLE storyos_owner;

-- PostgreSQL shortens long default constraint names, so find each CHECK by its column.
DO $$
DECLARE
  value_check record;
BEGIN
  FOR value_check IN
    SELECT constraint_row.conrelid::regclass AS table_name, constraint_row.conname
      FROM pg_constraint AS constraint_row
      JOIN pg_attribute AS checked_column
        ON checked_column.attrelid = constraint_row.conrelid
       AND checked_column.attnum = constraint_row.conkey[1]
     WHERE constraint_row.contype = 'c'
       AND cardinality(constraint_row.conkey) = 1
       AND (constraint_row.conrelid, checked_column.attname) IN (
         ('storyos.external_contract_compatibility_decisions'::regclass,
          'runtime_qualification'),
         ('storyos.project_external_use_binding_revisions'::regclass, 'budget_bounds')
       )
  LOOP
    EXECUTE format(
      'ALTER TABLE %s DROP CONSTRAINT %I', value_check.table_name, value_check.conname
    );
  END LOOP;
END
$$;

-- ADR 0049: an author-authorized runtime qualification admits a pending profile. The Decision
-- keeps the link to the authorization.
ALTER TABLE storyos.external_contract_compatibility_decisions
  ADD CONSTRAINT external_contract_compatibility_decisions_qualification_check
    CHECK (runtime_qualification IN ('qualified', 'pending', 'qualification_authorized')),
  ADD COLUMN qualification_authorization text
    CHECK (char_length(qualification_authorization) BETWEEN 1 AND 500),
  ADD CONSTRAINT external_contract_compatibility_decisions_authorization_shape CHECK (
    (runtime_qualification = 'qualification_authorized') = (qualification_authorization IS NOT NULL)
  );

-- The author-adopted request bounds of the Agent Plan route.
ALTER TABLE storyos.project_external_use_binding_revisions
  ADD CONSTRAINT project_external_use_binding_revisions_budget_bounds_check
    CHECK (budget_bounds IN ('not_required', 'unqualified', 'agent_plan_request_v1'));
