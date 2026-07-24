# StoryOS Service, Client, and External Trust Boundaries Threat Model

- Status: accepted
- Wayfinder resolution: [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57)
- Repository baseline: `15a6c4d343145c9f825d52067f13d09eec6ead37`
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Protected-client decision: [ADR 0013](../adr/0013-trust-the-storyos-web-client-for-author-command-admission.md)
- Author-command admission: [Author Command Admission](author-command-admission.md)
- Evidence classification: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Web editor continuity: [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md)
- Public protocol: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Security verification: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md)
- Storage contract: [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md)
- Context and egress contract: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- MCP App lifecycle: [ADR 0002](../adr/0002-specify-transcript-and-mcp-app-lifecycle-semantics.md)
- Deployment and isolation boundary: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)

# Overview

## 1. Purpose and evidence discipline

This document is the repository-scoped Foundation threat model for StoryOS. It
identifies only attack paths that are reachable through the accepted service,
Client, database, extension, external-processing, import/export, and recovery
architecture and that can violate a fixed StoryOS invariant. It is not a
generic web-security checklist and does not authorize production Rust, SQL,
frontend, account, deployment, or security implementation.

Statements use three evidence classes:

- **StoryOS fixed fact** is already accepted in this repository and is not
  reopened here.
- **Source fact** is a behavior or requirement stated by an official
  specification, platform, or provider source.
- **Threat-model conclusion** is the structural mitigation or verification
  obligation derived from a reachable StoryOS path.

Provider policy statements are dated external evidence, not StoryOS execution
guarantees. A no-training, zero-retention, encryption, or compliance statement
never means that no Outbound Disclosure occurred and never replaces
StoryOS-owned admission, minimization, Attempt, or disclosure evidence.

## 2. Product and deployment outline

StoryOS supports Dean Koontz-style Discovery Writing: the author develops the
novel through the passage and choices currently before them. The system does
not introduce an Agent-authored outline, Author Plan, or preplanned story
structure. One stable User owns each Project and acts as its sole Project
Author. Authoritative State changes only through a narrow Direct Author Action
or an inspectable Core Proposal followed by explicit author Acceptance.

The Foundation Validation Deployment runs one StoryOS Server and PostgreSQL
locally for one bootstrapped User. The same logical service may later run in a
controlled cloud deployment for many mutually isolated Users. Model and
embedding inference always use external APIs. Bailian is one current test
Provider, not an architectural dependency.

Release 1 has one Protected Web Client: the exact controlled StoryOS Web
application build named by its immutable asset set, accepted client-contract
identity, security-policy identity, and current Client Session Binding
generation. Only that controlled application code and its admitted browser
platform boundary participate in the author-command trust claim. Rendered or
imported content, model, Tool, MCP, or App output, browser extensions,
third-party scripts, stale application/service-worker caches, Local Edit
Journal content, and browser projections remain untrusted inputs or
non-authoritative continuity state.

PostgreSQL is the authoritative database. A Project is never identified by a
directory, database file, or filesystem path. Project Export Archives and
author-provided files are untrusted inputs or portable evidence, not alternate
authority stores. There is no per-Project database, SQLite, Neo4j, standalone
vector database, message broker, microservice split, whole-system Event
Sourcing, arbitrary shell, or unrestricted filesystem access.

## 3. Security objectives

The Foundation must preserve these security objectives:

1. **Exact Project Isolation.** Every project-bearing operation, record,
   reference, cursor, index, cache, idempotency fact, recovery fact, and
   disclosure binds the exact pair { owner_user_id, project_id }.
2. **Bounded protected-client claim.** Author Command Admission may assert only
   that the exact Protected Web Client submitted one exact digest-covered
   command for the Server-derived User and existing or prospective Project
   Scope while every protected-client, session, contract, editor, writer,
   action, nonce/idempotency, target/Head, and lifetime input matched. It never
   attests one physical human gesture, trusted display, user presence, or user
   verification.
3. **Durable truth.** PostgreSQL canonical facts, immutable payloads, exact
   Revisions, Receipts, manifests, Attempts, and uncertainty records outrank
   network, browser, process, cache, projection, and Provider state.
4. **Minimum-necessary external processing.** Every model, embedding, Tool,
   MCP, research, telemetry, or nested external call receives only an exact,
   currently eligible, purpose-bound Projection after manifest-before-egress.
5. **Secret confinement.** Only the narrow execution boundary resolves a
   Credential Reference. Secret values and value digests never enter ordinary
   records, Tool arguments or outputs, model context, MCP Apps, logs, backups,
   support material, or Project exports.
6. **Replay-safe recovery.** A crash, reconnect, retry, lease expiry, duplicate
   delivery, or stale worker cannot invent success, erase OutcomeUnknown,
   repeat authority, or settle with an obsolete fence.
7. **Inspectable evidence.** Historical source, context, authorization,
   action, disclosure, and recovery evidence remains attributable and cannot
   be silently rewritten by compaction, projection rebuild, later policy, or
   UI replay.
8. **Bounded execution.** Untrusted resources, messages, files, archives,
   fetched content, model output, and replay ranges cannot grow without
   explicit hard limits and backpressure.
9. **Protected-client integrity.** Untrusted content, external output,
   dependency or asset substitution, third-party script, browser extension,
   stale tab, or stale cached build cannot silently become the admitted
   Protected Web Client or widen its current command authority.

# Threat Model, Trust Boundaries, and Assumptions

## 4. Assets

| Asset | Security property that matters |
|---|---|
| Authoritative State and exact Revisions | only author-authorized Core transitions change it; history is immutable and scope-bound |
| Artifacts, Proposals, Receipts, and Provenance | exact identity, source lineage, integrity, lifecycle, and no implicit authority |
| AgentRun, Subrun, Transcript, Mailbox, and recovery facts | durable ordering, idempotency, fencing, uncertainty, and replay correctness |
| Project Scope and requester identity | no cross-User or cross-Project discovery, existence oracle, join, reuse, delivery, or disclosure |
| Context and disclosure evidence | seven ordered gates, exact source closure, minimum disclosure, and manifest-before-egress |
| Credential values and resolver authority | never model-, Client-, Tool-, App-, database-payload-, log-, backup-, or export-visible |
| PostgreSQL schema, roles, constraints, and migration ledger | least privilege, forced RLS, same-scope references, checksum and compatibility integrity |
| Project Export Archives, backups, and WAL | confidentiality, integrity, complete recovery chain, exact-scope restore, and no secret material |
| App UI resources and bridge state | exact resource identity, sandbox integrity, origin/source binding, revocation, and no ambient authority |
| Protected Web Client build and admission context | exact immutable asset/build identity, matching client-contract and security-policy identities, current Client Session and writer generations, and no false physical-human attestation |
| Admission and editor evidence | exact positive class, owning lifecycle, source attribution, immutable settlement, and no promotion of an Operational Record, Draft, journal, cache, projection, or missing response into authority |
| Availability and resource budgets | one malicious input cannot exhaust the host, critical recovery capacity, or another Project |

## 5. Principals and attacker capabilities

| Principal or attacker | Capabilities assumed in scope |
|---|---|
| Project Author/User | supplies prose, instructions, URLs, files, settings, Approvals, and Acceptance; may make mistakes but owns only exact authorized Projects |
| Other User in a future cloud deployment | has a valid principal and their own Projects; may guess IDs, forge scope fields, race requests, and probe errors or cursors |
| Untrusted web origin or page script | can cause browser requests allowed by the web platform, open EventSource connections where policy permits, and send postMessage traffic to reachable windows |
| Protected Web Client | runs the exact controlled StoryOS application build admitted by matching asset, client-contract, security-policy, Client Session, Editor Session, and writer-generation evidence; it requests but never creates or supplies Author Command Admission identity |
| Untrusted browser-side influence | controls rendered/imported content, model/Tool/MCP/App output, third-party script or dependency input, browser extensions, cached or stale client state, and DOM/bridge messages without gaining a trusted-client identity |
| Malicious model output or Provider response | can emit persuasive text, malformed streams, forged Tool requests, oversized output, or misleading identity/usage evidence |
| Malicious Tool, MCP server, or MCP App | controls discovered metadata, schemas, results, HTML, bridge messages, redirects, and declared annotations within its reachable Registration |
| Malicious research source or author-provided file | controls content, imperative text, URLs, parser inputs, archive entries, compression ratio, and embedded metadata |
| Duplicate or stale worker | retains old process state, lease token, queued work, sockets, or late external responses across recovery |
| Network attacker | may observe or alter traffic where transport protection is absent; cannot break correctly configured TLS |
| Compromised maintenance credential | may have migration, backup, restore, or broad database visibility according to that role; runtime paths must not possess it |

## 6. Explicit assumptions and exclusions

The model does not claim to prevent a fully privileged host administrator,
kernel compromise, PostgreSQL superuser, malicious controlled-cloud control
plane, or compromised secret-service operator from reading or changing
everything inside that administrative boundary. Role separation, independent
recovery evidence, integrity validation, and audit reduce exposure and improve
detection; they are not Byzantine protection against the platform owner.

The Foundation also excludes a complete account/login product, billing, teams,
ownership transfer, shared ownership, real-time collaboration, multi-author
editing, production cloud operations, automated failover, and Internet-scale
denial-of-service engineering. Those exclusions do not weaken the exact User
and Project boundary. The protocol must have a trusted requester binding in
both deployments even though the local deployment bootstraps its one User
without login UX.

Provider-internal training, retention, logging, hidden cache, subprocessors,
and model attention remain outside StoryOS durable truth. Browser or sandbox
zero-days, OS credential theft after full host compromise, and cryptographic
primitive failure are residual platform risks rather than Foundation features.

A pure browser claim is intentionally narrower than physical-human
attestation. Browser-observed input, user activation, a valid session,
anti-forgery evidence, a command digest, or even a credential ceremony does
not by itself prove what one physical person saw or meant. A stronger claim
requires a separately trusted display and confirmation surface that is outside
the selected Release 1 boundary.

## 7. Trust boundaries and data flows

| Boundary | Data crossing it | Required invariant |
|---|---|---|
| TB-0 Build/release → Protected Web Client | immutable first-party assets, dependency graph, client-contract and security-policy identities, service-worker/application-cache activation | only one exact reviewed asset graph is executable as the admitted client; stale, substituted, third-party, or mismatched code cannot inherit the protected-client claim |
| TB-1 Client ↔ StoryOS Server HTTP | commands, queries, files, protected-client and Author Command Admission inputs, Approvals, Acceptance | Server derives requester and scope, validates every exact protected-client/session/editor/digest/lifetime input, and creates the durable admission identity for the exact command |
| TB-2 StoryOS Server → Client SSE | scoped immutable events, cursors, replay and resync signals | current authorization plus exact stream/scope/sequence binding on every connect and replay |
| TB-3 Server/runtime ↔ PostgreSQL | canonical facts, payloads, scope settings, projections, outbox | non-owner runtime, forced RLS, composite scope constraints, atomic transitions |
| TB-4 Maintenance ↔ PostgreSQL | migrations, whole-service backup/restore, role/grant manifests | separate non-request credentials, isolated execution, checksum and restore validation |
| TB-5 Server/worker ↔ Credential Resolver | opaque reference, ephemeral resolved value, availability | resolve only after admitted operation; value reaches only the exact transport boundary |
| TB-6 Model/embedding destination | minimum context, wire projection, ephemeral credential, returned output | exact destination identity, grant, manifest, Attempt, Disclosure Event, and no cross-scope batch/cache |
| TB-7 Tool Gateway ↔ Tool/MCP server | declared inputs, effects, credentials, results, nested destinations | exact Registration and ToolSpec, non-escalating grant, Approval, effect evidence, contract-drift fence |
| TB-8 MCP App iframe ↔ Host bridge | UI resource, instance negotiation, presentation signals, App Action Requests | cross-origin sandbox, exact instance/source/origin, schema and capability mediation, no auto-forward |
| TB-9 Research fetcher ↔ network | generated or supplied URL/query, redirects, response bytes | public-network-only policy, no ambient credentials, bounded capture and exact provenance |
| TB-10 Import/export ↔ author-provided bytes | Project Export Archive or imported source | stage and validate without authority, path, parser, secret, or scope escape |
| TB-11 PostgreSQL ↔ backup/WAL store | whole-service canonical data and recovery metadata | independent restricted failure domain, confidentiality, integrity, gap detection, restore proof |
| TB-12 Durable store ↔ journals/projections/caches/UI | journaled, indexed, summarized, replayed, cached, or displayed views | current scope, generation, eligibility, and exact dependency closure; browser state and projections never become truth, authorization, settlement, or a recovery oracle |

### 7.1 Exact Release 1 protected-client trust inputs

The threat model consumes, but does not redefine, the admission fields owned by
[Author Command Admission](author-command-admission.md). The trusted claim is
eligible only when all of these inputs are present and equal at the applicable
boundary:

| Input | Threat-model requirement |
|---|---|
| exact Protected Web Client build | the immutable reviewed asset graph named by the accepted client-contract and security-policy identities; a different or stale build is not trusted |
| Client Session Binding | exact opaque Server-held binding identity, current `session_generation`, allowed Host and first-party Origin, Server-derived User, and bounded session lifetime |
| client and policy identities | exact accepted `client_contract_revision` and `security_policy_revision`; neither the Client nor a cache may select, downgrade, or alias them |
| requester and Project | Server-derived `requester_user_id` plus one exact existing or Server-allocated prospective `ProjectScope { owner_user_id, project_id }` |
| editor binding | exact `editor_session_id` and Project `writer_generation` when editor-bound, or explicit not-applicable values |
| action and request contract | one closed author action class plus API major, method, route template, command schema, and command kind |
| command meaning | Server-recomputed canonical command digest and Digest Profile covering exact targets, expected Heads or Revisions, and every typed command field |
| replay protection | exact pre-domain idempotency-record identity and key plus the consumed one-use anti-forgery nonce-record identity; a nonce value is never durable authority |
| lifetime and invocation | Server-clock `issued_at` and `expires_at`, with first invocation admitted only within the owned lifetime and recovery limited to the contract's exact direct-edit branch |
| settlement | one append-only Admission lifecycle with read-only reconciliation and exactly one `ReceiptSettled` or `RequiresReconfirmation` terminal settlement |

Release 1 therefore claims only: the exact Protected Web Client submitted this
digest-covered command for this Server-derived User and Project Scope, and the
Server admitted that one command for Core evaluation. It makes no claim about
browser-event cardinality, physical-human identity, user presence, user
verification, or a trusted display of command semantics.

### 7.2 Security-relevant evidence classes are fixed inputs

This threat model assesses spoofing, tampering, replay, disclosure, recovery,
and availability attacks against the positive classes below. It does not
create another class or lifecycle:

| Evidence or content | Fixed classification consumed here |
|---|---|
| Admission issuance, `outcome_unknown`, reconciliation observations, terminal settlement, sanitized pre-admission refusal, Editor Input Fence, Author Action, and every typed Receipt | Operational Records under their owning contracts |
| `RefusedEditDraft` and `RecoveryDraft` | non-authoritative Draft Artifacts with Artifact revisions, retention, and reversible Draft closure |
| Proposal Conflict | the `conflicted` condition on one exact Proposal Revision's validation axis, projected from immutable Core evidence |

An identity or evidence record is not reusable authority. Browser cache,
Local Edit Journal, Pending Edit Projection, process state, network state,
missing response, Artifact, Operational Record, external output, or repeated
observation never becomes Authoritative State merely because it exists.

## 8. Entry points

The reachable entry points are versioned HTTP command/query endpoints, the SSE
endpoint and reconnect cursor, file/import endpoints, Project Export/Restore
maintenance commands, model and embedding responses, Tool and MCP discovery
and results, MCP App resources and bridge messages, research URLs and fetched
bytes, secret-rebind commands, migration/backup/restore tooling, outbox and
wakeup delivery, worker lease recovery, and every disposable cache/index
rebuild reader.

# Attack Surface, Mitigations, and Attacker Stories

## 9. AP-01: Cross-User or cross-Project object substitution

**Source to sink.** Another User, a compromised Client, App Action, Tool
argument, opaque object ID, cursor, or queued item supplies one valid identity
from another Project or substitutes only owner_user_id or project_id. If the
Server authorizes from the body, ID uniqueness, parentage, or a process-global
User, the value can reach a canonical query, cache/index hit, outbox delivery,
SSE replay, restore, or external disclosure under the wrong Project.

**Affected assets.** Project Isolation, Authoritative State, project content,
credentials, context, disclosure history, and the existence of otherwise
private objects.

**Accepted controls.** Project Scope is the exact pair
{ owner_user_id, project_id }; every project-bearing row and reference repeats
it; composite foreign keys and forced RLS independently reject mismatches;
cache, retrieval, idempotency, outbox, restore, and disclosure facts remain
scoped. [S1] [S4]

**Required structural mitigation.** Every public command, query, cursor,
bridge request, delivery, and recovery lookup must begin from a trusted
requester binding and require an exact matching scope before object lookup.
Client scope fields may be compared but never establish access. Mismatch and
not-found behavior must not become a cross-scope existence oracle. No cache,
batch, queue, global Provider handle, or parent/child identity may omit either
scope member.

**Verifiable evidence.** Exhaustive pairwise tests substitute each scope
member across HTTP, SQL, SSE, Tool/MCP, App, cache, embedding, outbox,
idempotency, import/restore, and projection rebuild paths and prove refusal
without content- or existence-bearing differences.

**Residual risk and owner.** A compromised runtime or database-owner boundary
can still defeat application isolation. Exact envelopes and errors belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
adversarial proof belongs to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60);
the minimum enforcement slice belongs to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 10. AP-02: RLS, role, migration, or backup authority bypass

**Source to sink.** A request reaches a connection using the table owner,
superuser, BYPASSRLS, migrator, backup, or restore authority; inherits a role
that can SET ROLE; invokes unsafe security-definer SQL; or uses whole-table
operations. PostgreSQL documents that superusers and BYPASSRLS roles bypass
RLS, table owners normally bypass unless FORCE ROW LEVEL SECURITY is active,
and TRUNCATE and referential-integrity checks are not governed by RLS. [P1]
An overpowered runtime can therefore read or destroy all Projects even when
policy expressions look correct. In a networked deployment, a permissive HBA
rule or libpq connection that does not verify the server hostname can also
send the runtime credential and data to the wrong database endpoint. [P6]

**Affected assets.** All project data, schema integrity, migration history,
backup contents, and availability.

**Accepted controls.** The owner is NOLOGIN; runtime is non-owner,
non-superuser, NOBYPASSRLS; RLS is enabled and forced; trusted scope is
transaction-local; migration, backup, and restore use separate roles absent
from the request pool; same-scope constraints remain independent of RLS. [S4]

**Required structural mitigation.** Runtime must have no owner/migrator/
backup membership, DDL, TRUNCATE, role-management, or maintenance entry point.
Every database function reachable by runtime uses explicit ownership and
qualified object names and cannot widen scope through caller-controlled
search_path or dynamic SQL. Maintenance credentials and pools are separately
provisioned, audited, rotated, and unavailable to request handlers. Restore
runs with traffic disabled and validates the complete role/grant manifest
before enabling runtime. Local validation uses an exact controlled socket or
loopback/HBA rule; a cloud database connection requires TLS with hostname
verification, an exact database identity, and no `trust` authentication.

**Verifiable evidence.** A release gate inspects effective role attributes,
memberships, grants, table ownership, forced-RLS posture, function definitions,
and pooled transaction-local scope; direct SQL probes show that missing,
partial, stale, and cross-scope settings match no row and that runtime cannot
invoke maintenance operations. Wrong certificate, hostname, source network,
database, and role connections fail before any scoped query.

**Residual risk and owner.** A stolen maintenance or platform-admin credential
retains its intended broad blast radius. Protocol exposes no such route;
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60)
owns posture and isolation gates, and
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62)
must include role separation before validation data is trusted.

## 11. AP-03: Origin, Host, or CSRF request forgery

**Source to sink.** A malicious web origin induces a browser to send a simple
GET or form-like POST to a local StoryOS port, exploits permissive CORS,
forges Host or Origin, or reuses an ambient cookie/token. Browsers can send
simple cross-origin requests without preflight, so
CORS response blocking is not a CSRF defense. Credentialed CORS also requires
an explicit origin rather than a wildcard. [W1] A forged request could create
a Run, Approval, Acceptance, credential rebind, export, or other effect.

**Affected assets.** Author authority, Project data, outbound grants,
credentials, and external effects.

**Accepted controls.** Release 1 trusts only the exact Protected Web Client
defined in section 7.1.
The Server creates a command-specific Author Command Admission only after
deriving requester identity and exact existing or Server-allocated prospective
Project Scope and validating the exact Protected Web Client build,
client-contract and security-policy identities, Client Session Binding and
generation, applicable Editor Session and writer generation, action class,
canonical command digest, target and expected Heads, one-use nonce record,
idempotency record, and admission lifetime. Approval and Acceptance bind exact
immutable inputs.

**Required structural mitigation.** The protocol must define one concrete
request-authentication and anti-forgery binding for both the trusted local
Client launch and future cloud sessions without requiring account UX now.
Every state-changing request must use an exact versioned non-simple command,
reject absent or disallowed Origin and Host at the Server boundary, and bind
its anti-forgery/session evidence to the trusted User, Project Scope, method,
and command digest. CORS is deny-by-default with an exact first-party origin;
no wildcard credential policy, URL bearer credential, GET mutation, or
loopback exception is allowed. Cloud transport requires TLS. Local network
binding and browser private-network behavior are defense in depth, not
authorization.

The deployed client uses exact immutable assets, a restrictive content security
policy, no ambient third-party script, and version-matched generated contracts.
IndexedDB journal content, browser extension input, restored local state, and
all client-supplied identities are still structurally validated at the Server
and Core boundaries. A stale build, session or writer generation, mismatched
contract or policy identity, reused nonce, changed idempotency digest, or
expired admission cannot create or invoke an admission.

**Verifiable evidence.** Browser integration tests originate requests from
malicious HTTPS and local pages, forms, fetch modes, null Origin, forged Host,
stale tokens, another Project, and preflight variants; all mutations and
sensitive reads fail before a domain attempt or disclosure.

**Residual risk and owner.** Integrity of the exact Protected Web Client is
part of the Release 1 trusted computing boundary, but a valid protected-client
submission does not prove a physical human gesture or trusted display.
Controlled assets, exact auth, Origin, Host, CSRF, command, and error contracts
belong to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
`DVG-02`, `DVG-03`, and `DVG-11` own hostile-origin, binding-substitution, and
zero-effect evidence.

## 12. AP-04: SSE replay, cursor confusion, or stale authorization

**Source to sink.** EventSource reconnects using the last event ID supplied by
the stream; the HTML standard describes Last-Event-ID as a string returned to
the Server on reconnection. [W2] If StoryOS treats that value as a global row
offset, object capability, or trusted aggregate selector, a Client can replay
another Project, skip security-relevant events, request an unbounded backlog,
or retain a stream after permission or Project state changes.

**Affected assets.** Project confidentiality, Run/Transcript inspection,
approval and recovery UI correctness, and availability. SSE is never allowed
to be command or authority truth.

**Accepted controls.** HTTP owns commands; SSE projects durable,
monotonically sequenced events; Last-Event-ID only resumes from persistent
history; the connection is not Run truth.

**Required structural mitigation.** The versioned cursor must bind the exact
stream kind, Project Scope, root/aggregate identity, sequence profile, and
retention generation. The Server reauthorizes every connection and replay
range, validates the cursor against that exact stream, returns a typed resync
or cursor-expired outcome when history is unavailable, and hard-bounds each
page and backlog. Duplicate delivery is harmless and Client projections dedupe
by the scoped durable event identity. Event data, event names, and cursor
values never create Author Command Admission, resolve a Wait, or select another Project.

**Verifiable evidence.** Tests use arbitrary UTF-8 cursor strings, another
Project cursor, forged aggregate IDs, old retention generations, duplicates,
gaps, reconnect after revocation, concurrent windows, and very large lag.
Authoritative and Operational Records remain unchanged while the UI either
replays the correct sequence or performs an explicit scoped resync.

**Residual risk and owner.** A Client may render stale information while
offline, but it cannot commit stale authority. Cursor/envelope/error semantics
belong to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
cursor retention and compaction belong to
[Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md);
replay tests belong to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).

## 13. AP-05: MCP App bridge spoofing or iframe authority escalation

**Source to sink.** Malicious App HTML, a sibling frame, a replaced resource,
or a forged postMessage sends a plausible JSON-RPC method, reuses a request ID,
selects another MCP server, or exploits an SDK auto-forwarder. Any window in
an iframe hierarchy may send messages; receivers must verify origin and source,
senders should use an exact targetOrigin, and message syntax still requires
validation. [W3] A weak bridge can call Tools, read resources, update model
context, impersonate author speech, or create an alternate authoritative path.

**Affected assets.** Author authority, Tool capability, Project data,
credentials, transcript integrity, model context, and browser security.

**Accepted controls.** Stable MCP Apps use a different-origin sandbox proxy,
restricted iframe, CSP, initialization ordering, and mediated bridge. StoryOS
binds exact immutable UI Resource Revisions, one-shot execution admission,
Instance negotiation, App Action Requests, static fallback, and generation-
fenced revocation. [M4] [S2]

**Required structural mitigation.** Validate expected proxy/App origin,
source window, Instance identity, Resource Revision and digest, eligibility
generation, method, request ID, nonce or equivalent channel binding, payload
schema, size, rate, and initialization state at every relay. Use exact
targetOrigin, never wildcard delivery. Do not give the SDK bridge a raw MCP
client. Persist every semantic App Action Request before routing it through a
new typed Host command or root AgentRun with fresh scope, grant, budget, and
Approval. An App request is never author speech or Direct Author Action.
Revocation terminates Instances and stale bridge generations immediately;
historical replay uses stored inert bytes only after fresh admission or a
trusted static fallback.

**Verifiable evidence.** The matrix covers sibling-frame and wrong-origin
injection, replaced window, duplicate ID with changed payload, pre-init call,
oversized/flooded messages, stale generation, cross-server call, missing
capability, CSP/permission request, teardown race, and replay after resource
revocation. No denied request reaches Tool Gateway or model context.

**Residual risk and owner.** A browser sandbox escape is a platform compromise;
the App still holds no credential, host cookie, direct database path, or
ambient Project context. Bridge DTOs and reason codes belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
the adversarial host matrix belongs to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).

## 14. AP-06: Tool/MCP confused deputy, contract drift, or token misuse

**Source to sink.** A malicious MCP server lies in annotations, changes its
schema under a stable name, requests a cross-server Tool, tricks a Host proxy
into using prior consent, accepts a token for the wrong audience, or forwards
the Client token downstream. MCP security guidance identifies confused-deputy
risk and forbids token passthrough; authorization requires tokens intended for
the MCP server and a separate upstream token. [M1] [M2] Tool annotations from
untrusted servers are not enforcement. [M3]

**Affected assets.** Capabilities, external accounts, Project data,
credentials, disclosure grants, audit attribution, and external effects.

**Accepted controls.** Discovery, Registration, project enablement, Exposure,
Capability, Approval, and ToolCall are separate. A Host-owned ToolSpec and
effect envelope are pinned to an exact Registration; drift quarantines it.
Every StoryOS-dispatched call crosses one Tool Gateway under the intersection
of policy, Run grant, and exact effect request.

**Required structural mitigation.** Bind the MCP server and connection trust
identity, callable contract digests, adapter, destination, credential
reference, and allowed caller routes. Do not transfer grants by name equality.
Same-server App visibility is only eligibility. MCP HTTP authorization must
validate audience/resource and use distinct downstream credentials; local
servers default to stdio or authenticated restricted IPC and least OS/network
privilege. Host credentials are injected after admission and are never
available to the model, App, generated program, or arguments. Every nested
external call is a separate destination operation; no controlled adapter may
be an unrecorded egress proxy.

**Verifiable evidence.** Tests mutate each contract field and annotation,
reuse names across servers, substitute token audience, attempt token
passthrough, invoke a hidden or app-only Tool, request undeclared effects,
redirect nested egress, and return invalid/oversized results. Exposure clears
or the call fails closed before execution.

**Residual risk and owner.** An external service can misuse data that was
legitimately disclosed within an exact grant; StoryOS records but cannot
control that destination. Wire Tool/MCP contracts belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
contract-drift and deputy tests belong to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).

## 15. AP-07: Prompt or content injection crosses an authority boundary

**Source to sink.** A research page, imported file, manuscript passage,
author-owned outline, MCP resource, Tool result, App context contribution, or
model output contains imperative text telling the Agent to ignore policy,
retrieve another Project, disclose prose, invoke a Tool, change a credential,
or write story structure. If content position, signature, ownership,
repetition, or execution trust grants Instruction Authority, the injection can
reach Tool Gateway, a Provider, or Authoritative State.

**Affected assets.** Author authority, Project Isolation, context,
credentials, disclosures, and creative state.

**Accepted controls.** These sources are Data-only Context. Instruction
Authority is a closed independent trust axis. Eligibility precedes ranking;
model output and Tool requests are provisional until the Host validates and
persists an Agent Decision; Tools and Apps cannot mutate Authoritative State;
generated changes remain Proposal-gated.

**Required structural mitigation.** Preserve source attribution and trust axes
through retrieval, excerpts, summaries, model context, and Tool results.
Never concatenate untrusted content into Host control instructions or promote
it through delimiters, signatures, ownership, or a trusted transport. The Host
validates the complete model decision and Tool-request batch against the exact
Step Snapshot, Tool Exposure, schemas, scope, grants, and effects; one invalid
member rejects the batch. Non-model destinations receive no Ambient Context.
No output can create an Author Command Admission, Approval, Acceptance,
Credential Reference selection, or destination grant.

**Verifiable evidence.** A versioned adversarial corpus places equivalent
instructions in every source class, encoding, nested summary, Tool field,
App contribution, and retrieval rank. Tests prove the data may influence an
ordinary draft but cannot widen discovery, cross scope, select credentials,
cause unapproved egress/effects, or bypass Proposal/Acceptance.

**Residual risk and owner.** A model may still produce poor or misleading
creative output from eligible malicious data; author inspection and evidence
quality address that product risk. Hard security effects remain Host-gated.
Exact input/output discriminants belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
the injection matrix belongs to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).

## 16. AP-08: Research fetching becomes SSRF or ambient credential egress

**Source to sink.** An author, model, imported source, Tool result, MCP server,
App resource, or redirect controls a URL. A StoryOS fetcher then reaches
loopback, PostgreSQL, the local StoryOS Server, private cloud services,
link-local metadata, another protocol, or a credential-bearing origin. OWASP
describes SSRF as abuse of a server to reach its own or internal network and
calls out redirects, localhost/private/link-local ranges, multiple A/AAAA
answers, and DNS pinning. [N1]

**Affected assets.** Credential values, database and host services, cloud
metadata, Project data, network authority, and availability.

**Accepted controls.** Research is a Tool/external-read operation with exact
Purpose, Project Scope, Destination Intake Contract, Capability, disclosure,
Attempt, and provenance. A controlled fetcher downstream call is a separate
External Processing Destination, not Host-internal work.

**Required structural mitigation.** Use a dedicated fetch path through Tool
Gateway. Allow only HTTP/HTTPS, validate the parsed URL, resolve every A and
AAAA answer, block loopback, unspecified, private, link-local, multicast,
special-use, and deployment metadata destinations, and repeat validation
after every redirect and connection resolution. Disable implicit redirects or
admit each hop under the same rules. Never forward browser cookies,
Authorization, Credential References, Client headers, or ambient network
identity. Apply method, port, DNS, connect, response-time, byte,
decompression, MIME, parser, and redirect bounds and capture the final URL,
address class, redirect chain, headers needed for provenance, content digest,
and truncation/failure evidence. Network isolation should make internal
destinations unreachable even if validation fails.

**Verifiable evidence.** Tests cover IPv4/IPv6 textual variants, localhost,
private/link-local/metadata addresses, userinfo, mixed schemes, DNS rebinding,
multiple answers, public-to-private redirects, redirect loops, file/gopher/
data schemes, decompression bombs, slow responses, oversized bodies, MIME
mismatch, and credential/header reflection.

**Residual risk and owner.** A public endpoint may itself perform harmful GET
side effects or return parser exploits; the fetcher remains credential-free,
read-only, bounded, and its captured bytes stay untrusted. URL/redirect/result
contracts belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
SSRF and parser tests belong to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60);
the isolated minimal fetcher belongs to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 17. AP-09: Provider or embedding disclosure escapes its admitted manifest

**Source to sink.** A routing default, adapter revision, fallback, hidden retry,
cross-Project batch, attachment helper, embedding cache, or Provider feature
adds content or changes destination after admission. Project prose, research,
identifiers, or another Project's vector input then reaches an external API
without a matching manifest. Provider retention and training behavior varies
by provider, endpoint, feature, account, region, and time; Provider statements
cannot make an unrecorded disclosure safe. [E1] [E2]

**Affected assets.** Project confidentiality, Project Isolation, author intent,
disclosure provenance, Credential confinement, and accurate Attempt history.

**Accepted controls.** The seven Context Assembly gates finish before egress;
the immutable manifest records destination, wire projection, purpose, grant,
source closure, adapter revision, credential reference, and budgets; an
Attempt is durable before sending. Model and embedding results are
non-authoritative and Bailian is only a test Provider. [S5]

**Required structural mitigation.** Admission binds the exact provider,
endpoint, account/region when relevant, feature set, adapter revision, wire
bytes or canonical digest, and one Project Scope. Fallback, retry, batching,
tool-mediated nested egress, and destination changes require a new admission
decision and Attempt. Embedding/index/cache keys include exact scope, source
revision, eligibility policy, provider/model and projection revision. Provider
credentials are resolved only for the admitted destination.

**Verifiable evidence.** Adapter contract tests capture the actual request and
compare its destination, headers, fields, attachments, and canonical payload
to the admitted manifest. Tests change route, feature, adapter, Project,
embedding source, batch composition, and retry point and prove fail-closed
behavior or a distinct recorded Attempt.

**Residual risk and owner.** StoryOS cannot prove a Provider's internal
retention, training, breach response, or legal disclosure; it can prove what
it sent, where, why, and what outcome it observed. Wire and retry contracts
belong to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58),
retention evidence to
[Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md),
adapter proofs to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60),
and the first admitted Provider path to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 18. AP-10: Credential Reference resolution leaks or crosses authority

**Source to sink.** A Client, model, Tool, App, imported record, old Attempt,
or wrong Project supplies a Credential Reference. A resolver that treats the
opaque reference as sufficient, returns the value to a caller, resolves after
revocation, or keys only by provider can expose one User's secret or use it for
an unapproved destination. Serialization, exception, debug, or digest paths
can then persist the value into PostgreSQL, logs, events, exports, backups, or
model context.

**Affected assets.** External accounts, all data reachable with their tokens,
Project Isolation, disclosure authorization, and trustworthy audit evidence.

**Accepted controls.** PostgreSQL stores only opaque Credential References;
Keychain and a future cloud secret service implement one resolver contract;
the value is injected only at the final transport boundary and never enters
canonical payloads, events, prompts, Tool arguments/results, exports, or
backups. [S4] Platform secret stores enforce access policy but do not replace
StoryOS authorization. [K1]

**Required structural mitigation.** Resolution requires the already-admitted
{ owner_user_id, project_id }, destination, purpose, operation/Attempt,
credential version, and live grant. The transport consumes a non-displayable,
short-lived secret handle; ordinary code cannot read or format its value.
Revocation and rotation invalidate future resolution without rewriting
history, and an old Attempt never silently receives a newer credential.

**Verifiable evidence.** Canary credentials are sought across the database,
events, traces, errors, crash reports, Tool/MCP traffic, model requests,
exports, backups, and support bundles. Wrong scope/destination/purpose/version,
revoked references, stale Attempts, and unavailable resolvers fail without a
secret-bearing distinction; audit contains reference/version/result only.

**Residual risk and owner.** A fully compromised host, secret-service operator,
or destination process can observe the secret at its necessary use boundary.
Reference and failure semantics belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), evidence
retention to [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md),
leak and rotation gates to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60),
and resolver integration to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 19. AP-11: Logs, telemetry, support evidence, or error surfaces leak data

**Source to sink.** SQL statement or parameter logging, HTTP body capture,
tracing fields, debug events, crash reports, Provider SDK logging, browser
console state, support bundles, or error strings copy payloads, credentials,
URLs, cross-scope identifiers, or existence facts into a broader and longer-
lived channel. PostgreSQL can log statements, bind values, and errors depending
on configuration. [P2]

**Affected assets.** Project and credential confidentiality, Project
Isolation, retention promises, and accurate minimum-disclosure evidence.

**Accepted controls.** Durable inspectability records typed evidence and
references, not arbitrary raw runtime payloads. Secret values and reversible
digests are forbidden outside the narrow transport boundary; external
telemetry is itself an Outbound Disclosure.

**Required structural mitigation.** All operational records use a typed
allowlist and explicit classification. SQL binds, HTTP bodies, complete
prompts/files, authorization headers, secret-bearing URLs, App messages, and
Provider wire payloads are off by default. Error mapping is scope-safe and
does not expose database detail. Support and telemetry projections have a
declared audience, purpose, retention, redaction version, and manifest before
egress; debug mode cannot weaken production data rules.

**Verifiable evidence.** A corpus of unique canary content and credentials
traverses success, rejection, timeout, panic, retry, OutcomeUnknown, archive,
and restore paths; automated scans prove absent raw values and reversible
digests in every operational channel. Cross-scope probes receive equivalent
status, timing class, and non-identifying errors.

**Residual risk and owner.** Operators may still infer bounded operational
metadata that the service intentionally records. The event/error vocabulary
belongs to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58),
classification and expiry to
[Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md),
leak testing to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60),
and production-safe defaults to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 20. AP-12: Backup, WAL, or restore leaks or silently changes authority

**Source to sink.** A backup reader, WAL archive destination, restore staging
area, copied connection configuration, incomplete archive chain, overwritten
segment, or privileged restore role exposes or alters the whole service. A
base backup covers the PostgreSQL cluster, not one Project; WAL requires
confidential handling, and `pg_verifybackup` does not replace an actual restore
and database-level validation. [P3] [P4]

**Affected assets.** Every User and Project, immutable history, credentials if
incorrectly persisted, migration and role posture, RPO/RTO, and availability.

**Accepted controls.** Base backup plus continuous WAL is the Foundation
recovery unit; stores use an independent restricted failure domain; RPO is at
most 15 minutes and RTO at most 2 hours; restore proves schema, roles, forced
RLS, projections, and credentials before service. A Project Export Archive is
not a database backup. [S4]

**Required structural mitigation.** Backup and archive roles remain separate
from runtime and migration, and reader/writer privileges remain separate where
the backend permits. Backup/WAL objects are encrypted, versioned or
non-overwriting, gap-checked, retained by policy, and never exposed through a
Project endpoint. Restore occurs in an isolated target with traffic disabled;
externalized configuration, role grants, migration checksums, credential
references, and scope invariants are re-established before cutover.

**Verifiable evidence.** Each release-candidate recovery drill combines
manifest/checksum verification with an actual startup, point-in-time restore,
role/grant and forced-RLS inspection, cross-scope negative tests, projection
rebuild equality, canary secret scan, missing/corrupt/duplicate WAL failures,
and measured RPO/RTO.

**Residual risk and owner.** Infrastructure administrators retain broad access,
and correlated loss of database plus recovery stores remains environmental
risk. Retention/archival policy and restore evidence belong to
[Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md);
gates belong to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60);
the first operational recovery proof belongs to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 21. AP-13: Import archives or author files escape scope or parser bounds

**Source to sink.** An author-provided file or purported Project Export Archive
contains absolute or parent paths, links or device entries, duplicate or
Unicode/case-colliding names, a decompression bomb, malformed parser input,
forged scope/identity, foreign references, hidden credentials, or SQL/database
objects. Extraction or partial import can overwrite host files, exhaust the
service, expose existing files, or make attacker-controlled bytes authoritative.
Archive formats can traverse paths and expand far beyond compressed size; a
PostgreSQL dump is executable SQL and is not a safe Project interchange format.
[F1] [P5]

**Affected assets.** Host integrity, service availability, Project Isolation,
canonical history, Credential confinement, and author authority.

**Accepted controls.** Project identity is database-owned Project Scope, not a
path or embedded owner claim. Export is one exact Project closure with a
versioned StoryOS manifest, entry set, schema, sizes, digests, and provenance;
import never means merge, copy, ownership transfer, SQL restore, or acceptance
of proposed creative state. [S4]

**Required structural mitigation.** Prefer streaming fixed, allowlisted regular
entries into private non-authoritative staging instead of general extraction.
Reject absolute/parent/NUL paths, links, devices, duplicates and normalized
collisions; cap compressed bytes, expanded bytes, ratio, entry count, nesting,
parse depth, text tokens, and time. Verify format version, schema, digests,
referential closure, caller-owned destination Project, and absence of secret
values before one atomic PostgreSQL import. On any failure, no imported state
is visible and staging is discarded safely.

**Verifiable evidence.** An adversarial archive/file corpus covers traversal,
link escape, devices, bombs, recursion, duplicate and Unicode/case collisions,
digest/schema mismatch, forged owners/projects, foreign references, SQL dumps,
credentials, malformed media, partial failure, and retry. Failure leaves the
database unchanged; success is exactly scoped and reproducible.

**Residual risk and owner.** Memory-safe parsers may still contain denial-of-
service or logic defects, so isolation and hard budgets remain mandatory.
Archive/command schemas belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), archive
retention to [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md),
the corpus to
[Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60),
and the narrow importer to
[Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 22. AP-14: Retry or OutcomeUnknown duplicates an external or authoritative effect

**Source to sink.** A disconnect or crash occurs before, during, or after a
database commit, author-command Receipt, SSE observation, HTTP acknowledgement,
or external side effect. A stale tab, Client, worker, outbox dispatcher, or
recovery loop treats a missing response, local journal entry, Pending Edit
Projection, cache, or process observation as an oracle, retries without an
exact-scoped idempotency fact, or converts uncertainty into failure. The same
Acceptance, edit, proposal transition, Tool call, Provider request,
notification, or export may run twice, while a response from an older Attempt
or Admission may settle the newer one.

**Affected assets.** Author authority, immutable history, external accounts,
Attempt truth, ordering, and replay-safe recovery.

**Accepted controls.** Canonical state, immutable payloads, durable Admissions,
Receipts, Attempts, outbox facts, and OutcomeUnknown outrank browser, process,
network, cache, journal, projection, Artifact, Operational Record, and external
output observations.

**Required structural mitigation.** Every command and external effect binds
{ owner_user_id, project_id }, operation identity, canonical request digest,
Attempt or Admission identity, idempotency record, and expected version. The
durable transition and outbox record commit atomically; the Attempt exists
before egress, and an Author Command Admission has one append-only lifecycle
and one terminal settlement. Same key plus different digest conflicts.

A missing acknowledgement after possible author-command invocation enters
read-only reconciliation. Recovery may invoke only the same unexpired,
fully-matching direct edit when the authoritative check proves no Receipt;
explicit commands, expired Admissions, changed bindings or digest, or
unrecoverable intent require visible reconfirmation and a new Admission.
A missing observation after possible external egress becomes OutcomeUnknown
and requires destination-specific reconciliation or author-visible resolution,
never blind retry or invented success. Late results can settle only their exact
still-current Attempt or Admission under its owning contract.

**Verifiable evidence.** Crash injection at every durability/egress/response
edge plus acknowledgement-before/after-SSE permutations, tab reload, stale
writer takeover, duplicated, reordered, and delayed deliveries proves one
canonical effect, stable replay, preserved uncertainty, exactly one terminal
Admission settlement, Draft preservation where required, and no cross-scope
key collision.

**Residual risk and owner.** Some destinations offer no authoritative lookup;
uncertainty may remain permanently visible, and a lost human intention cannot
be reconstructed from browser state. Admission semantics remain owned by
[Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68),
editor reconciliation by
[Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70),
wire/idempotency by
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58),
and `DVG-03`, `DVG-07`, `DVG-08`, and `DVG-11` own fault, replay, and
zero-duplicate evidence.

## 23. AP-15: Retrieval, embedding, or cache poisoning changes eligible context

**Source to sink.** Malicious or stale source content, forged provenance,
cross-scope embedding rows, global cache keys, deleted/revised sources, or a
changed ranking/provider revision enters retrieval. If ranking or cached output
is treated as authorization, ineligible or another Project's content reaches
model context, an App, or an Outbound Disclosure. [R1]

**Affected assets.** Project Isolation, Context eligibility, provenance,
minimum disclosure, and deterministic rebuilds.

**Accepted controls.** Indexes, embeddings, rankings, and caches are
PostgreSQL-hosted rebuildable projections, never authority; the seven Context
gates cap, authorize, and manifest final context. [S5]

**Required structural mitigation.** Every projected item and key binds exact
Project Scope, source identity/revision/digest, eligibility policy, projection
and provider/model revision. Scope and current eligibility filter the candidate
set before ranking; ranks never grant access. Deletion or supersession
invalidates dependencies.

**Verifiable evidence.** Cross-User/Project corpora, poisoned chunks, cache-key
collisions, stale revisions, changed Provider models, deletion, corruption,
and full rebuilds prove no ineligible hit and equality of eligible outputs and
dependency closure.

**Residual risk and owner.** Authorized malicious prose can still influence
model quality, but cannot obtain authority. Projection schemas belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), invalidation and
retention to [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md), adversarial
rebuild proof to [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), and
the first bounded retrieval slice to [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 24. AP-16: Historical evidence or provenance is rewritten after the fact

**Source to sink.** Runtime UPDATE/DELETE, migration, restore, compaction,
projection rebuild, policy change, or UI reconstruction alters an immutable
payload, substitutes current metadata for historical metadata, drops a source
dependency, or presents a projection as the original. Approval, Acceptance,
Tool effect, disclosure, and recovery evidence can then be forged or erased.

**Affected assets.** Author authority, auditability, provenance, disclosure
history, conflict decisions, and recovery correctness.

**Accepted controls.** Historical payloads are immutable and projections are
rebuildable from canonical PostgreSQL facts.

**Required structural mitigation.** Corrections append typed facts. Facts
carry stable identity, schema/version, timestamps, source closure,
actor/authority basis, and digest
where defined. A projection is disposable and reconstructs from canonical
facts; retention or redaction creates explicit tombstone/summary evidence and
cannot silently change the meaning of surviving references. Migration and
restore preserve or explicitly transform facts under checksummed versions.

**Verifiable evidence.** Runtime cannot mutate historical payloads; deliberate
corruption is detected. Rebuild-from-empty equals the stored projection;
migration and restore retain Proposal/Acceptance, context, disclosure, Tool,
Attempt, cursor, and provenance lineages or emit the specified loss marker.

**Residual risk and owner.** A database/platform administrator can rewrite both
facts and ordinary checks, so independent recovery evidence is needed for
detection. Historical wire schemas belong to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58),
compaction/redaction/tombstones to [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md),
rebuild and tamper gates to [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60),
and minimum audit lineage to [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 25. AP-17: A stale lease or fence completes work after recovery

**Source to sink.** A worker pauses past lease expiry while holding old memory,
sockets, queued output, or an external response. A replacement obtains the
work, but the stale worker later writes state, marks outbox delivery, seals a
Mailbox, completes a Proposal, or attaches a response without an atomic fence
check, overwriting the current owner or duplicating an effect.

**Affected assets.** Run/Subrun truth, outbox and Mailbox ordering, proposal
state, external effects, idempotency, and recovery availability.

**Accepted controls.** Run/Subrun, Attempt, outbox, Mailbox, Proposal, and
recovery transitions are durable typed facts rather than process state.

**Required structural mitigation.** Each claim carries a monotonically
changing epoch/fencing token. Every canonical transition, outbox claim/delivery,
Mailbox seal, and Attempt settlement atomically checks exact Project Scope,
operation/Attempt, current fence, expected version, and allowed prior state.
Lease renewal cannot revive an obsolete epoch. External calls remain tied to
their originating Attempt; a late result is evidence, not automatic authority.

**Verifiable evidence.** Pause a worker before each write/egress boundary,
expire and reassign it, then resume it. All stale database writes, settlements,
seals, proposal completions, and delivery claims fail; replay produces one
stable outcome without losing the late evidence.

**Residual risk and owner.** A non-idempotent external effect may remain
OutcomeUnknown even though stale local writes are fenced. Fence/event contracts
belong to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), lease and
snapshot retention to [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md),
race tests to [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), and the
first crash-safe worker path to [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 26. AP-18: Untrusted input or replay exhausts bounded service resources

**Source to sink.** Oversized or infinite SSE replay, archive expansion,
recursive schemas, Tool/App messages, model streams, research responses,
transcript history, retrieval candidates, retries, or projection rebuilds
consume memory, database connections, storage, CPU, tokens, or outbound quota.
One Project can starve recovery or another User even without crossing data.

**Affected assets.** Availability, isolation, author work, recovery objectives,
external spend, and the bounded-context invariant.

**Accepted controls.** Context Assembly is incrementally built, attributable,
inspectable, and bounded; one injected item may never exceed 10K tokens.

**Required structural mitigation.** Every remaining crossing declares byte,
item, time, token, depth, and attempt budgets with hard server-side ceilings.
Work admission and queues are
scope-aware, cancellable, and backpressured; partial results cannot bypass
validation. Replay past the retained or bounded window requires a Snapshot
handoff. Recovery capacity and maintenance connections are reserved from
ordinary work.

**Verifiable evidence.** Boundary-size, over-limit, slow-loris, infinite-stream,
decompression, replay-gap, retry-storm, and concurrent noisy-neighbor tests
prove deterministic rejection/truncation, bounded resource growth, intact
canonical facts, and continued control/recovery operations.

**Residual risk and owner.** Foundation limits are not Internet-scale DDoS
protection or billing policy. Budget/error contracts belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), expiry/Snapshot/replay
floors to [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md), stress gates
to [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), and bounded default
configuration to [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62).

## 27. AP-19: Browser code or content escapes into the protected-client boundary

**Source to sink.** Hostile manuscript, imported, model, Tool, MCP, App, or
external content reaches an unsafe DOM or script sink; a compromised dependency,
build artifact, separately fetched asset, service worker, or third-party script
executes under the first-party origin; or a browser extension interferes with
the DOM, events, storage, messages, or requests. The executing code can spoof
what the author sees, read disclosed Project content, reuse the current session,
or submit an attacker-selected command that appears to satisfy the same-origin
client path. Browser platform controls reduce these paths but do not turn a
successful same-origin compromise into proof of physical-human intent. [S6]

**Affected assets.** Author authority, Project confidentiality, exact command
meaning, Protected Web Client integrity, credentials available to the current
origin, and trustworthy admission evidence.

**Accepted controls.** Only the exact controlled build and immutable asset graph
named by the accepted client-contract, security-policy, and current Client
Session Binding generation participates in the Release 1 claim. All rendered
or imported content and external output are data, not executable client code;
browser extensions and third-party scripts remain outside the trusted claim.

**Required structural mitigation.** The release bundles or self-hosts the
minimum executable asset graph, locks and reviews dependencies, records build
provenance and asset digests, and permits no ambient third-party runtime script.
A strict CSP disallows unapproved script execution and dangerous fallback
sources; context-safe rendering and Trusted Types enforcement protect supported
DOM injection sinks. If a separately fetched immutable asset is ever necessary,
its exact digest and origin are pinned, including Subresource Integrity where
the browser contract supports it, without making that origin authoritative.
Application and service-worker updates activate atomically under the exact
release identity; an old or mixed asset graph is refused rather than silently
upgraded. Client content never renders as trusted executable HTML, and Server
admission still recomputes every protected-client, scope, digest, nonce, and
lifetime input.

**Verifiable evidence.** The hostile-browser corpus covers stored and reflected
script payloads at every renderer, unsafe DOM sinks, policy bypass attempts,
dependency and asset substitution, mixed-release assets, stale service-worker
activation, third-party network/script requests, and extension-like DOM,
storage, event, and message interference. A mismatched build, client contract,
security policy, or session generation cannot receive an Admission; a
successful same-origin execution path is classified as a critical failure, not
as a protected-client success.

**Residual risk and owner.** A browser, extension, or dependency zero-day, or a
successful compromise of the exact trusted asset graph, can defeat the
Release 1 client claim while Server evidence still proves only the submitted
bytes. Stronger human-intent attestation requires a separately trusted display
and confirmation surface and is not selected here. Asset, contract, and policy
wire identities belong to
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
`DVG-01`, `DVG-02`, `DVG-03`, and `DVG-11` own release-drift, substitution,
browser-integrity, and claim-ceiling evidence.

## 28. AP-20: A stale tab or local continuity state survives writer takeover

**Source to sink.** A suspended, restored, duplicated, or offline tab retains an
old Client Session, Editor Session, writer generation, nonce, Admission,
Local Edit Journal entry, Pending Edit Projection, cached Receipt, or rendered
Head. After another tab takes over, the stale tab resumes and submits, replays,
discards, or presents that local state as if it were current authority. A lost
acknowledgement can make both tabs attempt recovery and duplicate or overwrite
the author's work.

**Affected assets.** Author prose, writer ownership, command order, current
Heads, Receipt convergence, Draft preservation, and truthful recovery.

**Accepted controls.** There is one current writer generation per Project.
The Local Edit Journal and Pending Edit Projection preserve continuity but are
never authority, settlement, or evidence that no Server effect occurred.
Admission and Receipt lifecycles remain durable Operational Records; refused
or unrecoverable text remains a Draft Artifact.

**Required structural mitigation.** Writer takeover monotonically advances the
Project writer generation and makes every older editor read-only. Every
submission and reconciliation binds the current Protected Web Client, Client
Session, client-contract, security-policy, Editor Session, writer generation,
scope, action class, digest, target/Heads, nonce/idempotency identities, and
lifetime. Reload and reconnect use authoritative snapshots and typed Receipts;
local cache, journal, projection, missing HTTP response, and SSE ordering cannot
settle or retry a command. Recovery follows AP-14: only the exact eligible
direct-edit branch may reuse its still-live Admission after read-only proof;
otherwise the author receives preserved Draft content and visibly reconfirms a
new command.

**Verifiable evidence.** Two-tab and multi-device fixtures pause at every
journal, submission, commit, acknowledgement, Event, takeover, reload, and
reconciliation edge. Old generations cannot mutate; duplicates converge to one
Receipt; changed or expired commands require reconfirmation; corrupt or missing
local state cannot delete canonical prose; and all refused or unrecoverable
content remains inspectable and copyable under its fixed Draft classification.

**Residual risk and owner.** A currently admitted writer or compromised browser
can submit commands within its bounded client claim, and local storage
compromise can expose content already present on that device. Editor and
recovery semantics remain owned by
[Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70),
Admission lifecycle by
[Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68),
wire shape by
[Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58),
and `DVG-02`, `DVG-03`, `DVG-07`, and `DVG-11` own takeover and recovery
evidence.

## 29. AP-21: Controlled-cloud classification collapses an external boundary

**Source to sink.** Deployment code treats a shared hostname, private network,
cloud account, operator, Provider brand, VPC endpoint, or service location as
proof that a destination is StoryOS-controlled. A shared connection pool,
cache, queue, telemetry sink, credential, maintenance path, or external service
then receives another User's Project data, bypasses an Outbound Disclosure
manifest, or gains runtime or maintenance authority.

**Affected assets.** Project Isolation, User identity, Project data,
credentials, disclosure provenance, maintenance authority, and the meaning of
the StoryOS Controlled Processing Boundary.

**Accepted controls.** “Controlled” is a registered StoryOS contract about the
exact processor, identity, data path, policy, Project Scope, and operator
authority. Network proximity, provider ownership, account membership, or
deployment topology is never authorization or proof of Project Isolation.

**Required structural mitigation.** Every processing destination and transport
has one exact registered identity, control classification, allowed purpose,
credential generation, TLS peer identity, policy revision, and Project-scoped
data path. Controlled services preserve server-derived User and Project Scope
through scope-keyed pools, queues, caches, storage, logs, and support evidence,
with runtime and maintenance roles separated. Any destination lacking that
complete controlled registration is external and must pass the seven disclosure
gates, manifest-before-egress, Credential Resolver, and ordinary
OutcomeUnknown handling. Shared infrastructure never aliases User, Project,
credential, cache, or idempotency identities.

**Verifiable evidence.** Multi-User deployment fixtures vary hostname, region,
VPC/private link, provider/account ownership, shared pool/queue/cache,
maintenance credential, TLS identity, telemetry route, and registration
revision independently. Only exact controlled registrations retain the
classification; every other path either preserves complete Project Isolation
under its controlled contract or emits an authorized external-disclosure
manifest before bytes leave.

**Residual risk and owner.** A privileged cloud control-plane or platform
administrator remains capable of violating its intended boundary and requires
operational and recovery evidence beyond application isolation. Controlled
destination and disclosure semantics remain with their existing Context,
Provider, Tool/MCP, Protocol, PostgreSQL, and retention owners; `DVG-02`,
`DVG-04`, `DVG-05`, `DVG-10`, and `DVG-11` own classification, isolation,
egress, credential, and restore evidence.

# Attack-Path Completeness and Deterministic Gate Handoff

The required Foundation surfaces are closed by credible source-to-sink paths,
not by generic control labels:

| Required surface | Credible source to sink | Owning paths | Deterministic handoff |
|---|---|---|---|
| Origin, Host, and CSRF | hostile origin or forged routing/session input → state-changing HTTP command | AP-03 | `DVG-02`, `DVG-03`, `DVG-11` |
| XSS and dependency/supply-chain compromise | hostile content or substituted executable asset → first-party DOM/session/command authority | AP-19 | `DVG-01`–`DVG-03`, `DVG-11` |
| browser extensions and third-party scripts | outside browser code → DOM, storage, display, or request manipulation | AP-19 | `DVG-02`, `DVG-03`, `DVG-11` |
| stale tab, service worker, or writer takeover | old build/generation/local continuity state → current edit or recovery sink | AP-19, AP-20 | `DVG-01`–`DVG-03`, `DVG-07`, `DVG-11` |
| replay | old command, cursor, bridge message, result, or fence → current authority or projection | AP-03–AP-05, AP-14, AP-17, AP-20 | `DVG-02`, `DVG-03`, `DVG-05`, `DVG-07`–`DVG-09`, `DVG-11` |
| acknowledgement loss and OutcomeUnknown | missing HTTP/SSE/external observation → blind retry, invented settlement, or duplicate effect | AP-14, AP-17, AP-20 | `DVG-03`, `DVG-06`–`DVG-08`, `DVG-11` |
| reconciliation and recovery | browser/process/external observation → unauthorized authority, destructive loss, or stale settlement | AP-12, AP-14, AP-16–AP-20 | `DVG-03`, `DVG-06`–`DVG-11` |
| disclosure and external destinations | eligible or hostile content → model, Tool/MCP/App, research, telemetry, support, export, or Provider sink | AP-05–AP-11, AP-13, AP-15, AP-18, AP-21 | `DVG-04`, `DVG-05`, `DVG-08`–`DVG-11` |
| credentials | opaque or cross-scope reference/secret-bearing diagnostics → wrong destination or disclosure channel | AP-02, AP-05, AP-08–AP-11, AP-21 | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-10`, `DVG-11` |
| Tool, MCP, and App authority | untrusted schema/result/App bridge → Tool execution, context, credential, or author-state sink | AP-05–AP-07, AP-09–AP-10, AP-14, AP-18 | `DVG-04`, `DVG-05`, `DVG-08`, `DVG-11` |
| controlled cloud | topology/account/provider inference or shared runtime state → cross-Project access or unmanifested egress | AP-01–AP-03, AP-08–AP-12, AP-16, AP-18, AP-21 | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-10`, `DVG-11` |
| Project isolation | client, runtime, DB, cache, cursor, archive, backup, destination, or recovery input → another User/Project | AP-01–AP-21 | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-07`–`DVG-11` |

Every path also has one explicit deterministic proof owner:

| Attack path | Deterministic gate handoff |
|---|---|
| AP-01 | `DVG-02`, `DVG-11` |
| AP-02 | `DVG-02`, `DVG-07`, `DVG-10`, `DVG-11` |
| AP-03 | `DVG-02`, `DVG-03`, `DVG-11` |
| AP-04 | `DVG-02`, `DVG-09`, `DVG-11` |
| AP-05 | `DVG-05`, `DVG-11` |
| AP-06 | `DVG-05`, `DVG-11` |
| AP-07 | `DVG-04`, `DVG-05`, `DVG-11` |
| AP-08 | `DVG-05`, `DVG-11` |
| AP-09 | `DVG-04`, `DVG-05`, `DVG-08`, `DVG-11` |
| AP-10 | `DVG-02`, `DVG-05`, `DVG-11` |
| AP-11 | `DVG-11` |
| AP-12 | `DVG-10`, `DVG-11` |
| AP-13 | `DVG-10`, `DVG-11` |
| AP-14 | `DVG-03`, `DVG-07`, `DVG-08`, `DVG-11` |
| AP-15 | `DVG-04`, `DVG-11` |
| AP-16 | `DVG-07`, `DVG-09`, `DVG-10`, `DVG-11` |
| AP-17 | `DVG-06`, `DVG-07`, `DVG-08`, `DVG-11` |
| AP-18 | `DVG-01`, `DVG-04`, `DVG-05`, `DVG-09`, `DVG-10`, `DVG-11` |
| AP-19 | `DVG-01`, `DVG-02`, `DVG-03`, `DVG-11` |
| AP-20 | `DVG-02`, `DVG-03`, `DVG-07`, `DVG-11` |
| AP-21 | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-10`, `DVG-11` |

# Severity Calibration

Severity is based on the impact to accepted StoryOS invariants and a credible
path through the Foundation architecture, not on generic vulnerability names.
Deployment configuration and exposure determine likelihood; a later review
must lower severity only with tested controls, not with intended behavior.

| Severity | StoryOS calibration | Representative paths |
|---|---|---|
| Critical | cross-User/Project disclosure or authoritative mutation at service scale; Credential value disclosure; unmanifested external disclosure; App/Tool-created Acceptance; backup/restore compromise that silently changes canonical truth; compromise of the exact Protected Web Client asset graph; controlled-cloud boundary collapse | AP-01, AP-02, AP-05–AP-06, AP-09–AP-10, AP-12, AP-16, AP-19, AP-21 |
| High | forged author command; SSRF into a protected service; archive host escape; duplicate non-idempotent effect; stale worker or writer overriding current truth; persistent cross-scope retrieval | AP-03, AP-08, AP-13–AP-15, AP-17, AP-20 |
| Medium | scoped replay or presentation confusion without authority; bounded sensitive operational metadata; one-Project resource exhaustion with recovery preserved | AP-04, AP-11, AP-18 |
| Low | safely denied malformed input or intentionally sanitized operational metadata with no meaningful confidentiality, integrity, authority, or availability effect | evidence-only unless it composes with another path |

The highest applicable impact controls triage when paths compose. For example,
a low-information error oracle becomes Critical if it supplies the object
identity needed for AP-01, and prompt injection becomes Critical only when a
broken downstream authority boundary lets it cross into Acceptance, secret
resolution, or unmanifested egress.

# Downstream Security Handoff

This threat model owns the attack paths and Foundation security objectives.
It deliberately leaves each implementable contract or proof with one existing
Wayfinder owner:

| Downstream owner | Security obligations received from this model | Attack paths |
|---|---|---|
| [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68) | exact protected-client, User, existing/prospective Project Scope, editor/writer, action, digest, nonce/idempotency, and lifetime binding; bounded claim ceiling; one append-only lifecycle and terminal settlement; direct-versus-explicit recovery | AP-01, AP-03, AP-14, AP-20 |
| [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70) | Local Edit Journal validation; non-authoritative pending projection; one Project writer generation; stale-tab fencing; acknowledgement/Event convergence; resync; Draft preservation; explicit-command reconfirmation | AP-01, AP-03, AP-04, AP-14, AP-17, AP-18, AP-20 |
| [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | exact protected-client/requester/scope envelopes; non-oracular errors; CSRF and Origin/Host inputs; build, client-contract, security-policy and session identities; scoped SSE cursors and Snapshot handoff; idempotency/Attempt/OutcomeUnknown/fence states; Capability, bridge, Tool/MCP and credential-reference contracts; import/export schema; controlled/external destination manifests; explicit hard budgets | AP-01, AP-03–AP-10, AP-13–AP-21 |
| [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md) | replay floors and Snapshot semantics; Admission/Attempt/outbox/Mailbox/late-result evidence; immutable-history compaction, redaction, tombstones and source closure; logs/support/telemetry classification and expiry; disclosure, export, backup/WAL and restore-proof retention | AP-04, AP-09–AP-12, AP-14–AP-18, AP-20–AP-21 |
| [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | cross-scope and role/RLS tests; hostile-origin, XSS/DOM, asset/dependency, third-party-script, extension, stale-build/tab, controlled-cloud, bridge/Tool/MCP/prompt/SSRF/provider/archive corpora; claim-ceiling checks; secret and log leak scanning; adapter-wire comparison; fault, retry, fence, replay, rebuild, tamper, restore and resource-bound proofs | AP-01–AP-21 |
| [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62) | refuse production-shaped handoff until the slice demonstrates exact Protected Web Client release identity, non-owner forced-RLS runtime, exact-scoped HTTP/SSE, manifest-before-egress with Credential Resolver, mediated Tool/MCP boundary, durable Attempt/recovery, bounded input, safe operational defaults and actual restore evidence at the slice's accepted scope | AP-01–AP-21 |

No separate parallel security map or security runtime follows from this threat
model. This contract owns trusted-computing boundaries, source-to-sink attack
analysis, structural mitigations, and residual risks. It does not re-own the
evidence classification fixed by the Artifact contract, Admission
identity/lifecycle/settlement fixed by issue #68, Core effects, editor
recovery fixed by issue #70, wire shapes fixed by issue #58, or gate selection
fixed by issue #60. Those owners must close their assigned contracts and
deterministic negative evidence in the map's single serial chain before the
editor-first implementation handoff.

# Source Index

Repository sources are fixed StoryOS facts; external sources establish only
the cited platform or protocol behavior. Existing external sources were
accessed for the original model on 2026-07-21; the protected-client primary
source review records its own 2026-07-24 access date.

- **[S1]** [StoryOS repository instructions](../../AGENTS.md): product,
  authority, Project Scope, disclosure, durability, and App/editor invariants.
- **[S2]** [ADR 0002](../adr/0002-specify-transcript-and-mcp-app-lifecycle-semantics.md):
  sandboxed MCP App lifecycle and Proposal/Acceptance boundary.
- **[S4]** [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md):
  authoritative PostgreSQL, roles, forced RLS, immutable payloads, export,
  Credential Reference, backup/WAL, restore, and verification contracts.
- **[S5]** [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md):
  seven gates, retrieval/projection provenance, manifest-before-egress,
  Attempt, OutcomeUnknown, and destination contracts.
- **[S6]** [Protected Web Client Security Boundary: Primary-Source Evidence](../research/protected-web-client-security-boundary-primary-sources.md):
  browser-enforced script, DOM-injection, asset-integrity, extension, and
  update/cache limits used to calibrate AP-19 without upgrading platform
  guidance into StoryOS authority.
- **[P1]** [PostgreSQL Row Security Policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html):
  policy combination, owner/superuser/BYPASSRLS behavior, FORCE RLS, and
  operations outside row-security policy control.
- **[P2]** [PostgreSQL Error Reporting and Logging](https://www.postgresql.org/docs/current/runtime-config-logging.html):
  statement, parameter, error, and destination logging behavior.
- **[P3]** [PostgreSQL pg_basebackup](https://www.postgresql.org/docs/current/app-pgbasebackup.html)
  and [Continuous Archiving and PITR](https://www.postgresql.org/docs/current/continuous-archiving.html):
  cluster backup authority, connection configuration, WAL confidentiality,
  archive continuity, and overwrite behavior.
- **[P4]** [PostgreSQL pg_verifybackup](https://www.postgresql.org/docs/current/app-pgverifybackup.html):
  integrity verification scope and explicit need for test restores.
- **[P5]** [PostgreSQL pg_dump](https://www.postgresql.org/docs/current/app-pgdump.html):
  row-security behavior and the arbitrary-code risk of restoring untrusted
  dumps.
- **[P6]** [PostgreSQL Client Authentication](https://www.postgresql.org/docs/current/auth-pg-hba-conf.html)
  and [libpq connection strings](https://www.postgresql.org/docs/current/libpq-connect.html):
  first-match HBA behavior, unsafe trust authentication, and TLS server-name
  verification modes.
- **[W1]** [MDN Cross-Origin Resource Sharing](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS):
  simple requests, credential restrictions, Origin behavior, and why CORS is
  not command authorization.
- **[W2]** [WHATWG Server-sent events](https://html.spec.whatwg.org/dev/server-sent-events.html):
  automatic reconnection and Last-Event-ID behavior.
- **[W3]** [MDN Window.postMessage](https://developer.mozilla.org/en-US/docs/Web/API/Window/postMessage):
  exact targetOrigin and receiver origin/source/message validation.
- **[M1]** [MCP Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices):
  confused-deputy, token-passthrough, local-server, SSRF, and scope risks.
- **[M2]** [MCP Authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization):
  token audience/resource indicators and separate upstream authorization.
- **[M3]** [MCP Tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools):
  untrusted annotations, confirmations, input display, validation, timeouts,
  and audit guidance.
- **[M4]** [MCP Apps 2026-01-26](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx):
  sandbox proxy, cross-origin iframe, CSP, resource identity, Host mediation,
  and app-only Tool constraints.
- **[N1]** [OWASP SSRF Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html):
  scheme, address, DNS, redirect, and network-layer SSRF controls.
- **[E1]** [OpenAI API data controls](https://developers.openai.com/api/docs/guides/your-data#default-usage-policies-by-endpoint):
  endpoint- and feature-dependent retention and control statements.
- **[E2]** [Alibaba Cloud Model Studio privacy notice](https://www.alibabacloud.com/help/en/model-studio/privacy-notice):
  Bailian provider statements, treated as external claims rather than StoryOS
  guarantees.
- **[K1]** [Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services/):
  protected credential storage and controlled item access on the local host.
- **[F1]** [Python tarfile extraction filters](https://docs.python.org/3/library/tarfile.html#extraction-filters):
  path, link, device, metadata, and denial-of-service hazards when extracting
  untrusted archives.
- **[R1]** [OWASP RAG Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/RAG_Security_Cheat_Sheet.html):
  poisoning, provenance, access enforcement, and tenant-isolation risks.

Repository: FrankQDWang/StoryOS

Version: 15a6c4d343145c9f825d52067f13d09eec6ead37
