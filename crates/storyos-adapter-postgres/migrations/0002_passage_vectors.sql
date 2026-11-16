-- The passage vectors of the manuscript (ADR 0095 sections 2 and 4), in the form of the record of
-- S7-C1. The table exists only when the database offers pgvector. A database without the extension
-- activates with the same sources and has no vector table. The vector extension is not trusted, so
-- the admin role of the Storage Activation creates it in the schema extensions. The read form is an
-- exact scan in the Project Scope, so the table has no vector index. The rows are a Retrieval
-- Index: they hold no text of the author, and the Project export does not hold them.

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
            chapter_id uuid NOT NULL,
            embedding_registration_revision uuid NOT NULL,
            cut_revision text NOT NULL,
            passage_digest text NOT NULL,
            embedding extensions.vector(1024) NOT NULL,
            created_at timestamp with time zone NOT NULL,
            CONSTRAINT passage_vectors_cut_revision_check CHECK ((length(cut_revision) BETWEEN 1 AND 128)),
            CONSTRAINT passage_vectors_passage_digest_check CHECK ((passage_digest ~ '^sha256:[0-9a-f]{64}$'::text)),
            CONSTRAINT passage_vectors_pkey PRIMARY KEY (owner_user_id, project_id, chapter_id, embedding_registration_revision, cut_revision, passage_digest),
            CONSTRAINT passage_vectors_chapter_fkey FOREIGN KEY (owner_user_id, project_id, chapter_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL
        );
        ALTER TABLE storyos.passage_vectors ENABLE ROW LEVEL SECURITY;
        ALTER TABLE storyos.passage_vectors FORCE ROW LEVEL SECURITY;
        CREATE POLICY passage_vectors_exact_scope ON storyos.passage_vectors USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));
        GRANT SELECT,INSERT,DELETE ON TABLE storyos.passage_vectors TO storyos_runtime;
    $ddl$;
END
$migration$;
SET LOCAL ROLE storyos_owner;
