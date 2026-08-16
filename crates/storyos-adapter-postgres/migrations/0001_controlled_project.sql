CREATE SCHEMA storyos AUTHORIZATION storyos_owner;
SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.users (
  user_id uuid PRIMARY KEY
);
CREATE TABLE storyos.projects (
  owner_user_id uuid NOT NULL REFERENCES storyos.users(user_id),
  project_id uuid NOT NULL,
  title text NOT NULL CHECK (octet_length(title) BETWEEN 1 AND 1024),
  current_chapter_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id)
);
CREATE TABLE storyos.manuscript_objects (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  manuscript_object_id uuid NOT NULL,
  object_kind text NOT NULL CHECK (object_kind IN ('chapter')),
  title text NOT NULL CHECK (octet_length(title) BETWEEN 1 AND 1024),
  PRIMARY KEY (owner_user_id, project_id, manuscript_object_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);
ALTER TABLE storyos.projects ADD CONSTRAINT projects_current_chapter_fk
  FOREIGN KEY (owner_user_id, project_id, current_chapter_id)
  REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id)
  MATCH FULL DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE storyos.authoritative_payloads (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  payload_id uuid NOT NULL,
  canonical_bytes bytea NOT NULL CHECK (octet_length(canonical_bytes) <= 1048576),
  PRIMARY KEY (owner_user_id, project_id, payload_id),
  FOREIGN KEY (owner_user_id, project_id)
    REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);
CREATE TABLE storyos.authoritative_revisions (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  manuscript_object_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  payload_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, revision_id),
  UNIQUE (owner_user_id, project_id, manuscript_object_id, revision_id),
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id)
    REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, payload_id)
    REFERENCES storyos.authoritative_payloads(owner_user_id, project_id, payload_id) MATCH FULL
);
CREATE TABLE storyos.authoritative_heads (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  manuscript_object_id uuid NOT NULL,
  current_revision_id uuid NOT NULL,
  PRIMARY KEY (owner_user_id, project_id, manuscript_object_id),
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, current_revision_id)
    REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id)
    MATCH FULL
);

ALTER TABLE storyos.users ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.users FORCE ROW LEVEL SECURITY;
CREATE POLICY users_exact_identity ON storyos.users
  USING (user_id = NULLIF(current_setting('storyos.user_id', true), '')::uuid)
  WITH CHECK (user_id = NULLIF(current_setting('storyos.user_id', true), '')::uuid);

ALTER TABLE storyos.projects ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.projects FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.manuscript_objects ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.manuscript_objects FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.authoritative_payloads ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.authoritative_payloads FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.authoritative_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.authoritative_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.authoritative_heads ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.authoritative_heads FORCE ROW LEVEL SECURITY;

CREATE POLICY projects_exact_scope ON storyos.projects USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);

DO $policy$
DECLARE relation_name text;
BEGIN
  FOREACH relation_name IN ARRAY ARRAY['manuscript_objects', 'authoritative_payloads', 'authoritative_revisions', 'authoritative_heads']
  LOOP
    EXECUTE format(
      'CREATE POLICY %I_exact_scope ON storyos.%I USING (
        owner_user_id = NULLIF(current_setting(''storyos.owner_user_id'', true), '''')::uuid
        AND project_id = NULLIF(current_setting(''storyos.project_id'', true), '''')::uuid
      ) WITH CHECK (
        owner_user_id = NULLIF(current_setting(''storyos.owner_user_id'', true), '''')::uuid
        AND project_id = NULLIF(current_setting(''storyos.project_id'', true), '''')::uuid
      )', relation_name, relation_name);
  END LOOP;
END
$policy$;

GRANT USAGE ON SCHEMA storyos TO storyos_runtime;
GRANT SELECT ON storyos.users, storyos.projects, storyos.manuscript_objects,
  storyos.authoritative_payloads, storyos.authoritative_revisions,
  storyos.authoritative_heads TO storyos_runtime;
