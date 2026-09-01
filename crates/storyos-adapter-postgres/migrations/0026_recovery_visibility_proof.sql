SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.recovery_copies (
  recovery_copy_id uuid PRIMARY KEY,
  method text NOT NULL CHECK (method = 'pg_basebackup'),
  chain_sha256 text NOT NULL CHECK (chain_sha256 ~ '^[0-9a-f]{64}$'),
  required_wal_member text NOT NULL CHECK (octet_length(required_wal_member) BETWEEN 1 AND 128),
  recovery_target_lsn text NOT NULL CHECK (octet_length(recovery_target_lsn) BETWEEN 1 AND 32),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE storyos.backup_wal_evidence (
  recovery_copy_id uuid NOT NULL
    REFERENCES storyos.recovery_copies(recovery_copy_id),
  path text NOT NULL CHECK (octet_length(path) BETWEEN 1 AND 512),
  sha256 text NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (recovery_copy_id, path)
);

CREATE TABLE storyos.restore_proofs (
  restore_proof_id uuid PRIMARY KEY,
  recovery_copy_id uuid NOT NULL
    REFERENCES storyos.recovery_copies(recovery_copy_id),
  isolated_target_identity text NOT NULL
    CHECK (octet_length(isolated_target_identity) BETWEEN 1 AND 256),
  recovery_target_lsn text NOT NULL CHECK (octet_length(recovery_target_lsn) BETWEEN 1 AND 32),
  state text NOT NULL CHECK (state IN ('recovery_hold', 'visible')),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE storyos.recovery_visibility_proofs (
  visibility_proof_id uuid PRIMARY KEY,
  restore_proof_id uuid NOT NULL UNIQUE
    REFERENCES storyos.restore_proofs(restore_proof_id),
  recovery_copy_id uuid NOT NULL
    REFERENCES storyos.recovery_copies(recovery_copy_id),
  isolated_target_identity text NOT NULL
    CHECK (octet_length(isolated_target_identity) BETWEEN 1 AND 256),
  catalog_identity text NOT NULL
    CHECK (catalog_identity = 'storyos.persistence.catalog.release-1.v3'),
  checks jsonb NOT NULL,
  search_rebuild jsonb NOT NULL,
  statistics_rebuild jsonb NOT NULL,
  lifecycle_range jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

GRANT USAGE ON SCHEMA storyos TO storyos_restore, storyos_backup;
GRANT SELECT, INSERT ON storyos.recovery_copies, storyos.backup_wal_evidence,
  storyos.restore_proofs, storyos.recovery_visibility_proofs TO storyos_restore;
GRANT SELECT ON storyos.recovery_copies, storyos.backup_wal_evidence TO storyos_backup;

RESET ROLE;

CREATE FUNCTION storyos.pass_recovery_visibility_proof(
  p_recovery_copy_id uuid,
  p_restore_proof_id uuid,
  p_visibility_proof_id uuid,
  p_isolated_target_identity text,
  p_chain_sha256 text,
  p_required_wal_member text,
  p_recovery_target_lsn text,
  p_method text,
  p_catalog_identity text
) RETURNS text
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, storyos
AS $fn$
DECLARE
  runtime_login boolean;
  restore_state text;
  missing_rls text;
  head_gaps bigint;
  chapter_gaps bigint;
  replay_gaps bigint;
  archived_without_decision bigint;
  decision_without_archive bigint;
  export_revival bigint;
  archive_revival bigint;
  live_search jsonb;
  live_stats jsonb;
BEGIN
  IF p_method IS DISTINCT FROM 'pg_basebackup'
     OR p_catalog_identity IS DISTINCT FROM 'storyos.persistence.catalog.release-1.v3'
     OR p_chain_sha256 !~ '^[0-9a-f]{64}$'
     OR octet_length(p_isolated_target_identity) = 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: recovery input';
  END IF;

  SELECT rolcanlogin INTO runtime_login
    FROM pg_roles WHERE rolname = 'storyos_runtime';
  IF runtime_login THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: already visible';
  END IF;
  IF NOT EXISTS (
       SELECT 1 FROM pg_roles
        WHERE rolname = 'storyos_runtime'
          AND NOT rolsuper AND NOT rolbypassrls AND NOT rolcanlogin
     )
     OR NOT EXISTS (
       SELECT 1 FROM pg_roles
        WHERE rolname = 'storyos_restore'
          AND NOT rolsuper AND NOT rolbypassrls AND rolcanlogin
     )
     OR NOT EXISTS (
       SELECT 1 FROM pg_roles
        WHERE rolname = 'storyos_backup'
          AND NOT rolsuper AND NOT rolbypassrls AND rolreplication AND rolcanlogin
     ) THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: roles';
  END IF;

  IF NOT EXISTS (
       SELECT 1 FROM storyos.recovery_copies
        WHERE recovery_copy_id = p_recovery_copy_id
          AND method = p_method
          AND chain_sha256 = p_chain_sha256
          AND required_wal_member = p_required_wal_member
          AND recovery_target_lsn = p_recovery_target_lsn
     )
     OR NOT EXISTS (
       SELECT 1 FROM storyos.backup_wal_evidence
        WHERE recovery_copy_id = p_recovery_copy_id
     ) THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: recovery copy';
  END IF;

  SELECT state INTO restore_state
    FROM storyos.restore_proofs
   WHERE restore_proof_id = p_restore_proof_id
     AND recovery_copy_id = p_recovery_copy_id
     AND isolated_target_identity = p_isolated_target_identity
     AND recovery_target_lsn = p_recovery_target_lsn;
  IF restore_state IS DISTINCT FROM 'recovery_hold' THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: restore proof';
  END IF;
  IF EXISTS (
       SELECT 1 FROM storyos.recovery_visibility_proofs
        WHERE restore_proof_id = p_restore_proof_id
           OR visibility_proof_id = p_visibility_proof_id
     ) THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: duplicate proof';
  END IF;

  SELECT string_agg(c.relname, ',' ORDER BY c.relname) INTO missing_rls
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_attribute a1
      ON a1.attrelid = c.oid AND a1.attname = 'owner_user_id' AND NOT a1.attisdropped
    JOIN pg_attribute a2
      ON a2.attrelid = c.oid AND a2.attname = 'project_id' AND NOT a2.attisdropped
   WHERE n.nspname = 'storyos' AND c.relkind = 'r'
     AND NOT (c.relrowsecurity AND c.relforcerowsecurity);
  IF missing_rls IS NOT NULL THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: forced rls';
  END IF;

  SELECT count(*) INTO head_gaps
    FROM storyos.authoritative_heads AS head
    LEFT JOIN storyos.authoritative_revisions AS revision
      ON (revision.owner_user_id, revision.project_id,
          revision.manuscript_object_id, revision.revision_id) =
         (head.owner_user_id, head.project_id,
          head.manuscript_object_id, head.current_revision_id)
   WHERE revision.revision_id IS NULL;
  SELECT count(*) INTO chapter_gaps
    FROM storyos.projects AS project
    LEFT JOIN storyos.manuscript_objects AS chapter
      ON (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id) =
         (project.owner_user_id, project.project_id, project.current_chapter_id)
     AND chapter.object_kind = 'chapter'
   WHERE chapter.manuscript_object_id IS NULL;
  SELECT count(*) INTO replay_gaps
    FROM storyos.projects AS project
    LEFT JOIN storyos.replay_generations AS generation
      ON (generation.owner_user_id, generation.project_id) =
         (project.owner_user_id, project.project_id)
    LEFT JOIN storyos.replay_floors AS floor
      ON (floor.owner_user_id, floor.project_id, floor.replay_generation) =
         (generation.owner_user_id, generation.project_id, generation.replay_generation)
   WHERE generation.replay_generation IS NULL OR floor.floor_position IS NULL;
  IF head_gaps <> 0 OR chapter_gaps <> 0 OR replay_gaps <> 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: canonical heads';
  END IF;

  SELECT count(*) INTO archived_without_decision
    FROM storyos.projects AS project
   WHERE project.lifecycle_state = 'archived'
     AND NOT EXISTS (
       SELECT 1 FROM storyos.project_archival_decisions AS decision
        WHERE decision.owner_user_id = project.owner_user_id
          AND decision.project_id = project.project_id
          AND decision.resulting_lifecycle_state = 'archived'
     );
  SELECT count(*) INTO decision_without_archive
    FROM storyos.project_archival_decisions AS decision
    JOIN storyos.projects AS project
      ON (project.owner_user_id, project.project_id) =
         (decision.owner_user_id, decision.project_id)
   WHERE project.lifecycle_state IS DISTINCT FROM 'archived';
  IF archived_without_decision <> 0 OR decision_without_archive <> 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: lifecycle range';
  END IF;

  SELECT coalesce(
           jsonb_agg(to_jsonb(live) ORDER BY live.project_id, live.chapter_id),
           '[]'::jsonb
         )
    INTO live_search
    FROM (
      SELECT project.project_id::text AS project_id,
             chapter.manuscript_object_id::text AS chapter_id,
             convert_from(payload.canonical_bytes, 'UTF8') AS payload_text,
             octet_length(payload.canonical_bytes) AS byte_length
        FROM storyos.projects AS project
        JOIN storyos.manuscript_objects AS chapter
          ON (chapter.owner_user_id, chapter.project_id) =
             (project.owner_user_id, project.project_id)
         AND chapter.object_kind = 'chapter'
        JOIN storyos.manuscript_objects AS volume
          ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
             (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
         AND volume.object_kind = 'volume'
        JOIN storyos.authoritative_heads AS head
          ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
             (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id,
              revision.manuscript_object_id, revision.revision_id) =
             (head.owner_user_id, head.project_id,
              head.manuscript_object_id, head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
       WHERE project.lifecycle_state = 'active'
         AND NOT EXISTS (
           SELECT 1 FROM storyos.chapter_removal_decisions AS removal
            WHERE removal.owner_user_id = chapter.owner_user_id
              AND removal.project_id = chapter.project_id
              AND removal.chapter_id = chapter.manuscript_object_id
         )
         AND NOT EXISTS (
           SELECT 1 FROM storyos.volume_removal_decisions AS removal
            WHERE removal.owner_user_id = volume.owner_user_id
              AND removal.project_id = volume.project_id
              AND removal.volume_id = volume.manuscript_object_id
         )
    ) AS live;

  SELECT jsonb_build_object(
           'counting_profile', 'storyos.statistics.unicode-16.0.0.v1',
           'chapters', coalesce(
             jsonb_agg(jsonb_build_object(
               'project_id', item->>'project_id',
               'chapter_id', item->>'chapter_id',
               'character_count', char_length(item->>'payload_text')
             )),
             '[]'::jsonb
           )
         )
    INTO live_stats
    FROM jsonb_array_elements(live_search) AS item;

  SELECT count(*) INTO export_revival
    FROM storyos.human_readable_manuscript_exports AS export
    JOIN storyos.projects AS project
      ON (project.owner_user_id, project.project_id) =
         (export.owner_user_id, export.project_id)
   WHERE project.lifecycle_state IS DISTINCT FROM 'active';
  SELECT count(*) INTO archive_revival
    FROM storyos.project_export_manifests AS archive
    JOIN storyos.projects AS project
      ON (project.owner_user_id, project.project_id) =
         (archive.owner_user_id, archive.project_id)
   WHERE project.lifecycle_state IS DISTINCT FROM 'active';
  IF export_revival <> 0 OR archive_revival <> 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: revival';
  END IF;

  INSERT INTO storyos.recovery_visibility_proofs (
    visibility_proof_id, restore_proof_id, recovery_copy_id, isolated_target_identity,
    catalog_identity, checks, search_rebuild, statistics_rebuild, lifecycle_range
  ) VALUES (
    p_visibility_proof_id,
    p_restore_proof_id,
    p_recovery_copy_id,
    p_isolated_target_identity,
    p_catalog_identity,
    jsonb_build_object(
      'schema_catalog', p_catalog_identity,
      'forced_rls', true,
      'canonical_heads', true,
      'roles_grants', true,
      'recovery_input', p_chain_sha256
    ),
    live_search,
    live_stats,
    jsonb_build_object(
      'archived_without_decision', archived_without_decision,
      'decision_without_archive', decision_without_archive
    )
  );

  UPDATE storyos.restore_proofs
     SET state = 'visible'
   WHERE restore_proof_id = p_restore_proof_id
     AND state = 'recovery_hold';
  IF NOT FOUND THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: restore state';
  END IF;

  ALTER ROLE storyos_runtime LOGIN;
  RETURN 'visible';
END
$fn$;

REVOKE ALL ON FUNCTION storyos.pass_recovery_visibility_proof(
  uuid, uuid, uuid, text, text, text, text, text, text
) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION storyos.pass_recovery_visibility_proof(
  uuid, uuid, uuid, text, text, text, text, text, text
) TO storyos_restore;
