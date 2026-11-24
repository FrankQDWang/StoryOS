#!/bin/sh
set -eu

# The parts of make check-full after the steps of make check. scripts/check-lanes.py --project-scope runs each
# part in a lane after the release package:
#   scripts/verify-project-scope.sh activation      The storage activation checks.
#   scripts/verify-project-scope.sh provider        The storage activation checks on the image of the hosted provider.
#   scripts/verify-project-scope.sh journeys FILE...  The exact-dist journeys FILE, paths relative to apps/web.
# The two modes of the recovery drill are scripts/verify-recovery-hold.sh.

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

# make check-fast runs the Web source guards of scripts/verify-web-source-guards.sh, so this script does not run them.
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

. "$repository_root/scripts/lib/exact-dist.sh"

copy_migrations_sql() {
  docker cp "$repository_root/crates/storyos-adapter-postgres/migrations/." \
    "$1:/tmp/storyos-release1-bootstrap" >/dev/null
}

apply_migrations_sql() {
  python3 - "$1" "${2:-}" <<'PY'
import subprocess, sys
from pathlib import Path
container, mode = sys.argv[1], sys.argv[2]
args = ["docker", "exec", container, "psql", "-X", "-v", "ON_ERROR_STOP=1",
        "--single-transaction", "-U", "postgres"]
for source in sorted(Path("crates/storyos-adapter-postgres/migrations").glob("*.sql")):
    name = source.name
    if mode == "fault" and name == "0001_release_1_baseline.sql":
        # The fault follows the first table of the baseline, so it runs inside the bootstrap transaction.
        sql = source.read_text()
        cut = sql.index("\n);\n", sql.index("CREATE TABLE")) + len("\n);\n")
        name = "faulted_baseline.sql"
        subprocess.run(
            ["docker", "exec", "-i", container, "sh", "-c", f"cat > /tmp/storyos-release1-bootstrap/{name}"],
            input=(sql[:cut] + "SELECT 1 / 0;\n" + sql[cut:]).encode(),
            check=True,
        )
    args.extend(["-f", f"/tmp/storyos-release1-bootstrap/{name}"])
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
  STORYOS_MODEL_DESTINATION=host_fake \
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
  STORYOS_MODEL_DESTINATION=host_fake \
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
  STORYOS_MODEL_DESTINATION=host_fake \
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

# The activation checks run ignored tests of the library test binary of the workspace build. A cargo test of one
# package would build a second binary, because the features of one package are not the features of the workspace.
find_postgres_test_binary() {
  cargo test --locked --workspace --all-targets --all-features --no-run --message-format=json | python3 -c '
import json, sys
paths = {message["executable"] for message in map(json.loads, sys.stdin)
         if message.get("reason") == "compiler-artifact" and message["target"]["name"] == "storyos_adapter_postgres"
         and message["target"]["kind"] == ["lib"] and message["profile"]["test"] and message.get("executable")}
if len(paths) != 1:
    raise SystemExit(f"The workspace build has not one library test binary of storyos-adapter-postgres: {paths}")
print(paths.pop())'
}

run_postgres_tests() {
  (cd "$repository_root/crates/storyos-adapter-postgres" && "$postgres_test_binary" --ignored --exact "$@")
}

run_activation_checks() {
  postgres_test_binary=$(find_postgres_test_binary)
  echo "Running migrations SQL apply and faulted rollback without Server or Worker"
  start_postgres "$oracle_container"
  copy_migrations_sql "$oracle_container"
  if apply_migrations_sql "$oracle_container" fault; then
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
  apply_migrations_sql "$oracle_container"
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
  expected_sources=$(find crates/storyos-adapter-postgres/migrations -maxdepth 1 -name '*.sql' | wc -l | tr -d ' ')
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
       attempt_id, schema_version, migration_id, checksum, release_identity, runner_revision,
       started_at, status
     ) VALUES (
       2, 'forged', 'forged', 'sha256:3333333333333333333333333333333333333333333333333333333333333333',
       'forged', 'forged', clock_timestamp(), 'applied'
     )" >/dev/null 2>&1; then
    echo "storyos_runtime ran the storage state machine" >&2
    exit 1
  fi
  # The vector extension is optional (ADR 0095 section 4): a database that does not offer it
  # activates with the same storage identity and has no vector table. This is the one use of the
  # image without pgvector.
  echo "Running packaged storyos-storage against a fresh database without the vector extension"
  start_postgres "$legacy_activation_container" postgres:17-alpine
  legacy_admin=$(postgres_admin_url "$legacy_activation_container")
  STORYOS_STORAGE_ADMIN_URL="$legacy_admin" "$storage_bin"
  activation_facts() {
    docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
      "SELECT proof.phase || '/' || proof.migration_chain_digest || '/' ||
              proof.database_schema_identity || '/' ||
              COALESCE(to_regclass('storyos.passage_vectors')::text, 'absent') || '/' ||
              COALESCE((SELECT extnamespace::regnamespace::text || ':' || extversion
                          FROM pg_extension WHERE extname = 'vector'), 'none')
         FROM storyos.storage_activation_proofs AS proof
        WHERE proof.proof_id = 'release-1'"
  }
  vector_facts=$(activation_facts "$activation_container")
  legacy_facts=$(activation_facts "$legacy_activation_container")
  case "$vector_facts" in
    */storyos.passage_vectors/extensions:0.8.7) ;;
    *)
      echo "The pgvector database has no vector table: $vector_facts" >&2
      exit 1
      ;;
  esac
  if [ "$legacy_facts" != "${vector_facts%/storyos.passage_vectors/extensions:0.8.7}/absent/none" ]; then
    echo "The database without pgvector did not activate the same identity: $legacy_facts" >&2
    exit 1
  fi
  set_runtime_password "$legacy_activation_container"
  for vector_container in "$activation_container" "$legacy_activation_container"; do
    STORYOS_TEST_DATABASE_URL=$(postgres_runtime_url "$vector_container") \
    STORYOS_TEST_ADMIN_DATABASE_URL=$(postgres_admin_url "$vector_container") \
      run_postgres_tests \
        passage_vectors::tests::the_vector_store_is_available_exactly_when_the_database_offers_the_extension
  done
  # The ranked passages need no vector store: on the database without pgvector the result is the
  # exact-term list, and no request declares the hybrid search. The mark of the Worker path gives
  # no passage index work.
  load_controlled_fixture "$legacy_activation_container"
  STORYOS_TEST_DATABASE_URL=$(postgres_runtime_url "$legacy_activation_container") \
  STORYOS_TEST_ADMIN_DATABASE_URL=$(postgres_admin_url "$legacy_activation_container") \
    run_postgres_tests \
      passage_ranking::tests::the_result_with_no_vector_store_is_the_exact_term_list \
      passage_vectors::tests::the_passage_index_work_follows_the_vector_store_of_the_database \
      agent_run_hybrid_search::tests::the_tool_follows_the_vector_store_of_the_database
  docker rm -fv "$legacy_activation_container" >/dev/null
  activation_runtime=$(postgres_runtime_url "$activation_container")
  assert_offline_storage_activation_checks
  echo "Binding packaged Server and Worker only from the Active proof"
  assert_packaged_server_binds "active-proof" "$activation_runtime"
  STORYOS_DATABASE_URL="$activation_runtime" \
  STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
    "$worker_bin" --claim-only

  prepare_server_database "$container"
  prove_bound_request_path_activation
}

# The privileges of the StoryOS objects: one line for each grant on the schema storyos and on each relation, column,
# function, and type in it, with the default grants of PostgreSQL when the object has no ACL, and one line for each
# membership in a storyos_* role or of a storyos_* role, with its options.
storyos_privileges() {
  docker exec -i "$1" psql -X -v ON_ERROR_STOP=1 -U postgres -d postgres -At <<'SQL'
WITH object_acl AS (
  SELECT 'schema storyos' AS object, nspowner AS owner, COALESCE(nspacl, acldefault('n'::"char", nspowner)) AS acl
    FROM pg_namespace WHERE nspname = 'storyos'
  UNION ALL
  SELECT 'relation ' || oid::regclass::text, relowner,
         COALESCE(relacl, acldefault((CASE WHEN relkind = 'S' THEN 's' ELSE 'r' END)::"char", relowner))
    FROM pg_class WHERE relnamespace = 'storyos'::regnamespace AND relkind IN ('r', 'p', 'v', 'm', 'S', 'f')
  UNION ALL
  SELECT 'column ' || attribute.attrelid::regclass::text || '.' || quote_ident(attribute.attname), relation.relowner,
         attribute.attacl
    FROM pg_attribute AS attribute JOIN pg_class AS relation ON relation.oid = attribute.attrelid
   WHERE relation.relnamespace = 'storyos'::regnamespace AND attribute.attacl IS NOT NULL
  UNION ALL
  SELECT 'function ' || oid::regprocedure::text, proowner, COALESCE(proacl, acldefault('f'::"char", proowner))
    FROM pg_proc WHERE pronamespace = 'storyos'::regnamespace
  UNION ALL
  SELECT 'type ' || oid::regtype::text, typowner, COALESCE(typacl, acldefault('T'::"char", typowner))
    FROM pg_type WHERE typnamespace = 'storyos'::regnamespace
)
SELECT object || ' owner=' || pg_get_userbyid(owner) || ' grantee='
       || CASE WHEN grant_row.grantee = 0 THEN 'PUBLIC' ELSE pg_get_userbyid(grant_row.grantee) END
       || ' ' || grant_row.privilege_type || CASE WHEN grant_row.is_grantable THEN ' grantable' ELSE '' END
  FROM object_acl, LATERAL aclexplode(acl) AS grant_row
UNION
SELECT 'membership ' || pg_get_userbyid(roleid) || ' member=' || pg_get_userbyid(member)
       || CASE WHEN admin_option THEN ' admin' ELSE '' END || CASE WHEN inherit_option THEN ' inherit' ELSE '' END
       || CASE WHEN set_option THEN ' set' ELSE '' END
  FROM pg_auth_members
 WHERE pg_get_userbyid(roleid) LIKE 'storyos\_%' OR pg_get_userbyid(member) LIKE 'storyos\_%'
ORDER BY 1;
SQL
}

# The effective access to the StoryOS objects. The query examines each role that is not predefined (an OID from
# 16384), not a superuser, not a storyos_* role, and not the admin login. It also examines each role that such a
# role can become with SET ROLE. Each line gives the role, the role that it becomes ("as"), and one privilege on the
# schema storyos, a relation, a sequence, or a function of it. A function is callable only with USAGE on the schema,
# and the line says if the function runs with the rights of its owner (definer). The mark read_all_data shows the
# rights of the predefined role pg_read_all_data.
storyos_effective_access() {
  docker exec -i "$1" psql -X -v ON_ERROR_STOP=1 -U postgres -d postgres -At <<'SQL'
WITH other_role AS (
  SELECT oid, rolname FROM pg_roles
   WHERE oid >= 16384 AND NOT rolsuper AND rolname NOT LIKE 'storyos\_%' AND rolname <> current_user
), assumed AS (
  SELECT other_role.rolname, target.oid AS target, target.rolname AS target_name,
         pg_has_role(target.oid, 'pg_read_all_data', 'USAGE') AS read_all_data
    FROM other_role JOIN pg_roles AS target ON pg_has_role(other_role.oid, target.oid, 'SET')
), access AS (
  SELECT assumed.*, 'schema ' || kind.privilege AS privilege
    FROM assumed, (VALUES ('USAGE'), ('CREATE')) AS kind (privilege)
   WHERE has_schema_privilege(assumed.target, 'storyos', kind.privilege)
  UNION
  SELECT assumed.*, 'relation ' || kind.privilege
    FROM assumed, pg_class AS relation,
         (VALUES ('SELECT'), ('INSERT'), ('UPDATE'), ('DELETE'), ('TRUNCATE'), ('REFERENCES'), ('TRIGGER'),
                 ('MAINTAIN')) AS kind (privilege)
   WHERE relation.relnamespace = 'storyos'::regnamespace AND relation.relkind IN ('r', 'p', 'v', 'm', 'f')
     AND has_table_privilege(assumed.target, relation.oid, kind.privilege)
  UNION
  SELECT assumed.*, 'sequence ' || kind.privilege
    FROM assumed, pg_class AS sequence, (VALUES ('USAGE'), ('SELECT'), ('UPDATE')) AS kind (privilege)
   WHERE sequence.relnamespace = 'storyos'::regnamespace AND sequence.relkind = 'S'
     AND has_sequence_privilege(assumed.target, sequence.oid, kind.privilege)
  UNION
  SELECT assumed.*, 'function EXECUTE ' || CASE WHEN function.prosecdef THEN 'definer' ELSE 'invoker' END
    FROM assumed, pg_proc AS function
   WHERE function.pronamespace = 'storyos'::regnamespace
     AND has_schema_privilege(assumed.target, 'storyos', 'USAGE')
     AND has_function_privilege(assumed.target, function.oid, 'EXECUTE')
)
SELECT rolname || CASE WHEN target_name <> rolname THEN ' as ' || target_name ELSE '' END || ' ' || privilege
       || CASE WHEN read_all_data THEN ' read_all_data' ELSE '' END
  FROM access ORDER BY 1;
SQL
}

# Prints each line of the effective access $1 that is more than the read of pg_read_all_data (#1851, "Coordinator
# decisions"): USAGE on the schema, SELECT on a relation or a sequence, and a call of a function that runs with the
# rights of the caller. An empty access has no line.
access_beyond_read() {
  printf '%s\n' "$1" | grep -Ev \
    '^$|^[a-z_]+( as [a-z_]+)? (schema USAGE|relation SELECT|sequence SELECT|function EXECUTE invoker) read_all_data$' \
    || true
}

provider_psql() {
  docker exec -i "$provider_container" psql -X -v ON_ERROR_STOP=1 -U postgres -d postgres -At "$@"
}

# The superuser of the provider image makes the relations of the provider and of an application role.
provider_superuser_psql() {
  docker exec -i "$provider_container" psql -X -v ON_ERROR_STOP=1 -U supabase_admin -d postgres -At "$@"
}

# Runs storyos-storage with the admin login of the provider container. It sets storage_output and storage_status.
run_provider_storage() {
  if storage_output=$(STORYOS_STORAGE_ADMIN_URL=$(postgres_admin_url "$provider_container") "$storage_bin" 2>&1); then
    storage_status=0
  else
    storage_status=$?
  fi
}

# The Release 1 Storage Activation on a database of the hosted provider (ADR 0104 section 5). The admin login
# postgres of the provider image is not a superuser.
run_provider_activation_checks() {
  start_postgres "$provider_container" "$storyos_provider_postgres_image"
  if [ "$(provider_psql -c "SELECT rolsuper::text FROM pg_roles WHERE rolname = current_user")" != false ]; then
    echo "The admin login of the provider image is a superuser" >&2
    exit 1
  fi
  # The image differs from a new hosted project: there, storage and realtime have relations of the provider, the
  # admin login owns the views of pg_stat_statements, and each statement of the admin login ends after 120 s.
  provider_superuser_psql -c "CREATE TABLE storage.provider_objects (id integer);
    ALTER TABLE storage.provider_objects OWNER TO supabase_storage_admin;
    CREATE ROLE supabase_realtime_admin NOLOGIN;
    CREATE TABLE realtime.provider_messages (id bigserial, at date NOT NULL) PARTITION BY RANGE (at);
    CREATE TABLE realtime.provider_messages_now PARTITION OF realtime.provider_messages DEFAULT;
    ALTER TABLE realtime.provider_messages OWNER TO supabase_realtime_admin;
    ALTER TABLE realtime.provider_messages_now OWNER TO supabase_realtime_admin;
    ALTER VIEW extensions.pg_stat_statements OWNER TO postgres;
    ALTER VIEW extensions.pg_stat_statements_info OWNER TO postgres;
    ALTER ROLE postgres SET statement_timeout = '120s'" >/dev/null
  echo "Refusing a relation of the admin login or of an application role, also in a provider schema"
  provider_superuser_psql -c "CREATE ROLE application_owner NOLOGIN" >/dev/null
  for relation_owner in public.t:postgres extensions.t:postgres storage.t:application_owner; do
    relation=${relation_owner%%:*}
    provider_superuser_psql -c "CREATE TABLE $relation (id integer);
      ALTER TABLE $relation OWNER TO ${relation_owner#*:}" >/dev/null
    run_provider_storage
    if [ "$storage_status" -eq 0 ] || [ "$storage_output" != "STORYOS_STORAGE_ACTIVATION=Refused
STORYOS_STORAGE_ACTIVATION_REASON=non_empty_without_ledger" ]; then
      echo "storyos-storage did not refuse the relation $relation_owner: $storage_status $storage_output" >&2
      exit 1
    fi
    installed=$(provider_psql -c "SELECT COALESCE(to_regnamespace('storyos')::text, 'absent') || '/' ||
      (SELECT count(*) FROM pg_roles WHERE rolname LIKE 'storyos\_%')::text")
    if [ "$installed" != "absent/0" ]; then
      echo "A refused activation installed StoryOS state: $installed" >&2
      exit 1
    fi
    provider_superuser_psql -c "DROP TABLE $relation" >/dev/null
  done
  provider_superuser_psql -c "DROP ROLE application_owner" >/dev/null
  echo "Activating the provider database with its admin login"
  for run in first second; do
    run_provider_storage
    if [ "$storage_status" -ne 0 ] || [ "$storage_output" != "STORYOS_STORAGE_ACTIVATION=Active" ]; then
      echo "The $run activation of the provider database failed: $storage_status $storage_output" >&2
      exit 1
    fi
  done
  # The admin login made the vector extension and the vector table, and it can change the login of the runtime
  # role, as the SECURITY DEFINER function of the baseline does in a recovery.
  vector_state=$(provider_psql -c "SELECT COALESCE(to_regclass('storyos.passage_vectors')::text, 'absent') || '/' ||
    (SELECT extnamespace::regnamespace::text FROM pg_extension WHERE extname = 'vector')")
  if [ "$vector_state" != "storyos.passage_vectors/extensions" ]; then
    echo "The provider activation has no vector table in the schema extensions: $vector_state" >&2
    exit 1
  fi
  provider_psql -c "BEGIN; ALTER ROLE storyos_runtime NOLOGIN; ALTER ROLE storyos_runtime LOGIN; ROLLBACK" >/dev/null
  echo "Comparing the privileges of the StoryOS objects with a local activation"
  start_postgres "$activation_container"
  STORYOS_STORAGE_ADMIN_URL=$(postgres_admin_url "$activation_container") "$storage_bin" >/dev/null
  # PostgreSQL gives a creator of a role that is not a superuser the admin option on that role, and no rights of it.
  creator_admin='^membership storyos_(owner|runtime|backup|restore) member=postgres admin$'
  provider_privileges=$(storyos_privileges "$provider_container")
  if [ "$(printf '%s\n' "$provider_privileges" | grep -Ec "$creator_admin")" != 4 ]; then
    echo "The admin login of the provider database is not the creator of the four StoryOS roles" >&2
    exit 1
  fi
  provider_privileges=$(printf '%s\n' "$provider_privileges" | grep -Ev "$creator_admin")
  if ! printf '%s\n' "$provider_privileges" | grep -x 'membership storyos_owner member=postgres inherit set' >/dev/null; then
    echo "The privileges of the provider database have no membership of the admin login in storyos_owner" >&2
    exit 1
  fi
  if [ "$provider_privileges" != "$(storyos_privileges "$activation_container")" ]; then
    echo "The privileges of the StoryOS objects on the provider database are not those of a local database" >&2
    exit 1
  fi
  # Each grantee is the owner of the object or a storyos_* role. PUBLIC has only the defaults of PostgreSQL for a
  # function and a type, and no right on the schema storyos, so it cannot use them.
  if printf '%s\n' "$provider_privileges" | grep -Ev \
    '^(schema|relation|column|function|type) .* owner=([a-z_]+) grantee=(\2|storyos_[a-z]+) |^function .* grantee=PUBLIC EXECUTE$|^type .* grantee=PUBLIC USAGE$|^membership storyos_owner member=postgres inherit set$'; then
    echo "A role that is not a StoryOS role or the owner has a privilege on a StoryOS object" >&2
    exit 1
  fi
  # The provider gives pg_read_all_data to some of its roles, which then read each table with no grant of the
  # schema (an exception of #1851). First the check proves that it finds more access: a write through SET ROLE,
  # and the call of a function. An empty access gives no line.
  if [ -n "$(access_beyond_read "")" ]; then
    echo "The access check finds a violation in an empty access" >&2
    exit 1
  fi
  provider_superuser_psql -c "CREATE ROLE access_probe LOGIN;
    GRANT pg_write_all_data TO access_probe WITH INHERIT FALSE, SET TRUE;
    GRANT pg_read_all_data TO access_probe" >/dev/null
  probe_access=$(storyos_effective_access "$provider_container" | grep '^access_probe ' || true)
  provider_superuser_psql -c "DROP ROLE access_probe" >/dev/null
  case "$(access_beyond_read "$probe_access")" in
    *"access_probe as pg_write_all_data relation INSERT"*) ;;
    *)
      echo "The access check does not find a write through SET ROLE: $probe_access" >&2
      exit 1
      ;;
  esac
  if ! printf '%s\n' "$probe_access" | grep -x 'access_probe function EXECUTE invoker read_all_data' >/dev/null; then
    echo "The access check does not find the call of a function: $probe_access" >&2
    exit 1
  fi
  effective_access=$(storyos_effective_access "$provider_container")
  printf 'Roles of the provider with access to StoryOS objects:\n%s\n' "$effective_access"
  beyond_read=$(access_beyond_read "$effective_access")
  if [ -n "$beyond_read" ]; then
    printf '%s\n' "$beyond_read" >&2
    echo "A role of the provider has access to a StoryOS object that is not the read of pg_read_all_data" >&2
    exit 1
  fi
  provider_psql -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" >/dev/null
  provider_psql < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
  provider_runtime=$(postgres_runtime_url "$provider_container")
  echo "Binding the packaged Server and the Worker on the provider database"
  assert_packaged_server_binds "provider" "$provider_runtime"
  STORYOS_DATABASE_URL="$provider_runtime" \
  STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
    "$worker_bin" --claim-only
  (
    container=$provider_container
    STORYOS_TEST_DATABASE_URL=$provider_runtime
    prove_bound_request_path_activation
  )
  echo "Running the Project Isolation tests and the vector store test on the provider database"
  STORYOS_TEST_DATABASE_URL="$provider_runtime" \
  STORYOS_TEST_ADMIN_DATABASE_URL=$(postgres_admin_url "$provider_container") \
    cargo test --locked --workspace --all-features --test project_scope -- --ignored --nocapture
  postgres_test_binary=$(find_postgres_test_binary)
  STORYOS_TEST_DATABASE_URL="$provider_runtime" \
  STORYOS_TEST_ADMIN_DATABASE_URL=$(postgres_admin_url "$provider_container") \
    run_postgres_tests passage_vectors::tests::the_vector_store_is_available_exactly_when_the_database_offers_the_extension
  run_provider_upgrade_checks
}

# Each row of the proof and of the ledger of the provider database, as one text.
provider_ledger_state() {
  provider_psql -c "SELECT jsonb_build_array(
      (SELECT jsonb_agg(row_to_json(proof)) FROM storyos.storage_activation_proofs AS proof),
      (SELECT jsonb_agg(row_to_json(attempt) ORDER BY attempt_id) FROM storyos.schema_migrations AS attempt),
      (SELECT jsonb_agg(row_to_json(phase) ORDER BY attempt_id, phase_id) FROM storyos.migration_phases AS phase),
      (SELECT jsonb_agg(row_to_json(file) ORDER BY source_path) FROM storyos.migration_phase_checksums AS file)
    )::text"
}

# The upgrade edge on a new provider database (ADR 0104 section 4). The test activates the package without its
# last file, writes a Project with a Chapter, and takes the edge to the package and then to another route catalog.
# Then the package of this build takes the edge back to its identity, refuses a changed checksum of a recorded file
# with no change, and accepts its identity.
run_provider_upgrade_checks() {
  docker rm -fv "$provider_container" >/dev/null
  # The helpers of the provider database use $provider_container.
  provider_container=$provider_upgrade_container
  start_postgres "$provider_container" "$storyos_provider_postgres_image"
  echo "Taking the upgrade edge on a provider database"
  STORYOS_TEST_UPGRADE_ADMIN_URL=$(postgres_admin_url "$provider_container") \
    run_postgres_tests storage_activation::upgrade_tests::an_active_database_of_an_earlier_package_takes_the_upgrade_edge
  source_count=$(find crates/storyos-adapter-postgres/migrations -maxdepth 1 -name '*.sql' | wc -l | tr -d ' ')
  run_provider_storage
  if [ "$storage_status" -ne 0 ] || [ "$storage_output" != "STORYOS_STORAGE_ACTIVATION=Active
STORYOS_STORAGE_UPGRADE=from $source_count files to $source_count files" ]; then
    echo "The package did not take the upgrade edge from another route catalog: $storage_status $storage_output" >&2
    exit 1
  fi
  baseline_path=crates/storyos-adapter-postgres/migrations/0001_release_1_baseline.sql
  baseline_checksum=$(provider_psql -c "SELECT checksum FROM storyos.migration_phase_checksums
    WHERE source_path = '$baseline_path'")
  provider_psql -c "UPDATE storyos.migration_phase_checksums
    SET checksum = 'sha256:1111111111111111111111111111111111111111111111111111111111111111'
    WHERE source_path = '$baseline_path'" >/dev/null
  changed_state=$(provider_ledger_state)
  run_provider_storage
  if [ "$storage_status" -eq 0 ] || [ "$storage_output" != "STORYOS_STORAGE_ACTIVATION=Refused
STORYOS_STORAGE_ACTIVATION_REASON=identity_mismatch" ]; then
    echo "The package did not refuse a changed checksum of a recorded file: $storage_status $storage_output" >&2
    exit 1
  fi
  if [ "$(provider_ledger_state)" != "$changed_state" ]; then
    echo "The refusal of a changed checksum changed the proof or the ledger" >&2
    exit 1
  fi
  provider_psql -c "UPDATE storyos.migration_phase_checksums SET checksum = '$baseline_checksum'
    WHERE source_path = '$baseline_path'" >/dev/null
  run_provider_storage
  if [ "$storage_status" -ne 0 ] || [ "$storage_output" != "STORYOS_STORAGE_ACTIVATION=Active" ]; then
    echo "The package did not accept its identity after the upgrade: $storage_status $storage_output" >&2
    exit 1
  fi
  upgrade_state=$(provider_psql -c "SELECT (SELECT count(*) FROM storyos.schema_migrations)::text || '/' ||
      (SELECT string_agg(title, ',') FROM storyos.projects) || '/' ||
      COALESCE(to_regclass('storyos.passage_vectors')::text, 'absent')")
  if [ "$upgrade_state" != "4/Empty Novel/storyos.passage_vectors" ]; then
    echo "The upgraded provider database has not the attempts, the Project, or the vector table: $upgrade_state" >&2
    exit 1
  fi
  if [ "$(docker exec "$provider_container" psql -X -v ON_ERROR_STOP=1 -At \
    "postgresql://storyos_runtime:runtime@127.0.0.1:5432/postgres" -c "SELECT current_user")" != storyos_runtime ]; then
    echo "The password of the runtime role does not work after the upgrade" >&2
    exit 1
  fi
}

# Runs the exact-dist journeys $1... with the order of the arguments, each file one time. The Stage 2
# AI-independent journeys run first, with a Server without a model destination. Vitest writes its report of each
# run to $STORYOS_JOURNEY_REPORT-<run>.json, which scripts/check-lanes.py reads.
run_journeys() {
  record_google_chrome_version
  report=${STORYOS_JOURNEY_REPORT:-$repository_root/target/check/journeys-$$}
  rm -f "$report"-*.json
  ai_disabled_files=""
  model_files=""
  for journey in "$@"; do
    if exact_dist_ai_disabled_journey "$journey"; then
      ai_disabled_files="$ai_disabled_files $journey"
    else
      model_files="$model_files $journey"
    fi
  done
  prepare_server_database "$container"
  reset_exact_dist_fixture "$container"
  journey_run=0
  if [ -n "$ai_disabled_files" ]; then
    start_ai_disabled_exact_dist_server
    # shellcheck disable=SC2086
    run_journey_files "$STORYOS_AI_DISABLED_SERVER" $ai_disabled_files
  fi
  if [ -n "$model_files" ]; then
    start_exact_dist_server
    # shellcheck disable=SC2086
    run_journey_files "$STORYOS_DEV_SERVER" $model_files
  fi
  stop_exact_dist_server
}

run_journey_files() {
  journey_server=$1
  shift
  journey_run=$((journey_run + 1))
  STORYOS_DEV_SERVER="$journey_server" STORYOS_VITEST_FILE_ORDER=$(printf '%s:' "$@") \
    sh "$repository_root/scripts/browser-vitest.sh" --project browser-exact-dist "$@" --passWithNoTests=false \
      --allowOnly=false --reporter=default --reporter=json --outputFile="$report-$journey_run.json"
}

container="storyos-issue105-$$"
oracle_container="storyos-storage-oracle-$$"
activation_container="storyos-storage-activation-$$"
legacy_activation_container="storyos-storage-activation-legacy-$$"
provider_container="storyos-storage-provider-$$"
provider_upgrade_container="storyos-storage-provider-upgrade-$$"
export CARGO_NET_OFFLINE=true
cleanup() {
  stop_exact_dist_server
  docker rm -fv "$container" "$oracle_container" "$activation_container" \
    "$legacy_activation_container" "$provider_container" "$provider_upgrade_container" >/dev/null 2>&1 || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cleanup

case "${1:-}" in
  activation)
    run_activation_checks
    ;;
  provider)
    run_provider_activation_checks
    ;;
  journeys)
    shift
    if [ "$#" -eq 0 ]; then
      echo "usage: scripts/verify-project-scope.sh journeys FILE..." >&2
      exit 2
    fi
    run_journeys "$@"
    ;;
  *)
    echo "usage: scripts/verify-project-scope.sh activation | provider | journeys FILE..." >&2
    exit 2
    ;;
esac
