# Glossary: Identity, session, and Project

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**User**:
A stable StoryOS principal identified by one durable `UserId`. The Foundation Validation Deployment bootstraps one local User without requiring a login or account-management product, while the same identity contract permits a later StoryOS service to host many isolated Users; credentials, display names, pen names, and billing accounts are not the User's domain identity.
_Avoid_: Operating-system current user, global singleton, login session, pen name, account feature set

**Protected Web Client**:
The exact controlled StoryOS Web application build whose immutable asset set, client-contract identity, security-policy identity, and current Client Session Binding generation place it inside the Release 1 author-command trust boundary. Rendered or imported content, model, Tool, MCP, or App output, browser extensions, third-party scripts, browser-local caches, journals, and projections remain outside that claim, which proves an exact protected-client submission rather than a physical human gesture, trusted display, user presence, or user verification.
_Avoid_: Any browser page, browser as author, trusted human gesture, local editor authority

**Client Session Binding**:
The opaque server-held request-authentication binding established by a trusted local bootstrap or a future identity flow for one server-derived User, exact allowed Host and first-party Origin, current session generation, accepted Protected Web Client contract and security-policy identities, bounded lifetime, and browser session handle. Every state-changing request also consumes a non-reusable anti-forgery nonce bound to that Binding, exact Project Scope, method, command kind, idempotency record, and canonical command digest; the Binding is an authenticated input to Author Command Admission.
_Avoid_: Login session as User identity, client-asserted role, URL access token, reusable command nonce

**Command Challenge**:
The single-use anti-forgery nonce that the Server issues for one exact project command and its pre-domain idempotency record. It binds the Client Session Binding generation, Project Scope, command kind, canonical command digest, and the Challenge Rate Class policy revision. An exact retry returns the same Challenge; a Challenge grants no authority after consumption or expiry.
_Avoid_: Reusable token, author confirmation step, CSRF cookie

**Challenge Rate Class**:
The Server-derived class of a project command kind that selects which rate budget a new Command Challenge uses. Release 1 has an `author_edit` class for Author Edit and Author Undo submissions and a `shared` class for all other project commands; each class has its own versioned policy and counter for each User, Project, and session generation.
_Avoid_: Client-selected class, per-command quota, author-tunable rate limit, one shared writing quota

**Trusted Local Session Bootstrap**:
The packaged Server issues one Client Session Binding for the single configured User when the author opens the printed Protected Web origin, without a login product or cookie injection as the product path. Local names that single-User path, not a loopback-only transport; the printed origin may be the loopback HTTP origin or the Foundation Validation Public Origin.
_Avoid_: Login, account signup, operator cookie injection, test cookie injection as product issuance, multi-user identity picker, treating local as loopback-only, Release 1 Storage Activation, database bootstrap

**Project Author**:
The one User who owns a Project and may exercise its author-only intents, settings, Acceptance, and other creative-authority commands. `Author` names this project-scoped role rather than a second durable person identity; shared ownership, collaborators, ownership transfer, and multi-author editing require a separate later contract.
_Avoid_: Separate AuthorId, display byline, collaborator, global author singleton

**Project**:
A durable novel workspace identified by one `ProjectId` and owned by one User acting as its Project Author. Its identity is independent of filesystem path, database placement, deployment location, or display name; the current Foundation has no shared ownership or ownership transfer.
_Avoid_: Project directory, database, tenant, collaboration workspace

**Project Scope**:
The trusted pair `{ owner_user_id: UserId, project_id: ProjectId }` that identifies one Project and its sole Project Author. Every project-scoped operation, durable record, reference, index entry, cache key, context decision, and disclosure must bind this pair directly or through a currently validated Project reference; caller-supplied fields, a process-global current User, or ProjectId alone never establish access authority.
_Avoid_: Tenant inferred from session, project path, unscoped global ID, client-asserted owner

**Project Ownership Boundary**:
The current Foundation rule that every Project has one exact Project Author and one immutable Project Scope, and every project-scoped durable object remains bound to both identities. StoryOS may serve multiple isolated Users and Projects without changing this contract, while shared project ownership, multi-author collaboration, ownership transfer, and cross-author access require a separate later domain design rather than a singleton-User assumption or an implicit shared workspace.
_Avoid_: Global current user, single-tenant shortcut, shared project, collaboration role model

**Project Isolation**:
The fail-closed StoryOS boundary that prevents Authoritative State, Artifacts, Operational Records, Context Candidates, manifests, caches, indexes, Credential References, Processing Destination Identities, authorized project-use bindings, and destination disclosures from being discovered, joined, retrieved, authorized, or reused across either member of a Project Scope. Every such record and disposable projection must retain the exact `owner_user_id` and `project_id`, and the authoritative persistence boundary independently rejects scope-mismatched reads, writes, references, and joins; caller-side filtering is not a substitute, while missing or ambiguous identity makes the operation ineligible.
_Avoid_: UI-only filtering, global vector namespace, shared prompt cache, caller-supplied project scope
