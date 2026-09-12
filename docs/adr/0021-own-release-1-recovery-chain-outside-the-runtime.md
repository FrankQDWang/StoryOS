---
status: accepted
---

# Own Release 1 Recovery Chain Outside the Runtime

The Foundation Recovery Service Profile requires host-loss recovery for the author database. A packaged maintenance owner, not the Server or Worker, owns that procedure. The owner is a new binary, `storyos-recovery`. This ADR follows [ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md).

Production PostgreSQL is hosted Supabase. StoryOS does not own that host, PGDATA, `archive_command`, or downloadable physical backups and WAL files. Supabase holds the physical chain. For Foundation Validation the adopted promise is the vendor daily physical backup, not a StoryOS-owned fifteen-minute WAL archive. A later tightening may enable vendor PITR. A timed logical dump is an operator practice. It is not a Recovery Copy and it is not the Profile.

Development stays on a local Mac with OrbStack PostgreSQL. The existing isolated recovery drill remains the StoryOS-owned Hold and Recovery Visibility Proof oracle against that local database. Server, Worker, and Web stay paired on a Linux VPS. They are not the database host.

## Development scope and production decision

Current development and verification use PostgreSQL in Docker through OrbStack on the operator Mac. There is no monthly subscription budget. The operator accepts loss of development data. Hosted compatibility, backup setup, backup scheduling, and hosted restore acceptance are deferred until deployment preparation and do not block local feature development.

Before production deployment, the operator will assess hosted compatibility and backup requirements, and may select a paid Supabase plan. When recovery work resumes, the recovery capability must pass a complete restore and continued-writing check on a separate test project. A Free development project does not claim the Foundation Recovery Service Profile.

## Withdrawn for hosted production

These earlier working choices assumed an operator-owned PostgreSQL host. They do not apply to hosted Supabase:

- Configuring `archive_command` on the live cluster
- Taking `pg_basebackup` as the first Recovery Copy
- Writing chain members into an operator-attested filesystem destination
- Installing systemd on the PostgreSQL host
- Using `STORYOS_RECOVERY_ADMIN_URL` to change archive settings
- Treating in-place vendor PITR as a release-candidate isolated restore

## Considered options

- Extending `storyos-storage`, or replacing the owner with a dashboard runbook, was rejected. `storyos-recovery` is a thin vendor adapter: it proves vendor backups exist for the bound project, and it runs Recovery Visibility Proof against a restored copy that is not the live production project.
- Checking backup freshness on every Server request was rejected. Runtime may observe only an install-once proof that this storage identity is bound to a hosted project with vendor backups enabled.
- A new public Problem for a missing install-once proof was rejected. Absence reuses `project_store_unavailable`. An identity mismatch reuses `upgrade_required`.
- Keeping the Profile at a fifteen-minute StoryOS-owned RPO while adopting daily vendor backups was rejected. Before production recovery work resumes, align the tracked storage-contract backup section with the current hosted recovery contract. Local development and deferred compatibility testing do not claim that production contract alignment is complete.
- Absorbing Supabase Auth, Realtime, Storage, or PostgREST was rejected.
- This decision does not absorb structural-authority settlement, frozen command acknowledgements, live Project Activity delivery, or public TLS and Host/Origin ownership. [Own Public Host, HTTPS Origin, and Cookie Secure](https://github.com/FrankQDWang/StoryOS/issues/604) owns that sibling gap. It does not resume Stage 3.

## Consequences

- Install-once means: the Active storage identity is bound to one hosted project reference, and vendor daily backups are enabled. It does not mean StoryOS possesses WAL files.
- `verify` lists vendor backups through the Management API and refuses when none exist. It does not take a backup.
- Isolated Hold and Recovery Visibility Proof against production data use a restored copy that is not the live project. In-place `restore-pitr` is a disaster action, not the release proof.
- The local OrbStack drill stays the oracle for Hold posture and Visibility Proof mechanics.
- Hosted compatibility and recovery Issues are closed as `not_planned` for the current local development phase. Their closure does not claim implementation or acceptance. Before future execution, revalidate the retained findings and update the current contract. Existing recovery schema, maintenance roles, and the local isolated oracle remain in place.
- This decision does not resume Stage 3.
