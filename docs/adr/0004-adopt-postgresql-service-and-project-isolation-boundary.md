---
status: accepted
---

# Adopt a PostgreSQL Service and Project Isolation Boundary

StoryOS uses one deployment-independent service architecture backed by PostgreSQL. The Foundation Validation Deployment runs the StoryOS Server and PostgreSQL locally for one bootstrapped User writing a real novel, while every model and embedding inference uses a configured external API. The same logical service may later run in the cloud for many Users whose Projects remain isolated. Physical location never determines authority: StoryOS-controlled records and enforcement do.

The durable principal is `UserId`. `Author` is the project-scoped role of the one User who owns a Project, not a second person identity. Every project-scoped operation, record, reference, index, cache, context decision, and disclosure binds one trusted `ProjectScope { owner_user_id, project_id }` directly or through a currently validated Project reference. Missing, ambiguous, caller-asserted, or mismatched scope fails closed. A process-global current User, a filesystem path, ProjectId alone, or globally shared retrieval and cache namespaces cannot establish access authority.

This decision does not introduce account management, login UX, billing, teams, shared project ownership, real-time collaboration, or multi-author editing. The local validation deployment may bootstrap its sole User without those product surfaces. Future collaboration, ownership transfer, and membership roles require a separate explicit contract; they cannot be inferred from the ability to deploy the same service for multiple isolated Users.

Current issue bodies and tracked contracts express this boundary directly: PostgreSQL is the authoritative physical database; local and controlled-cloud deployments use the same logical service, Project Scope, migration, and recovery contracts; inference uses configured external Providers through the Context Assembly and disclosure boundary. Bailian may be one configured Provider during testing, but it is neither a kernel dependency nor a permanently selected release Provider.

## Considered options

- One self-contained directory with a per-project SQLite database was rejected as the runtime authority boundary because it conflicts with the chosen PostgreSQL service architecture and encourages storage, indexing, caching, secret, and recovery designs that assume a single local process and User.
- A local-only prototype followed by a later database and identity rewrite was rejected because project ownership, isolation keys, constraints, cache identities, retrieval namespaces, and disclosure evidence are expensive and unsafe to retrofit after real novel data exists.
- A process-global current User was rejected because it would make locally convenient code unsafe when the same service later hosts multiple Users and would leave cross-project retrieval and cache mixing structurally possible.
- Separate durable `UserId` and `AuthorId` identities were rejected for the current Foundation because one Project has one owning User and no collaborator or persona security model; bylines and pen names are project content rather than authorization identities.
- Building accounts, billing, teams, ownership transfer, or collaborative editing now was rejected because those product surfaces are not required to validate one-author Discovery Writing and would prematurely widen the domain.
- A local-model fallback was rejected because the product's model and embedding destinations are external APIs. Sensitive content is excluded, minimized, projected, or blocked under disclosure policy rather than silently routed to an unplanned local inference runtime.

## Consequences

- PostgreSQL is the authoritative physical database from initial development onward; the accepted [PostgreSQL Project Storage, Isolation, and Migration Contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md) owns its schema, transaction isolation, payload layout, migrations, backup, restore, export, and secret-store integration.
- Initial local execution and later controlled cloud deployment must exercise the same domain identities, Project Isolation, Context Assembly, disclosure, recovery, and migration contracts.
- A Project has one `owner_user_id`; ownership transfer and additional membership relations do not exist in the current Foundation.
- Project-scoped commands and external DTOs are authorized from trusted requester context plus exact Project Scope. A client-supplied owner field is evidence to validate, never authority.
- Authoritative State, Artifacts, Operational Records, AgentRuns, Subruns, Messages, Proposals, Receipts, ToolCalls, Memory, indexes, embeddings, caches, manifests, Outbound Disclosure Events, idempotency records, and replay cursors cannot be read, joined, reused, or delivered across Project Scope.
- Globally reusable definitions such as schema versions, ToolSpecs, provider-neutral adapters, and public capability metadata may remain unscoped only when they contain no project-derived data or project authority.
- Model and embedding submissions always cross the StoryOS Controlled Processing Boundary and follow the full Context Assembly and destination-specific Outbound Disclosure contract.
- Local development and tests may use environment-backed inputs, while the Foundation-local deployment resolves Credential References through macOS Keychain and a later controlled-cloud deployment uses a managed secret service through the same backend-neutral resolver contract; credential values never enter project records, ordinary logs, backups, or exports.
- Research assets remain attributable evidence. Current issue bodies and tracked contracts state the executable contract.
