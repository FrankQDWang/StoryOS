#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

verify_web_migration_guards() {
  legacy_web_files=$(find apps/web \
    \( -path apps/web/dist -o -path apps/web/node_modules \) -prune -o \
    -type f \( -name '*.js' -o -name '*.jsx' -o -name '*.mjs' -o -name '*.cjs' \) -print)
  if [ -n "$legacy_web_files" ]; then
    echo "Hand-written Web JavaScript remains:" >&2
    printf '%s\n' "$legacy_web_files" >&2
    exit 1
  fi

  raw_harness_matches=$(rg -n \
    'DevTools listening|webSocketDebuggerUrl|Runtime\.evaluate|remote-debugging-(port|pipe)|new WebSocket\(' \
    apps/web --glob '!dist/**' --glob '!node_modules/**' || true)
  if [ -n "$raw_harness_matches" ]; then
    echo "An active raw browser harness signature remains:" >&2
    printf '%s\n' "$raw_harness_matches" >&2
    exit 1
  fi

  broad_cdp_matches=$(rg -n \
    'newCDPSession|CDPSession|session\.send\(' \
    apps/web --glob '!dist/**' --glob '!node_modules/**' \
      --glob '!**/test/support/browser-commands.ts' || true)
  if [ -n "$broad_cdp_matches" ]; then
    echo "A CDP primitive escaped the typed Browser Command boundary:" >&2
    printf '%s\n' "$broad_cdp_matches" >&2
    exit 1
  fi

  unsupported_cdp_matches=$(rg -n 'session\.send\(' \
    apps/web/test/support/browser-commands.ts \
      | rg -v 'Input\.imeSetComposition' || true)
  if [ -n "$unsupported_cdp_matches" ]; then
    echo "The IME Browser Command uses an unsupported CDP method:" >&2
    printf '%s\n' "$unsupported_cdp_matches" >&2
    exit 1
  fi

  active_legacy_entry_matches=$(rg -n \
    'production-page-browser\.integration\.test\.mjs|s1-jrn-001-browser\.integration\.test\.mjs|author-edit-batch-browser-process\.test\.mjs|author-edit-batch-prerelease-browser-harness\.mjs' \
    Makefile package.json apps/web/package.json scripts .github || true)
  if [ -n "$active_legacy_entry_matches" ]; then
    echo "An active legacy browser harness entry remains:" >&2
    printf '%s\n' "$active_legacy_entry_matches" >&2
    exit 1
  fi

  browser_skip_matches=$(rg -n \
    '\.(skip|skipIf|runIf|todo)\b|\bskip\s*:|Chrome or Chromium is unavailable|CHROME_BIN|chromium-browser|/usr/bin/chromium' \
    apps/web/test/browser-source apps/web/test/browser-exact-dist \
      apps/web/test/support/browser-command-client.ts \
      apps/web/test/support/browser-command-contract.ts \
      apps/web/test/support/production-host-command.ts \
      apps/web/test/support/browser-commands.ts apps/web/vitest.config.ts \
      --glob '*.ts' --glob '*.tsx' \
      --glob '*.js' --glob '*.jsx' --glob '*.mjs' --glob '*.cjs' || true)
  if [ -n "$browser_skip_matches" ]; then
    echo "A browser skip or fallback remains:" >&2
    printf '%s\n' "$browser_skip_matches" >&2
    exit 1
  fi

  injected_session_matches=$(rg -n \
    'addCookies\(|updateClientSessionCookie\(\{ action: "set"' \
    apps/web/test/browser-exact-dist \
    apps/web/test/support/production-host-command.ts || true)
  if [ -n "$injected_session_matches" ]; then
    echo "A single-User exact-dist journey still injects a test storyos_session cookie:" >&2
    printf '%s\n' "$injected_session_matches" >&2
    exit 1
  fi

  type_escape_matches=$(rg -n \
    '\bany\b|@ts-(ignore|nocheck)|declare module|\bas unknown as\b|\bas [A-Za-z0-9_.$<>\[\] |]+ as\b' \
    apps/web --glob '*.ts' --glob '*.tsx' --glob '!dist/**' --glob '!node_modules/**' || true)
  if [ -n "$type_escape_matches" ]; then
    echo "A prohibited TypeScript escape remains:" >&2
    printf '%s\n' "$type_escape_matches" >&2
    exit 1
  fi
}

record_google_chrome_version() {
  if [ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]; then
    chrome_executable="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
  elif command -v google-chrome-stable >/dev/null 2>&1; then
    chrome_executable=$(command -v google-chrome-stable)
  elif command -v google-chrome >/dev/null 2>&1; then
    chrome_executable=$(command -v google-chrome)
  else
    echo "Google Chrome Stable is required" >&2
    exit 1
  fi
  chrome_version=$("$chrome_executable" --version)
  case "$chrome_version" in
    "Google Chrome "*) ;;
    *)
      echo "The browser is not Google Chrome Stable: $chrome_version" >&2
      exit 1
      ;;
  esac
  printf 'Google Chrome Stable: %s\n' "$chrome_version"
}

verify_web_migration_guards
record_google_chrome_version
if [ "${STORYOS_WEB_TYPECHECKED:-}" != "1" ]; then
  make release-package
fi

container="storyos-issue105-$$"
s1_server_pid=""
s1_server_log=""
export CARGO_NET_OFFLINE=true
cleanup() {
  if [ -n "$s1_server_pid" ]; then
    kill "$s1_server_pid" >/dev/null 2>&1 || true
    wait "$s1_server_pid" >/dev/null 2>&1 || true
  fi
  if [ -n "$s1_server_log" ]; then
    rm -f "$s1_server_log"
  fi
  docker rm -f "$container" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
cleanup

docker run --detach --name "$container" \
  --env POSTGRES_PASSWORD=admin \
  --publish 127.0.0.1::5432 postgres:16-alpine >/dev/null

attempt=0
until docker logs "$container" 2>&1 | grep -q "PostgreSQL init process complete" \
  && docker exec "$container" pg_isready -U postgres >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 40 ]; then
    echo "PostgreSQL did not become ready" >&2
    exit 1
  fi
  sleep 0.25
done

docker cp "$repository_root/crates/storyos-adapter-postgres/migrations/." \
  "$container:/tmp/storyos-release1-bootstrap" >/dev/null
if docker exec "$container" psql -X -v ON_ERROR_STOP=1 --single-transaction -U postgres \
  -f /tmp/storyos-release1-bootstrap/0000_roles.sql \
  -f /tmp/storyos-release1-bootstrap/0001_controlled_project.sql \
  -f /tmp/storyos-release1-bootstrap/0002_project_command_challenges.sql \
  -c "SELECT 1 / 0" \
  -f /tmp/storyos-release1-bootstrap/0004_editor_sessions.sql \
  -f /tmp/storyos-release1-bootstrap/0005_author_edits.sql \
  -f /tmp/storyos-release1-bootstrap/0006_snapshot_replay.sql \
  -f /tmp/storyos-release1-bootstrap/0007_takeover_admission_activity.sql \
  -f /tmp/storyos-release1-bootstrap/0008_create_project_challenge.sql \
  -f /tmp/storyos-release1-bootstrap/0009_create_empty_project.sql \
  -f /tmp/storyos-release1-bootstrap/0010_list_owned_projects.sql \
  -f /tmp/storyos-release1-bootstrap/0011_update_project.sql \
  -f /tmp/storyos-release1-bootstrap/0012_archive_project.sql \
  -f /tmp/storyos-release1-bootstrap/0013_manuscript_tree.sql \
  -f /tmp/storyos-release1-bootstrap/0014_create_volume.sql \
  -f /tmp/storyos-release1-bootstrap/0015_create_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0016_update_volume.sql \
  -f /tmp/storyos-release1-bootstrap/0017_update_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0018_manuscript_blocks.sql \
  -f /tmp/storyos-release1-bootstrap/0019_set_current_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0020_undo_latest_author_action.sql \
  -f /tmp/storyos-release1-bootstrap/0021_delete_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0022_delete_volume.sql \
  -f /tmp/storyos-release1-bootstrap/0023_export_human_readable_manuscript.sql \
  -f /tmp/storyos-release1-bootstrap/0024_export_project_archive_admission.sql \
  -f /tmp/storyos-release1-bootstrap/0025_export_project_archive_entries.sql \
  -f /tmp/storyos-release1-bootstrap/0026_recovery_visibility_proof.sql \
  -f /tmp/storyos-release1-bootstrap/0027_author_command_admission_reconfirmations.sql \
  -f /tmp/storyos-release1-bootstrap/0028_tree_revision_conflict_names.sql \
  -f /tmp/storyos-release1-bootstrap/0029_human_readable_manuscript_export_operations.sql \
  -f /tmp/storyos-release1-bootstrap/0030_human_readable_export_worker_claim.sql \
  -f /tmp/storyos-release1-bootstrap/0031_project_export_operations.sql \
  -f /tmp/storyos-release1-bootstrap/0032_create_volume_canonical_sibling_order.sql >/dev/null 2>&1; then
  echo "The faulted Release 1 bootstrap unexpectedly committed" >&2
  exit 1
fi
rollback_state=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT (SELECT count(*) FROM pg_roles
            WHERE rolname IN ('storyos_owner', 'storyos_runtime'))::text
          || '/' || COALESCE(to_regnamespace('storyos')::text, 'absent')")
if [ "$rollback_state" != "0/absent" ]; then
  echo "The faulted Release 1 bootstrap exposed partial state: $rollback_state" >&2
  exit 1
fi

docker exec "$container" psql -X -v ON_ERROR_STOP=1 --single-transaction -U postgres \
  -f /tmp/storyos-release1-bootstrap/0000_roles.sql \
  -f /tmp/storyos-release1-bootstrap/0001_controlled_project.sql \
  -f /tmp/storyos-release1-bootstrap/0002_project_command_challenges.sql \
  -f /tmp/storyos-release1-bootstrap/0004_editor_sessions.sql \
  -f /tmp/storyos-release1-bootstrap/0005_author_edits.sql \
  -f /tmp/storyos-release1-bootstrap/0006_snapshot_replay.sql \
  -f /tmp/storyos-release1-bootstrap/0007_takeover_admission_activity.sql \
  -f /tmp/storyos-release1-bootstrap/0008_create_project_challenge.sql \
  -f /tmp/storyos-release1-bootstrap/0009_create_empty_project.sql \
  -f /tmp/storyos-release1-bootstrap/0010_list_owned_projects.sql \
  -f /tmp/storyos-release1-bootstrap/0011_update_project.sql \
  -f /tmp/storyos-release1-bootstrap/0012_archive_project.sql \
  -f /tmp/storyos-release1-bootstrap/0013_manuscript_tree.sql \
  -f /tmp/storyos-release1-bootstrap/0014_create_volume.sql \
  -f /tmp/storyos-release1-bootstrap/0015_create_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0016_update_volume.sql \
  -f /tmp/storyos-release1-bootstrap/0017_update_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0018_manuscript_blocks.sql \
  -f /tmp/storyos-release1-bootstrap/0019_set_current_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0020_undo_latest_author_action.sql \
  -f /tmp/storyos-release1-bootstrap/0021_delete_chapter.sql \
  -f /tmp/storyos-release1-bootstrap/0022_delete_volume.sql \
  -f /tmp/storyos-release1-bootstrap/0023_export_human_readable_manuscript.sql \
  -f /tmp/storyos-release1-bootstrap/0024_export_project_archive_admission.sql \
  -f /tmp/storyos-release1-bootstrap/0025_export_project_archive_entries.sql \
  -f /tmp/storyos-release1-bootstrap/0026_recovery_visibility_proof.sql \
  -f /tmp/storyos-release1-bootstrap/0027_author_command_admission_reconfirmations.sql \
  -f /tmp/storyos-release1-bootstrap/0028_tree_revision_conflict_names.sql \
  -f /tmp/storyos-release1-bootstrap/0029_human_readable_manuscript_export_operations.sql \
  -f /tmp/storyos-release1-bootstrap/0030_human_readable_export_worker_claim.sql \
  -f /tmp/storyos-release1-bootstrap/0031_project_export_operations.sql \
  -f /tmp/storyos-release1-bootstrap/0032_create_volume_canonical_sibling_order.sql >/dev/null

runtime_secret_state=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT CASE WHEN rolpassword IS NULL THEN 'absent' ELSE 'present' END
     FROM pg_authid WHERE rolname = 'storyos_runtime'")
if [ "$runtime_secret_state" != "absent" ]; then
  echo "The tracked Release 1 bootstrap installed a runtime password" >&2
  exit 1
fi
docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" >/dev/null

docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null

published=$(docker port "$container" 5432/tcp)
port=${published##*:}
export STORYOS_TEST_DATABASE_URL="postgres://storyos_runtime:runtime@127.0.0.1:$port/postgres"
export STORYOS_TEST_ADMIN_DATABASE_URL="postgres://postgres:admin@127.0.0.1:$port/postgres"
export STORYOS_TEST_POSTGRES_CONTAINER="$container"
echo "Running PostgreSQL Application and RLS tests"
cargo test -p storyos-adapter-postgres --test project_scope -- --ignored --nocapture
cargo test -p storyos-adapter-postgres --test project_command_challenge -- --ignored --nocapture
cargo test -p storyos-adapter-postgres --lib -- --ignored --nocapture
echo "Running HTTP Project Scope tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/project-http.integration.test.ts
echo "Running HTTP ApplyAuthorEdit process-cut tests"
pnpm --dir apps/web exec vitest run --project node-process-cut \
  test/node-process-cut/apply-author-edit-process-cut.integration.test.ts
echo "Running HTTP Activity Stream duplicate-resume tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/activity-stream-duplicate-http.integration.test.ts
echo "Running HTTP Activity Stream cross-table tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/activity-stream-cross-table-http.integration.test.ts
echo "Running HTTP Snapshot and Activity Stream tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/snapshot-replay-http.integration.test.ts
echo "Running HTTP createProjectChallenge tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/create-project-challenge-http.integration.test.ts
echo "Running HTTP createProject tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/create-project-http.integration.test.ts
echo "Running HTTP listProjects tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/list-projects-http.integration.test.ts
echo "Running HTTP updateProject tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/update-project-http.integration.test.ts
echo "Running HTTP archiveProject tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/archive-project-http.integration.test.ts
echo "Running HTTP createVolume tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/create-volume-http.integration.test.ts
echo "Running HTTP updateVolume tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/update-volume-http.integration.test.ts
echo "Running HTTP createChapter tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/create-chapter-http.integration.test.ts
echo "Running HTTP updateChapter tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/update-chapter-http.integration.test.ts
echo "Running HTTP setCurrentChapter tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/set-current-chapter-http.integration.test.ts
echo "Running HTTP deleteChapter tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/delete-chapter-http.integration.test.ts
echo "Running HTTP deleteVolume tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/delete-volume-http.integration.test.ts
echo "Running HTTP getManuscriptTree tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/manuscript-tree-http.integration.test.ts
echo "Running HTTP searchManuscript tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/manuscript-search-http.integration.test.ts
echo "Running HTTP takeOverProjectWriter tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/takeover-http.integration.test.ts
echo "Running HTTP fenced-writer late-result tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/takeover-late-result-http.integration.test.ts
echo "Running HTTP exportProjectArchive tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/project-export-admission-http.integration.test.ts
echo "Running HTTP exportHumanReadableManuscript admission tests"
pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/readable-export-admission-http.integration.test.ts
echo "Running HTTP human-readable export process-cut tests"
docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
  "DO \$\$ DECLARE tbl text; BEGIN
     FOR tbl IN SELECT tablename FROM pg_tables WHERE schemaname = 'storyos' LOOP
       EXECUTE format('TRUNCATE TABLE storyos.%I CASCADE', tbl);
     END LOOP;
   END \$\$;" >/dev/null
docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
pnpm --dir apps/web exec vitest run --project node-process-cut \
  test/node-process-cut/readable-export-admission-process-cut.integration.test.ts
echo "Running HTTP Project Export Archive process-cut tests"
docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
  "DO \$\$ DECLARE tbl text; BEGIN
     FOR tbl IN SELECT tablename FROM pg_tables WHERE schemaname = 'storyos' LOOP
       EXECUTE format('TRUNCATE TABLE storyos.%I CASCADE', tbl);
     END LOOP;
   END \$\$;" >/dev/null
docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
pnpm --dir apps/web exec vitest run --project node-process-cut \
  test/node-process-cut/project-export-admission-process-cut.integration.test.ts
echo "Restoring the controlled Project fixture for S1-JRN-001"
docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
  "DO \$\$ DECLARE tbl text; BEGIN
     FOR tbl IN SELECT tablename FROM pg_tables WHERE schemaname = 'storyos' LOOP
       EXECUTE format('TRUNCATE TABLE storyos.%I CASCADE', tbl);
     END LOOP;
   END \$\$;" >/dev/null
docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
echo "Running the exact-dist S1-JRN-001 and real production-host Chrome journeys"
s1_server_log=$(mktemp "${TMPDIR:-/tmp}/storyos-s1-server.XXXXXX")
stage1_user_id="018f0000-0000-7001-8000-000000000001"
STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
STORYOS_BOOTSTRAP_SESSIONS="{\"session-a\":\"$stage1_user_id\"}" \
STORYOS_CHALLENGE_SECRET="test-only-challenge-secret-that-is-at-least-thirty-two-bytes" \
  "$repository_root/target/release-package/storyos-server" --bind 127.0.0.1:0 \
  --web-root "$repository_root/target/release-package/web" \
  >"$s1_server_log" 2>&1 &
s1_server_pid=$!
attempt=0
while ! grep -q '^STORYOS_SERVER_URL=http://' "$s1_server_log"; do
  if ! kill -0 "$s1_server_pid" >/dev/null 2>&1; then
    cat "$s1_server_log" >&2
    echo "The StoryOS Server exited before the exact-dist journey" >&2
    exit 1
  fi
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ]; then
    cat "$s1_server_log" >&2
    echo "The StoryOS Server did not become ready for the exact-dist journey" >&2
    exit 1
  fi
  sleep 0.05
done
STORYOS_DEV_SERVER=$(sed -n 's/^STORYOS_SERVER_URL=//p' "$s1_server_log" | head -n 1)
export STORYOS_DEV_SERVER
export STORYOS_STAGE1_AUTHORITY_ORACLE=1
pnpm --dir apps/web exec vitest run --project browser-exact-dist
kill "$s1_server_pid" >/dev/null 2>&1 || true
wait "$s1_server_pid" >/dev/null 2>&1 || true
s1_server_pid=""
echo "Running isolated Recovery Copy restore and Recovery Visibility Proof"
"$repository_root/scripts/verify-recovery-hold.sh"
