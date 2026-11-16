-- The Project Isolation proof of a vector(1024) table (S7-C1, #1572). Run it as the superuser of a
-- new database on an image with pgvector:
--   docker exec -i <container> psql -X -v ON_ERROR_STOP=1 -U postgres < <this file>
-- The table has forced row level security and the exact Scope policy of the Release 1 baseline.
-- probe_runtime has the rights of storyos_runtime. Project 1 has 20,000 rows. Project 2 has
-- 1,000 rows that are nearer to the question vector than each row of Project 1.

\set ON_ERROR_STOP 1
\pset pager off
SET client_min_messages = warning;

CREATE ROLE probe_owner NOLOGIN;
CREATE ROLE probe_runtime NOLOGIN NOSUPERUSER NOBYPASSRLS;
CREATE SCHEMA extensions;
CREATE EXTENSION vector WITH SCHEMA extensions;
GRANT USAGE ON SCHEMA extensions TO probe_owner, probe_runtime;
CREATE SCHEMA probe AUTHORIZATION probe_owner;
GRANT USAGE ON SCHEMA probe TO probe_runtime;

SET ROLE probe_owner;

-- A curve in 1,024 dimensions. Near t, the nearest rows are the rows with the nearest t. The curve
-- also comes near itself at other places, so the exact scan is the reference. A scalar subquery keeps the
-- 1,024 values out of the plan text, and computes the question vector one time.
CREATE FUNCTION probe.curve(t double precision) RETURNS extensions.vector
    LANGUAGE sql STABLE PARALLEL SAFE
    AS $$ SELECT array_agg(cos(t * 0.0003 * j + j) ORDER BY j)::extensions.vector(1024)
          FROM generate_series(1, 1024) AS j $$;
GRANT EXECUTE ON FUNCTION probe.curve(double precision) TO probe_runtime;

CREATE TABLE probe.passage_vectors (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    passage_id integer NOT NULL,
    position double precision NOT NULL,
    embedding extensions.vector(1024) NOT NULL,
    PRIMARY KEY (owner_user_id, project_id, passage_id)
);
ALTER TABLE probe.passage_vectors ENABLE ROW LEVEL SECURITY;
ALTER TABLE probe.passage_vectors FORCE ROW LEVEL SECURITY;
CREATE POLICY passage_vectors_exact_scope ON probe.passage_vectors USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));
GRANT SELECT, INSERT ON TABLE probe.passage_vectors TO probe_runtime;

RESET ROLE;

-- The rows go in through the policy, in the Scope of each Project.
BEGIN;
SET LOCAL ROLE probe_runtime;
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c1001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c1002', true) \g /dev/null
INSERT INTO probe.passage_vectors
SELECT '018f0000-0000-7001-8000-0000000c1001', '018f0000-0000-7001-8000-0000000c1002', i, i, probe.curve(i)
FROM generate_series(1, 20000) AS i;
COMMIT;

BEGIN;
SET LOCAL ROLE probe_runtime;
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c2001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c2002', true) \g /dev/null
INSERT INTO probe.passage_vectors
SELECT '018f0000-0000-7001-8000-0000000c2001', '018f0000-0000-7001-8000-0000000c2002', 100000 + k,
       10000.5 + (k - 500.5) * 0.0004, probe.curve(10000.5 + (k - 500.5) * 0.0004)
FROM generate_series(1, 1000) AS k;
COMMIT;

SET maintenance_work_mem = '1GB';
SET ROLE probe_owner;
CREATE INDEX passage_vectors_embedding ON probe.passage_vectors
    USING hnsw (embedding extensions.vector_cosine_ops);
RESET ROLE;
ANALYZE probe.passage_vectors;

\echo '== 1. Scope reads'
BEGIN;
SET LOCAL ROLE probe_runtime;
SELECT count(*) AS no_scope_rows FROM probe.passage_vectors;
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c1001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c1002', true) \g /dev/null
SELECT count(*) AS project_1_rows,
       count(*) FILTER (WHERE project_id <> '018f0000-0000-7001-8000-0000000c1002') AS other_rows
FROM probe.passage_vectors;
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c2001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c2002', true) \g /dev/null
SELECT count(*) AS project_2_rows,
       count(*) FILTER (WHERE project_id <> '018f0000-0000-7001-8000-0000000c2002') AS other_rows
FROM probe.passage_vectors;
-- A wrong owner with the right Project sees no row.
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c2001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c1002', true) \g /dev/null
SELECT count(*) AS crossed_scope_rows FROM probe.passage_vectors;
COMMIT;

\echo '== 2. Nearest 10 rows of Project 1 to curve(10000.5)'
BEGIN;
SET LOCAL ROLE probe_runtime;
-- The exact scan. With the index, min() and max() also become HNSW reads.
SET LOCAL enable_indexscan = off;
SET LOCAL enable_bitmapscan = off;
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c2001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c2002', true) \g /dev/null
SELECT max(embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5))) AS project_2_farthest_distance
FROM probe.passage_vectors \gset
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c1001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c1002', true) \g /dev/null
SELECT :project_2_farthest_distance AS project_2_farthest_distance,
       min(embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5))) AS project_1_nearest_distance
FROM probe.passage_vectors;

SELECT array_agg(passage_id ORDER BY distance, passage_id) AS exact_ids FROM (
    SELECT passage_id, embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) AS distance
    FROM probe.passage_vectors ORDER BY distance LIMIT 10) AS nearest \gset
\echo exact scan: :exact_ids
EXPLAIN (ANALYZE, COSTS OFF, SUMMARY ON, BUFFERS OFF)
SELECT passage_id FROM probe.passage_vectors
ORDER BY embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) LIMIT 10;
SET LOCAL enable_indexscan = on;
SET LOCAL enable_bitmapscan = on;

\echo '-- HNSW with the default settings (hnsw.iterative_scan = off, hnsw.ef_search = 40)'
SELECT coalesce(array_agg(passage_id ORDER BY distance, passage_id), '{}') AS default_ids, count(*) AS default_count FROM (
    SELECT passage_id, embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) AS distance
    FROM probe.passage_vectors ORDER BY distance LIMIT 10) AS nearest \gset
\echo default index scan: :default_ids rows :default_count
SELECT :'default_ids' = :'exact_ids' AS default_equals_exact;

\echo '-- HNSW with SET LOCAL hnsw.iterative_scan = strict_order'
SET LOCAL hnsw.iterative_scan = strict_order;
SELECT array_agg(passage_id ORDER BY distance, passage_id) AS strict_ids FROM (
    SELECT passage_id, embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) AS distance
    FROM probe.passage_vectors ORDER BY distance LIMIT 10) AS nearest \gset
\echo strict index scan: :strict_ids
SELECT :'strict_ids' = :'exact_ids' AS strict_equals_exact;
EXPLAIN (ANALYZE, COSTS OFF, SUMMARY ON, BUFFERS OFF)
SELECT passage_id FROM probe.passage_vectors
ORDER BY embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) LIMIT 10;

\echo '-- The same read with hnsw.max_scan_tuples = 500, below the 1,000 nearer rows of Project 2'
SET LOCAL hnsw.max_scan_tuples = 500;
SELECT count(*) AS bounded_count FROM (
    SELECT passage_id FROM probe.passage_vectors
    ORDER BY embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) LIMIT 10) AS nearest;
COMMIT;

\echo '== 3. The read with no vector index and the default settings'
DROP INDEX probe.passage_vectors_embedding;
BEGIN;
SET LOCAL ROLE probe_runtime;
SELECT set_config('storyos.owner_user_id', '018f0000-0000-7001-8000-0000000c1001', true),
       set_config('storyos.project_id', '018f0000-0000-7001-8000-0000000c1002', true) \g /dev/null
SELECT array_agg(passage_id ORDER BY distance, passage_id) AS plain_ids FROM (
    SELECT passage_id, embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) AS distance
    FROM probe.passage_vectors ORDER BY distance LIMIT 10) AS nearest \gset
SELECT :'plain_ids' = :'exact_ids' AS plain_equals_exact;
EXPLAIN (ANALYZE, COSTS OFF, SUMMARY ON, BUFFERS OFF)
SELECT passage_id FROM probe.passage_vectors
ORDER BY embedding OPERATOR(extensions.<=>) (SELECT probe.curve(10000.5)) LIMIT 10;
COMMIT;

\echo '== 4. Versions'
SELECT extversion AS pgvector_version, extnamespace::regnamespace AS extension_schema
FROM pg_extension WHERE extname = 'vector';
SELECT current_setting('server_version') AS server_version;
