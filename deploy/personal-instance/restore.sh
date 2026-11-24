#!/bin/sh
# Loads one dump of backup.sh into a database with no schema storyos, in a new database project (ADR 0104 section
# 7): the roles of 0000_roles.sql, the schema extensions with vector, the dump, the grants outside the schema
# storyos, and the password of the runtime role.
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
[ "$schemas" = 0 ] || fail "the database has the schema storyos. Restore only into a database with no schema storyos"
# The role file runs in one transaction. A failed restore leaves all of its roles or none, and a second attempt into
# the same database uses the roles of the first attempt.
roles=$(admin_psql -c "SELECT count(*) FROM pg_catalog.pg_roles WHERE rolname IN
  ('storyos_owner', 'storyos_runtime', 'storyos_backup', 'storyos_restore')")
case $roles in
  0) admin_psql --single-transaction -f "$here/0000_roles.sql" ;;
  4) ;;
  *) fail "the database has $roles of the 4 roles of 0000_roles.sql" ;;
esac
member=$(admin_psql -c "SELECT pg_catalog.pg_has_role(current_user, 'storyos_owner', 'SET')")
[ "$member" = t ] || fail "the admin login is not a member of storyos_owner"
# Each step before the load can run again, and the load is the last step.
admin_psql <<EOF
SET client_min_messages = warning;
CREATE SCHEMA IF NOT EXISTS extensions;
CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA extensions;
GRANT USAGE ON SCHEMA extensions TO storyos_owner, storyos_runtime;
ALTER ROLE storyos_runtime PASSWORD '$runtime_password';
EOF
pg_restore --no-password --exit-on-error --single-transaction --dbname="$PGDATABASE" "$1"
echo "Restored $1. Start the services with: systemctl start storyos-server storyos-worker"
