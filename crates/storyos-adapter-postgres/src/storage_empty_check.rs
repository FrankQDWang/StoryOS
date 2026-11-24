//! The check of an empty database before the Storage Activation (ADR 0020, ADR 0104 section 5).

/// The schemas of the hosted provider. The source of each name is the schema list of the image
/// `supabase/postgres` that `scripts/lib/controlled-postgres.sh` pins, and for
/// `supabase_migrations` the preflight `docs/research/supabase-free-compatibility-preflight.md`.
/// A schema name alone does not prove that a relation is of the provider: see `USER_STATE_SQL`.
pub(crate) const PROVIDER_SCHEMAS: [&str; 9] = [
    "auth",
    "extensions",
    "graphql",
    "graphql_public",
    "pgbouncer",
    "realtime",
    "storage",
    "supabase_migrations",
    "vault",
];

/// True when the database holds the StoryOS schema or a relation that is not of the provider. A
/// relation in a provider schema is of the provider only when it is a member of an extension or
/// when a role other than the login of the activation owns it. Thus a table that the admin login
/// made in a provider schema still refuses the activation.
const USER_STATE_SQL: &str = "
    SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = 'storyos')
           OR EXISTS (
             SELECT 1
             FROM pg_catalog.pg_class AS cls
             JOIN pg_catalog.pg_namespace AS ns ON ns.oid = cls.relnamespace
             WHERE ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
               AND ns.nspname NOT LIKE 'pg_temp_%'
               AND ns.nspname NOT LIKE 'pg_toast_temp_%'
               AND cls.relkind IN ('r', 'p', 'v', 'm', 'S')
               AND NOT (
                 ns.nspname = ANY($1::text[])
                 AND (
                   cls.relowner <> (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = current_user)
                   OR EXISTS (
                     SELECT 1
                     FROM pg_catalog.pg_depend AS dep
                     WHERE dep.classid = 'pg_catalog.pg_class'::regclass
                       AND dep.objid = cls.oid
                       AND dep.deptype = 'e'
                   )
                 )
               )
           )";

pub(crate) async fn database_has_user_state(
    client: &tokio_postgres::Client,
) -> Result<bool, tokio_postgres::Error> {
    Ok(client
        .query_one(USER_STATE_SQL, &[&PROVIDER_SCHEMAS.as_slice()])
        .await?
        .get(0))
}
