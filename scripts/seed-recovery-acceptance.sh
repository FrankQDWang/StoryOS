#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
if [ -z "${STORYOS_VERIFICATION_RUN:-}" ]; then
  exec python3 scripts/verification.py step recovery-seed -- sh "$0"
fi
before=$(docker exec "$STORYOS_TEST_POSTGRES_CONTAINER" psql -XAt -v ON_ERROR_STOP=1 -U postgres \
  -c 'SELECT count(*) FROM storyos.proposal_validation_conditions')
# These scenarios produce the six retained conditions and the refusal proof.
STORYOS_VITEST_FILE_ORDER=test/node-postgresql/accept-proposal-http.integration.test.ts:test/node-postgresql/acceptance-refusal-http.integration.test.ts:test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts: \
  pnpm --dir apps/web exec vitest run --project node-postgresql \
    test/node-postgresql/accept-proposal-http.integration.test.ts \
    test/node-postgresql/acceptance-refusal-http.integration.test.ts \
    test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts \
    --testNamePattern='^(acceptProposal retains |Acceptance retains a stale-writer refusal|partial Acceptance preserves )'
after=$(docker exec "$STORYOS_TEST_POSTGRES_CONTAINER" psql -XAt -v ON_ERROR_STOP=1 -U postgres \
  -c 'SELECT count(*) FROM storyos.proposal_validation_conditions')
if [ "$((after - before))" -ne 6 ]; then
  echo "Recovery seed did not create all six Acceptance conditions" >&2
  exit 1
fi
