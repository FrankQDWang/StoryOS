---
status: accepted
---

# Prefer Widely Validated Hosted Infrastructure

Production uses widely validated hosted PostgreSQL instead of rebuilding an equivalent physical database host. Local development uses a Mac with OrbStack PostgreSQL. Production PostgreSQL is hosted Supabase. StoryOS Server, Worker, and Web stay paired on a Linux VPS. A TLS reverse proxy such as Caddy terminates HTTPS in front of that Server. A vendor does not become a StoryOS domain authority.

Operators stated this production choice before any tracked file recorded it. No earlier GOAL, ADR, foundation contract, or Issue named the hosted database, the VPS, or the rejected second Web host. ADR 0004 therefore sent later agents toward operator-owned local PostgreSQL. That was process drift against an unwritten production choice.

## Considered options

- Hosting production Web on Vercel was rejected. ADR 0013 and ADR 0016 already bind paired same-origin delivery, the Server-issued session cookie on the HTML GET, Server-owned CSP and Trusted Types, and exact-dist proof of the bytes the Server serves. A second Web origin would move cookie issuance, CSP, and admission, and it would stop exact-dist from proving the served bytes. The gain is a CDN for a small single-author asset set.
- Running production PostgreSQL on the same VPS was rejected for Foundation Validation. Until a StoryOS-owned physical chain exists, host loss deletes the novel. After that chain exists, the operator is the DBA.
- Rebuilding physical WAL archival on an operator-owned cluster was rejected when hosted Supabase already keeps off-host physical backups.
- Absorbing Supabase Auth, Realtime, Storage, or PostgREST was rejected. Adopted infrastructure is the PostgreSQL service, not the StoryOS admission or Core surface.
- Leaving ADR 0004, ADR 0016, and the Foundation Recovery Service Profile unchanged while running hosted PostgreSQL was rejected. Silent override is not allowed.

## Consequences

- [ADR 0016](0016-deliver-and-verify-the-paired-production-web-host.md) remains the production Web host contract. Public Host, Origin, `https`, and cookie `Secure` are missing from the current Server. [Own Public Host, HTTPS Origin, and Cookie Secure](https://github.com/FrankQDWang/StoryOS/issues/604) owns that gap. The gap is independent of which PostgreSQL host is used.
- [ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md) still owns Project Isolation. Local OrbStack PostgreSQL is the development and isolated-oracle host. It is not the production recovery host.
- [ADR 0021](0021-own-release-1-recovery-chain-outside-the-runtime.md) follows this premise: hosted Supabase holds the production physical chain. StoryOS owns the procedure, not the files.
- Storage Activation against a new Supabase project must admit vendor schemas, a non-superuser `postgres` role, explicit role grants, and TLS. Those are compatibility changes, not a new activation owner.
- This decision does not resume Stage 3.
