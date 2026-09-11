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

start_postgres() {
  name=$1
  docker run --detach --name "$name" \
    --env POSTGRES_PASSWORD=admin \
    --publish 127.0.0.1::5432 postgres:16-alpine >/dev/null
  attempt=0
  until docker logs "$name" 2>&1 | grep -q "PostgreSQL init process complete" \
    && docker exec "$name" pg_isready -U postgres >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 40 ]; then
      echo "PostgreSQL did not become ready: $name" >&2
      exit 1
    fi
    sleep 0.25
  done
}

copy_catalogued_sql() {
  docker cp "$repository_root/crates/storyos-adapter-postgres/migrations/." \
    "$1:/tmp/storyos-release1-bootstrap" >/dev/null
}

apply_catalogued_sql() {
  python3 - "$1" "${2:-}" <<'PY'
import json, subprocess, sys
from pathlib import Path
container, mode = sys.argv[1], sys.argv[2]
catalog = json.loads(Path("docs/foundation/postgresql-release-1-persistence-catalog.json").read_text())
args = ["docker", "exec", container, "psql", "-X", "-v", "ON_ERROR_STOP=1",
        "--single-transaction", "-U", "postgres"]
for source in catalog["migration_chain"]["bootstrap"]["sources"]:
    name = source["path"].rsplit("/", 1)[-1]
    args.extend(["-f", f"/tmp/storyos-release1-bootstrap/{name}"])
    if mode == "fault" and name == "0002_project_command_challenges.sql":
        args.extend(["-c", "SELECT 1 / 0"])
raise SystemExit(subprocess.call(
    args,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL if mode == "fault" else None,
))
PY
}

postgres_admin_url() {
  published=$(docker port "$1" 5432/tcp)
  printf 'postgres://postgres:admin@127.0.0.1:%s/postgres\n' "${published##*:}"
}

postgres_runtime_url() {
  published=$(docker port "$1" 5432/tcp)
  printf 'postgres://storyos_runtime:runtime@127.0.0.1:%s/postgres\n' "${published##*:}"
}

set_runtime_password() {
  docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres \
    -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" >/dev/null
}

storage_bin="$repository_root/target/release-package/storyos-storage"
server_bin="$repository_root/target/release-package/storyos-server"
worker_bin="$repository_root/target/release-package/storyos-worker"
web_root="$repository_root/target/release-package/web"
if [ ! -x "$storage_bin" ] || [ ! -x "$server_bin" ] || [ ! -x "$worker_bin" ]; then
  echo "The release package does not contain storyos-storage, Server, and Worker" >&2
  exit 1
fi

gate_sessions="{\"session-a\":\"018f0000-0000-7001-8000-000000000001\"}"
gate_secret="test-only-challenge-secret-that-is-at-least-thirty-two-bytes"
closed_postgres_url="postgres://storyos_runtime:runtime@127.0.0.1:1/postgres"
canary_admin_url="postgres://postgres:wrong@127.0.0.1:1/postgres"

assert_offline_storage_activation_checks() {
  echo "Checking packaged offline Server and Worker checks access no PostgreSQL"
  STORYOS_DATABASE_URL="$closed_postgres_url" \
  STORYOS_STORAGE_ADMIN_URL="$closed_postgres_url" \
    "$server_bin" --check-web-root "$web_root"
  STORYOS_DATABASE_URL="$closed_postgres_url" \
  STORYOS_STORAGE_ADMIN_URL="$closed_postgres_url" \
    "$worker_bin" --check
}

assert_packaged_server_refuses_bind() {
  name=$1
  runtime_url=$2
  log=$(mktemp "${TMPDIR:-/tmp}/storyos-gate-server.XXXXXX")
  STORYOS_DATABASE_URL="$runtime_url" \
  STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
  STORYOS_BOOTSTRAP_SESSIONS="$gate_sessions" \
  STORYOS_CHALLENGE_SECRET="$gate_secret" \
  STORYOS_WORKER=1 \
    "$server_bin" --bind 127.0.0.1:0 --web-root "$web_root" >"$log" 2>&1 &
  pid=$!
  attempt=0
  while kill -0 "$pid" >/dev/null 2>&1; do
    if grep -q '^STORYOS_SERVER_URL=' "$log"; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
      cat "$log" >&2
      echo "Packaged Server bound without a matching Active proof: $name" >&2
      rm -f "$log"
      exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 40 ]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
      cat "$log" >&2
      echo "Packaged Server stayed up without refusing Activation: $name" >&2
      rm -f "$log"
      exit 1
    fi
    sleep 0.05
  done
  wait "$pid" || true
  if grep -q '^STORYOS_SERVER_URL=' "$log"; then
    cat "$log" >&2
    echo "Packaged Server printed a ready URL without a matching Active proof: $name" >&2
    rm -f "$log"
    exit 1
  fi
  rm -f "$log"
}

assert_packaged_server_binds() {
  name=$1
  runtime_url=$2
  log=$(mktemp "${TMPDIR:-/tmp}/storyos-gate-bind.XXXXXX")
  STORYOS_DATABASE_URL="$runtime_url" \
  STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
  STORYOS_BOOTSTRAP_SESSIONS="$gate_sessions" \
  STORYOS_CHALLENGE_SECRET="$gate_secret" \
  STORYOS_WORKER=0 \
    "$server_bin" --bind 127.0.0.1:0 --web-root "$web_root" >"$log" 2>&1 &
  pid=$!
  attempt=0
  while ! grep -q '^STORYOS_SERVER_URL=http://' "$log"; do
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      cat "$log" >&2
      echo "Packaged Server did not bind with an Active proof: $name" >&2
      rm -f "$log"
      exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
      cat "$log" >&2
      echo "Packaged Server did not become ready with an Active proof: $name" >&2
      rm -f "$log"
      exit 1
    fi
    sleep 0.05
  done
  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" >/dev/null 2>&1 || true
  rm -f "$log"
}

assert_packaged_worker_refuses_claim() {
  name=$1
  runtime_url=$2
  if STORYOS_DATABASE_URL="$runtime_url" \
     STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
     "$worker_bin" --claim-only; then
    echo "Packaged Worker claimed without a matching Active proof: $name" >&2
    exit 1
  fi
}

prove_bound_request_path_activation() {
  echo "Checking bound Server request-path Activation refusals"
  log=$(mktemp "${TMPDIR:-/tmp}/storyos-gate-http.XXXXXX")
  STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
  STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
  STORYOS_BOOTSTRAP_SESSIONS="$gate_sessions" \
  STORYOS_CHALLENGE_SECRET="$gate_secret" \
  STORYOS_WORKER=0 \
    "$server_bin" --bind 127.0.0.1:0 --web-root "$web_root" >"$log" 2>&1 &
  pid=$!
  attempt=0
  while ! grep -q '^STORYOS_SERVER_URL=http://' "$log"; do
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      cat "$log" >&2
      echo "Packaged Server exited before request-path Activation checks" >&2
      rm -f "$log"
      exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
      cat "$log" >&2
      echo "Packaged Server did not become ready for request-path Activation checks" >&2
      rm -f "$log"
      exit 1
    fi
    sleep 0.05
  done
  base=$(sed -n 's/^STORYOS_SERVER_URL=//p' "$log" | head -n 1)
  headers=$(mktemp "${TMPDIR:-/tmp}/storyos-gate-headers.XXXXXX")
  body=$(mktemp "${TMPDIR:-/tmp}/storyos-gate-body.XXXXXX")
  fetch_project() {
    curl -sS -D "$headers" -o "$body" \
      -H "origin: $base" \
      -H "cookie: storyos_session=session-a" \
      "$base/api/v1/projects/018f0000-0000-7001-8000-000000000002"
  }
  fetch_project
  status=$(awk 'NR==1 { print $2 }' "$headers")
  if [ "$status" != "200" ]; then
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
    echo "Active proof did not admit a Project read: $status" >&2
    cat "$body" >&2
    rm -f "$log" "$headers" "$body"
    exit 1
  fi
  restore_sql=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT format(
       'INSERT INTO storyos.storage_activation_proofs (
          proof_id, phase, catalog_id, catalog_checksum, migration_chain_id,
          migration_chain_digest, database_schema_identity, active_schema_version,
          public_release, route_catalog_id, route_catalog_sha256, activated_at
        ) VALUES (%L, %L, %L, %L, %L, %L, %L, %L, %L, %L, %L, %L)',
       proof_id, phase, catalog_id, catalog_checksum, migration_chain_id,
       migration_chain_digest, database_schema_identity, active_schema_version,
       public_release, route_catalog_id, route_catalog_sha256, activated_at
     ) FROM storyos.storage_activation_proofs WHERE proof_id = 'release-1'")
  original_catalog=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT catalog_id FROM storyos.storage_activation_proofs WHERE proof_id = 'release-1'")
  docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
    "DELETE FROM storyos.storage_activation_proofs WHERE proof_id = 'release-1'" >/dev/null
  fetch_project
  status=$(awk 'NR==1 { print $2 }' "$headers")
  if [ "$status" != "503" ] || ! grep -q '"code":"project_store_unavailable"' "$body"; then
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
    echo "A vanished proof did not use project_store_unavailable: $status" >&2
    cat "$body" >&2
    docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c "$restore_sql" >/dev/null
    rm -f "$log" "$headers" "$body"
    exit 1
  fi
  docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c "$restore_sql" >/dev/null
  docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
    "UPDATE storyos.storage_activation_proofs
        SET catalog_id = 'wrong.catalog'
      WHERE proof_id = 'release-1'" >/dev/null
  fetch_project
  status=$(awk 'NR==1 { print $2 }' "$headers")
  docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
    "UPDATE storyos.storage_activation_proofs
        SET catalog_id = '$original_catalog'
      WHERE proof_id = 'release-1'" >/dev/null
  if [ "$status" != "409" ] || ! grep -q '"code":"upgrade_required"' "$body"; then
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
    echo "An identity mismatch did not return upgrade_required: $status" >&2
    cat "$body" >&2
    rm -f "$log" "$headers" "$body"
    exit 1
  fi
  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" >/dev/null 2>&1 || true
  rm -f "$log" "$headers" "$body"
}

reload_controlled_fixture() {
  docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
    "DO \$\$ DECLARE tbl text; BEGIN
       FOR tbl IN SELECT tablename FROM pg_tables
         WHERE schemaname = 'storyos'
           AND tablename NOT IN (
             'storage_activation_proofs',
             'schema_migrations',
             'migration_phases',
             'migration_phase_checksums'
           )
       LOOP
         EXECUTE format('TRUNCATE TABLE storyos.%I CASCADE', tbl);
       END LOOP;
     END \$\$;" >/dev/null
  docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
    < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
}

# The node-postgresql files share one database and one fixture, so ScriptOrderSequencer
# keeps the given order instead of the Vitest default order.
run_http_files() {
  STORYOS_VITEST_FILE_ORDER=$(printf '%s:' "$@") \
    pnpm --dir apps/web exec vitest run --project node-postgresql "$@"
}

container="storyos-issue105-$$"
oracle_container="storyos-storage-oracle-$$"
activation_container="storyos-storage-activation-$$"
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
  docker rm -f "$container" "$oracle_container" "$activation_container" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
cleanup

echo "Running catalogued SQL apply and faulted rollback without Server or Worker"
start_postgres "$oracle_container"
copy_catalogued_sql "$oracle_container"
if apply_catalogued_sql "$oracle_container" fault; then
  echo "The faulted Release 1 bootstrap unexpectedly committed" >&2
  exit 1
fi
rollback_state=$(docker exec "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT (SELECT count(*) FROM pg_roles
            WHERE rolname IN ('storyos_owner', 'storyos_runtime'))::text
          || '/' || COALESCE(to_regnamespace('storyos')::text, 'absent')")
if [ "$rollback_state" != "0/absent" ]; then
  echo "The faulted Release 1 bootstrap exposed partial state: $rollback_state" >&2
  exit 1
fi
apply_catalogued_sql "$oracle_container"
oracle_secret=$(docker exec "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT CASE WHEN rolpassword IS NULL THEN 'absent' ELSE 'present' END
     FROM pg_authid WHERE rolname = 'storyos_runtime'")
if [ "$oracle_secret" != "absent" ]; then
  echo "The tracked Release 1 bootstrap installed a runtime password" >&2
  exit 1
fi
oracle_active=$(docker exec "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT count(*)::text FROM storyos.storage_activation_proofs")
if [ "$oracle_active" != "0" ]; then
  echo "The SQL apply oracle wrote Activation rows" >&2
  exit 1
fi
set_runtime_password "$oracle_container"
oracle_runtime=$(postgres_runtime_url "$oracle_container")
echo "Refusing Server bind and Worker claim without an Active proof"
assert_packaged_server_refuses_bind "sql-apply-without-active" "$oracle_runtime"
assert_packaged_worker_refuses_claim "sql-apply-without-active" "$oracle_runtime"
docker exec -i "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
oracle_admin=$(postgres_admin_url "$oracle_container")
if STORYOS_STORAGE_ADMIN_URL="$oracle_admin" "$storage_bin"; then
  echo "storyos-storage adopted a non-empty database without a ledger" >&2
  exit 1
fi
oracle_title=$(docker exec "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT title FROM storyos.projects
    WHERE project_id = '018f0000-0000-7001-8000-000000000002'")
if [ "$oracle_title" != "Project A" ]; then
  echo "A refused Preflight changed domain rows: $oracle_title" >&2
  exit 1
fi
docker exec "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres -c \
  "INSERT INTO storyos.storage_activation_proofs (
     proof_id, phase, catalog_id, catalog_checksum, migration_chain_id,
     migration_chain_digest, database_schema_identity, active_schema_version,
     public_release, route_catalog_id, route_catalog_sha256, activated_at
   ) VALUES (
     'release-1', 'active', 'wrong.catalog',
     'sha256:0000000000000000000000000000000000000000000000000000000000000000',
     'wrong.chain',
     'sha256:1111111111111111111111111111111111111111111111111111111111111111',
     'wrong.schema', 'wrong.version', 'wrong.release', 'wrong.route',
     'sha256:2222222222222222222222222222222222222222222222222222222222222222',
     clock_timestamp()
   )" >/dev/null
if STORYOS_STORAGE_ADMIN_URL="$oracle_admin" "$storage_bin"; then
  echo "storyos-storage Activated over a mismatched identity" >&2
  exit 1
fi
mismatch_state=$(docker exec "$oracle_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT catalog_id || '/' || title
     FROM storyos.storage_activation_proofs, storyos.projects
    WHERE proof_id = 'release-1'
      AND project_id = '018f0000-0000-7001-8000-000000000002'")
if [ "$mismatch_state" != "wrong.catalog/Project A" ]; then
  echo "An identity mismatch changed stored proof or domain rows: $mismatch_state" >&2
  exit 1
fi
echo "Refusing Server bind and Worker claim on an identity mismatch"
assert_packaged_server_refuses_bind "identity-mismatch" "$oracle_runtime"
assert_packaged_worker_refuses_claim "identity-mismatch" "$oracle_runtime"

echo "Running packaged storyos-storage against a fresh empty database"
start_postgres "$activation_container"
activation_admin=$(postgres_admin_url "$activation_container")
if STORYOS_DATABASE_URL="$activation_admin" "$storage_bin"; then
  echo "storyos-storage reused STORYOS_DATABASE_URL" >&2
  exit 1
fi
STORYOS_DATABASE_URL="postgres://storyos_runtime:wrong@127.0.0.1:1/postgres" \
STORYOS_STORAGE_ADMIN_URL="$activation_admin" \
  "$storage_bin"
STORYOS_STORAGE_ADMIN_URL="$activation_admin" \
  "$storage_bin"
expected_sources=$(python3 -c "
import json
from pathlib import Path
catalog = json.loads(Path('docs/foundation/postgresql-release-1-persistence-catalog.json').read_text())
print(len(catalog['migration_chain']['bootstrap']['sources']))
")
activation_state=$(docker exec "$activation_container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT proof.phase || '/' ||
          CASE WHEN owner.rolcanlogin THEN 'login' ELSE 'nologin' END || '/' ||
          CASE WHEN runtime.rolpassword IS NULL THEN 'absent' ELSE 'present' END || '/' ||
          (SELECT count(*) FROM storyos.schema_migrations)::text || '/' ||
          (SELECT count(*) FROM storyos.migration_phases)::text || '/' ||
          (SELECT count(*) FROM storyos.migration_phase_checksums)::text
     FROM storyos.storage_activation_proofs AS proof,
          pg_roles AS owner,
          pg_authid AS runtime
    WHERE proof.proof_id = 'release-1'
      AND owner.rolname = 'storyos_owner'
      AND runtime.rolname = 'storyos_runtime'")
if [ "$activation_state" != "active/nologin/absent/1/4/$expected_sources" ]; then
  echo "storyos-storage did not persist the Active proof: $activation_state" >&2
  exit 1
fi
docker exec "$activation_container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" >/dev/null
runtime_select=$(docker exec "$activation_container" psql -X -v ON_ERROR_STOP=1 \
  -U storyos_runtime -d postgres -Atc \
  "SELECT phase FROM storyos.storage_activation_proofs WHERE proof_id = 'release-1'")
if [ "$runtime_select" != "active" ]; then
  echo "storyos_runtime cannot SELECT the activation proof" >&2
  exit 1
fi
if docker exec "$activation_container" psql -X -v ON_ERROR_STOP=1 \
  -U storyos_runtime -d postgres -c \
  "INSERT INTO storyos.schema_migrations (
     schema_version, migration_id, checksum, release_identity, runner_revision,
     started_at, status
   ) VALUES (
     'forged', 'forged', 'sha256:3333333333333333333333333333333333333333333333333333333333333333',
     'forged', 'forged', clock_timestamp(), 'applied'
   )" >/dev/null 2>&1; then
  echo "storyos_runtime ran the storage state machine" >&2
  exit 1
fi
activation_runtime=$(postgres_runtime_url "$activation_container")
assert_offline_storage_activation_checks
echo "Binding packaged Server and Worker only from the Active proof"
assert_packaged_server_binds "active-proof" "$activation_runtime"
STORYOS_DATABASE_URL="$activation_runtime" \
STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
  "$worker_bin" --claim-only

echo "Preparing the Server-facing verify database"
start_postgres "$container"
container_admin=$(postgres_admin_url "$container")
if STORYOS_DATABASE_URL="$container_admin" "$storage_bin"; then
  echo "storyos-storage reused STORYOS_DATABASE_URL for the Server-facing database" >&2
  exit 1
fi
STORYOS_STORAGE_ADMIN_URL="$container_admin" \
  "$storage_bin"
runtime_secret_state=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT CASE WHEN rolpassword IS NULL THEN 'absent' ELSE 'present' END
     FROM pg_authid WHERE rolname = 'storyos_runtime'")
if [ "$runtime_secret_state" != "absent" ]; then
  echo "The tracked Release 1 bootstrap installed a runtime password" >&2
  exit 1
fi
set_runtime_password "$container"
server_facing_active=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT phase FROM storyos.storage_activation_proofs WHERE proof_id = 'release-1'")
if [ "$server_facing_active" != "active" ]; then
  echo "The Server-facing verify database was not Activated by storyos-storage" >&2
  exit 1
fi
docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null

published=$(docker port "$container" 5432/tcp)
port=${published##*:}
export STORYOS_TEST_DATABASE_URL="postgres://storyos_runtime:runtime@127.0.0.1:$port/postgres"
export STORYOS_TEST_ADMIN_DATABASE_URL="postgres://postgres:admin@127.0.0.1:$port/postgres"
export STORYOS_TEST_POSTGRES_CONTAINER="$container"
prove_bound_request_path_activation
echo "Running PostgreSQL Application and RLS tests"
# `make contracts` builds these targets with `--workspace --all-features`. The same
# selection reuses those artifacts. A package-only selection unifies dependency features
# differently and recompiles storyos-adapter-postgres for each target. The `--lib` step
# runs every ignored lib test in the workspace; today only storyos-adapter-postgres has them.
cargo test --workspace --all-features --test project_scope -- --ignored --nocapture
cargo test --workspace --all-features --test project_command_challenge -- --ignored --nocapture
cargo test --workspace --all-features --lib -- --ignored --nocapture
echo "Running HTTP protocol host and Project Scope tests"
run_http_files \
  test/node-postgresql/protocol-http-host.integration.test.ts \
  test/node-postgresql/project-http.integration.test.ts
echo "Running HTTP ApplyAuthorEdit process-cut tests"
pnpm --dir apps/web exec vitest run --project node-process-cut \
  test/node-process-cut/apply-author-edit-process-cut.integration.test.ts
echo "Running HTTP Activity Stream, Project, structure, query, and export tests"
run_http_files \
  test/node-postgresql/activity-stream-duplicate-http.integration.test.ts \
  test/node-postgresql/activity-stream-cross-table-http.integration.test.ts \
  test/node-postgresql/snapshot-replay-http.integration.test.ts \
  test/node-postgresql/create-project-challenge-http.integration.test.ts \
  test/node-postgresql/create-project-http.integration.test.ts \
  test/node-postgresql/list-projects-http.integration.test.ts \
  test/node-postgresql/update-project-http.integration.test.ts \
  test/node-postgresql/archive-project-http.integration.test.ts \
  test/node-postgresql/create-volume-http.integration.test.ts \
  test/node-postgresql/update-volume-http.integration.test.ts \
  test/node-postgresql/create-chapter-http.integration.test.ts \
  test/node-postgresql/update-chapter-http.integration.test.ts \
  test/node-postgresql/set-current-chapter-http.integration.test.ts \
  test/node-postgresql/delete-chapter-http.integration.test.ts \
  test/node-postgresql/delete-volume-http.integration.test.ts \
  test/node-postgresql/manuscript-tree-http.integration.test.ts \
  test/node-postgresql/manuscript-search-http.integration.test.ts \
  test/node-postgresql/takeover-http.integration.test.ts \
  test/node-postgresql/takeover-late-result-http.integration.test.ts \
  test/node-postgresql/project-export-admission-http.integration.test.ts \
  test/node-postgresql/project-export-pinned-source-http.integration.test.ts \
  test/node-postgresql/readable-export-admission-http.integration.test.ts \
  test/node-postgresql/readable-export-pinned-source-http.integration.test.ts
echo "Running HTTP human-readable export process-cut tests"
reload_controlled_fixture
pnpm --dir apps/web exec vitest run --project node-process-cut \
  test/node-process-cut/readable-export-admission-process-cut.integration.test.ts
echo "Running HTTP Project Export Archive process-cut tests"
reload_controlled_fixture
pnpm --dir apps/web exec vitest run --project node-process-cut \
  test/node-process-cut/project-export-admission-process-cut.integration.test.ts
echo "Restoring the controlled Project fixture for S1-JRN-001"
reload_controlled_fixture
echo "Running the exact-dist S1-JRN-001 and real production-host Chrome journeys"
s1_server_log=$(mktemp "${TMPDIR:-/tmp}/storyos-s1-server.XXXXXX")
stage1_user_id="018f0000-0000-7001-8000-000000000001"
STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
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
STORYOS_RECOVERY_DRILL=fixture-only "$repository_root/scripts/verify-recovery-hold.sh"
echo "Running mixed empty and populated isolated Recovery Copy restore"
STORYOS_RECOVERY_DRILL=mixed "$repository_root/scripts/verify-recovery-hold.sh"
