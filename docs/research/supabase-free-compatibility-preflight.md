# Supabase Free Compatibility Preflight

Observation date: 2026-09-12.

Current disposition: hosted compatibility work is deferred until deployment preparation. Development and verification continue on local OrbStack PostgreSQL. This report retains the observations; it is not an active implementation plan or a required cloud test.

## Purpose and result

Check whether the existing Release 1 package can use a separate Supabase Free database for development. The operator accepts loss of development data. Backup setup, scheduling, paid recovery features, and hosted recovery acceptance are deferred under [ADR 0021](../adr/0021-own-release-1-recovery-chain-outside-the-runtime.md).

Result: hosted activation is not yet supported by the observed package. Three compatibility problems are confirmed: required TLS, provider objects in the empty-baseline predicate, and missing explicit owner-role grants. No successful Storage Activation, hosted Server or Worker connection, or hosted browser journey is claimed.

## Observed environment

| Item | Observed value |
| --- | --- |
| Repository HEAD | `a07b303d578f2642749fa61cd6f85b04d4f9fe7a` |
| Repository tree | `5997136ea4bfbd4862b7454e54b8f28ee65d9d14` |
| Package manifest source commit | `c9560c4483b853e333f421ca45d8aa09496c123d` |
| Package manifest source tree | `5997136ea4bfbd4862b7454e54b8f28ee65d9d14` |
| Organization | SextantLabs, plan `free` |
| Project | `storyos-dev-validation`, reference `mylevmbqhzuovdrucnaw` |
| Region | `ap-southeast-1` |
| Project status | `ACTIVE_HEALTHY` |
| PostgreSQL | `17.6` |
| SQL probe session role | `postgres` |

The package tree equals the observed committed repository tree. Uncommitted changes at the start of the investigation were two ADR edits that record the current development scope. No implementation change was present.

The Supabase plugin successfully performed project and organization lookups, documentation search, and database queries. These calls prove plugin access. They do not authenticate the StoryOS package to PostgreSQL.

## Connection probe

The direct PostgreSQL endpoint and the session pooler endpoint both accepted a TCP connection from the operator Mac.

The existing packaged `storyos-storage` was run against the session pooler with `sslmode=require`, a ten-second connection timeout, and no password. It returned exit code 1:

```text
STORYOS_STORAGE_ACTIVATION=Refused
STORYOS_STORAGE_ACTIVATION_REASON=unavailable
error performing TLS handshake
```

This failure occurs before password authentication or migration execution. Both [Storage Activation](../../crates/storyos-adapter-postgres/src/storage_activation.rs) and the [runtime connection pool](../../crates/storyos-adapter-postgres/src/connection_pool.rs) use `tokio_postgres::NoTls` at the observed baseline. The result does not prove invalid credentials or a provider outage.

## Empty-baseline predicate

The exact predicate from `database_has_user_state` was run as a read-only query through the plugin. It returned `true`. The activation proof and schema migration ledger were absent. The matched provider relations were:

| Schema | Matching relations |
| --- | ---: |
| auth | 24 |
| extensions | 2 |
| realtime | 4 |
| storage | 8 |
| vault | 2 |

The state machine returns `NonEmptyWithoutLedger` for this state. This is a code-path conclusion from the live predicate result. The packaged process did not reach that predicate because its required TLS connection failed first.

A future provider-aware predicate must keep refusing unknown application state. A schema name alone is not proof that all objects in that schema belong to the provider.

## Role and ownership probes

The live `postgres` role is not a superuser. It has `CREATEROLE`, `CREATEDB`, `REPLICATION`, and `BYPASSRLS`. A privilege query confirmed access to `pg_catalog.pg_authid` without selecting any stored password value or hash.

Two capability probes used the plugin migration endpoint. Each probe created unique temporary roles and schemas inside nested transactions. Each nested transaction raised a deliberate exception to roll back its changes. The outer block returned the collected results through a deliberate `Z0002` exception, so no migration committed. `Z0001` in an individual result means that the tested operation succeeded and then reached the deliberate rollback.

| Operation | Result |
| --- | --- |
| Create a `NOLOGIN NOSUPERUSER NOBYPASSRLS REPLICATION` role | PASS |
| Create an owner role, then `SET LOCAL ROLE` without an explicit grant | Refused, SQLSTATE `42501` |
| Create a schema owned by that role without an explicit grant | Refused, SQLSTATE `42501` |
| Grant the owner role to `postgres`, create its schema, switch role, create a table, and force RLS | PASS |
| Grant with `INHERIT TRUE, SET TRUE`, switch to owner, create tables, then reset to admin for a forced-RLS backfill and foreign-key validation | PASS |
| Check that the new role password is NULL through `pg_authid`, without returning its value | PASS |

The original role source creates roles without granting membership to the maintenance login. The next source creates a schema with `AUTHORIZATION storyos_owner` and switches to that role. The probes confirm that an explicit maintenance grant is required in this environment.

The successful replication-role and role-catalog checks correct earlier hypotheses. They are not confirmed blockers. Keep the existing recovery roles and schema unless a later reproducible failure requires a separate decision. A successful role or function creation does not prove hosted recovery.

After both probes, queries confirmed zero probe roles, zero probe schemas, zero StoryOS schemas, and zero migration history rows. The migration endpoint created an empty `supabase_migrations` bookkeeping schema outside the failed probe transaction. That provider-tooling side effect remains recorded here; it is not StoryOS activation state.

## Limits and next acceptance

- The full embedded bootstrap has not run against the hosted project. The permission probes are bounded compatibility evidence, not a substitute for the packaged state machine.
- Runtime credentials remain an external secret step after activation. No PostgreSQL password was supplied to the package during these probes.
- A candidate must use the existing maintenance owner and exact catalogued source identity. Do not run alternate SQL through the plugin and then claim packaged activation.
- The candidate must retain local OrbStack verification, forced RLS, role separation, atomic activation, identity mismatch refusal, and the isolated recovery oracle.
- Hosted acceptance must activate the new database through the package, start Server and Worker as `storyos_runtime`, write and save in the real browser, reload, and finish a Worker-owned export.
- Hosted acceptance is an explicit operator check. Local `make verify` does not need a cloud account, cloud credentials, or cloud availability.
- No provider plan upgrade, backup schedule, restored copy, public VPS deployment, or Stage 3 implementation occurred in this investigation.

The results apply to this project and PostgreSQL version. Future activation work must repeat relevant checks on its exact execution baseline.
