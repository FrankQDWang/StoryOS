---
status: accepted
---

# Own Release 1 Storage Activation Outside the Runtime

The production package must prove Release 1 Storage Activation before Server or Worker traffic. A maintenance owner, not the Server or Worker, runs the catalogued state machine from `NewInstallRequired` to `Active` on an empty baseline. That owner persists the ledger and the activation record. Runtime processes only consume that proof: they refuse to bind or claim when it is absent, and they refuse again on a protected request or Worker claim. They never run DDL.

## Considered options

- Adopting a non-empty database that already has schema or roles but no accepted ledger was rejected. Preflight requires a new installation and an empty predecessor set. The current verify SQL apply is a test oracle, not a production Active path.
- Letting the Server bootstrap an empty database at startup was rejected. `storyos_runtime` has no migration authority. Mixing DDL with listener bind would hide the missing production owner.
- Treating Active as Recovery Visibility Proof was rejected. A live Recovery Copy chain and restore proof stay with the production recovery owner. First Foundation install may reach Active without claiming host-loss recovery. The isolated recovery drill remains the current recovery oracle.
- Shipping the catalogued SQL as a loose directory beside the Server was rejected. A transferred package has no Git tree, and a second SQL list can drift from the catalog checksums.
- Letting runtime infer Active from a sidecar file or a schema version was rejected. The proof must be the same ledger fact the maintenance owner persisted.
- Reusing `STORYOS_DATABASE_URL` for the maintenance owner was rejected. That URL is the `storyos_runtime` login. The maintenance owner needs a distinct admin connection that can `SET ROLE storyos_owner`.
- A new public Problem just for "not Active" was rejected. A missing or not-Active proof reuses the existing store-unavailable refusal. An identity mismatch reuses `upgrade_required`.
- A new `storyos-storage` crate, or a Server-owned binary, was rejected. The catalogued SQL and runner stay in `storyos-adapter-postgres`.

## Consequences

- The release package includes one independent Rust maintenance binary. That binary embeds the exact catalogued SQL set and checksums. It does not read a checkout or a copied `migrations/` directory.
- `storyos_runtime` receives SELECT-only access to one narrow activation proof relation. Server and Worker read that proof at start and again on a protected request or claim.
- `verify-project-scope.sh` keeps the catalogued single-transaction SQL apply and faulted rollback as an input oracle. Verify also runs the production maintenance owner against a fresh empty database until that path reaches Active.
- Offline Server `--check-web-root` and Worker `--check` stay free of PostgreSQL.
- Runtime credential provisioning stays an external secret step after the atomic bootstrap and before Server or Worker start. `storyos_owner` remains `NOLOGIN`.
- A later run of the maintenance owner against an already Active database with the same identity succeeds without a second install. A mismatched identity fails closed and does not touch domain rows.
- The maintenance binary is `storyos-storage`, built from `storyos-adapter-postgres`. The release package copies it beside Server and Worker. Its admin connection is a distinct environment value, recommended name `STORYOS_STORAGE_ADMIN_URL`.
- If a bound Server or Worker later sees a missing or not-Active proof, the public refusal is the existing `project_store_unavailable` path. An identity mismatch returns `upgrade_required` before any domain attempt.
- This decision does not resume Stage 3.
