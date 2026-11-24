#!/bin/sh
# The first activation of the personal instance (ADR 0104 section 5), after install.sh. It runs storyos-storage of
# the current package with the admin URL, sets the password of the runtime role, makes the one User row of the
# configured session with the admin connection, and starts the Server and the Worker. A second run is safe.
#
# Usage: activate.sh
#
# The first Create Project needs only the User row (crates/storyos-adapter-postgres/tests/fixture.sql): the
# Project, its Chapter, and each other row come from the commands of the author.
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib.sh"
require_root
load_instance
[ "$#" = 0 ] || fail "usage: activate.sh"
run_storage "$opt/current" || fail "storyos-storage refused the database"
use_admin_connection
runtime_password=$(read_secret runtime-password)
user_id=$(cat "$etc/user-id")
# The text goes to psql on its standard input, so no secret is a process argument.
admin_psql <<EOF
SET client_min_messages = warning;
-- The admin login of the provider is not a superuser (ADR 0104 section 5). A second grant changes nothing.
GRANT storyos_owner TO CURRENT_USER WITH INHERIT TRUE, SET TRUE;
ALTER ROLE storyos_runtime PASSWORD '$runtime_password';
BEGIN;
SET LOCAL ROLE storyos_owner;
SET LOCAL storyos.user_id = '$user_id';
INSERT INTO storyos.users(user_id) VALUES ('$user_id') ON CONFLICT (user_id) DO NOTHING;
COMMIT;
EOF
echo "Set the password of the runtime role and made the User row"
systemctl start storyos-server.service storyos-worker.service
wait_for_server
