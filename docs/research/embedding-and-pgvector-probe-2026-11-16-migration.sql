-- A migration text that makes a vector table only when the database offers pgvector (S7-C1,
-- #1572; ADR 0095 section 4). It runs in the transaction of the Storage Activation after the
-- baseline, which leaves the role storyos_owner set. The vector extension is not trusted, so the
-- admin role of the activation creates it. S7-C2b1 owns the columns of the real table.

RESET ROLE;
DO $migration$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_available_extensions WHERE name = 'vector') THEN
        RETURN;
    END IF;
    CREATE SCHEMA IF NOT EXISTS extensions;
    CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA extensions;
    IF (SELECT extnamespace::regnamespace::text FROM pg_catalog.pg_extension
        WHERE extname = 'vector') <> 'extensions' THEN
        RAISE EXCEPTION 'the vector extension is not in the schema extensions';
    END IF;
    GRANT USAGE ON SCHEMA extensions TO storyos_owner, storyos_runtime;
    SET LOCAL ROLE storyos_owner;
    EXECUTE $ddl$
        CREATE TABLE storyos.passage_vectors (
            owner_user_id uuid NOT NULL,
            project_id uuid NOT NULL,
            passage_id uuid NOT NULL,
            embedding extensions.vector(1024) NOT NULL,
            PRIMARY KEY (owner_user_id, project_id, passage_id)
        );
        ALTER TABLE storyos.passage_vectors ENABLE ROW LEVEL SECURITY;
        ALTER TABLE storyos.passage_vectors FORCE ROW LEVEL SECURITY;
        CREATE POLICY passage_vectors_exact_scope ON storyos.passage_vectors USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));
        CREATE INDEX passage_vectors_embedding ON storyos.passage_vectors
            USING hnsw (embedding extensions.vector_cosine_ops);
        GRANT SELECT, INSERT, DELETE ON TABLE storyos.passage_vectors TO storyos_runtime;
    $ddl$;
END
$migration$;
SET LOCAL ROLE storyos_owner;
