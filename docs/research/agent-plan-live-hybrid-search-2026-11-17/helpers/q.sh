#!/bin/sh
# read-only query: q.sh "<sql>"
exec docker exec -e PGOPTIONS='-c default_transaction_read_only=on' storyos-dev-postgres-4cac9ca41c5d262d2e95 psql -At -F ' | ' -U postgres -d postgres -c "$1"
