# StoryOS Service, Client, and External Trust Boundaries Threat Model

- Status: accepted
- Wayfinder resolution: [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57)
- Repository baseline: `dd4775c982f903e04ea5a9cf047968d489808a01`
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
- Deployment and isolation boundary: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md) and [ADR 0022](../adr/0022-prefer-widely-validated-hosted-infrastructure.md)
- Model continuation: [ADR 0033](../adr/0033-use-volcengine-responses-for-the-first-real-model-path.md)
- Hosted operations: [ADR 0034](../adr/0034-bound-provider-hosted-tool-operations.md)
- Generated Memory: [ADR 0035](../adr/0035-use-background-generated-project-memory.md)

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

### 1.1 Current Provider evidence limits

Official pages were rechecked on 2026-09-14. The rows below are source facts
and limited inferences; they do not certify an account or selected model.

| Source fact | Threat-model implication |
|---|---|
| Agent Plan documents its own `/api/plan/v3` route and key; general Ark Responses uses `/api/v3/responses`. [E3] [E4] | A general API field is not proof of Agent Plan route/model/account support. |
| A prior-response reference includes earlier input/output but does not inherit `instructions`. [E4] | Supply required current instructions; do not infer cross-account, key, endpoint, or model portability from a handle. The inspected pages give no such guarantee. |
| Response storage has expiry; implicit cache is automatically enabled on supported models and cannot be disabled through that cache feature. [E4] [E5] | `store=false`, expiry, or deleting a response does not establish zero retention or erasure of every Provider copy. |
| Explicit caching retains initial Tool definitions, restricts later Tool configuration, excludes `instructions`, and has `json_schema` combination limits. [E5] | Validate the whole request mapping; cached Tools grant no current permission. |
| `max_tool_calls` is best effort and limits rounds, not calls per round. The Doubao search mode ignores `limit`, `max_keyword`, and `user_location`. [E4] | Those fields alone do not prove a finite maximum cost, Tool count, or outward-processing boundary. |
| Remote MCP exposes `server_url`, Tool-name filtering, and approval controls. [E4] | Configuration alone does not prove parameter-level resource, effect, or egress isolation. |
| Retrieval returns a completed result; deletion reports removal by ID. The pages do not promise create idempotency, resumed streams, or remote termination. [E6] | Reconcile original evidence without treating deletion, stream loss, or expiry as non-execution. |
| ArkCLI can configure search MCP separately without changing the model Provider or base URL. [E7] | Harness Tool availability does not certify in-Response sandbox support or its intake/network/resource controls. |

Unknown scope or capability is not proof of a Provider vulnerability. It is
insufficient evidence to enable a StoryOS path that requires that property.

## 2. Product and deployment outline

StoryOS supports Dean Koontz-style Discovery Writing: the author develops the
novel through the passage and choices currently before them. The system does
not introduce an Agent-authored outline, Author Plan, or preplanned story
structure. One stable User owns each Project and acts as its sole Project
Author. Authoritative State changes only through a narrow Direct Author Action
or an inspectable Core Proposal followed by explicit author Acceptance.

Local development uses a Mac and OrbStack PostgreSQL. Under ADR 0022,
production PostgreSQL is hosted Supabase; Server, Worker, and Web stay paired
on a Linux VPS behind TLS. One bootstrapped User exercises the same Project
Isolation required by a later multi-User service. Hosted compatibility and
recovery acceptance remain deferred to deployment preparation. [S10]

Volcengine Agent Plan Responses is the selected first real-model path, subject
to exact route, account, model, and capability validation. Continuation, cache,
native compaction, retrieval, cancellation, and hosted Tools are separate
capabilities. Codex CLI supplies the primary general Agent design reference,
not runtime code or evidence of Volcengine support. [S7]

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
authority stores. Messages, captured Research, Memory Documents, and recoverable
context records use PostgreSQL. Markdown is a content form, not a second
Server file store. Active request assembly may use process memory; Provider
state cannot replace the durable record. There is no per-Project database,
SQLite, Neo4j, standalone vector database, message broker, microservice split,
whole-system Event
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
4. **Minimum-necessary external processing.** Every StoryOS-controlled
   submission receives only admitted input or references after
   manifest-before-egress. Hosted work has its complete input, Tool set,
   outward processing, effects, and resource scope bounded before submission;
   invisible internal steps are not claimed as separate Host-gated calls.
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
| Context and disclosure evidence | seven ordered gates at StoryOS submissions, known input/reference dependencies, bounded hosted scope, and manifest-before-egress |
| Generated Memory and publication | exact Project Scope and source/publication versions, bounded maintenance, no authority promotion, and no stale or partial publication |
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
| Malicious model output or Provider response | can emit persuasive text, forged or partial events, substituted continuation/call identities, oversized output, or false status, source, and usage reports |
| Poisoned Memory input or stale maintenance job | can repeat false or imperative content through extraction, consolidation, navigation, and recall, or try to publish with stale scope, permissions, or generation |
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
and model attention remain outside StoryOS durable truth. A hosted mode must
have a validated way to constrain admitted intake, Tool set, outward processing,
effects, and finite resource use; a prompt or service label alone is insufficient.
Invisible internal events remain Provider-reported or unknown. Host checks prove
admission and local handling, not that the Provider obeyed every bound. [S8]

Ordinary creative corrections and non-use guidance remain Messages or Steering
Input for the Agent. They are not access revocations or a deterministic semantic
exclusion mechanism. The Host enforces actual permissions, Project Isolation,
and retained-copy restrictions without proving all past model influence was
removed. Model adherence, Memory truth, and exact forgetting are not security
oracles. Browser or sandbox zero-days, OS credential theft after full host compromise, and cryptographic
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
| TB-6 Model/embedding destination | admitted input and prior references, wire projection, ephemeral credential, returned output | exact Scope and applicable conversation/destination bindings, current admission, manifest, Attempt, disclosure evidence, and no cross-scope reuse |
| TB-6a Provider-hosted operation | explicit intake and intentionally referenced state, enabled Tool set, outward processing, returned reports | bound the whole operation before its owning Model Attempt submits; no Ambient Context, invented internal Host gates, or duplicate accounting |
| TB-7 Tool Gateway ↔ Tool/MCP server | declared inputs, effects, credentials, results, nested destinations | exact Registration and ToolSpec, non-escalating grant, Approval, effect evidence, contract-drift fence |
| TB-8 MCP App iframe ↔ Host bridge | UI resource, instance negotiation, presentation signals, App Action Requests | cross-origin sandbox, exact instance/source/origin, schema and capability mediation, no auto-forward |
| TB-9 Research fetcher ↔ network | generated or supplied URL/query, redirects, response bytes | public-network-only policy, no ambient credentials, bounded capture and exact provenance |
| TB-10 Import/export ↔ author-provided bytes | Project Export Archive or imported source | stage and validate without authority, path, parser, secret, or scope escape |
| TB-11 PostgreSQL ↔ backup/WAL store | whole-service canonical data and recovery metadata | independent restricted failure domain, confidentiality, integrity, gap detection, restore proof |
| TB-12 Durable store ↔ journals/projections/caches/UI | journaled, indexed, summarized, replayed, cached, or displayed views | current scope, generation, access, and required versioned dependencies; views never become authority, settlement, or a recovery oracle |
| TB-13 Conversation records ↔ Memory maintenance ↔ recall | bounded settled inputs, generated documents, Memory Notes, publication, summary, and tool reads | one Project Scope, separate generation/use settings, bounded maintenance permission, atomic current publication, and Data-only Context |

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
wakeup delivery, worker lease recovery, Provider continuation and compaction
references, Memory generation/settings/publication/read paths, and every
disposable cache/index rebuild reader.

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
and command digest. The Server uses separate versioned HMAC-SHA256 domains for the opaque session-binding digest and one-use nonce. The Server secret is the HMAC key and never enters the message, durable record, response, diagnostic, or log. CORS is deny-by-default with an exact first-party origin;
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
available to the model, App, generated program, or arguments. Every
StoryOS-controlled nested external dispatch is a separate destination
operation; no controlled adapter may be an unrecorded egress proxy.

A Provider-hosted operation instead binds its complete enabled Tool set,
Registrations, ToolSpecs, explicit intake, permitted processors and outward
destinations, effect ceiling, current authority, required Approval, and finite
bounds before the owning Model Attempt submits. A shared Provider or previous
response reference gives a hosted Tool no Ambient Context. If the mode cannot
restrict intake or outward processing, refuse it; use a separate bounded
request only when it meets the same contract. Internal query values may be
unknown. Do not invent Host ToolCalls or dispatches for them. [S8]

Tool Approval targets one exact StoryOS ToolCall or one exact hosted operation;
neither approves the other. Destination Disclosure Approval grants no Tool
execution. An operation covered by the current Run Grant needs no extra
confirmation. Provider approval callbacks grant no author authority; a selected
callback path needs validated correlation and current admission before use.
The first hosted scope permits search, reading, and bounded temporary
computation. External business writes, messages, publication, and direct StoryOS
writes are excluded. Scratch files are not durable results; importing returned
files is a separate validated Host operation. Business writes use separately
authorized StoryOS ToolCalls, and creative changes retain Proposal/Acceptance.

**Verifiable evidence.** Tests mutate each contract field and annotation,
reuse names across servers, substitute token audience, attempt token
passthrough, invoke a hidden or app-only Tool, request undeclared effects,
redirect controlled nested egress, and return invalid/oversized results.
Hosted fixtures vary enabled tools, input access, outward destinations, effects,
Approval target, and enforceable bounds before submission. An unknown required
bound refuses the mode. Reported violations fence later Host work; they do not
retroactively undo internal work or become proof of per-step Host inspection.

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
or write story structure. Extraction, consolidation, or compaction can repeat
that text in apparently familiar summaries. If content position, signature,
ownership,
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
Credential Reference selection, or destination grant. Generated Memory and
compaction retain source roles and known input evidence; neither becomes a new
Host policy. General prompts guide model behavior but are not the enforcement
boundary. A returned-result rejection cannot undo already admitted hosted work;
its disclosure, effect, and uncertainty evidence must remain intact.

**Verifiable evidence.** A versioned adversarial corpus places equivalent
instructions in every source class, encoding, nested summary, Tool field,
App contribution, and retrieval rank. Tests prove the data may influence an
ordinary draft but cannot widen permitted discovery scope, cross scope, select
credentials, cause unapproved egress/effects, or bypass Proposal/Acceptance.

**Residual risk and owner.** A model may still produce poor or misleading
creative output from eligible malicious data; author inspection and evidence
quality address that product risk. Each StoryOS-controlled effect remains
Host-gated; hosted effects remain limited to the pre-admitted operation with
Provider-internal compliance uncertainty. A later result check cannot prevent
earlier internal consumption.
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

These hop-level checks apply to a StoryOS-controlled fetcher. For hosted search
or reading, ADR 0034 requires a validated way to bound intake, resources,
outward destinations, and effects before submission. StoryOS cannot claim to
observe or validate every internal DNS result or redirect. Unknown required
network controls make that mode ineligible. Returned URLs remain untrusted;
a later StoryOS fetch passes this full fetcher boundary. A Provider source
report is not a captured Research Source Snapshot.

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

## 17. AP-09: Provider continuation or hosted disclosure escapes admission

**Source to sink.** A substituted prior-response ID, account, endpoint, model,
Adapter, cache entry, implicit attachment, or hosted Tool exposes a different
conversation or wider input than the new request admits. A forged, partial,
rejected, or late result advances the active chain, or a small transport delta
conceals intentionally referenced prior state. A Provider cache flag or service
entitlement is mistaken for current permission or exact capability evidence.

**Affected assets.** Project confidentiality, Project Isolation, author intent,
continuation integrity, Credential confinement, and truthful disclosure history.

**Accepted controls.** Volcengine Agent Plan Responses is the first selected
route, not a blanket capability certification. The seven gates govern each
StoryOS submission. Current admission binds the actual input and required prior
references; the durable dispatch claim precedes I/O and initially records
OutcomeUnknown. Provider acknowledgement is not model-attention evidence. [S5] [S7]
Model and embedding submissions still bind exact purpose, destination, input,
wire payload, and Credential Reference. Implicit attachments and cross-Project
batches remain forbidden; embedding/cache keys retain the source, projection,
and Provider/model identity under AP-15.

**Required structural mitigation.** Preserve the immutable Model Continuation
Binding to its original Model Attempt, exact Project Scope and Project
Conversation, destination identity/evidence, Model Registration, Adapter mapping,
and original use/compatibility records. Every later Attempt obtains current
admission with its own use and compatibility facts. A reference grants no
permission, budget, or instruction authority. A new conversation cannot attach
to another conversation's chain, even inside the same Project. [S7]

A changed account boundary, destination, Registration, or Adapter cannot silently
reuse the old binding. Credential rotation alone does not prove account identity.
New runs bind their current grants and Project Instruction. Supply required
current instructions and the exact Working Target under the validated profile;
a cache or summary cannot silently replace them. Use a delta/reference only
under a mapping that represents the current request. Otherwise admit full input
or a new transport continuation without changing conversation identity.

Validate Responses transport, continuation, cache, compaction, hosted Tools,
retrieval, and cancellation separately, including their supported combinations.
Agent Plan entitlement is not general Ark endpoint or exact model/account proof.
Unknown required behavior blocks the route. Hidden SDK retries and model fallback
are ineligible; each physical resubmission has fresh admission and Attempt
and disclosure evidence. One hosted submission uses its owning Model Attempt
once; invisible internal steps are not separate Host submissions. [S7] [S8]

Preserve native item/call identities and result correlation. Only the selected
complete output whose entire Agent Decision validates and becomes durable may
advance ordinary continuation. Provider completion alone, partial arguments,
rejected output, cancelled work, and fenced late results cannot do so. A repair
path may use admitted Host validation diagnostics, not a rejected response's
whole prior-state handle. Retrieval for reconciliation is separately admitted
and never re-executes the original work. A complete validated outer result may
report an incomplete research objective with complete sub-results, sources,
and remaining gaps. It is distinct from an incomplete stream, and cannot
hide unknown effects or be presented as full task success.

Inspectors distinguish exact application-held and sent input, known references,
Provider reports, and opaque internal state. Preserve submission uncertainty;
do not infer exact internal content or minimum disclosure from a request delta,
cache hit, source citation, or final answer. Provider retention and training
statements are external claims, not local execution guarantees.

**Verifiable evidence.** Adapter and fake-destination fixtures substitute each
Scope, conversation, account, destination, credential generation, Registration,
use binding, compatibility Decision, prior reference, and native call identity.
They prove current admission, exact Host wire/reference association, immutable
original bindings, no cross-conversation reuse, and refusal of unsupported
combinations. Duplicate, reordered, forged, partial, rejected, and late output
cannot settle another Attempt or advance a fenced chain. Hosted fixtures test
whole-operation admission and single accounting, not simulated proof of unseen
Provider steps. Real-route checks qualify only the behavior actually observed.

**Residual risk and owner.** StoryOS cannot prove Provider attention, retention,
internal queries, enforcement, or legal disclosure. It can prove prepared input,
local dispatch, attributable reports, and best-known receipt. Observed scope
violations block later work and ordinary intake under the existing drift and
safety contract, while preserving prior disclosure and unknown effects.
The Model, Tool, and Context owners retain these semantics; protocol represents
them, retention preserves their evidence, and the existing proof/release owners
validate the accepted boundary before its product stage is released.

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

**Accepted controls.** ADR 0021 and ADR 0022 own current recovery custody:
hosted production uses vendor-held daily physical backups; StoryOS does not possess production WAL
files. The existing local OrbStack drill proves isolated recovery mechanics,
not hosted acceptance. Production compatibility and recovery remain deferred
to deployment preparation. A Project Export Archive is not a database backup. [S10]

**Required structural mitigation.** Backup and archive roles remain separate
from runtime and migration, and reader/writer privileges remain separate where
the backend permits. Backup/WAL objects are encrypted, versioned or
non-overwriting, gap-checked, retained by policy, and never exposed through a
Project endpoint. Restore occurs in an isolated target with traffic disabled;
externalized configuration, role grants, migration checksums, credential
references, and scope invariants are re-established before cutover.

**Verifiable evidence.** Each release-candidate recovery drill combines
manifest/checksum verification with actual startup of an isolated restore,
role/grant and forced-RLS inspection, cross-scope negative tests, projection
rebuild equality, canary secret scan, and measured recovery bounds. WAL-specific
checks apply to the local physical-chain oracle. Hosted proof needs vendor
backup evidence and a restored non-live project that passes visibility and
continued-writing checks; local success certifies neither vendor RPO nor custody.

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
Attempt or Admission under its owning contract. A stale or cancelled result may
reconcile external evidence and usage; it cannot supply a new Agent Decision,
execute business Tools, or advance a fenced continuation.

Model recovery distinguishes confirmed continuation unavailability from an
unknown create outcome. Settled expiry may rebuild admitted context. For an
unknown create, try permitted retrieval first. ADR 0033 allows at most one
automatic successor only with the same request/route, fresh current admission,
budget for both Attempts, and no unresolved Tool or hosted effect. Persist the
predecessor fence before that successor; restart cannot reset the allowance.
Cancellation prohibits it. Read-only intent does not prove that a lost hosted
operation had no effect, cost, or disclosure. A local timeout or requested abort
does not prove remote termination. Persist the Host cancellation fence before
best-effort Provider abort. Revocation and observed bound violations fence new
Host work without promising remote rollback. [S7] [S8]

**Verifiable evidence.** Crash injection at every durability/egress/response
edge plus acknowledgement-before/after-SSE permutations, tab reload, stale
writer takeover, duplicated, reordered, and delayed deliveries proves one
canonical effect, stable replay, preserved uncertainty, exactly one terminal
Admission settlement, Draft preservation where required, and no cross-scope
key collision. Model fixtures also distinguish expiry from unknown create,
retain reservation and late evidence, enforce the one-successor limit across
restart, and forbid it after cancellation or unresolved hosted work.

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

## 23. AP-15: Retrieval or cache reuse bypasses current access

**Source to sink.** Poisoned content, forged source identity, cross-scope index
rows, global cache keys, or a revoked grant enters fresh retrieval or a reused
context reference. Ranking is mistaken for permission, or an ordinary creative
correction is incorrectly treated as proof that every recorded copy and past
model influence has been erased. [R1]

**Affected assets.** Project Isolation, current source access, recorded history,
minimum disclosure, and honest inspection of stale or unavailable material.

**Accepted controls.** Indexes and retrieval caches are rebuildable projections.
Recorded Messages, captured results, and Memory Documents have their own durable
identities and lifecycle. Current source reads and recorded history are distinct;
ordinary source edits, deletion, or non-use guidance do not rewrite history or
automatically invalidate its continuation chain. [S5] [S9]

**Required structural mitigation.** Bind fresh reads and retrieval keys to exact
Project Scope, current source/publication identity, applicable lifecycle/access,
projection revision, and Provider/model when used. Apply eligibility before
ranking and check the actual dependencies again at final admission. Indexes,
cache hits, known links, and prior grants never grant access.

Enforce actual access revocation, retained-copy redaction/deletion, and Project
Isolation on every copy covered by the owning policy. Stop opaque-reference
reuse when these restrictions cannot be enforced; any rebuilt input needs
current admission. If affected generated content cannot be safely isolated,
use the conservative document-set unavailability/rebuild boundary defined by
storage and retention, not a model-generated semantic influence graph.

Append ordinary corrections as Messages or Steering Input. A Memory Note asks
for later consolidation; it is neither a deletion receipt nor an access ban.
Current-source unavailability and stale remembered context remain inspectable.
No semantic exclusion classifier, per-claim Memory admission, or automatic
ordinary-correction chain reset is introduced. A missing required current input
can still block the next request. General compaction can prepare a later call
without erasing earlier requests or certifying precise forgetting.

**Verifiable evidence.** Fixtures substitute source/publication revisions,
Scope, policy, use/generation settings, and grants before reads and dispatch.
They prove denied fresh access and refusal of forbidden retained-copy reuse.
Separate cases show that ordinary steering appends input without rewriting prior
records or triggering a semantic reset. Deterministic index rebuilds preserve
eligible stored inputs; regenerated Memory or compaction wording need not match.
No assertion claims that the model forgot a fact or stopped all semantic use.

**Residual risk and owner.** Permitted stale or hostile data may affect quality.
The Context and Memory contracts own interpretation and intake; storage and
retention own real-copy restrictions and publication, protocol exposes these
facts, and the proof owner tests observable access and history boundaries.

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
carry stable identity, schema/version, timestamps, known source references,
actor/authority basis, and digest
where defined. A projection is disposable and reconstructs from canonical
facts; retention or redaction creates explicit tombstone/summary evidence and
cannot silently change the meaning of surviving references. Migration and
restore preserve or explicitly transform facts under checksummed versions.

Active context compaction records known inputs/prior projections, producer,
output or validated opaque reference, and loss/unknown facts for a later request.
It preserves earlier Messages, results, Steps, and request evidence under their
retention contract. Additional model work is separately admitted. A known input
list is not a complete semantic influence closure, and a generated summary is
not a replacement for original Research evidence or instruction authority.

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
inspectable, and bounded; one Host-injected item may never exceed 10K tokens.
This is not a claim to count opaque Provider-internal items.

**Required structural mitigation.** Every remaining crossing declares byte,
item, time, token, depth, and attempt budgets with hard server-side ceilings.
Work admission and queues are
scope-aware, cancellable, and backpressured; partial results cannot bypass
validation. Replay past the retained or bounded window requires a Snapshot
handoff. Recovery capacity and maintenance connections are reserved from
ordinary work. Memory extraction, consolidation, publication, navigation, and
reads have separate finite input/work bounds and cannot block ordinary writing.

Hosted profiles identify the actual controls and finite worst-case reservation
for cost, output, Tool use, time, and resources. A best-effort Tool counter is not
a hard ceiling; a local timeout bounds Host waiting, not remote cost or execution.
Refuse a route whose required bound is unknown or unenforceable. Settle one
physical use once, and retain unknown usage and its required reservation. [S8]

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
Admission and Receipt lifecycles remain durable Operational Records. A
`RefusedEditDraft` remains a Draft Artifact. Only an `ApplyAuthorEdit` Core
Transition with the `RefusedToDraft` effect creates it. A `RecoveryDraft`
remains a Draft Artifact. The Host creates it only under the closed Editor
Recovery ingress matrix. Preserved browser-local records, in-memory text,
clipboard text, and Pending Edit Projections remain local-only and are not
Artifacts.

**Required structural mitigation.** Writer takeover monotonically advances the
Project writer generation and makes every older editor read-only. Every
submission and reconciliation binds the current Protected Web Client, Client
Session, client-contract, security-policy, Editor Session, writer generation,
scope, action class, digest, target/Heads, nonce/idempotency identities, and
lifetime. Reload and reconnect use authoritative snapshots and typed Receipts;
local cache, journal, projection, missing HTTP response, and SSE ordering cannot
settle or retry a command. Recovery follows AP-14: only the exact eligible
direct-edit branch may reuse its still-live Admission after read-only proof;
otherwise the text remains available to the author, and the author visibly
reconfirms a new command.

**Verifiable evidence.** Two-tab and multi-device fixtures pause at every
journal, submission, commit, acknowledgement, Event, takeover, reload, and
reconciliation edge. Old generations cannot mutate; duplicates converge to one
Receipt; changed or expired commands require reconfirmation; corrupt or missing
local state cannot delete canonical prose; and all refused or unrecoverable
content remains inspectable and copyable. Preserved browser-local records,
in-memory text, clipboard text, and Pending Edit Projections are not Artifacts.
The fixed Draft Artifact classification applies to a `RefusedEditDraft` only
when an `ApplyAuthorEdit` Core Transition has the `RefusedToDraft` effect. It
also applies to a `RecoveryDraft` created by the Host under the closed Editor
Recovery ingress matrix.

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
complete controlled registration is external. Its StoryOS-controlled submission
passes the seven gates, manifest-before-egress, Credential Resolver, and ordinary
OutcomeUnknown handling. A Provider-hosted internal processor remains external
and is bounded through the owning operation before submission, not reclassified
as controlled or represented by an invented Host gate. Its unobserved transfers
remain unknown. Shared infrastructure never aliases User, Project,
credential, cache, or idempotency identities.

**Verifiable evidence.** Multi-User deployment fixtures vary hostname, region,
VPC/private link, provider/account ownership, shared pool/queue/cache,
maintenance credential, TLS identity, telemetry route, and registration
revision independently. Only exact controlled registrations retain the
classification; every other path either preserves complete Project Isolation
under its controlled contract or has prior external-disclosure admission at the
actual Host boundary. Hosted fixtures refuse unbounded modes and preserve
reported/unknown internal processing without certifying every internal hop.

**Residual risk and owner.** A privileged cloud control-plane or platform
administrator remains capable of violating its intended boundary and requires
operational and recovery evidence beyond application isolation. Controlled
destination and disclosure semantics remain with their existing Context,
Provider, Tool/MCP, Protocol, PostgreSQL, and retention owners; `DVG-02`,
`DVG-04`, `DVG-05`, `DVG-10`, and `DVG-11` own classification, isolation,
egress, credential, and restore evidence.

## 30. AP-22: Generated Memory launders content or maintenance authority

**Source to sink.** Hostile or stale Messages, source material, prior summaries,
or forged Memory Notes pass through extraction and consolidation into familiar
navigation and recall text. An injected procedure asks a maintenance Agent to
alter policy, Skills, another Project, or Authoritative State. A stale worker
publishes over newer documents, or a note is reported as completed erasure.

**Affected assets.** Project Isolation, fiction authority, maintenance grants,
source evidence, current publication, and truthful recall/change status.

**Accepted controls.** Memory Documents are versioned, non-authoritative
Artifacts generated from eligible prior conversation snapshots. Separate settings control generation
and use. Background extraction, consolidation, active compaction, and Provider
continuation are distinct. Known source references do not prove each statement
or complete semantic ancestry. Markdown does not create a file store. [S9]

**Required structural mitigation.** Extract only bounded durable snapshots of
sufficiently idle conversations whose scope, access, and generation settings
permit that job. Provisional streams and active editor buffers are not inputs.
Bind exact inputs, model/prompt versions, job/ToolCall authority, and current
publication version. Model work uses normal Context/disclosure admission.

Memory maintenance may update only its admitted generated-document set. It
cannot install Skills, change ToolSpecs, select credentials, widen policy,
create Author Preferences, or write Authoritative State. Direct Memory-update
tools require an author request; background extraction uses its separate
eligible-input contract. A source telling the model to create a note cannot
supply that author request. Validate result role, scope, source associations,
size, and secret handling without pretending to validate every claim's truth.

Only one publication advances the Project's current set at a time; fence stale
jobs and publish the complete set atomically. Revalidate current maintenance
permission and any source or retained-copy restriction that governs publication.
A job cannot publish forbidden content merely because it was eligible at dispatch.
Generation settings govern new extraction; disabling use does not itself delete
published documents or rewrite recorded context.
Preserve earlier revision identities used by recorded contexts under retention.
Summary and search/read tools expose only currently permitted published content
within their bounds, and never treat a citation or read count as model attention.

A Memory Note records a requested later change, not a published update, access
prohibition, or physical deletion. Apply AP-15 to actual copy restrictions.
General prompts may ask for uncertainty, current-source checks, and useful
recall. They do not create a semantic classifier or require per-claim approval.

**Verifiable evidence.** Feed hostile instructions and false citations through
both phases and recall tools; observe requested maintenance operations at the
real Host boundary. Wrong Scope, prohibited writes, forged author-note origin,
disabled generation, unavailable inputs, oversized output, duplicate delivery,
stale publication, and partial document sets are refused without authority
change. Disable use independently of generation; distinguish note-recorded,
publication-success, no-op, failure, and unavailable-source observations.
Rebuild indexes from published documents without model calls. Tests do not
require identical regenerated prose, truthful inference, or exact forgetting.

**Residual risk and owner.** Memory may preserve bad advice, lose useful context,
or repeat old material even when every access check succeeds. Inspection,
current author guidance, and current sources help the Agent; they are not
semantic guarantees. The Memory owner retains its mechanism, storage and
retention define current publication and covered-copy restrictions, protocol
exposes inspectable outcomes, and proof/release own later implementation gates.

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
| Project isolation | client, runtime, DB, cache, cursor, archive, backup, destination, Memory, or recovery input → another User/Project | AP-01–AP-22 | `DVG-02`, `DVG-04`, `DVG-05`, `DVG-07`–`DVG-11` |
| Provider continuation and hosted work | substituted reference/account, implicit Tool intake, or forged result → wider disclosure or active-chain advancement | AP-06, AP-08, AP-09, AP-14, AP-18, AP-21 | `DVG-04`–`DVG-08`, `DVG-11` |
| ordinary steering, compaction, and real revocation | old source or generated summary → false erasure claim, forbidden reuse, or rewritten history | AP-07, AP-15, AP-16 | `DVG-04`, `DVG-07`–`DVG-09`, `DVG-11` |
| generated Memory and maintenance | poisoned prior records or stale job → authority promotion, cross-scope recall, or partial publication | AP-07, AP-15, AP-22 | `DVG-04`–`DVG-07`, `DVG-09`, `DVG-11` |

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
| AP-22 | `DVG-04`, `DVG-05`, `DVG-06`, `DVG-07`, `DVG-09`, `DVG-11` |

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
resolution, or unmanifested egress. AP-22 is calibrated by the same actual
impact: bad remembered advice is a quality limit; cross-scope recall or an
unauthorized maintenance write is a security failure.

# Downstream Security Handoff

This threat model owns the attack paths and Foundation security objectives.
It deliberately leaves each implementable contract or proof with one existing
Wayfinder owner:

| Downstream owner | Security obligations received from this model | Attack paths |
|---|---|---|
| [Specify Author Command Admission](https://github.com/FrankQDWang/StoryOS/issues/68) | exact protected-client, User, existing/prospective Project Scope, editor/writer, action, digest, nonce/idempotency, and lifetime binding; bounded claim ceiling; one append-only lifecycle and terminal settlement; direct-versus-explicit recovery | AP-01, AP-03, AP-14, AP-20 |
| [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70) | Local Edit Journal validation; non-authoritative pending projection; one Project writer generation; stale-tab fencing; acknowledgement/Event convergence; resync; Draft preservation; explicit-command reconfirmation | AP-01, AP-03, AP-04, AP-14, AP-17, AP-18, AP-20 |
| [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | exact protected-client/requester/scope envelopes; non-oracular errors; CSRF and Origin/Host inputs; build, client-contract, security-policy and session identities; scoped SSE cursors and Snapshot handoff; idempotency/Attempt/OutcomeUnknown/fence states; Capability, bridge, Tool/MCP and credential-reference contracts; import/export schema; controlled/external destination manifests; explicit hard budgets; continuation bindings, hosted Approval targets and report/unknown evidence; Memory notes/settings/publication | AP-01, AP-03–AP-10, AP-13–AP-22 |
| [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md) | replay floors and Snapshot semantics; Admission/Attempt/outbox/Mailbox/late-result evidence; immutable-history compaction, redaction, tombstones and known source references; logs/support/telemetry classification and expiry; disclosure, export, backup/WAL and restore-proof retention; known Memory inputs, document revisions and actual copy restrictions | AP-04, AP-09–AP-12, AP-14–AP-18, AP-20–AP-22 |
| [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | cross-scope and role/RLS tests; hostile-origin, XSS/DOM, asset/dependency, third-party-script, extension, stale-build/tab, controlled-cloud, bridge/Tool/MCP/prompt/SSRF/provider/archive corpora; claim-ceiling checks; secret and log leak scanning; adapter-wire comparison; fault, retry, fence, replay, rebuild, tamper, restore and resource-bound proofs | AP-01–AP-22 |
| [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62) | refuse production-shaped handoff until the slice demonstrates exact Protected Web Client release identity, non-owner forced-RLS runtime, exact-scoped HTTP/SSE, manifest-before-egress with Credential Resolver, mediated Tool/MCP boundary, durable Attempt/recovery, bounded input, safe operational defaults and actual restore evidence at the slice's accepted scope | AP-01–AP-22 |

The [PostgreSQL storage owner](postgresql-project-storage-isolation-and-migration-contract.md)
receives AP-09, AP-15, AP-16, and AP-22: scoped immutable associations, current
publication, rebuildable indexes, and conservative enforcement of covered-copy
restrictions. It owns physical representation and any required migration.

The protocol owner next maps continuation/reference identity, whole hosted
operation/Approval targets, native result correlation, evidence classes, and
Memory settings/notes/publication to versioned records. Storage and retention
then align physical families, current reads, publication, exports, and real-copy
restrictions. Proof and release must update their existing crosswalks for AP-22,
ordinary steering, compaction, hosted opacity, and bounded model recovery.
Deterministic proof observes Host gates and records, not model truth, attention,
exact forgetting, or every internal Provider hop. Source drift or an unbounded
selected mode returns to the original semantic owner before acceptance.

This document changes no runtime, persisted format, generated API, or security
platform. Existing editor implementation remains the code truth; the new Agent
obligations are not claimed as implemented or tested product behavior. Stage 3
and later implementation stays on EXECUTION HOLD until the existing protocol,
storage, retention, release, proof, specification, and ticket chain is aligned.

No separate parallel security map or security runtime follows from this threat
model. This contract owns trusted-computing boundaries, source-to-sink attack
analysis, structural mitigations, and residual risks. It does not re-own the
evidence classification fixed by the Artifact contract, Admission
identity/lifecycle/settlement, Core effects, editor recovery, versioned wire
shapes, or deterministic gate selection owned by the linked contracts above. Those owners must close their assigned contracts and
deterministic negative evidence in the map's single serial chain before the
editor-first implementation handoff.

# Source Index

Repository sources are fixed StoryOS facts; external sources establish only
the cited platform or protocol behavior. Existing external sources were
accessed for the original model on 2026-07-21; the protected-client primary
source review records its own 2026-07-24 access date. The Model, hosted Tool,
Context, and Memory revision uses accepted September 2026 contract inputs;
external capability evidence remains distinct from exact account acceptance.

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
- **[S7]** [ADR 0033](../adr/0033-use-volcengine-responses-for-the-first-real-model-path.md):
  selected Provider, continuation bindings, current admission, complete results,
  compaction, one-successor recovery, and cancellation fences.
- **[S8]** [ADR 0034](../adr/0034-bound-provider-hosted-tool-operations.md):
  complete hosted intake/Tool/outward/effect/resource bounds, Approval targets,
  single physical accounting, report/unknown evidence, and result/recovery limits.
- **[S9]** [Memory contract](fiction-memory-and-research-provenance-semantics.md)
  and [ADR 0035](../adr/0035-use-background-generated-project-memory.md):
  bounded extraction/consolidation, publication, selective recall, ordinary
  corrections, fallible source links, and actual retained-copy restrictions.
- **[S10]** [ADR 0022](../adr/0022-prefer-widely-validated-hosted-infrastructure.md)
  and [ADR 0021](../adr/0021-own-release-1-recovery-chain-outside-the-runtime.md):
  hosted production data and recovery custody, local development, and separate
  deployment acceptance.
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
- **[E3]** [Volcengine Agent Plan Codex setup](https://docs.volcengine.com/docs/82379/2556054?lang=zh):
  Agent Plan route, Responses configuration, and dedicated key instructions.
- **[E4]** [Volcengine Create Response](https://docs.volcengine.com/docs/82379/1569618?lang=zh)
  and [Base URL and authentication](https://docs.volcengine.com/docs/82379/1298459?lang=zh):
  general Ark input/reference, storage, hosted Tool, and best-effort limit fields;
  not exact Agent Plan account acceptance.
- **[E5]** [Volcengine context cache](https://docs.volcengine.com/docs/82379/1398933?lang=zh):
  separate implicit/explicit cache, lifecycle, initial Tool definitions, and
  instruction/structured-output combination restrictions.
- **[E6]** [Volcengine Retrieve Response](https://docs.volcengine.com/docs/82379/1783709?lang=zh)
  and [Delete Response](https://docs.volcengine.com/docs/82379/1584286?lang=zh):
  documented retrieval/deletion behavior, not cancellation or create-idempotency proof.
- **[E7]** [Official ArkCLI helper](https://github.com/volcengine/ark-cli/blob/main/skills/arkcli-helper/references/arkcli-helper.md):
  independently configured search MCP and Harness capability boundaries.
- **[K1]** [Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services/):
  protected credential storage and controlled item access on the local host.
- **[F1]** [Python tarfile extraction filters](https://docs.python.org/3/library/tarfile.html#extraction-filters):
  path, link, device, metadata, and denial-of-service hazards when extracting
  untrusted archives.
- **[R1]** [OWASP RAG Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/RAG_Security_Cheat_Sheet.html):
  poisoning, provenance, access enforcement, and tenant-isolation risks.

Repository: FrankQDWang/StoryOS

Version: codex-trust-contract-2026-09-14-v1
