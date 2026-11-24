//! The check of an empty database before the Release 1 Storage Activation (ADR 0020, ADR 0104
//! section 5).

/// The schemas of the hosted provider. The source of each name is the schema list of the image
/// `supabase/postgres` that `scripts/lib/controlled-postgres.sh` pins, and for
/// `supabase_migrations` the preflight `docs/research/supabase-free-compatibility-preflight.md`.
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

/// The roles of the hosted provider that own its relations. The source is the same image: these
/// roles own each relation of its provider schemas, also the members of its extensions.
pub(crate) const PROVIDER_OWNERS: [&str; 3] = [
    "supabase_admin",
    "supabase_auth_admin",
    "supabase_storage_admin",
];

/// True when the database holds the StoryOS schema or a relation that is not of the provider. A
/// relation is of the provider only when it is in a provider schema and a provider role owns it.
/// A schema name alone is not proof, so a relation in a provider schema that the admin login or an
/// application role owns still refuses the activation.
const USER_STATE_SQL: &str = "
    SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = 'storyos')
           OR EXISTS (
             SELECT 1
             FROM pg_catalog.pg_class AS cls
             JOIN pg_catalog.pg_namespace AS ns ON ns.oid = cls.relnamespace
             WHERE ns.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
               AND ns.nspname NOT LIKE 'pg\\_temp\\_%'
               AND ns.nspname NOT LIKE 'pg\\_toast\\_temp\\_%'
               AND cls.relkind IN ('r', 'p', 'v', 'm', 'S', 'f')
               AND NOT (
                 ns.nspname = ANY($1::text[])
                 AND pg_catalog.pg_get_userbyid(cls.relowner) = ANY($2::text[])
               )
           )";

pub(crate) async fn database_has_user_state(
    client: &tokio_postgres::Client,
) -> Result<bool, tokio_postgres::Error> {
    Ok(client
        .query_one(
            USER_STATE_SQL,
            &[&PROVIDER_SCHEMAS.as_slice(), &PROVIDER_OWNERS.as_slice()],
        )
        .await?
        .get(0))
}
