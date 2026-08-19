SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.replay_generations (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  replay_generation numeric(20, 0) NOT NULL
    CHECK (replay_generation BETWEEN 1 AND 18446744073709551615),
  opened_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  closing_project_activity_position numeric(20, 0)
    CHECK (closing_project_activity_position IS NULL
      OR closing_project_activity_position BETWEEN 0 AND 18446744073709551615),
  PRIMARY KEY (owner_user_id, project_id, replay_generation),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

CREATE TABLE storyos.replay_floors (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  replay_generation numeric(20, 0) NOT NULL,
  floor_position numeric(20, 0) NOT NULL
    CHECK (floor_position BETWEEN 0 AND 18446744073709551615),
  PRIMARY KEY (owner_user_id, project_id, replay_generation),
  FOREIGN KEY (owner_user_id, project_id, replay_generation)
    REFERENCES storyos.replay_generations(owner_user_id, project_id, replay_generation) MATCH FULL
);

CREATE TABLE storyos.project_snapshots (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  snapshot_id uuid NOT NULL,
  replay_generation numeric(20, 0) NOT NULL,
  project_activity_position numeric(20, 0) NOT NULL
    CHECK (project_activity_position BETWEEN 0 AND 18446744073709551615),
  snapshot_kind text NOT NULL CHECK (snapshot_kind = 'canonical'),
  redaction_profile text NOT NULL CHECK (redaction_profile = 'storyos.author.v1'),
  schema_profile text NOT NULL CHECK (schema_profile = 'storyos.public.release.1'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  expires_at timestamptz,
  PRIMARY KEY (owner_user_id, project_id, snapshot_id),
  FOREIGN KEY (owner_user_id, project_id, replay_generation)
    REFERENCES storyos.replay_generations(owner_user_id, project_id, replay_generation) MATCH FULL
);
CREATE INDEX project_snapshots_replay_generation_fk_idx
  ON storyos.project_snapshots (owner_user_id, project_id, replay_generation);

DO $policy$
DECLARE relation_name text;
BEGIN
  FOREACH relation_name IN ARRAY ARRAY[
    'replay_generations', 'replay_floors', 'project_snapshots'
  ] LOOP
    EXECUTE format('ALTER TABLE storyos.%I ENABLE ROW LEVEL SECURITY', relation_name);
    EXECUTE format('ALTER TABLE storyos.%I FORCE ROW LEVEL SECURITY', relation_name);
    EXECUTE format(
      'CREATE POLICY %I_exact_scope ON storyos.%I USING (
        owner_user_id = current_setting(''storyos.owner_user_id'')::uuid
        AND project_id = current_setting(''storyos.project_id'')::uuid
      ) WITH CHECK (
        owner_user_id = current_setting(''storyos.owner_user_id'')::uuid
        AND project_id = current_setting(''storyos.project_id'')::uuid
      )', relation_name, relation_name);
  END LOOP;
END
$policy$;

GRANT SELECT, INSERT ON storyos.replay_generations, storyos.replay_floors,
  storyos.project_snapshots TO storyos_runtime;
