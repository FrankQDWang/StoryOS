---
status: accepted
---

# Run the Personal Instance of the Author on the Hosted Topology With a Proxy Password, the Model Key on the Server, and a Frozen Baseline

The author decided this on 2026-11-24 in conversation, after the release of Stage 9. The words of the author: "就做这个" (the next work is real use: the author writes the own novel in StoryOS), "直接上托管环境" (the instance is on the hosted topology and not on the Mac), and "批准，开工" (the plan and the two defaults of sections 2 and 3). [Deliver Stage 12: The Hosted Personal Instance of the Author](https://github.com/FrankQDWang/StoryOS/issues/1849) delivers it.

## Context

[ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) says that the proof of the product position is use: the author writes the own novel in StoryOS. At `main` `47ac3803`, no instance exists in which the author can do that:

1. Each database is disposable. `scripts/dev-postgres.sh` removes its container at each start, and the check form turns off the safe write to the disk.
2. `storyos-storage` activates an empty database or accepts the exact active identity, and no upgrade edge exists ([ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md) section 1). Each change of a migration file makes each existing database refuse to start.
3. [ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md) names the production topology: hosted Supabase PostgreSQL, and the Server, the Worker, and the Web resources on one Linux VPS behind a TLS reverse proxy. The compatibility work was deferred. [The preflight of 2026-09-12](../research/supabase-free-compatibility-preflight.md) found three problems: the adapter connects with no TLS, the check of an empty database counts the schemas of the provider, and the role source needs explicit grants for an admin login that is not a superuser. A new hosted project has PostgreSQL 17, and the repository uses PostgreSQL 16.
4. The product has no login. The packaged Server gives the cookie of the one configured session to each visitor that has no cookie (`crates/storyos-server/src/web_host.rs`). On a public origin, each visitor is the author.
5. [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md) keeps the real model route off the Linux VPS until a later ADR selects the resolver of the credential for that deployment.
6. `scripts/package-release.py` builds the package only for the host of the build, which is macOS.

The author can start to write on 2026-11-28.

## Decision

### 1. The personal instance

The personal instance of the author is the first deployment of StoryOS that holds data. It uses the topology of ADR 0022 with no change: one hosted Supabase PostgreSQL project, and one Linux VPS with the packaged Server, the packaged Worker, the Web resources, and Caddy as the TLS reverse proxy. It has one User, the author, and one configured session. It is not a product for other users.

### 2. A password at the proxy

The reverse proxy asks for a user name and a password (HTTP Basic authentication over TLS) before each request reaches the Server. The operator sets the password, and Caddy keeps only its hash. The Server does not change: it has no login, and it trusts the proxy as the only public listener ([ADR 0023](0023-own-foundation-validation-public-https-without-pass-cloud.md)).

A public origin with no such gate is not permitted for an instance that holds data. This sentence amends the public HTTPS profile of ADR 0023 for this deployment.

### 3. The model key on the server

The real model route runs on the VPS of the personal instance. The resolver of the credential is the env file of [ADR 0050](0050-load-server-and-worker-settings-through-one-typed-settings-layer.md): only the Worker reads it, its path is `STORYOS_ENV_FILE`, the file has mode 600 and belongs to the service user of the Worker, and records keep only the Credential Reference. Only the author puts a key into that file. This section is the later ADR that ADR 0039 asks for. The route stays for the personal use of the author ([ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md)); a commercial use needs another route and a new ADR.

### 4. The baseline is frozen, and the storage has an upgrade edge

No database held data before this instance, so tickets could edit the baseline. With this instance, the rule of ADR 0079 section 1 starts: the baseline freezes, and each later change of the schema is one forward migration.

`storyos-storage` gets one new edge. It accepts a database that is Active with an earlier identity of the same catalog, when the migration files that the ledger records are a prefix of the embedded files, with the same path and the same checksum for each one. It then applies each later file in order, records it, and replaces the proof with the identity of the package, in one transaction. The edge also covers a package with no new file and a changed identity, for example a changed route catalog. Each other difference stays `identity_mismatch`: a recorded file that the package does not have, a changed checksum, and another catalog. The edge needs no flag and no confirmation: the operator runs `storyos-storage` of the new package before the new Server starts, as for a first activation.

The ledger of the baseline permits one attempt today. The ticket of the edge changes the baseline so that the ledger records each attempt. This is the last edit of the baseline: the edit merges before the first activation of the personal instance.

This section amends ADR 0020 and ADR 0079 section 1, which say that no upgrade edge exists.

### 5. Hosted PostgreSQL

- **Version.** A new Supabase project has PostgreSQL 17. Local development and each check move to a PostgreSQL 17 image with pgvector, so that the repository proves the version that the instance runs.
- **TLS.** The Storage Activation and the connection pool connect with TLS when the URL says `sslmode=require`. StoryOS verifies the certificate chain and the host name with the certificates of the operating system, and no setting turns the verification off. When the certificate authority of the provider is not in that store, the operator adds it to the store of the VPS. A URL whose host is not a loopback address must say `sslmode=require`; with another value, the binary refuses to start. A local URL stays as it is. This sentence amends the rule of ADR 0039 that `storyos-adapter-postgres` gets no TLS dependency: it gets the TLS dependency of the PostgreSQL connection, and no HTTP and no credential dependency.
- **Activation on a provider database.** The check of an empty database does not count the relations of the provider. The rule names the exact schemas of the provider, and each other relation outside the StoryOS schema still refuses the activation. The role source gives the admin login the membership that it needs when it is not a superuser.
- **Region.** The database project is in the region that is nearest to the VPS.
- **The first User.** The product has no path that makes a User. The operator makes the one User row with the admin connection when the instance is installed.

### 6. The Linux package

`make release-package` keeps its form. A second command builds the same package for Linux `x86_64` in a container with the pinned Rust version. The Web resources and the manifest come from the one build of the host, so each package of one commit has the same manifest digest. The Linux package runs its own `--check-web-root` and `--check` in the container before the command passes.

### 7. The backup

The hosted project is the off-host copy of the data. The Free plan of the provider has no backup that the operator can restore, so the instance makes its own: one logical dump of the StoryOS schema each day, kept on the VPS for 14 days. The operations document has the restore steps, and the deployment ticket does one restore drill into a new local database and starts a Server on it. A dump is a backup of the whole instance, and it is not Project Restore. When the author moves to a paid plan with provider backups, the daily dump stays.

### 8. What stays open

- A login product, a second User, and each form of access for other people.
- The production model for prose. No vendor is selected ([ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md)).
- The hosted recovery acceptance of [ADR 0021](0021-own-release-1-recovery-chain-outside-the-runtime.md), and the controlled-cloud handoff (`PASS-CLOUD`).
- Project Restore. The backup of section 7 is a backup of the whole database.

## Considered options

- An instance on the Mac of the author first. It needs no vendor and no money, and the local profile exists. The author refused it: the author wants the hosted topology at once, and has four days before the first writing day.
- A login in the product. It is the correct answer for a product with other users. For one author it is much more work than a password at the proxy, and ADR 0022 refused the authentication service of the vendor.
- A private network in the place of a public origin. Each device of the author would need a client program, and the Server would still need an `https` origin with a DNS name.
- An instance that stays at one package version until a later decision. Each fix that changes a table would then be out of reach, or the novel would be lost.
- A Project archive as the upgrade path. `exportProjectArchive` exists, and Project Restore does not.

## Consequences

- Stage 12 owns the tickets: the upgrade edge, TLS, the activation on a provider database, PostgreSQL 17, the Linux package, the operations scripts and document, and the deployment with its acceptance run.
- After the first activation of the instance, a ticket that changes the schema adds a forward migration file, and it does not edit `0000_roles.sql`, `0001_release_1_baseline.sql`, or an earlier forward file. The ticket of the upgrade edge writes this rule into [CODING_STANDARDS.md](../../CODING_STANDARDS.md).
- Each package that changes the identity needs one run of `storyos-storage` before the Server starts. The upgrade steps of the operations document say so.
- The author does these steps, and no agent does them: the password of the database, the password of the proxy, and the model key. A guided script asks for each one and writes it to a file with mode 600. No agent reads those files.
- The old Supabase projects and the old deployments on the VPS and on the domain can be removed. The coordinator shows the author the exact list before a removal.
- `docs/operations/local-real-model-route.md` and `docs/operations/production-web.md` change with the tickets. `Todos.md` section 4.4 gets a link to this record.
