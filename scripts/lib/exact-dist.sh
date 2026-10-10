# Source after setting repository_root. verify-project-scope.sh and verify-journey.sh
# share this exact-dist procedure, so the two cannot diverge.

. "$repository_root/scripts/lib/controlled-postgres.sh"

storage_bin="$repository_root/target/release-package/storyos-storage"
server_bin="$repository_root/target/release-package/storyos-server"
worker_bin="$repository_root/target/release-package/storyos-worker"
web_root="$repository_root/target/release-package/web"
canary_admin_url="postgres://postgres:wrong@127.0.0.1:1/postgres"
exact_dist_server_pid=""
exact_dist_server_log=""
exact_dist_ai_disabled_pid=""
exact_dist_ai_disabled_log=""

require_release_package() {
  if [ ! -x "$storage_bin" ] || [ ! -x "$server_bin" ] || [ ! -x "$worker_bin" ]; then
    echo "The release package does not contain storyos-storage, Server, and Worker" >&2
    exit 1
  fi
}

# Activate a new controlled PostgreSQL with storyos-storage, load the controlled
# fixture, and export the test database variables of the exact-dist journeys.
prepare_server_database() {
  echo "Preparing the Server-facing verify database"
  start_postgres "$1"
  container_admin=$(postgres_admin_url "$1")
  if STORYOS_DATABASE_URL="$container_admin" "$storage_bin"; then
    echo "storyos-storage reused STORYOS_DATABASE_URL for the Server-facing database" >&2
    exit 1
  fi
  STORYOS_STORAGE_ADMIN_URL="$container_admin" \
    "$storage_bin"
  runtime_secret_state=$(docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT CASE WHEN rolpassword IS NULL THEN 'absent' ELSE 'present' END
       FROM pg_authid WHERE rolname = 'storyos_runtime'")
  if [ "$runtime_secret_state" != "absent" ]; then
    echo "The tracked Release 1 bootstrap installed a runtime password" >&2
    exit 1
  fi
  set_runtime_password "$1"
  server_facing_active=$(docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
    "SELECT phase FROM storyos.storage_activation_proofs WHERE proof_id = 'release-1'")
  if [ "$server_facing_active" != "active" ]; then
    echo "The Server-facing verify database was not Activated by storyos-storage" >&2
    exit 1
  fi
  load_controlled_fixture "$1"

  published=$(docker port "$1" 5432/tcp)
  port=${published##*:}
  export STORYOS_TEST_DATABASE_URL="postgres://storyos_runtime:runtime@127.0.0.1:$port/postgres"
  export STORYOS_TEST_ADMIN_DATABASE_URL="postgres://postgres:admin@127.0.0.1:$port/postgres"
  export STORYOS_TEST_POSTGRES_CONTAINER="$1"
}

reset_exact_dist_fixture() {
  reload_controlled_fixture "$1"
  reset_command_challenge_rate_windows "$1"
}

# The Stage 2 AI-independent journeys run in a deployment that offers no model destination
# (ADR 0048). The other journeys use a deployment that offers the Contract-Faithful Fake Destination.
exact_dist_ai_disabled_journeys="test/browser-exact-dist/s2-jrn-001.integration.test.ts test/browser-exact-dist/s2-workspace.integration.test.ts"

# Starts one packaged Server. $1 is its model destination; an empty value offers none.
launch_exact_dist_server() {
  launched_log=$(mktemp "${TMPDIR:-/tmp}/storyos-s1-server.XXXXXX")
  stage1_user_id="018f0000-0000-7001-8000-000000000001"
  (
    if [ -n "$1" ]; then
      export STORYOS_MODEL_DESTINATION="$1"
    else
      unset STORYOS_MODEL_DESTINATION
    fi
    exec env STORYOS_WORKER=0 \
      STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
      STORYOS_STORAGE_ADMIN_URL="$canary_admin_url" \
      STORYOS_BOOTSTRAP_SESSIONS="{\"session-a\":\"$stage1_user_id\"}" \
      STORYOS_CHALLENGE_SECRET="test-only-challenge-secret-that-is-at-least-thirty-two-bytes" \
      "$server_bin" --bind 127.0.0.1:0 --web-root "$web_root"
  ) >"$launched_log" 2>&1 &
  launched_pid=$!
  attempt=0
  while ! grep -q '^STORYOS_SERVER_URL=http://' "$launched_log"; do
    if ! kill -0 "$launched_pid" >/dev/null 2>&1; then
      cat "$launched_log" >&2
      echo "The StoryOS Server exited before the exact-dist journey" >&2
      exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ]; then
      cat "$launched_log" >&2
      echo "The StoryOS Server did not become ready for the exact-dist journey" >&2
      exit 1
    fi
    sleep 0.05
  done
  launched_url=$(sed -n 's/^STORYOS_SERVER_URL=//p' "$launched_log" | head -n 1)
}

# Starts the exact-dist Server at STORYOS_DEV_SERVER. $1 is its model destination (default
# host_fake); an empty value offers none.
start_exact_dist_server() {
  launch_exact_dist_server "${1-host_fake}"
  exact_dist_server_log=$launched_log
  exact_dist_server_pid=$launched_pid
  STORYOS_DEV_SERVER=$launched_url
  export STORYOS_DEV_SERVER
}

# Starts a second Server without a model destination at STORYOS_AI_DISABLED_SERVER.
start_ai_disabled_exact_dist_server() {
  launch_exact_dist_server ""
  exact_dist_ai_disabled_log=$launched_log
  exact_dist_ai_disabled_pid=$launched_pid
  STORYOS_AI_DISABLED_SERVER=$launched_url
  export STORYOS_AI_DISABLED_SERVER
}

# Returns success when $1 is a Stage 2 AI-independent journey.
exact_dist_ai_disabled_journey() {
  # A POSIX shell function has no local variables, so this name must differ from each caller.
  for ai_disabled_journey in $exact_dist_ai_disabled_journeys; do
    case "$1" in
      *"${ai_disabled_journey#test/browser-exact-dist/}") return 0 ;;
    esac
  done
  return 1
}

# The cleanup trap of each caller also calls this function, so it must be idempotent.
stop_exact_dist_server() {
  if [ -n "$exact_dist_ai_disabled_pid" ]; then
    kill "$exact_dist_ai_disabled_pid" >/dev/null 2>&1 || true
    wait "$exact_dist_ai_disabled_pid" >/dev/null 2>&1 || true
    exact_dist_ai_disabled_pid=""
    rm -f "$exact_dist_ai_disabled_log"
  fi
  if [ -n "$exact_dist_server_pid" ]; then
    kill "$exact_dist_server_pid" >/dev/null 2>&1 || true
    wait "$exact_dist_server_pid" >/dev/null 2>&1 || true
    exact_dist_server_pid=""
  fi
  if [ -n "$exact_dist_server_log" ]; then
    rm -f "$exact_dist_server_log"
    exact_dist_server_log=""
  fi
}
