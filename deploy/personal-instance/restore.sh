#!/bin/sh
# Loads one dump of backup.sh into an empty database (ADR 0104 section 7): the roles of 0000_roles.sql, the schema
# extensions with vector, the dump, the grants outside the schema storyos, and the password of the runtime role.
# After it, the Server of the same package starts with no run of storyos-storage, because the dump holds the
# activation proof.
#
# Usage: restore.sh <dump>
#
# push.sh copies 0000_roles.sql beside this script. Set /etc/storyos/instance.env to the new database first.
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib.sh"
require_root
load_instance
[ "$#" = 1 ] && [ -f "$1" ] || fail "usage: restore.sh <dump>"
[ -f "$here/0000_roles.sql" ] || fail "$here/0000_roles.sql is absent. push.sh copies it beside the scripts"
use_admin_connection
runtime_password=$(read_secret runtime-password)
schemas=$(admin_psql -c "SELECT count(*) FROM pg_catalog.pg_namespace WHERE nspname = 'storyos'")
[ "$schemas" = 0 ] || fail "the database has the schema storyos. Restore only into an empty database"
admin_psql -f "$here/0000_roles.sql"
admin_psql <<'EOF'
CREATE SCHEMA IF NOT EXISTS extensions;
CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA extensions;
EOF
pg_restore --no-password --exit-on-error --single-transaction --dbname="$PGDATABASE" "$1"
admin_psql <<EOF
GRANT USAGE ON SCHEMA extensions TO storyos_owner, storyos_runtime;
ALTER ROLE storyos_runtime PASSWORD '$runtime_password';
EOF
echo "Restored $1. Start the services with: systemctl start storyos-server storyos-worker"
