#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
if [ -z "${STORYOS_VERIFICATION_RUN:-}" ]; then
  exec python3 scripts/verification.py step project-scope -- sh "$repository_root/scripts/verify-project-scope.sh" "$@"
fi

timed_stage() {
  PYTHONDONTWRITEBYTECODE=1 python3 "$repository_root/scripts/verification.py" step "$@"
}

# Accept only ripgrep status 0 (match) and 1 (no match); other statuses stop the script.
search_source() {
  rg_status=0
  rg_output=$(rg "$@") || rg_status=$?
  if [ "$rg_status" -gt 1 ]; then
    printf 'ripgrep failed with exit status %s: rg %s\n' "$rg_status" "$*" >&2
    exit 1
  fi
}

guard_source() {
  guard_message=$1
  shift
  search_source "$@"
  if [ "$rg_status" -eq 0 ]; then
    echo "$guard_message" >&2
    printf '%s\n' "$rg_output" >&2
    exit 1
  fi
}

verify_web_migration_guards() {
  if ! command -v rg >/dev/null 2>&1; then
    echo "ripgrep (rg) is required for the Web source guards" >&2
    exit 1
  fi

  legacy_web_files=$(find apps/web \
    \( -path apps/web/dist -o -path apps/web/node_modules \) -prune -o \
    -type f \( -name '*.js' -o -name '*.jsx' -o -name '*.mjs' -o -name '*.cjs' \) -print)
  if [ -n "$legacy_web_files" ]; then
    echo "Hand-written Web JavaScript remains:" >&2
    printf '%s\n' "$legacy_web_files" >&2
    exit 1
  fi

  guard_source "An active raw browser harness signature remains:" -n \
    'DevTools listening|webSocketDebuggerUrl|Runtime\.evaluate|remote-debugging-(port|pipe)|new WebSocket\(' \
    apps/web --glob '!dist/**' --glob '!node_modules/**'

  guard_source "A CDP primitive escaped the typed Browser Command boundary:" -n \
    'newCDPSession|CDPSession|session\.send\(' \
    apps/web --glob '!dist/**' --glob '!node_modules/**' \
      --glob '!**/test/support/browser-commands.ts'

  search_source -n 'session\.send\(' apps/web/test/support/browser-commands.ts
  if [ "$rg_status" -eq 0 ]; then
    printf '%s\n' "$rg_output" \
      | guard_source "The IME Browser Command uses an unsupported CDP method:" \
        -v 'Input\.imeSetComposition'
  fi

  guard_source "An active legacy browser harness entry remains:" -n \
    'production-page-browser\.integration\.test\.mjs|s1-jrn-001-browser\.integration\.test\.mjs|author-edit-batch-browser-process\.test\.mjs|author-edit-batch-prerelease-browser-harness\.mjs' \
    Makefile package.json apps/web/package.json scripts .github

  guard_source "A browser skip or fallback remains:" -n \
    '\.(skip|skipIf|runIf|todo)\b|\bskip\s*:|Chrome or Chromium is unavailable|CHROME_BIN|chromium-browser|/usr/bin/chromium' \
    apps/web/test/browser-source apps/web/test/browser-exact-dist \
      apps/web/test/support/browser-command-client.ts \
      apps/web/test/support/browser-command-contract.ts \
      apps/web/test/support/production-host-command.ts \
      apps/web/test/support/browser-commands.ts apps/web/vitest.config.ts \
      --glob '*.ts' --glob '*.tsx' \
      --glob '*.js' --glob '*.jsx' --glob '*.mjs' --glob '*.cjs'

  guard_source "A single-User exact-dist journey still injects a test storyos_session cookie:" -n \
    'addCookies\(|updateClientSessionCookie\(\{ action: "set"' \
    apps/web/test/browser-exact-dist \
    apps/web/test/support/production-host-command.ts

  guard_source "A prohibited TypeScript escape remains:" -n \
    '\bany\b|@ts-(ignore|nocheck)|declare module|\bas unknown as\b|\bas [A-Za-z0-9_.$<>\[\] |]+ as\b' \
    apps/web --glob '*.ts' --glob '*.tsx' --glob '!dist/**' --glob '!node_modules/**'
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
python3 scripts/verification_shared.py plan >/dev/null
if [ "${1:-}" = "--check-inputs" ]; then
  exit 0
fi
record_google_chrome_version
if [ "${STORYOS_WEB_TYPECHECKED:-}" != "1" ]; then
  make release-package
fi

. "$repository_root/scripts/lib/exact-dist.sh"

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

require_release_package

gate_sessions="{\"session-a\":\"018f0000-0000-7001-8000-000000000001\"}"
gate_secret="test-only-challenge-secret-that-is-at-least-thirty-two-bytes"
closed_postgres_url="postgres://storyos_runtime:runtime@127.0.0.1:1/postgres"

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

container="storyos-issue105-$$"
oracle_container="storyos-storage-oracle-$$"
activation_container="storyos-storage-activation-$$"
export CARGO_NET_OFFLINE=true
cleanup() {
  stop_exact_dist_server
  docker rm -fv "$container" "$oracle_container" "$activation_container" >/dev/null 2>&1 || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
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
load_controlled_fixture "$oracle_container"
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

prepare_server_database "$container"
prove_bound_request_path_activation
echo "Running PostgreSQL Application and RLS tests"
# `make contracts` builds these targets with `--workspace --all-features`. The same
# selection reuses those artifacts. A package-only selection unifies dependency features
# differently and recompiles storyos-adapter-postgres for each target. The `--lib` step
# runs every ignored lib test in the workspace; today only storyos-adapter-postgres has them.
timed_stage postgres-scope -- cargo test --workspace --all-features --test project_scope -- --ignored --nocapture
timed_stage postgres-challenge -- cargo test --workspace --all-features --test project_command_challenge -- --ignored --nocapture
timed_stage postgres-library -- cargo test --workspace --all-features --lib -- --ignored --nocapture
python3 scripts/verification_shared.py run
echo "Restoring the controlled Project fixture for S1-JRN-001"
reset_exact_dist_fixture "$container"
echo "Running the exact-dist S1-JRN-001 and real production-host Chrome journeys"
start_exact_dist_server
# The authority oracle compares the receipts of the complete exact-dist suite.
export STORYOS_STAGE1_AUTHORITY_ORACLE=1
timed_stage exact-dist -- pnpm --dir apps/web exec vitest run --project browser-exact-dist
stop_exact_dist_server
echo "Running isolated Recovery Copy restore and Recovery Visibility Proof"
STORYOS_RECOVERY_DRILL=fixture-only timed_stage recovery-fixture -- "$repository_root/scripts/verify-recovery-hold.sh"
echo "Running mixed empty and populated isolated Recovery Copy restore"
STORYOS_RECOVERY_DRILL=mixed timed_stage recovery-mixed -- "$repository_root/scripts/verify-recovery-hold.sh"
