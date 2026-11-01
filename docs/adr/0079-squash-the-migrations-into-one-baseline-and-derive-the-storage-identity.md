---
status: accepted
---

# Squash the Migrations Into One Baseline and Derive the Storage Identity

The repository cleanup specification of phase 6 ([#1352](https://github.com/FrankQDWang/StoryOS/issues/1352)) owns this ADR. The author decided its four parts on 2026-11-01. Ticket [#1353](https://github.com/FrankQDWang/StoryOS/issues/1353) derives the storage identity from the migrations directory. Ticket [#1354](https://github.com/FrankQDWang/StoryOS/issues/1354) generates the persistence catalog. Ticket [#1355](https://github.com/FrankQDWang/StoryOS/issues/1355) writes the baseline with the domains. Ticket [#1356](https://github.com/FrankQDWang/StoryOS/issues/1356) removes the dead table families. This ADR partly supersedes [ADR 0020](0020-own-release-1-storage-activation-outside-the-runtime.md), and it changes the sentences of [ADR 0070](0070-verify-locally-with-one-check-command-and-keep-github-checks-cheap.md) and [ADR 0074](0074-generate-the-route-catalog-from-the-operation-registry.md) that name the hand-kept persistence catalog.

## Context

The PostgreSQL adapter held 103 migration files with 37,870 lines. About 51 percent of those lines dropped and added again 13 CHECK constraints. 20 functions were defined 59 times. Each fresh installation replayed all 103 files in one transaction. The Receipt result shape constraint was rewritten 40 times, and 11 migrations extended it with a dynamic `OR` append, so its live definition was a nested `OR` chain of 34,000 characters.

The storage identity came from a hand-kept JSON catalog of 1,103 lines. The build script of the adapter read the source list and the checksums from that catalog and did not list the migrations directory. The manifest digest and the migration chain digest of the catalog were hand-refreshed values. Phase 5 ([#1344](https://github.com/FrankQDWang/StoryOS/issues/1344)) removed the Python verifier that recomputed them, so no tool checked them.

No database holds data. [ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md) defers the hosted database, and the Supabase preflight found zero schemas. Each change of the source list already makes every existing development database refuse to start with an identity mismatch. A squash costs the same as one new migration.

## Decision

### 1. One baseline and the rule for the next squash

- The migrations directory holds `0000_roles.sql`, unchanged, and `0001_release_1_baseline.sql`. The baseline is the current schema with each table, function, policy, and trigger defined once. The next forward migration is `0002`.
- The build script lists the migrations directory, sorts the files by name, computes the LF-normalized SHA-256 of each file, and embeds each file. The migration chain digest is the SHA-256 of the ordered path and checksum pairs. The catalog identifier is the constant `storyos.persistence.release-1`. The ledger keeps its three tables: one `schema_migrations` row, four `migration_phases` rows from constants of the adapter, and one `migration_phase_checksums` row for each embedded file.
- The activation state machine of ADR 0020 does not change. `storyos-storage` activates an empty installation or accepts an exact already-active identity. No upgrade edge exists.
- The rule for the next squash: while no database that holds data exists, the coordinator can squash the baseline again at each Stage release, and a ticket can edit the baseline file directly. When the first database that holds data exists, the baseline freezes, and each change is one forward migration.

### 2. Closed lists as domains

- Eight closed lists are PostgreSQL domains: command kind, event kind, result kind, Receipt result kind, scoped result kind, Agent Run status, attempt role, and action class. The rule: a list that two or more columns carry, or that has five or more members. A list that one column carries with fewer than five members stays a plain CHECK constraint.
- The command kind domain and the event kind domain hold the live kinds of the generated Route Catalog. One test of the adapter compares their members with the embedded Route Catalog.
- The five per-command shape constraints stay, each written once as a flat list of per-command branches with its current name.
- No lookup table and no PostgreSQL enum type. A domain keeps CHECK semantics, so a damaged row written under `session_replication_role = replica` still gets a refusal. The storage contract already permits checked text.

### 3. Generated persistence catalog

- One test of the adapter reads the activated schema through the PostgreSQL catalog views and writes one JSON file under `generated/persistence-catalog/`: each table with its columns, constraints, indexes, row-level security state, policies, and triggers; each domain with its members; each function name.
- `make check` runs the test, which fails when the committed file differs from the activated schema. `make generate-persistence-catalog` rewrites the file.
- The hand-kept catalog file goes. The Project archive export table list is a constant of the core crate with a test against the live table list. The generated catalog is a description for readers. No runtime code reads it.

### 4. The Project archive carries no removed feature

- The baseline holds only tables that a live command writes or a live query reads. The tables of the removed Draft, inline Proposal anchor, and Project archival features go, with their archive canonical entries, the Draft withholding code of the archive build, and the recovery drill expectations that named them.
- The historical Event `storyos.event.project-archival-changed.v1` of ADR 0078 goes from the operation registry, with the catalog rule exception that accepted it.

## Superseded parts

- [ADR 0020](0020-own-release-1-storage-activation-outside-the-runtime.md): "embeds the exact catalogued SQL set and checksums". The source set now comes from the migrations directory.
- [ADR 0070](0070-verify-locally-with-one-check-command-and-keep-github-checks-cheap.md): "The migration digest check of the PostgreSQL adapter build script is the proof of the persistence catalog." The generated catalog drift test is now that proof.
- [ADR 0074](0074-generate-the-route-catalog-from-the-operation-registry.md): "the persistence catalog is a hand-kept file".
- [ADR 0078](0078-remove-the-archive-project-command-and-the-archived-project-state.md): the stored archival decisions and the historical Event stay as history. No database holds that history, so they go.
- [PostgreSQL Project Storage Isolation and Migration Contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md): "The Release 1 persistence catalog is the sole exhaustive authority for the membership and order of that set", and the clause on an unlisted SQL file.

## Considered options

- Keep the 103 files and add migration 0104 for the domains. Rejected: it adds one more rewrite of each constraint, and the reading cost stays.
- Lookup tables with foreign keys. Rejected: a foreign key trigger is off under `session_replication_role = replica`, which 11 test sites use to write damaged rows, and a foreign key to a table under row-level security failed validation once during activation.
- PostgreSQL enum types. Rejected by the storage contract: a value cannot be removed without a type rebuild, and three command kinds were removed in the last month.
- Remove the database-side shape constraints. Rejected: they are defense in depth, and four test groups drop and restore them by name.
- Generate the catalog from the SQL text. Rejected: the activated schema is the fact, and `make check` already has a PostgreSQL container.
- Keep the dead tables in the baseline. Rejected: no database holds their rows, and the archive export carried 522 lines of code for payloads that no command writes.

## Consequences

- A fresh installation runs two files. A reader finds the current shape of a table in one file and in the generated catalog.
- A new command kind is one `ALTER DOMAIN` and one registry entry, and the cross-check test fails when one is missing.
- Each existing development database refuses to start after the baseline, as after any migration. `scripts/dev-postgres.sh up` provisions a new one.
- The tag `evidence/pre-cleanup-round-2` keeps the 103 files and the hand-kept catalog in history.
