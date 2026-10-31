# Sourced by verify-recovery-hold.sh. Do not execute this file.

recovery_drill=${STORYOS_RECOVERY_DRILL:-fixture-only}
empty_project_one=""
empty_project_two=""
empty_project_title_one="Lawful Empty One"
empty_project_title_two="Lawful Empty Two"

case "$recovery_drill" in
  fixture-only|mixed) ;;
  *)
    echo "STORYOS_RECOVERY_DRILL must be fixture-only or mixed" >&2
    exit 1
    ;;
esac

start_recovery_drill_server() {
  database_url=$1
  if [ ! -x "$server_bin" ] || [ ! -d "$web_root" ]; then
    echo "Release package is required for the isolated recovery Server" >&2
    exit 1
  fi
  drill_server_log=$(mktemp "${TMPDIR:-/tmp}/storyos-recovery-server.XXXXXX")
  STORYOS_DATABASE_URL=$database_url \
  STORYOS_STORAGE_ADMIN_URL="postgres://postgres:wrong@127.0.0.1:1/postgres" \
  STORYOS_BOOTSTRAP_SESSIONS="{\"session-a\":\"$owner_a\"}" \
  STORYOS_CHALLENGE_SECRET="test-only-challenge-secret-that-is-at-least-thirty-two-bytes" \
  STORYOS_MODEL_DESTINATION=host_fake \
    "$server_bin" --bind 127.0.0.1:0 \
    --web-root "$web_root" \
    >"$drill_server_log" 2>&1 &
  drill_server_pid=$!
  attempt=0
  while ! grep -q '^STORYOS_SERVER_URL=http://' "$drill_server_log"; do
    if ! kill -0 "$drill_server_pid" >/dev/null 2>&1; then
      cat "$drill_server_log" >&2
      echo "The StoryOS Server exited before the recovery drill" >&2
      exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ]; then
      cat "$drill_server_log" >&2
      echo "The StoryOS Server did not become ready for the recovery drill" >&2
      exit 1
    fi
    sleep 0.05
  done
  STORYOS_DEV_SERVER=$(sed -n 's/^STORYOS_SERVER_URL=//p' "$drill_server_log" | head -n 1)
  export STORYOS_DEV_SERVER
}

stop_recovery_drill_server() {
  if [ -n "$drill_server_pid" ]; then
    kill "$drill_server_pid" >/dev/null 2>&1 || true
    wait "$drill_server_pid" >/dev/null 2>&1 || true
  fi
  drill_server_pid=""
}

assert_empty_project_stays_empty() {
  empty_container=$1
  empty_id=$2
  empty_title=$3
  facts=$(docker exec "$empty_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT coalesce(current_chapter_id::text, '') || '|' ||
            (SELECT count(*)::text FROM storyos.manuscript_objects
              WHERE project_id = '$empty_id'::uuid) || '|' ||
            title
       FROM storyos.projects
      WHERE project_id = '$empty_id'::uuid")
  if [ "$facts" != "|0|$empty_title" ]; then
    echo "Empty Project $empty_id is not empty on $empty_container: $facts" >&2
    exit 1
  fi
}

assert_mixed_empty_projects() {
  empty_container=$1
  if [ "$recovery_drill" != "mixed" ]; then
    return 0
  fi
  if [ -z "$empty_project_one" ] || [ -z "$empty_project_two" ]; then
    echo "Mixed restore is missing public createProject IDs" >&2
    exit 1
  fi
  assert_empty_project_stays_empty "$empty_container" "$empty_project_one" "$empty_project_title_one"
  assert_empty_project_stays_empty "$empty_container" "$empty_project_two" "$empty_project_title_two"
}

assert_mixed_populated_stays_separate() {
  populated_container=$1
  if [ "$recovery_drill" != "mixed" ]; then
    return 0
  fi
  facts=$(docker exec "$populated_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT coalesce(current_chapter_id::text, '') || '|' || title
       FROM storyos.projects
      WHERE project_id = '$project_id'::uuid")
  if [ "$facts" != "$live_chapter|$wal_marker" ]; then
    echo "Populated Project did not stay separate after mixed restore: $facts" >&2
    exit 1
  fi
}

retained_export_facts() {
  export_container=$1
  docker exec "$export_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT project.title || '|' || project.project_id || '|' ||
            coalesce((SELECT export.export_id || ':' || export.content_sha256 || ':' ||
                             encode(convert_to(export.manuscript_utf8, 'UTF8'), 'hex')
                        FROM storyos.human_readable_manuscript_exports AS export
              WHERE (export.owner_user_id, export.project_id) =
                    (project.owner_user_id, project.project_id)), '') || '|' ||
            coalesce((SELECT manifest.export_id || ':' || manifest.immutable_root || ':' ||
                             encode(entry.payload, 'hex')
                        FROM storyos.project_export_manifests AS manifest
                        JOIN storyos.project_export_entries AS entry
                          ON (entry.owner_user_id, entry.project_id, entry.export_id) =
                             (manifest.owner_user_id, manifest.project_id, manifest.export_id)
                         AND entry.path = 'canonical/authoritative_payloads.json'
              WHERE (manifest.owner_user_id, manifest.project_id) =
                    (project.owner_user_id, project.project_id)), '')
       FROM storyos.projects AS project
      WHERE project.title IN ('Recovery Readable Export', 'Recovery Archive Export')
      ORDER BY project.title"
}

# Stage 4 model records: the global capability profiles, the standing grants, the manifest
# disclosure grants, and the Model Attempts with their disclosure grant. Each line is a count
# and a digest of the ordered rows.
stage4_model_facts() {
  facts_container=$1
  docker exec "$facts_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT 'model_capability_profiles|' || count(*) || '|' ||
            md5(coalesce(string_agg(row_to_json(capability)::text, ';'
              ORDER BY row_to_json(capability)::text), ''))
       FROM storyos.model_capability_profiles AS capability
     UNION ALL
     SELECT 'project_destination_grants|' || count(*) || '|' ||
            md5(coalesce(string_agg(grant_row.grant_id || ':' || grant_row.grant_scope, ';'
              ORDER BY grant_row.grant_id), ''))
       FROM storyos.project_destination_grants AS grant_row
     UNION ALL
     SELECT 'manifest_disclosure_grants|' || count(*) || '|' ||
            md5(coalesce(string_agg(manifest.context_assembly_manifest_id || ':' ||
              manifest.disclosure_grant_id || ':' || manifest.disclosure_grant_scope, ';'
              ORDER BY manifest.context_assembly_manifest_id), ''))
       FROM storyos.context_assembly_manifests AS manifest
      WHERE manifest.disclosure_grant_id IS NOT NULL
     UNION ALL
     SELECT 'model_attempts|' || count(*) || '|' ||
            md5(coalesce(string_agg(attempt.model_attempt_id || ':' || attempt.attempt_role ||
              ':' || attempt.dispatch_state || ':' || (attempt.payload->'disclosure_grant')::text,
              ';' ORDER BY attempt.model_attempt_id), ''))
       FROM storyos.model_attempts AS attempt
      WHERE attempt.payload ? 'disclosure_grant'"
}

assert_stage4_model_facts_present() {
  if [ "$(printf '%s\n' "$1" | grep -c '|0|')" != "0" ] \
    || [ "$(printf '%s\n' "$1" | wc -l | tr -d ' ')" != "4" ]; then
    echo "The recovery seed did not create each Stage 4 model record family: $1" >&2
    exit 1
  fi
}
