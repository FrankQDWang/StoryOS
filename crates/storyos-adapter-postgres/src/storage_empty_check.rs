//! The check of an empty database before the Release 1 Storage Activation (ADR 0020, ADR 0104
//! section 5).

/// The schemas of the hosted provider. The sources of the names are the schema list of the image
/// `supabase/postgres` that `scripts/lib/controlled-postgres.sh` pins and of a new hosted project
/// (2026-11-24), and for `supabase_migrations` the preflight
/// `docs/research/supabase-free-compatibility-preflight.md`.
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

/// The roles of the hosted provider that own its relations. The sources are the relation owners of
/// the same image and of the same hosted project, which has `supabase_realtime_admin`.
pub(crate) const PROVIDER_OWNERS: [&str; 4] = [
    "supabase_admin",
    "supabase_auth_admin",
    "supabase_realtime_admin",
    "supabase_storage_admin",
];

/// The extensions of the hosted provider that have relations. On the hosted project, the admin login
/// owns the views of `pg_stat_statements`, so the owner alone does not identify them.
pub(crate) const PROVIDER_EXTENSIONS: [&str; 2] = ["pg_stat_statements", "supabase_vault"];

/// True when the database holds the StoryOS schema or a relation that is not of the provider. A
/// relation is of the provider only when it is in a provider schema, and a provider role owns it
/// or it is a member of a provider extension. A schema name alone is not proof, so another relation
/// in a provider schema that the admin login or an application role owns still refuses the
/// activation. A composite type and an index are not relations with state, as before.
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
                 AND (
                   pg_catalog.pg_get_userbyid(cls.relowner) = ANY($2::text[])
                   OR EXISTS (
                     SELECT 1
                     FROM pg_catalog.pg_depend AS dep
                     JOIN pg_catalog.pg_extension AS ext ON ext.oid = dep.refobjid
                     WHERE dep.classid = 'pg_catalog.pg_class'::regclass
                       AND dep.objid = cls.oid
                       AND dep.refclassid = 'pg_catalog.pg_extension'::regclass
                       AND dep.deptype = 'e'
                       AND ext.extname = ANY($3::text[])
                   )
                 )
               )
           )";

pub(crate) async fn database_has_user_state(
    client: &tokio_postgres::Client,
) -> Result<bool, tokio_postgres::Error> {
    Ok(client
        .query_one(
            USER_STATE_SQL,
            &[
                &PROVIDER_SCHEMAS.as_slice(),
                &PROVIDER_OWNERS.as_slice(),
                &PROVIDER_EXTENSIONS.as_slice(),
            ],
        )
        .await?
        .get(0))
}
