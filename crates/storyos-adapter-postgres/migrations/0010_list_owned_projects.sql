SET LOCAL ROLE storyos_owner;

CREATE FUNCTION storyos.uuid_setting(setting_name text)
RETURNS uuid
LANGUAGE sql
STABLE
PARALLEL SAFE
SET search_path = pg_catalog
AS $function$
  SELECT CASE
    WHEN current_setting(setting_name, true)
      ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
    THEN current_setting(setting_name, true)::uuid
  END
$function$;

GRANT EXECUTE ON FUNCTION storyos.uuid_setting(text) TO storyos_runtime;

ALTER TABLE storyos.projects
  ADD COLUMN lifecycle_state text NOT NULL DEFAULT 'active'
    CHECK (lifecycle_state = 'active'),
  ADD COLUMN revision bigint NOT NULL DEFAULT 1
    CHECK (revision >= 1);

CREATE POLICY projects_user_owned_list ON storyos.projects
  FOR SELECT
  USING (
    current_setting('storyos.scope_mode', true) = 'user'
    AND owner_user_id = storyos.uuid_setting('storyos.user_id')
    AND storyos.uuid_setting('storyos.project_id') IS NULL
    AND storyos.uuid_setting('storyos.owner_user_id') IS NULL
  );
