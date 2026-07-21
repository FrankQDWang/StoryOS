---
status: accepted
---

# Adopt a PostgreSQL Service and Project Isolation Boundary

StoryOS uses one deployment-independent service architecture backed by PostgreSQL. The Foundation Validation Deployment runs the StoryOS Server and PostgreSQL locally for one bootstrapped User writing a real novel, while every model and embedding inference uses a configured external API. The same logical service may later run in the cloud for many Users whose Projects remain isolated. Physical location never determines authority: StoryOS-controlled records and enforcement do.

The durable principal is `UserId`. `Author` is the project-scoped role of the one User who owns a Project, not a second person identity. Every project-scoped operation, record, reference, index, cache, context decision, and disclosure binds one trusted `ProjectScope { owner_user_id, project_id }` directly or through a currently validated Project reference. Missing, ambiguous, caller-asserted, or mismatched scope fails closed. A process-global current User, a filesystem path, ProjectId alone, or globally shared retrieval and cache namespaces cannot establish access authority.

This decision does not introduce account management, login UX, billing, teams, shared project ownership, real-time collaboration, or multi-author editing. The local validation deployment may bootstrap its sole User without those product surfaces. Future collaboration, ownership transfer, and membership roles require a separate explicit contract; they cannot be inferred from the ability to deploy the same service for multiple isolated Users.

This ADR supersedes only the incompatible physical-deployment, storage-engine, local-model, and provider-binding clauses in the historical resolutions for [Keep Local Authority and Minimize External Disclosure](https://github.com/FrankQDWang/StoryOS/issues/20), [Store Each Novel as a Self-Contained Project Directory](https://github.com/FrankQDWang/StoryOS/issues/26), [Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48), [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50), and [Specify Subrun Control-Plane, Mailbox, and Observability Semantics](https://github.com/FrankQDWang/StoryOS/issues/63). Their remaining authority, Tool, routing, recovery, and Subrun semantics continue to apply. Bailian is one configured external Provider used during current testing, not a required kernel dependency or permanently selected first-slice Provider.

## Considered options

- One self-contained directory with a per-project SQLite database was rejected as the runtime authority boundary because it conflicts with the chosen PostgreSQL service architecture and encourages storage, indexing, caching, secret, and recovery designs that assume a single local process and User.
- A local-only prototype followed by a later database and identity rewrite was rejected because project ownership, isolation keys, constraints, cache identities, retrieval namespaces, and disclosure evidence are expensive and unsafe to retrofit after real novel data exists.
- A process-global current User was rejected because it would make locally convenient code unsafe when the same service later hosts multiple Users and would leave cross-project retrieval and cache mixing structurally possible.
- Separate durable `UserId` and `AuthorId` identities were rejected for the current Foundation because one Project has one owning User and no collaborator or persona security model; bylines and pen names are project content rather than authorization identities.
- Building accounts, billing, teams, ownership transfer, or collaborative editing now was rejected because those product surfaces are not required to validate one-author Discovery Writing and would prematurely widen the domain.
- A local-model fallback was rejected because the product's model and embedding destinations are external APIs. Sensitive content is excluded, minimized, projected, or blocked under disclosure policy rather than silently routed to an unplanned local inference runtime.

## Consequences

- PostgreSQL is the authoritative physical database from initial development onward; [the storage and migration ticket](https://github.com/FrankQDWang/StoryOS/issues/56) owns its schema, transaction isolation, separately stored payload policy if any, migrations, backup, restore, export, and secret-store integration.
- Initial local execution and later controlled cloud deployment must exercise the same domain identities, Project Isolation, Context Assembly, disclosure, recovery, and migration contracts.
- A Project has one `owner_user_id`; ownership transfer and additional membership relations do not exist in the current Foundation.
- Project-scoped commands and external DTOs are authorized from trusted requester context plus exact Project Scope. A client-supplied owner field is evidence to validate, never authority.
- Authoritative State, Artifacts, Operational Records, AgentRuns, Subruns, Messages, Proposals, Receipts, ToolCalls, Memory, indexes, embeddings, caches, manifests, Outbound Disclosure Events, idempotency records, and replay cursors cannot be read, joined, reused, or delivered across Project Scope.
- Globally reusable definitions such as schema versions, ToolSpecs, provider-neutral adapters, and public capability metadata may remain unscoped only when they contain no project-derived data or project authority.
- Model and embedding submissions always cross the StoryOS Controlled Processing Boundary and follow the full Context Assembly and destination-specific Outbound Disclosure contract.
- Local development may use environment-backed credentials or another development secret mechanism, but an operating-system keychain is not a portable service invariant and credential values never enter project records.
- Existing historical research remains evidence of its time. Current normative files and explicit supersession comments identify the accepted contract without rewriting that evidence.
