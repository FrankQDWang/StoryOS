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
  unset STORYOS_STAGE1_AUTHORITY_ORACLE
  drill_server_log=$(mktemp "${TMPDIR:-/tmp}/storyos-recovery-server.XXXXXX")
  STORYOS_DATABASE_URL=$database_url \
  STORYOS_STORAGE_ADMIN_URL="postgres://postgres:wrong@127.0.0.1:1/postgres" \
  STORYOS_BOOTSTRAP_SESSIONS="{\"session-a\":\"$owner_a\"}" \
  STORYOS_CHALLENGE_SECRET="test-only-challenge-secret-that-is-at-least-thirty-two-bytes" \
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
