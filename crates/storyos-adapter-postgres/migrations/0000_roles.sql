CREATE ROLE storyos_owner NOLOGIN;
CREATE ROLE storyos_runtime LOGIN NOSUPERUSER NOBYPASSRLS;
CREATE ROLE storyos_backup LOGIN NOSUPERUSER NOBYPASSRLS REPLICATION;
CREATE ROLE storyos_restore LOGIN NOSUPERUSER NOBYPASSRLS;
-- The login that runs the activation needs the rights of the owner also when it is not a superuser,
-- for example the admin login of a hosted provider database (ADR 0104 section 5).
GRANT storyos_owner TO CURRENT_USER WITH INHERIT TRUE, SET TRUE;
