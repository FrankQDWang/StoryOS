# StoryOS

StoryOS is a novel-project workspace in which the author retains authority over creative truth while Agents, Tools, Skills, and MCP Apps produce inspectable assistance around it.

## Language

**User**:
A stable StoryOS principal identified by one durable `UserId`. The Foundation Validation Deployment bootstraps one local User without requiring a login or account-management product, while the same identity contract permits a later StoryOS service to host many isolated Users; credentials, display names, pen names, and billing accounts are not the User's domain identity.
_Avoid_: Operating-system current user, global singleton, login session, pen name, account feature set

**Client Session Binding**:
The opaque server-held request-authentication binding established by a trusted local bootstrap or a future identity flow for one server-derived User, exact allowed Host and first-party Origin, bounded lifetime, and browser session handle. Every state-changing request also consumes a non-reusable anti-forgery nonce bound to that Binding, exact Project Scope, method, command kind, idempotency record, and canonical command digest; the Binding is an authenticated input to Author Command Admission.
_Avoid_: Login session as User identity, client-asserted role, URL access token, reusable command nonce

**Project Author**:
The one User who owns a Project and may exercise its author-only intents, settings, Acceptance, and other creative-authority commands. `Author` names this project-scoped role rather than a second durable person identity; shared ownership, collaborators, ownership transfer, and multi-author editing require a separate later contract.
_Avoid_: Separate AuthorId, display byline, collaborator, global author singleton

**Project**:
A durable novel workspace identified by one `ProjectId` and owned by one User acting as its Project Author. Its identity is independent of filesystem path, database placement, deployment location, or display name; the current Foundation has no shared ownership or ownership transfer.
_Avoid_: Project directory, database, tenant, collaboration workspace

**Project Scope**:
The trusted pair `{ owner_user_id: UserId, project_id: ProjectId }` that identifies one Project and its sole Project Author. Every project-scoped operation, durable record, reference, index entry, cache key, context decision, and disclosure must bind this pair directly or through a currently validated Project reference; caller-supplied fields, a process-global current User, or ProjectId alone never establish access authority.
_Avoid_: Tenant inferred from session, project path, unscoped global ID, client-asserted owner

**Authoritative State**:
The author-approved current truth of a novel project, including prose, established fictional-world truth, characters, relationships, timeline, and manuscript structure. Authority is a binary boundary reached only through an explicit author-authorized domain action; lifecycle, confidence, and lock status do not form authority levels.
_Avoid_: Canon (too narrow), accepted artifact, Agent memory

**Discovery Writing**:
The StoryOS authorship model, inspired by Dean Koontz's page-by-page process, in which the author develops the novel from a live premise and characters, repeatedly refines the current passage, and discovers the story through writing, while Agent assistance stays grounded in the current passage and the author's present creative choices.

**AI-Independent Editor Baseline**:
The first author-visible StoryOS release capability: from a new or controlled Project initialization, the author can organize volumes and chapters, write and revise manually, see save state, recover after reload or crash, navigate, search and replace, inspect basic writing statistics, use supported keyboard, clipboard, IME and undo behavior, sustain long sessions, and create a human-readable export without an available Agent or model.
_Avoid_: Chat-only demo, AI-dependent editor, pre-seeded current-passage demo

**Author Preference**:
An explicit, future-facing, scope-bounded author-owned creative or working constraint within Authoritative State. When an unambiguous author instruction maps deterministically to its domain action, that instruction is its authorization; local or ambiguous feedback cannot create one and may only source an Inferred Preference.
_Avoid_: Inferred Preference, hidden rule, globalized feedback, memory confirmation

**Project Instruction**:
An optional, author-edited, project-scoped instruction whose immutable Revisions are created through project settings and may define language, expression, author-Agent working style, and long-lived working requirements. Its absence never prevents the Default Context Experience, while its presence has Instruction Authority for model context but grants no Capability or disclosure permission, bypasses no Proposal or Acceptance boundary, and does not turn asserted story facts or an outline into Authoritative State or an Author Plan.
_Avoid_: Required project setup, system prompt, Author Plan, project document as canon

**Project Instruction Binding**:
The immutable top-level AgentRun fact that binds either the exact effective Project Instruction Revision at Run creation or the explicit fact that none was configured; every descendant Subrun and applicable Context Assembly Manifest independently references the same binding. Ordinary setting updates affect only new top-level AgentRuns, current-run changes require scoped Steering Input, and later ineligibility of the bound Revision makes dependent work explicitly Degraded or Blocked rather than switching, continuing, or rewriting prior Steps.
_Avoid_: Mutable active instruction, latest settings lookup, implicit Subrun inheritance, session prompt copy

**Project Ownership Boundary**:
The current Foundation rule that every Project has one exact Project Author and one immutable Project Scope, and every project-scoped durable object remains bound to both identities. StoryOS may serve multiple isolated Users and Projects without changing this contract, while shared project ownership, multi-author collaboration, ownership transfer, and cross-author access require a separate later domain design rather than a singleton-User assumption or an implicit shared workspace.
_Avoid_: Global current user, single-tenant shortcut, shared project, collaboration role model

**Project Isolation**:
The fail-closed StoryOS boundary that prevents Authoritative State, Artifacts, Operational Records, Context Candidates, manifests, caches, indexes, Credential References, Processing Destination Identities, authorized project-use bindings, and destination disclosures from being discovered, joined, retrieved, authorized, or reused across either member of a Project Scope. Every such record and disposable projection must retain the exact `owner_user_id` and `project_id`, and the authoritative persistence boundary independently rejects scope-mismatched reads, writes, references, and joins; caller-side filtering is not a substitute, while missing or ambiguous identity makes the operation ineligible.
_Avoid_: UI-only filtering, global vector namespace, shared prompt cache, caller-supplied project scope

**Project Export Archive**:
A versioned, self-describing, integrity-protected portable archive of one exact Project Scope, produced from one transactionally consistent boundary and containing every non-secret currently exportable canonical record and immutable payload, plus lifecycle, redaction, retention, and provenance facts for any known gap, required to restore that Project without consulting a disposable projection or external runtime. Every entry name is admitted under the manifest's exact versioned Archive Path Profile before sorting or digest coverage, so platform path rules, Unicode normalization, or case behavior cannot reinterpret archive identity. It preserves original User, Project, and object identities; excludes caches, retrieval and embedding projections, secret material, and Provider-held state; and grants neither destination access nor ownership transfer.
_Avoid_: Selected-table dump, backup, cache snapshot, credential bundle, project copy

**Project Restore**:
The validating import of one Project Export Archive as the same Project Scope into a target that is authorized for the same durable User identity and does not already contain that Scope. Restore stages and verifies the complete archive, schema compatibility, digests, referential closure, scope, and known lifecycle/redaction gaps before making the Project atomically visible, then deterministically rebuilds disposable projections; any identity conflict, partial archive, unsupported schema, or divergent existing Project fails closed without merge, overwrite, identity remapping, or resurrection of unavailable payload. Unresolvable Credential References remain explicitly Unbound. Creating a copy, fork, new Project identity, or ownership transfer requires a separate future domain contract.
_Avoid_: Import as new, ID remapping, partial merge, overwrite restore, ownership transfer

**Foundation Validation Deployment**:
The initial product stage in which one bootstrapped User uses StoryOS to write a real novel while exercising the same Project Scope and Project Isolation contracts required when more Users are served later. It is a validation stage, not a distinct single-user domain model or permission shortcut; deployment and persistence choices belong to architecture decisions.
_Avoid_: Product-wide single-user mode, global current User, throwaway domain model, implicit Project access

**Foundation Monorepo**:
The one StoryOS repository that jointly governs the Rust workspace, production Web Client, external-contract source, and checked-in generated contract artifacts so a compatible product change is reviewed and reproducibly verified as one unit. It does not make internal package boundaries an author setting or admit disposable prototypes or `.reference` as production members.
_Avoid_: Split runtime repositories, separately authoritative generated SDK, prototype workspace

**Server/Worker Separation Boundary**:
The modular-monolith boundary in which the Server owns public transport and trusted request admission, Core owns authoritative transitions, and the Worker executes only durably claimed asynchronous or external work through Core-owned contracts. Server and Worker remain independently startable and deployable, while the Foundation default may co-locate them; neither becomes a separate authority store, microservice, or broker-owned workflow.
_Avoid_: HTTP background thread as recovery boundary, mandatory separate service fleet, worker-owned truth

**Prototype Evidence Asset**:
A disposable, bounded-risk experiment retained only to reproduce and inspect the exact question, environment, observations, and limitations that informed an accepted StoryOS contract. It is frozen rather than absorbed into production, and may be deleted only through an explicit reviewable decision after its durable evidence record is sufficient.
_Avoid_: Production seed, evolving pre-production branch, runtime dependency, unrecorded experiment

**Reference Evidence Locator**:
A repository-owned non-runtime record that makes an upstream design or source observation reproducible by naming its canonical location, exact immutable revision or digest, license, retrieval date, and relevant scope. A local `.reference` snapshot is not a Locator merely because it exists and never becomes a production dependency, workspace member, test input, or accepted evidence without this independently reviewable identity.
_Avoid_: Machine-local snapshot, vendored runtime source, implicit dependency, unpinned citation

**Foundation Recovery Service Profile**:
The minimum durability and disaster-recovery promise for the Foundation Validation Deployment. Every author-visible successful commit survives an ordinary process or power crash with zero acknowledged-data loss; loss of the database host or disk has a recovery-point objective of at most fifteen minutes and a recovery-time objective of at most two hours. The deployment therefore uses synchronous PostgreSQL commit durability, a daily physical base backup plus continuous WAL archival into a failure domain independent of the database host, and a successful automated restore proof for every release candidate. Backup retention duration belongs to the later retention contract, but every claimed window must retain a complete verifiable recovery chain and Recovery Visibility Proof before a restored Project becomes readable. This Profile does not require a synchronous replica, automatic failover, or a high-availability cluster, and later controlled-cloud deployments may declare a stricter profile.
_Avoid_: Asynchronous author acknowledgement, same-disk backup, daily dump only, untested backup file, Foundation high-availability cluster

**Recovery Copy**:
A bounded PostgreSQL base backup, WAL segment, or equivalent recovery-chain member retained only to meet the Foundation Recovery Service Profile, never as a Project Export, ordinary read source, or independent authority. It remains subject to the exact retained lifecycle ledger and can serve a Project only through a successful Recovery Visibility Proof.
_Avoid_: Export archive, cold Project copy, raw recovery database, alternate source of truth

**Recovery Visibility Proof**:
The inspectable determination that a restored Project Scope includes and has applied every recoverable later lifecycle decision relevant to the selected recovery target, including Redaction, Tombstone, retention, and availability gaps, before any ordinary read or execution is enabled. A missing or unverifiable lifecycle range fails closed to a recovery hold rather than exposing an older view as current.
_Avoid_: Successful database boot, point-in-time restore alone, best-effort lifecycle replay

**Non-Revival Recovery Oracle**:
The deterministic recovery and replay test rule that compares recovered state against retained historical facts plus current lifecycle availability, not against a demand to reproduce unavailable payload bytes. It proves that Receipts, causation, replay/resync, provenance, and explicit availability gaps remain truthful while Redaction, Tombstone, compaction, archive, export, restore, cache, projection, and Provider continuity cannot make unavailable payload visible, eligible, or newly authoritative.
_Avoid_: Byte-identical redacted replay, tombstone-only assertion, cache resurrection, silent availability gap

**Physical Deletion Completion**:
The lifecycle fact recorded only after every StoryOS-controlled online, archive, and Recovery Copy retention window authorized for an erased payload has expired or been verifiably cleaned. It does not claim deletion from an already delivered Project Export or external destination and never changes the earlier logical Redaction or Tombstone effect.
_Avoid_: Immediate disk wipe claim, Provider erasure, logical redaction alone

**Project Deletion Request**:
The explicit author-owned command that begins deletion of one exact Project Scope, atomically preventing new Runs, outbox dispatch, Context Assembly, Export, Restore, and outbound disclosure while existing work enters controlled cancellation or recovery settlement. It is never inferred from a Retention Profile, archive threshold, or cache cleanup.
_Avoid_: Automatic project expiry, table drop, bulk cache eviction, implicit account deletion

**Project Deletion Settlement**:
The immutable terminal lifecycle decision reached after a Project Deletion Request records every known in-flight operation as settled or OutcomeUnknown, fences future work, and makes the Project Scope logically unreadable, unexecutable, unexportable, and unrestorable. It starts physical cleanup under the Retention Profile and preserves only the minimum deletion, availability, and external-effect evidence until Physical Deletion Completion.
_Avoid_: Immediate disk wipe, cancelled request, restored Project, forgotten external effect

**Effective Model Context**:
The Effective Destination Context for one Model Attempt, including the AgentRun's bound Project Instruction Revision whenever one exists. Compaction, window management, provider cache, prior-response, and continuity mechanisms may optimize transport only when StoryOS can prove the bound instruction remains logically present; otherwise it is retransmitted or the Attempt is blocked.
_Avoid_: Request delta, initial prompt only, provider session assumption

**Instruction Precedence**:
The fixed order in which StoryOS product, domain, safety, permission, Capability, ToolSpec protocol, Proposal, and Acceptance boundaries outrank current exact author instructions, Steering Input, Approval, and Wait answers; those outrank the Project Instruction Binding; it outranks selected Skill Instruction Context; and all outrank Data-only Context. Within one authority layer, a current applicable instruction with more specific scope prevails, while no lower layer can waive a higher boundary.
_Avoid_: Prompt order, last text wins, Skill override, model-chosen priority

**Fiction Assertion**:
An addressable fiction-domain statement composed of one Proposition, Story Scope, and Epistemic Scope. Its owning Authoritative Revision or Artifact determines authority; the Assertion shape itself does not, and only incompatible Propositions under genuinely overlapping scopes conflict.
_Avoid_: Flat entity property, universal canon fact, memory fact

**Proposition**:
The content asserted about exact project subjects, separate from where or when it applies, who holds or communicates it, and whether its owning source is authoritative.
_Avoid_: Unscoped fact, belief, database field

**Story Scope**:
The work, fictional world or continuity, branch, story time, scene, and narrative position within which a Fiction Assertion applies. Story-world validity and narrative revelation are distinct, while audit time is neither.
_Avoid_: Audit timestamp, chapter number as story time, global story fact

**Epistemic Scope**:
The holder or in-fiction source and epistemic relation that frames a Proposition as project fact or as something known, believed, suspected, claimed, remembered, or retold. Different holders and relations are not interchangeable, and project truth never automatically becomes character knowledge or permitted revelation.
_Avoid_: Belief owner only, narrator truth, automatic knowledge propagation

**Authoritative Revision**:
An immutable version of one authoritative domain object, created through a Direct Author Action, Acceptance, or safe compensation and guarded by an expected prior revision.
_Avoid_: Artifact Revision, mutable row

**Authoritative Commit**:
The Project Scope-ordered atomic record of one author-authorized domain transaction, identifying its Project Scope, actor, cause, and all prior and resulting Authoritative Revisions. Its scope-local sequence begins at one and advances without gaps only when an authority-changing transaction commits; refused, failed, and no-change attempts have no Commit sequence.
_Avoid_: Project snapshot, Run Event, attempted-command sequence, wall-clock order

**Durable Identity**:
A stable, opaque, strongly typed identity assigned to one durable StoryOS entity or record. Identity types are not interchangeable, and an identity never conveys authority, causality, freshness, project order, or capability.
_Avoid_: Content Digest, sortable clock, shared string ID, capability token

**Revision Lineage**:
The single-parent linear history of immutable Revisions under one Durable Identity, with every append guarded by the exact expected current Revision. A stale append conflicts without branching, overwriting, or automatic rebasing; alternative work receives a new Proposal or Artifact identity linked through Provenance.
_Avoid_: Revision tree, last-write-wins, implicit rebase, mutable history

**Revision Envelope**:
The common immutable identity, lineage, schema, creator, cause, audit-time, payload, and integrity boundary carried by every Authoritative Revision and Proposal Revision. Its parent is the exact expected head matched by the successful command, while physical payload placement is outside the domain contract.
_Avoid_: Mutable metadata row, storage blob layout, authority marker

**Canonical Payload**:
The exact immutable content owned by an Authoritative Revision, Artifact Revision, or Operational Record and required, while retained, to interpret, verify, replay, export, or migrate it. An authorized lifecycle operation may make its bytes unavailable only while preserving the exact identity, digest, availability fact, and known gap; a cache, index, Provider copy, or external runtime is never its sole source.
_Avoid_: Blob cache, derived projection, Provider-held source of truth

**Core Transition**:
The single logical atomic boundary in which StoryOS Core validates one idempotent domain command and durably records its complete outcome. Revisions, Commits, resolutions, heads, Receipts, lifecycle events, and required follow-up intent become visible together, while refusal or conflict records only its no-change Receipt and no partial domain effect.
_Avoid_: Partial commit, database rollback as undo, external effect as transaction truth

**Command Acknowledgement**:
The idempotently replayable public result of submitting one exact command, returned as Committed with its immutable Receipt only after the complete Core Transition commits, or as Accepted with a durable operation reference when later asynchronous settlement remains. Accepted proves only durable admission and never success; its settlement is observed through a bounded query or the Project Activity Stream rather than a delayed HTTP result.
_Avoid_: HTTP success as domain success, in-memory job acknowledgement, long-poll completion, duplicate execution after lost response

**Command Idempotency Fence**:
The compact Project Scope-bound continuation of one settled command's idempotency arbiter, preserving its command kind, key, digest, Command ID, immutable acknowledgement or replayable acknowledgement reference, final Receipt or operation reference, and retention provenance after larger execution payloads leave hot storage. A matching retry replays the same logical acknowledgement through current authorized redaction without re-executing; a different digest conflicts, and a known key never becomes a new command through expiry or compaction.
_Avoid_: Expired key reuse, response-cache entry, new command with old key, best-effort duplicate filter

**Protocol Compatibility Profile**:
The pre-1.0 same-release contract that binds one deployed Web Client, Server, Worker, generated client, public schemas, Event catalog, and Protocol Limit Profile. A release mismatch produces `upgrade_required` before domain admission or cursor advancement, while stored historical facts retain their own schema identity and project through the active release.
_Avoid_: Mixed-release runtime, ambient compatibility window, client-guessed safety semantics

**External Contract Compatibility Decision**:
The immutable Host result created only after one exact Project Scope-bound external-use binding exists, admitting or rejecting that binding against its global Registration and Adapter revision, protocol, schema or Tool digest, capability snapshot, wire mapping, exact Processing Destination Identity, Credential binding generation when applicable, and effect ceiling. It references but never creates, contains, or mutates the use binding; the binding never points forward to a Decision. Changing any binding field creates a new binding and then a new Decision, while changed observed protocol, schema, capability, or wire evidence that leaves the pinned binding and Registration/Adapter tuple unchanged creates only a new Decision. A global contract or Adapter observation contains no Project data, Credential Reference, actual account, or disclosure destination and cannot itself admit use. Every external use pins one exact observed contract and Adapter mapping; drift quarantines new use, and any widening of destination, disclosure, Credential binding, effect, or capability requires new authorization as applicable plus the corresponding new binding and Decision, while historical work remains bound to both original records.
_Avoid_: Semver-range trust, Provider alias compatibility, handshake as authorization, silent SDK upgrade, permanent external-version support

**Protocol Limit Profile**:
An immutable versioned contract fixing public validity ceilings and counting meaning for byte, item, depth, time, token, attempt, replay, expansion, rate, and concurrency at every public and external protocol crossing. Every numeric or semantic change creates a new Profile Revision activated with its matching StoryOS release, and dynamic resource pressure may only produce temporary rate or concurrency admission. Each Receipt, Attempt, Snapshot, and limit outcome binds both the Profile Revision and the actual effective bounds frozen from exact policy, grant, destination, and counting-profile inputs, while authors receive no routine limit configuration burden.
_Avoid_: Scattered magic limit, client-requested expansion, same-revision narrowing, unversioned token counting, author-facing protocol tuning

**Retention Profile**:
A versioned policy contract that supplies the bounded time, capacity, replay, checkpoint, archive, and compaction values for one Project Scope's Operational Retention Classes. Every lifecycle action binds its exact Profile Revision and frozen effective values; a new revision applies prospectively, while affecting an existing record requires a new inspectable Retention Decision rather than a silent retroactive expiry.
_Avoid_: Global mutable TTL, host configuration switch, per-Run author setting

**Retention Decision**:
The immutable Project Scope-bound lifecycle determination that applies one Retention Profile Revision to an exact record or payload role, recording its class, eligibility, source and settlement evidence, effective bounds, due condition, and resulting availability transition or refusal. A Profile update alone is not a Retention Decision and cannot delete, compact, archive, or revive prior payloads.
_Avoid_: Background cleanup log, mutable expiry column, profile update

**Application Wire Record**:
The immutable non-secret evidence retaining the exact schema-valid message-content bytes, wire schema and profile, typed-record reference, and digest for an authorized durable command or admission and for each supported public Event representation. It excludes transport headers, cookies, authorization and anti-forgery material, credential-bearing envelopes, unauthorized or malformed request bodies, repeated SSE deliveries, and rebuildable Query response bytes; canonical semantic bytes never stand in for separately claimed original wire bytes.
_Avoid_: HTTP traffic capture, request log, Query-response archive, JCS digest as original wire evidence

**Block Split Identity**:
When a stable top-level manuscript block is split, the fragment containing the original block start retains its Block ID and the new right fragment receives a new Block ID. Identity inheritance never depends on cursor direction or editor-generated IDs.
_Avoid_: Shared fragment identity, cursor-dependent inheritance, regenerated original ID

**Block Join Identity**:
When two adjacent stable top-level manuscript blocks join, the left block retains its Block ID and the right block leaves current Authoritative State. The right identity is never reassigned, while its historical Authoritative Revisions and Commit references remain addressable regardless of edit direction.
_Avoid_: Direction-dependent survivor, reused removed ID, erased block history

**Block Transfer Identity**:
An atomic, verifiable move preserves the Block ID while changing its location or parent structure; copying, duplication, ordinary paste, external import, or a non-atomic cut and paste creates a new Block ID with source provenance. Equal content alone never proves identity continuity.
_Avoid_: Content-derived identity, copied Block ID, inferred move

**Block Retype Identity**:
A one-to-one change of a stable manuscript block's type preserves its Block ID and appends an Authoritative Revision. A transformation that changes block cardinality or hierarchy follows the split, join, and transfer identity rules instead.
_Avoid_: Type-derived identity, one ID for multiple blocks, regenerated retyped block

**Block Identity Restoration**:
Undo and redo of one unchanged reversible structural action restore its exact historical Block IDs; an identity that left current state may return only as that same object's restoration. State drift makes redo unavailable, and a newly executed structural action receives newly generated identities.
_Avoid_: Fresh ID on exact redo, reassigned historical ID, redo across drift

**Direct Author Action**:
A deterministic, immediately visible change caused through the author's own editor input path against one exact authoritative target under direct manipulation, including manual paste. Bulk, cross-location, not-fully-previsible, Agent-, Tool-, MCP-, or extension-produced changes remain Proposal-gated even when an author click initiates them.
_Avoid_: Author-triggered automation, silent bulk edit

**Author Edit**:
One complete normalized editor intent submitted with an Author Command Admission for whole-command ownership classification by StoryOS Core. It produces an authoritative change, a Proposal Revision, a Refused Edit Draft, a conflict, or no effect without splitting one input across authority boundaries; raw editor transactions remain diagnostic evidence rather than the domain command.
_Avoid_: Client-selected write path, ProseMirror transaction as authority, partial mixed edit

**Editor Verification Split**:
The two complementary deterministic gates for an Author Edit. Browser integration verifies complete IME and editor intent capture, local journal durability, pending projection, and recovery continuity; Core verification proves command ownership classification, atomic durable settlement, and the exact Receipt.
_Avoid_: UI-only authority proof, server-only input-continuity proof, raw editor event as command truth

**Author Command Admission**:
The immutable StoryOS record identified by `AuthorCommandAdmissionId` that binds one authenticated User, exact existing or Server-allocated prospective Project Scope, trusted Web Client session, applicable Editor Session and writer generation, explicit action class, canonical command digest, target, expected Heads, nonce, idempotency record, bounded lifetime, and exactly one typed terminal settlement. It admits one author-owned command for Core evaluation and is not reusable; an unsettled explicit command requires author reconfirmation after recovery.
_Avoid_: Client-supplied actor, session role as authority, Approval, reusable authorization token

**Editor Session**:
One browser editing session for an exact User and Project Scope, identified by `EditorSessionId` and governed by the current Project writer generation. It owns local continuity and projection state while StoryOS Core and PostgreSQL retain authority.
_Avoid_: Browser tab as authority, Project identity, server transaction

**Local Edit Journal**:
The Project Scope-bound IndexedDB record of ordered, unsettled author editor intents for one Editor Session and writer generation. It preserves reload and crash continuity while remaining a non-authoritative input to Author Command Admission and Core settlement.
_Avoid_: Autosaved Authoritative State, server event log, ProseMirror history as truth

**Pending Edit Projection**:
The immediate author-facing editor view composed from one durable Server Snapshot plus the active Local Edit Journal. It exposes saving, saved, and needs-attention states without becoming an authoritative manuscript or Proposal Head.
_Avoid_: Optimistic authority, hidden pending state, network acknowledgement as domain truth

**Operational Record**:
A durable record of execution, context, authorization, usage, validation, or a state transition, such as an AgentRun, RunStep, RunPlan, Context Assembly Manifest, ToolCall, Approval, Artifact Lifecycle Event, Domain Receipt, or Run Event. It can reference and produce Artifacts but does not inherit Artifact lifecycle or authority.
_Avoid_: Artifact, temporary log

**Agent Memory**:
A Project Scope-bound, source-bearing, typed, and rebuildable retrieval projection over exact Authoritative State, Artifact, and Operational Record sources that supports continuity across threads and AgentRuns. It has no independent authority or writable truth and never interrupts active writing to demand memory confirmation; source conflict invalidates the projection, durable inference remains an Artifact, and only an explicit author-authorized domain action may change Authoritative State.
_Avoid_: Fourth truth store, hidden model memory, unified mutable memory blob, Agent memory as Authoritative State, interruptive memory approval

**Working Context**:
The operation-bounded view of live author input, in-progress model or Tool activity, short-term plans, and other unsettled material needed to continue active work. Only an immutable item version captured by the applicable Operation Input Snapshot may become a Context Candidate; for a RunStep, its Step Snapshot fulfills that boundary. A live mutable buffer, stream, or process object cannot be injected directly. Working Context evidence may remain in Operational Records for recovery, but it is excluded from long-term Agent Memory and cannot directly source a Memory Candidate.
_Avoid_: Agent Memory, durable project knowledge, hidden cross-Run memory

**Workspace Context**:
The StoryOS-owned interaction surface and exact view state from which an interactive operation begins, such as the active editor, Project, chapter, selection, or transcript location, bound to the same Project Scope and applicable Working Target. It supplies attributable UI origin and deterministic locators but grants no source authority, permission, Capability, or cross-project access. A noninteractive Service, Job, embedding, telemetry, or other operation with no interaction surface records an explicit not-applicable fact plus its exact typed cause and Operation Input Snapshot rather than fabricating UI state.
_Avoid_: Project identity, browser tab as authority, process-global current workspace, untrusted client selection, fabricated background UI state

**Context Assembly**:
The Host-owned seven-gate pipeline through which project-derived information must pass before it enters a model, Tool, MCP server, embedding service, or other processing destination: Operation Requirement Determination, Candidate Discovery, Source Eligibility Gate, Selection and Ranking, Bounded Projection, Context Assembly Manifest Commit, then Destination-specific Disclosure and Attempt. A returned external result crosses the same complete pipeline again before any later context use, and no gate may be skipped, merged, or reordered.
_Avoid_: Prompt construction, retrieval query, provider request builder

**Purpose**:
The explicit, bounded reason one operation processes context and the exact class of result it is allowed to produce for its named destination. Purpose constrains discovery, projection, disclosure, and completion but grants no source access, Capability, authority, or permission and cannot be broadened in place after assembly begins.
_Avoid_: Vague task label, destination identity, capability, post-hoc justification

**Working Target**:
The exact Project Scope-bound domain object, immutable input version, selection, question, or bounded object set on which one operation is allowed to work, together with any expected current Revisions needed to detect drift. It identifies the focus of work but grants no authority, permission, source eligibility, or right to expand to surrounding Project content. A legitimately untargeted noninteractive operation records an explicit not-applicable fact and remains bounded by its Purpose, typed cause, Project Scope, Operation Input Snapshot, and allowed source classes rather than inventing a target.
_Avoid_: Working Target Context, whole Project, editor tab, inferred story scope, fabricated Job target

**Operation Input Snapshot**:
The immutable, Project Scope-bound Operational Record capturing the exact inputs and source versions from which one operation begins. For a RunStep, its Step Snapshot fulfills this boundary; a Service, Job, embedding, telemetry, or other non-Run operation records an equivalent typed snapshot without inventing an AgentRun or Step Snapshot. Later input changes create a new Snapshot and never mutate one already used by Context Assembly.
_Avoid_: Live buffer, process memory, Step Snapshot for every operation, mutable request object

**Operation Requirement**:
The immutable Operational Record produced by Operation Requirement Determination, binding one Purpose, requester, Project Scope, applicable Workspace Context and Working Target or explicit not-applicable facts, Operation Input Snapshot, intended result, destination boundary, exact Processing Destination Identity when destination-bound, Mandatory Context, permitted dynamic source classes, budgets, applicable grants, authorization policy and approval requirement, and declared degradation behavior. A material boundary change creates a new Operation Requirement rather than widening the existing one.
_Avoid_: RunPlan, prompt, mutable request options, destination grant

**Operation Requirement Determination**:
The first Context Assembly gate, which binds one operation's Purpose, exact requester User, Project Scope, applicable Workspace Context and Working Target or explicit not-applicable facts with an exact typed cause and Operation Input Snapshot, intended result, destination requirement, required context, and permitted dynamic source classes before Candidate Discovery. An initial exact destination class is only a routing bound: before this gate completes, the Host must select one independently established Processing Destination Identity under the same Project Scope, then validate its applicable Project Destination Grant or other owning use-authorization boundary before any scoped external-use binding or compatibility Decision can enter routing. Context Assembly cannot begin without an explicit Purpose, valid Project Scope, exact destination identity for destination-bound work, and destination-permission boundary.
_Avoid_: RunPlan, Model Route Request, implicit prompt goal

**Mandatory Context**:
The closed, operation-specific context obligations that must participate in Context Assembly, divided into Host Control Context and a Mandatory Context Projection for each destination. Mandatory means required for a complete operation, not exempt from the Source Eligibility Gate, budget, projection, or disclosure gates.
_Avoid_: Entire prompt, full project context, eligibility bypass, always disclose

**Non-degradable Context Requirement**:
A Mandatory Context obligation whose absence, ineligibility, or unverifiability makes the current operation Blocked, including its Purpose and exact initiating instruction or typed cause, Project Isolation, exact Working Target when applicable or its explicit not-applicable fact and Operation Input Snapshot, applicable authority and safety constraints, effective capability and destination permission, and durable manifest boundary. A current author instruction is required when one initiated or steered the operation; an authorized event, schedule, Service, or Job cause records its exact identity without inventing author speech. The Requirement cannot be waived by a model, ranking policy, warning, or runtime fallback.
_Avoid_: High-priority context, soft requirement, best-effort prerequisite

**Declared Context Degradation**:
A named fallback fixed by the Operation Requirement before Candidate Discovery, specifying which otherwise-required context may be absent, why continuation remains valid, how the result and completion criteria narrow, which claims or effects become forbidden, and how the limitation remains inspectable. An undeclared fallback or one that changes the Purpose, destination, or intended result requires a new Operation Requirement rather than mutating the current operation.
_Avoid_: Silent omission, ad hoc model fallback, lower confidence, unchanged success

**Optional Dynamic Context**:
An allowed Context Candidate source whose absence or non-selection is an ordinary bounded-selection result rather than a degradation. Optionality grants no exemption from Source Eligibility or destination disclosure rules when the content is selected.
_Avoid_: Declared Context Degradation, low-priority mandatory context, unrestricted retrieval

**Context Sufficiency Decision**:
The immutable Host determination before Context Assembly Manifest commit that the operation is Complete, Degraded under one exact Declared Context Degradation with explicit unmet needs, or Blocked with reasons. A model cannot author the Decision, and a Degraded result cannot satisfy the original unmodified completion criteria.
_Avoid_: Warning, confidence score, model self-assessment, success with omissions

**Host Control Context**:
The exact Operation Requirement, identities, policies, grants, Approvals, budgets, destination contracts, eligibility results, and manifest evidence that the Host must use to govern one Context Assembly. It is mandatory control input but is not automatically model-, Tool-, MCP-, or provider-visible.
_Avoid_: System prompt, destination payload, hidden grant expansion

**Mandatory Context Projection**:
The minimum destination-visible projection required to perform one eligible Operation Requirement, including its Purpose and exact initiating instruction or typed cause, current author instruction when applicable, bounded Run Continuity Context and Working Target Context when applicable, applicable explicit Author Preferences and author-required sources, selected Skill instruction and outcome contracts, actual Tool contracts, and only the operational constraints needed for planning. Whole transcripts, manuscripts, Agent Memory, Research collections, and author-owned outlines remain dynamic sources by default rather than mandatory payloads.
_Avoid_: Host Control Context, full transcript, whole-project prompt, author outline as plan

**Run Continuity Context**:
The bounded, source-bearing prior author inputs, Steering Input or Wait Resolution, Agent Decisions, and settled results strictly necessary to interpret and continue the current RunStep. It is not the full project Transcript, a conversation copy, or permission to carry every prior item forward.
_Avoid_: Full thread, cloned transcript, complete Run history

**Working Target Context**:
The exact current object of work and its necessary local structure and Revisions, such as the current passage or selection, adjacent structure needed to interpret it, applicable Authoritative Revisions, and any Proposal under review. It does not make the surrounding chapter, manuscript, project library, or author outline mandatory by proximity.
_Avoid_: Whole manuscript, project dump, inferred story plan

**Context Source Version**:
The exact immutable owning-domain reference by which one Context Candidate is identified and replayed: an Authoritative or Artifact Revision where that domain uses Revisions, or an exact Operational Record, typed terminal outcome, Snapshot, ToolSpec, SkillPackage Snapshot, policy version, Registration revision, or other versioned contract where it does not. Context Assembly never invents a fake Revision or resolves mutable latest content after the fact.
_Avoid_: Mutable source, locator only, index row, universal Revision type

**Context Candidate**:
An exact source identity and Context Source Version discovered for possible use by one Operation Requirement, before current eligibility, selection, projection, or disclosure has been established. A mandatory source must become a Context Candidate for consideration but receives no exemption from later gates.
_Avoid_: Candidate Artifact, selected context, qualified source, index hit as permission

**Candidate Discovery**:
The second Context Assembly gate, which enumerates mandatory sources and locates dynamic Context Candidates only from source classes allowed by the Operation Requirement. Discovery and Retrieval Index hits establish neither current eligibility nor permission to use or disclose content.
_Avoid_: Memory Admission, Source Eligibility Gate, context selection

**Dynamic Retrieval**:
The bounded discovery of non-universal context for one Operation Requirement through Deterministic Requirement Retrieval, an Agent Retrieval Request, or Author-required Retrieval. Retrieved content becomes Context Candidates only and cannot mutate the current Step Snapshot or an in-flight Model Attempt.
_Avoid_: Automatic prompt injection, unrestricted project search, mutable current context

**Deterministic Requirement Retrieval**:
Host-initiated retrieval that resolves exact or typed context obligations already declared by the Operation Requirement, such as its Working Target, applicable Author Preference, or a selected Skill's required context role, without making a semantic relevance guess. Results remain subject to the complete Context Assembly pipeline before use.
_Avoid_: Similarity-based first-turn injection, Agent Retrieval Request, implicit source expansion

**Agent Retrieval Request**:
A typed request in one persisted Agent Decision that states the retrieval Purpose, allowed source classes, scope, and budget for later context work. Its results can enter only a subsequent RunStep through a new Context Assembly and never rewrite the requesting Step Snapshot or Model Attempt.
_Avoid_: Mid-step context mutation, free-form search side effect, automatic injection

**Author-required Retrieval**:
An explicit author instruction that makes its target mandatory for Candidate Discovery and eligible selection within the instruction's scope. Author origin grants neither source authority nor exemption from eligibility, budget, projection, or destination disclosure.
_Avoid_: Author-owned document as authority, unconditional payload inclusion, Author Plan

**Speculative Context Prefetch**:
A disposable optimization that may warm a StoryOS-controlled, Project Scope-bound index or cache without making its results selected, manifested, or destination-visible. Any prefetch requiring a model, Tool, MCP server, embedding service, or other External Processing Destination is a separate operation that must cross all seven Context Assembly gates.
_Avoid_: Background disclosure, pre-approved context, first-turn injection

**Source Eligibility Gate**:
The third Context Assembly gate, which fail-closed checks every Context Candidate's exact domain identity and Context Source Version, matching Project Scope and Project Isolation for project-bearing content, caller permission, Source Integrity, Context Trust Assessment, Disclosure Eligibility, and every owning-domain qualification that applies to that source kind. A globally or User-reusable schema, ToolSpec, policy, Adapter definition, public capability description, or other definition may remain source-unscoped only when it contains no project-derived data or project authority; its selection and use still bind the current Project Scope. Applicable qualifications include Memory Admission and Memory Suppression for a memory-derived or ordinary-recall path, Lifecycle, Archive, Tombstone, Retention State, Story Scope, and Epistemic Scope. A non-applicable qualification is recorded as such rather than invented; an applicable check that fails or cannot be established excludes the Context Candidate, while an ineligible required source blocks the operation or enters an explicit recorded degradation mode.
_Avoid_: Relevance threshold, confidence warning, ranking penalty, best-effort inclusion

**Context Trust Assessment**:
The orthogonal determination of a Context Candidate's Source Integrity, Instruction Authority, domain or evidentiary status, Execution Trust, and Disclosure Eligibility for one operation. No axis implies another: executable infrastructure does not make its content true, authoritative, instructive, or disclosable.
_Avoid_: Trusted boolean, source reputation score, server trust as content authority

**Source Integrity**:
The evidence that content matches its claimed source identity, exact Context Source Version, and applicable digest or capture boundary. Integrity proves attribution and unchanged bytes, not truth, authority, Instruction Authority, or current eligibility.
_Avoid_: Source truth, trusted content, evidentiary sufficiency

**Instruction Authority**:
The closed eligibility to direct Agent behavior, held only by applicable StoryOS Host product, domain, policy, and safety constraints; exact current author instructions, Steering Input, Wait Resolutions, and authoritative Author Preferences; and the current Step's exact selected Skill Instruction Context. Instruction Authority never arises from prose that merely looks imperative, source ownership, signatures, repetition, retrieval rank, Tool or MCP execution trust, or model output.
_Avoid_: Prompt position, trusted server instructions, author-owned document, model-generated rule

**Data-only Context**:
Context that may inform an Agent Decision but has no Instruction Authority, including Tool and MCP output, model output, Agent Memory, Research, external or imported documents, manuscript prose, and author-owned outlines unless separately expressed through an authoritative or instructional domain path. Imperative text inside it remains quoted data rather than executable instruction.
_Avoid_: Lower-priority instruction, untrusted system prompt, implicit Author Plan

**Execution Trust**:
The current Host determination that one exact registered model, Tool, MCP, adapter, or other implementation may be invoked through its governed execution boundary. It grants no content truth, Instruction Authority, Authoritative State, capability, or destination disclosure permission.
_Avoid_: Tool Exposure, Capability Grant, trusted output, server authority

**Disclosure Eligibility**:
The current operation- and destination-specific determination that one exact source or Projection may be included in an Outbound Disclosure under the same Project Scope and applicable policy, grants, Purpose, data categories, and minimization rules. It does not follow from source read permission, Source Integrity, Execution Trust, selection, or prior disclosure.
_Avoid_: Network access, source eligibility alone, cached consent, provider trust

**Selection and Ranking**:
The fourth Context Assembly gate, which gives eligible Mandatory Context budget priority and applies one exact Context Ranking Profile only to eligible dynamic Context Candidates. Selection and rank grant no truth, authority, evidentiary status, binding force, or disclosure permission.
_Avoid_: Memory Admission, authority ranking, permission score

**Context Ranking Profile**:
An immutable, versioned, Purpose- and source-class-specific comparison contract defining allowed relevance, scope specificity, structural or causal proximity, coverage, diversity, evidence-balance, and genuinely time-sensitive currency signals plus budget behavior and a stable non-semantic final tie-break. It cannot use a global trust score, source authority or ownership as a bonus, prior access or retrieval frequency, popularity, repetition, model confidence, or wall-clock decay for still-applicable fiction truth and historical evidence.
_Avoid_: Universal relevance score, trust ranking, access-frequency boost, authority weight

**Bounded Projection**:
The fifth Context Assembly gate and its attributable minimum-necessary transformation of selected exact Context Source Versions under one Context Projection Policy for one Purpose, exact Processing Destination Identity, and applicable intake and disclosure policy revisions. Its immutable Projection and lineage are Operational Records, never Artifacts or Authoritative State; a lossy Projection is a new source-linked item recording its transformation policy and omitted meaning, never an overwrite or substitute for original evidence. Generating it through a model, Tool, or external service is a separate operation that recursively crosses all seven gates.
_Avoid_: Source Revision, silent truncation, rewritten history, evidence replacement

**Context Projection Policy**:
The immutable, versioned source-class and destination contract selecting exactly one projection mode: Exact Required, Deterministic Excerpt, Derived Summary, or Reference Only, with hard item bounds and failure behavior. Budget is allocated first to non-degradable Mandatory Context, then declared degradable Mandatory Context, then ranked dynamic context; an unfit Exact Required item requires another eligible route, a new reframed operation, or a Blocked decision.
_Avoid_: Arbitrary token truncation, best-effort fit, provider-side default

**Exact Required Projection**:
A Projection mode that preserves the complete eligible content and semantics of an item such as the current author instruction, governing authority or safety constraint, exact required Skill or Tool contract, or declared Working Target. It cannot be silently truncated, summarized, or replaced to fit a destination limit.
_Avoid_: High-priority excerpt, auto-summary, head-tail truncation

**Deterministic Context Excerpt**:
A Projection mode that selects exact complete domain units or locator-bound ranges, such as paragraphs, Fiction Assertions, Research Claims, typed Tool fields, or event ranges, while recording original extent and every omitted boundary. It never cuts an arbitrary token span or claims that omitted material was inspected by the destination.
_Avoid_: Raw token slice, silent head-tail trim, summary

**Derived Context Summary**:
A lossy, source-bearing Projection created from exact input Context Source Versions and prior Projections under a recorded generation policy, generator identity, and Projection Loss Indicator. It never replaces its source history, and model-, Tool-, or externally generated summaries require their own complete Context Assembly operation.
_Avoid_: Rewritten history, opaque compaction, source evidence

**Context Compaction Projection**:
An immutable Operational Record and Derived Context Summary over an exact bounded context range, recording every input Context Source Version and prior Projection, compaction policy, generation contract and producer, usage, output, and Projection Loss Indicator. It may be selected by later Steps but never rewrites Messages, Run Events, Tool results, Step Snapshots, manifests, or the original context history and never becomes an Artifact or Authoritative State.
_Avoid_: Replacement history, mutable conversation summary, provider continuity object

**Compaction Source Closure**:
The complete transitive set of exact Context Source Versions, Projection lineages, compaction boundaries, and accumulated loss evidence underlying one Context Compaction Projection. Re-compaction must retain this closure rather than presenting a summary of a summary as original history.
_Avoid_: Latest summary only, flattened provenance, opaque context checkpoint

**Opaque Provider Continuity**:
A provider-specific cache handle, prior-response reference, encrypted compaction object, or other non-inspectable continuity mechanism used only as part of a Wire Payload Projection. It cannot become StoryOS history, a Context Candidate, or the only evidence of Effective Destination Context, and is ineligible when its complete StoryOS-held source closure and Project Scope cannot be revalidated and inspected.
_Avoid_: Context Compaction Projection, provider continuity as source of truth, independent replay state

**Context Cache Entry**:
A disposable prompt, retrieval, Projection, embedding, Tool-schema, or other acceleration product keyed by exact Project Scope, Context Source Versions, policy and transformation versions, qualification state, destination identity, grant, and Adapter mapping. It owns no source meaning, eligibility, authorization, historical evidence, or authority and is never reusable across either Project Scope identity.
_Avoid_: Context Candidate, Context Assembly Manifest, durable memory, cached permission

**Context Cache Reuse Decision**:
The current fail-closed determination that one Context Cache Entry's complete dependencies still satisfy source identity and version, every applicable owning-domain qualification such as Lifecycle and Retention plus Memory Admission and Memory Suppression when the dependency is memory-derived or ordinary recall, permission, policy, destination, grant, and Adapter requirements for one operation. Changed or unverifiable dependencies make the Entry immediately unusable even if physical invalidation or deletion is still pending.
_Avoid_: Cache hit, stale-while-revalidate, prompt-prefix preservation, prior consent

**Context Inspect**:
A read-only author audit of current or historical Operation Requirements, discovery, eligibility, selection, Projections and loss, manifests, Wire Payload Projections, Disclosure Events, and Destination Attempts, preserving historical facts while showing current invalidity separately. Its exact wire view means the exact non-secret application payload and protocol projection plus opaque secret-injection placeholders, never credential values, credential-value digests, or an unredacted transport envelope. Inspection obeys current Project Isolation, permissions, and redaction and distinguishes exactly reconstructable, reference-known, and provider-opaque context without presenting inference as fact.
_Avoid_: History rewrite, model-use claim, unredacted debug dump

**Context Include**:
An author control bound to one exact Operation Requirement that makes one named source, exact Context Source Version, fragment, or domain object a Mandatory Context Candidate for that operation only. It never follows a later source version or grants authority, Instruction Authority, any owning-domain qualification including Memory Admission, budget exemption, or Disclosure Eligibility.
_Avoid_: Context Pin, source promotion, automatic latest version

**Context Pin**:
A prospective Author Context Requirement scoped to the Next Operation, Current AgentRun, or Project and bound either to one Exact Context Source Version or to Follow Source Identity with fresh resolution and eligibility on every operation. An Artifact or Authoritative State source uses its exact Revision as that version; other source families use their owning immutable version boundary rather than inventing a Revision. It requires logical consideration rather than universal disclosure, inherits no prior Memory Admission, and applies Memory Suppression only when it resolves through a memory-derived or ordinary-recall path; direct governed use of the unchanged raw source remains separately eligible. It fails unmet rather than guessing when identity becomes ambiguous, split, merged, or deleted.
_Avoid_: User prerequisite, permanent prompt text, implicit latest, authority marker, fabricated Revision

**Context Exclude**:
An author control scoped to one Operation, AgentRun, or Destination that bars a named source, Context Source Version, fragment, data category, and its protected provenance closure from future unsubmitted Destination Attempts. It outranks Include and Pin; an already committed Manifest remains historical while pending work is cancelled and reassembled, and a resulting mandatory-context gap becomes explicitly Degraded or Blocked.
_Avoid_: Outbound Disclosure retraction, Manifest edit, Memory Suppression, hidden omission

**Author Context Control Precedence**:
The fail-closed order in which Tombstone, current permissions, Capability, and destination policy outrank Memory Suppression when the Candidate is memory-derived or enters through ordinary recall; applicable Memory Suppression then outranks applicable Exclude; Exclude outranks Include and Pin; and Include and Pin outrank ordinary dynamic ranking. Memory Suppression does not ban governed direct use of its unchanged raw source; a general source or disclosure ban requires Exclude or the owning policy. A positive control can never override a harder negative eligibility or disclosure boundary.
_Avoid_: Last control wins, UI order, ranking override

**Default Context Experience**:
The author-facing promise that a stable editor-integrated Agent automatically receives the eligible current Working Target and necessary project continuity without requiring the author to configure context scopes, source-version strategies, a character sheet, a Context Pin, or a Project Instruction. The precise controls and optional Project Instruction remain author conveniences surfaced only through plain-language or simple direct actions when needed, never prerequisites or routine context-management ceremony.
_Avoid_: Manual context setup, required character sheet, scope dropdown workflow, context confirmation on every step

**Context Reference**:
A Reference Only Projection exposing bounded catalog information and an exact source-qualified locator without exposing the referenced payload. It lets an Agent request later retrieval but does not prove that the referenced content was model-visible, eligible for disclosure, or used.
_Avoid_: Loaded context, citation as disclosure, implicit retrieval

**Projection Loss Indicator**:
The structured account of semantic classes, ranges, modalities, precision, or uncertainty intentionally omitted or transformed by a lossy Projection. It is inspection and sufficiency evidence rather than a generic warning or permission to hide unknown loss.
_Avoid_: Truncated flag, confidence score, disclaimer

**Context Assembly Manifest**:
The immutable provider-neutral Operational Record committed at the sixth Context Assembly gate before any StoryOS Controlled or External Processing Destination I/O, binding the exact requester User and Project Scope, Operation Requirement and Operation Input Snapshot, applicable Step Snapshot and Project Instruction Binding, Context Sufficiency Decision, considered Context Candidates and eligibility results, selected Skill and Tool context, exact selected Context Source Versions and Projections, Ranking Profile and results, budgets, exclusions, and unmet needs. It proves StoryOS's complete logical preparation rather than destination use or wire bytes; failure to persist it prevents all destination I/O.
_Avoid_: ContextManifest, prompt dump, model-use proof, mutable request log

**Destination-specific Disclosure and Attempt**:
The seventh Context Assembly gate, which independently minimizes and authorizes the Effective Destination Context for each exact model, Tool, MCP server, embedding service, or other processing destination, establishes one Destination Attempt for every concrete planned execution, and records every dispatch, retry, fallback, or destination change through its Destination Context Manifest and owning execution evidence. Only an External Processing Destination also creates Outbound Disclosure evidence; cache reuse, prior-response linkage, or an existing Tool result can optimize computation or transport only after the preceding six gates and current eligibility revalidation.
_Avoid_: Shared provider payload, cached authorization, prior Destination Attempt reuse

**Settled Source Version**:
An exact durable Revision or typed terminal outcome whose settlement meaning has committed and is stable enough to analyze as a derivation source. Settlement makes that version eligible for extraction but does not prove a generalization, make it permanently current, or prevent later source change from invalidating derived results.
_Avoid_: Live stream, intermediate event, latest value, permanently true source

**Retrieval Index**:
A disposable, rebuildable full-text, vector, graph, or other access projection over exact domain identities, Context Source Versions, current qualification records, and one build-policy version. Its internal IDs and scores have no domain meaning, and every result must fail closed unless the current source version, every applicable owning-domain qualification such as Lifecycle plus Memory Admission and Memory Suppression when relevant, scope, and permission eligibility can be revalidated before context use.
_Avoid_: Semantic memory, vector store of record, index ID as Durable Identity, ranking as truth

**AgentRun**:
A durable execution aggregate for one bounded user-, event-, or schedule-triggered intent, owning its plan, steps, Capability Grant, budget, approvals, ToolCalls, produced results, and terminal outcome. A nonterminal AgentRun survives process restarts and may wait, pause, and resume; a terminal AgentRun is retained and immutable, while continuation or retry creates a new causally linked AgentRun.
_Avoid_: Conversation, transcript, live model session, reopened Run

**Proactive Trigger**:
An author-enabled, versioned project rule that maps a schedule or exact project event to a bounded AgentRun grant template and admission policy. It is neither a running Agent nor a source of authority, and disabling it prevents only future occurrences.
_Avoid_: Cron process, background Agent, implicit permission, user-triggered Run

**Trigger Occurrence**:
An immutable, idempotently identified fact that one exact Proactive Trigger Revision matched one scheduled time or source event. StoryOS may create at most one root AgentRun for an Occurrence, so duplicate delivery or recovery scanning never duplicates execution.
_Avoid_: AgentRun, timer process, repeated event delivery

**Trigger Misfire Policy**:
The versioned rule for handling scheduled Trigger Occurrences that became due while StoryOS could not admit them: Skip, Run Latest Once, or Catch Up Bounded. It never implicitly replays every missed occurrence, and any catch-up bound remains subject to the Trigger's current grant, budget, frequency, and concurrency policy.
_Avoid_: Retry policy, event deduplication, unbounded backlog replay

**Trigger Batch**:
An immutable, provenance-preserving grouping of Trigger Occurrences from one Trigger Revision and semantic coalescing key for admission to at most one AgentRun. Every source Occurrence remains addressable; batching, supersession, and default single-flight execution prevent event storms without erasing what happened.
_Avoid_: Debounced event deletion, AgentRun, mutable pending list

**Trigger Admission Decision**:
A persisted, deterministic decision that evaluates one Trigger Occurrence or Trigger Batch and its exact Trigger Revision against current project policy, enablement, grant-template validity, budget and concurrency capacity, registered contract and credential availability, and project state. The Occurrence remains durable whether admission is admitted, boundedly deferred, coalesced, skipped, or denied; only an admitted decision freezes a Run Grant snapshot and may create at most one root AgentRun. Admission neither calls a model nor requests Approval to expand authority, and changed or insufficient authority fails closed with an inspectable reason.
_Avoid_: Trigger Occurrence, model decision, implicit authorization, Approval request

**Approval Escalation Policy**:
The versioned policy on a Proactive Trigger that determines whether an admitted AgentRun may interrupt the author with a durable Approval Wait for a precisely named authority-expansion class. The default is No Escalation: the Run must replan within its frozen Grant, finalize a partial or blocked result, or terminate. An opted-in request grants nothing until explicit author Approval, remains bounded by project policy, cannot release a Safety Hold, and may not repeatedly prompt for the same rejected or expired need.
_Avoid_: Trigger Admission, automatic Approval, unbounded approval prompt, Safety Hold recovery

**Run Lifecycle**:
The minimal irreversible state progression of every AgentRun: Queued, then Active, then Terminal; a Queued Run may also terminate safely before starting. Waiting, Paused, Blocked, Cancelling, Recovering, and Finalizing are represented by durable Waits, Holds, intents, Recovery Decisions, and gates while the Run remains Queued or Active, while success and failure belong to Run Outcome. An Active Run never returns to Queued, and a Terminal Run never reopens; continuation or retry creates a causally linked new AgentRun.
_Avoid_: UI status label, phase explosion, Run Wait, Run Hold, Run Outcome

**Run Transition**:
The atomic application of one uniquely identified author, host, or recovery command against an expected Run sequence after validating lifecycle, Run Lease, policy, and domain invariants. One transaction updates the normalized current records, appends the corresponding Run Events, advances the sequence, and enqueues any outbox messages or Run Wakeups; a duplicate command returns its prior result, while a sequence conflict is re-read and re-evaluated rather than overwritten.
_Avoid_: Direct status update, Transcript message, partial multi-table write, external side effect

**Run Event**:
An immutable, causally attributable, monotonically sequenced fact recording one committed Run Transition for inspection and recovery history. Run Events and normalized current records are written atomically without requiring pure event sourcing; the event fact remains historical even when an associated eligible operational payload later becomes unavailable through Operational History Compaction, while Checkpoints, caches, and read models are derived and external effects follow persisted intent through the Tool Gateway and append their outcomes afterward.
_Avoid_: Mutable status row, model transcript, telemetry log, cache entry

**Run Event Segment**:
A Project Scope-bound, losslessly encoded physical grouping of contiguous immutable Run Events for storage or cold Archive. It may be compressed, moved, or have its replay service bounded, but it never semantically deletes, reorders, rewrites, or replaces any committed Event or its causal meaning.
_Avoid_: Semantic event deletion, lossy transcript summary, mutable event batch, new truth stream

**Project Activity Stream**:
The one canonical public replay stream for an exact Project Scope, assigning every committed client-visible activity event a strictly increasing project-local position while preserving its typed identity, cause, and any owning Run or aggregate sequence. Run-, Artifact-, and other filtered streams are cursor-bound derived views rather than separate truth streams; every cursor is bounded by its Replay Generation and resumes only through that generation or a fresh Activity Stream Resync, while Mailbox, Worker, Provider, MCP, and Adapter protocols remain outside this public envelope.
_Avoid_: Global event bus, per-Run truth stream, internal event log, universal protocol envelope

**Replay Generation**:
A project-local bounded replay epoch of the Project Activity Stream, published with one authorized replay floor and fresh Snapshot at a compaction or archival boundary. A cursor never crosses generations: a below-floor cursor fails explicitly rather than being translated, guessed, or silently advanced.
_Avoid_: Infinite cursor migration, guessed offset, stream fork

**Activity Stream Resync**:
The authorized recovery from an expired Project Activity cursor that loads a fresh canonical Snapshot and resumes strictly after its recorded Activity position. It exposes the replay-generation boundary and never treats a cursor-too-old failure as an empty stream, a successful replay, or permission to skip historical facts.
_Avoid_: Cursor translation, silent reset, empty history

**Canonical Query Snapshot**:
An authorized, time-bounded stable reading boundary over Project Scope-bound durable facts, binding its Activity position, query/view inputs, redaction, schema, and replay generation. It may expire and be reissued, but is neither a Run Checkpoint, a backup, nor a permanent second copy of history.
_Avoid_: Run Checkpoint, backup, permanent query result, live process view

**Canonical Query**:
A public read of exact Authoritative State, Artifact, Receipt, Approval, Run, or other canonical facts at one committed Project Scope-bound Snapshot. It supports read-your-acknowledgement against a required Project Activity Stream position, and every page remains bound to the same Snapshot and stable order or fails with an explicit resync outcome.
_Avoid_: Eventually consistent authority read, mixed-Snapshot pagination, cache result as current truth

**Projection Query**:
A public read of one bounded rebuildable search, embedding, retrieval, history, or other projection, returning its exact source Snapshot, projection watermark, completeness, and lag. It never presents lag as an empty canonical result; an unmet required watermark returns an explicit projection-not-ready outcome, while Snapshot lifetime remains a retention policy.
_Avoid_: Canonical Query, hidden eventual consistency, empty result for stale projection, unbounded projection dump

**Run Hold**:
A durable gate that prevents an active AgentRun from starting new work until an explicit author or host resolution releases it. Author pause, a tripped guardrail, or required recovery adjudication may create a Hold even when every existing Wait has resolved.
_Avoid_: Run Wait, process suspension, terminal state

**Resource Hold**:
A Run Hold caused by insufficient budget or renewable execution capacity. The author may extend the affected Budget Hard Ceiling only within its parent project policy, or may replan, finalize, or stop; the existing usage and reservation history never resets.
_Avoid_: Safety Hold, automatic budget increase, cleared usage

**Safety Hold**:
A Run Hold caused by a safety guardrail such as repeated no progress, a retry storm, goal drift, or unresolved effect uncertainty. Additional budget cannot release it; resumption requires an inspectable Recovery Decision showing a material change to the plan, goal, Tool, or execution strategy, while all prior counters and evidence remain.
_Avoid_: Resource Hold, budget extension prompt, reset circuit breaker

**Run Wait**:
A durable, uniquely addressable unresolved dependency, such as author input, Approval, an external result, or an exact Subrun observation condition, that blocks only the work depending on it. Only a typed response bound to that exact Wait may resolve it; a Subrun observation deadline ends only that observation and never changes the child or its Join, while independent branches may continue.
_Avoid_: Run Hold, global paused state, in-memory waiter

**Run Wakeup**:
A durable, uniquely identified request to re-evaluate an exact Run, Wait, or Hold at or after a persisted due time and generation. Scheduler delivery is at-least-once: duplicates and superseded Wakeups become inspectable no-ops through idempotency and Run sequence checks. A due Wakeup neither calls a model nor replays a ToolCall; it first requires a current Run Lease and live revalidation of lifecycle, policy, budget, contracts, credentials, and the target dependency.
_Avoid_: In-process timer, Proactive Trigger, guaranteed execution time, automatic retry

**Wait Resolution**:
The immutable, idempotent resolution of one exact active Run Wait by its bound author response, Approval Decision, external result, expiry, or cancellation. A stale, duplicate, or unrelated Transcript message cannot resolve or reopen another Wait.
_Avoid_: Generic reply, Steering Input, Run resume

**Steering Input**:
An immutable, ordered author instruction submitted to a nonterminal AgentRun for consideration at its next safe decision boundary. It never rewrites an existing Step Snapshot, Agent Decision, or confirmed effect; Pause, Cancel, Approval, and answers to exact Run Waits use their own typed commands instead.
_Avoid_: Live prompt mutation, Run Pause, generic approval response

**Subrun Interrupt**:
An idempotent control command targeting the exact current Execution Attempt on one Subrun Run Lane, requesting cooperative interruption without creating a Hold, cancelling the Subrun, changing its lifecycle, or propagating to descendants. The Attempt and any uncertain Tool effects remain durable evidence, and subsequent work requires an explicit Recovery Decision.
_Avoid_: Run Pause, Run Cancellation, process kill, Subrun Follow-up

**Run Pause**:
An author control that immediately creates a Run Hold, prevents new work from starting, and requests cooperative interruption of work that remains safe to cancel. The Hold propagates to descendant Subruns and preserves confirmed or uncertain effects; resuming the parent clears only the inherited Hold, not a child's own Hold or Wait.
_Avoid_: Run Wait, process kill, Run Cancellation

**Run Cancellation**:
An irreversible intent to stop an AgentRun that immediately prevents new work and propagates cancellation through its in-flight work and descendant Subruns. The AgentRun records a cancelled Run Outcome only after every affected operation has reached a confirmed terminal or outcome-unknown boundary; cancellation never rolls back an effect.
_Avoid_: Run Pause, process kill, rollback

**Run Finalization Gate**:
The automatic, deterministic, idempotent host check that turns a persisted Agent Finalize Intent into one terminal Run Outcome only after all in-flight operations, direct child Subruns, Required Joins, Subrun Result Dispositions, Waits, Holds, reservations, effect uncertainties, final Artifacts, Proposals, provenance, and unfinished-work dispositions are durably settled. It requires no routine author confirmation, cannot be bypassed by a model completion claim, and can resume safely after a crash without duplicating output or terminal events.
_Avoid_: Author confirmation dialog, Approval, model-declared completion, conversation-turn end

**Run Outcome**:
The immutable Succeeded, PartiallySucceeded, Failed, or Cancelled result recorded only when an AgentRun passes the Run Finalization Gate and becomes terminal. PartiallySucceeded requires at least one usable completed deliverable plus an explicit account of unmet criteria; budget exhaustion, Approval rejection, contract drift, and similar facts are typed reasons rather than additional top-level outcomes. Waiting, Paused, and Blocked remain nonterminal, while an unknown Tool or external effect blocks success and may be carried with complete Recovery Decision evidence only into a Failed or Cancelled outcome.
_Avoid_: Run status, step result, Tool Effect Outcome, failure-reason enum, OutcomeUnknown

**RunStep**:
One immutable, recoverable Agent decision cycle within an AgentRun, beginning from one Step Snapshot and owning one Agent Decision. A RunStep may cause multiple independently authorized and recoverable ToolCalls or Subruns, and it settles only after the records required by that decision are durably resolved.
_Avoid_: Plan step, ToolCall, conversation turn, mutable loop iteration

**Run Lane**:
The ordered sequence of RunSteps belonging to the root AgentRun or to one Subrun, with at most one active RunStep at a time. ToolCalls and distinct Subrun lanes may progress concurrently, but decisions within one lane never compete for the same next position.
_Avoid_: Operating-system thread, worker, conversation thread, concurrent RunSteps in one lane

**Run Lease**:
A renewable execution-ownership lease for one Run Lane carrying a monotonically increasing fencing token. State-changing writes require the current token, expected Run sequence, and an idempotency key, so a stale Worker cannot mutate the Run after recovery assigns a newer owner. Lease expiry permits reconciliation and takeover but proves neither ToolCall failure nor effect absence; the Lease is coordination metadata, never authority, budget, or durable execution truth.
_Avoid_: Capability Grant, process lock as source of truth, ToolCall timeout, permission

**Execution Capacity Reservation**:
A durable atomic scheduler allocation permitting one exact RunStep or Execution Attempt to occupy execution capacity under the Host, project, root AgentRun, ancestor-budget, depth, and fan-out limits. It is bound to the current Run Lease fencing token and released when execution stops, waits, or holds; it is neither Subrun existence, lifecycle, authority, nor a resident model session.
_Avoid_: Subrun count, Run Lease, Capability Grant, resident session

**Subrun Request**:
The immutable declaration in one Agent Decision that identifies one intended child execution under a stable request key. Repeated delivery of the same Request resolves to the same Subrun, while an intentional retry or repetition requires a new causally linked Request and never reopens a terminal Subrun.
_Avoid_: Task name, objective text, spawn command, reopened Subrun

**Subrun Lifecycle**:
The minimal irreversible progression of every Subrun from Queued to Active to Terminal, with safe pre-start termination also permitted from Queued. Waits, Holds, recovery, cancellation, finalization, child-turn activity, and individual Attempt outcomes remain orthogonal records; only finalization with one immutable Subrun Result makes the Subrun Terminal.
_Avoid_: Child-turn state, model-session status, worker status, phase-expanded lifecycle

**Subrun Context Bundle**:
The immutable, attributable, hard-bounded projection of exact project revisions, Artifacts, transcript fragments, Skill snapshots, and other context supplied to one Subrun. Parent context never flows into it implicitly; every later addition is a persisted bounded supplement applied only through a subsequent Step Snapshot.
_Avoid_: Transcript fork, live parent context, implicit inheritance, prompt copy

**Subrun Capability Grant**:
The exact versioned attenuation requested for one Subrun and validated as a complete subset of the project policy ceiling, root AgentRun Grant, and every ancestor Subrun Capability Grant. Parent expansion never flows implicitly and prospective expansion requires an explicit new Grant Revision, while live policy, revocation, contract, and effect checks may still narrow execution.
_Avoid_: Copied parent grant, inherited permission, credential set, permanent authorization

**Subrun**:
A durable hierarchical child execution within one root AgentRun, bound to an immutable direct parent and owning its own Run Lane, narrowed context and capabilities, budget slice, Waits, Holds, and typed outcome. It is not a top-level AgentRun, has no independent project-level grant, cannot be reparented or outlive its direct parent, and can never commit Authoritative State.
_Avoid_: Child AgentRun, background process, cloned conversation, orphaned child, permanent Agent

**Subrun Message**:
An immutable typed communication record between one Subrun and its direct parent, carrying a stable Message ID, per-direction sequence, causal references, and a bounded payload or Artifact reference. Transport is at least once while durable reception and effects are idempotent by Message ID, and Delivered, Acknowledged, and Consumed remain distinct facts.
_Avoid_: Transcript message, best-effort notification, sibling message, exactly-once transport

**Queue-Only Subrun Message**:
A Subrun Message whose delivery never schedules a RunStep or Run Wakeup and never interrupts current work. It may be explicitly consumed only in a later RunStep scheduled for another reason.
_Avoid_: Subrun Follow-up, Steering Input, interrupt signal, trigger-turn flag

**Subrun Progress Report**:
An immutable hard-bounded Subrun Message that summarizes completed facts, current work, blockers, Artifact references, usage, and requested attention while citing an exact child-event range and watermark. It is informational rather than execution truth, Subrun Result, or Join Resolution; the parent must consume it explicitly, while full child events remain available only through bounded observability queries and the Author UI stream.
_Avoid_: Transcript dump, raw log stream, guessed percentage, Subrun Result

**Subrun Follow-up**:
An immutable idempotent direct-parent control command that adds bounded pending work to an existing nonterminal Subrun and schedules a future RunStep. An idle child may be woken after admission, an active child receives the work only after its current RunStep reaches a safe boundary, and a terminal child rejects it rather than reopening.
_Avoid_: Queue-Only Subrun Message, new Subrun Request, implicit interrupt, trigger-turn flag

**Subrun Mailbox**:
The durable, ordered, bounded channel for typed progress, observation, Artifact, question, plan-change proposal, queued context, terminal-result, and control-notice messages between one Subrun and its direct parent. It grants no shared writable state, direct author access, scheduling effect, or control authority; any author question is escalated by the root lane, while Tool Gateway may independently surface an exact Approval Wait for a child ToolCall.
_Avoid_: Shared memory, cloned transcript, direct author chat, permission channel

**Subrun Mailbox Backpressure**:
The explicit durable admission condition raised when a Subrun Mailbox's ordinary unconsumed-count or payload-byte capacity is exhausted, rejecting new ordinary messages without silently dropping existing ones. A separate non-borrowable critical reserve protects terminal, safety, cancellation-settlement, and recovery notices, while Progress Report supersession may compact only the active projection and preserves its cited event history.
_Avoid_: Silent message drop, unbounded queue, TTL cleanup, shared critical capacity

**Subrun Mailbox Seal**:
The durable terminal boundary proving that one root AgentRun has no unsettled Subrun deliveries and that every sender generation is closed at recorded directional high-watermarks. Message payload retention is independent, but Message ID deduplication evidence cannot be discarded by age before the Seal and may afterward be compacted only into a Seal Deduplication Fence that still rejects every replay or invalid late message.
_Avoid_: TTL expiry, Inbox deletion, delivery acknowledgement, payload retention policy

**Seal Deduplication Fence**:
The compact Operational Evidence Floor created from one sealed root's mailbox-deduplication records, binding its exact Seal, direction, sender generation, and closed sequence high-watermark. It rejects a late message at or below that boundary as replay or invalid delivery and above it as a closed-generation violation without reusing its payload, consumption, Run Wakeup, or effect; it remains at least as long as the root's Evidence Floor.
_Avoid_: Per-message payload archive, expired dedup key, open sender generation, best-effort duplicate filter

**Undeliverable Subrun Message**:
A durable invariant-violation record created only when an exact direct parent's persistent identity or lifecycle cannot validly accept a Subrun Message after recovery. It preserves the message and reason, creates a root Safety Hold, and never reroutes delivery, reparents the child, or lets the root consume on the parent's behalf; an offline Worker or application is not undeliverable.
_Avoid_: Transient parent outage, dropped message, root delivery, automatic reparenting

**Subrun Outcome**:
The immutable Succeeded, PartiallySucceeded, Failed, or Cancelled settlement of one Subrun against its exact completion criteria. PartiallySucceeded requires a usable deliverable and explicit unmet criteria, while waiting, interruption, and outcome-unknown effects are not outcomes and unresolved effect uncertainty blocks success.
_Avoid_: Subrun Lifecycle, child-turn status, Run Outcome, OutcomeUnknown

**Subrun Finalization Gate**:
The automatic deterministic idempotent host check that may turn a persisted Subrun Finalize Intent into one terminal Subrun Outcome and Result only after its work, direct children and dispositions, Mailbox obligations, Waits, Holds, effects, reservations, usage, deliverables, provenance, and unfinished work are durably settled. It atomically records the Result, terminal Run Event, and direct-parent delivery intent, cannot be bypassed by a model completion claim, and returns the same settlement when recovered or retried.
_Avoid_: Model-declared completion, child-turn end, author confirmation dialog, best-effort final message

**Subrun Result**:
The immutable hard-bounded typed terminal envelope of one Subrun, binding its Subrun Outcome and completion settlement to produced Artifact and Core Proposal references, observations, effects and disclosures, usage settlement, unresolved work, event range, exact contracts, context, capabilities, and provenance. Its persistence and terminal Mailbox delivery intent form one atomic transition, it never changes Authoritative State, and it affects parent execution only through one Subrun Result Disposition by its direct parent.
_Avoid_: Final chat message, parent outcome, shared state mutation

**Subrun Result Disposition**:
The single immutable direct-parent record that settles one exact Subrun Result as Integrated, ConsideredNotUsed, Superseded, or UnconsumedByTermination, bound to the responsible parent RunStep and Agent Decision or to an exact host termination cause. Applicable results are presented in Subrun Request declaration order rather than completion order; disposition commits atomically with the parent decision when one exists, cannot be reopened, and later references do not create another disposition.
_Avoid_: Mailbox consumption, implicit merge, retry request, multiple consumers

**Subrun Join**:
The immutable Required or Advisory dependency declared by the direct parent in one Subrun Request. Required prevents the creating RunStep from settling until any terminal Subrun Result exists, while Advisory permits it to settle; neither kind automatically merges the result or propagates child success or failure, and any outcome-unknown effect still escalates to the root AgentRun.
_Avoid_: Failure propagation, thread join, implicit result merge

**Step Snapshot**:
The immutable, attributable view of the exact plan revision, Skill Selection Set and SkillPackage Snapshots, context sources, contract versions, capabilities, budget remainder, guardrail counters, and project revisions used for one RunStep. It preserves decision evidence but grants no lasting authority, so effects still require live revalidation before execution.
_Avoid_: Prompt text, checkpoint, authorization token, current project state

**SkillPackage**:
An open Agent Skills standard-compatible directory rooted at one `SKILL.md` with the required name, description, and Markdown instructions plus optional package resources. StoryOS consumes the standard authoring format without turning a Skill into a workflow runtime, Tool, Capability Grant, or source of author authority.
_Avoid_: StoryOS workflow manifest, Agent, Tool, Capability Grant, prompt snippet

**Skill Source**:
The host-verified provenance identity and locator from which a SkillPackage was resolved, such as the StoryOS distribution, a project-authored location, a user library, or a third-party repository. It disambiguates same-name packages and supports signature, installation, and project-allowlist policy but creates neither instruction precedence nor runtime authority.
_Avoid_: Skill Installation Scope, Skill priority, Capability Grant, package name, trusted execution

**Skill Installation Scope**:
The host-controlled ownership and visibility boundary of an installed SkillPackage: SystemOfficial is read-only and bundled with StoryOS, UserLocal is user-owned and available across projects, and ProjectLocal belongs to one novel project and travels with that project. Source and scope are independent: a ThirdParty source must still be installed into UserLocal or ProjectLocal, and same-name packages in any scopes coexist through distinct SkillPackage References rather than overriding or shadowing one another.
_Avoid_: Skill Source, search-path precedence, name override, AgentRun binding

**SkillPackage Reference**:
The immutable host-owned reference to one resolved SkillPackage, recording its Skill Source, standard name, optional declared version and provenance, and a host-verified digest over the exact package contents. Name and version aid selection, source disambiguates collisions, the digest proves the bytes used, and a running AgentRun never silently upgrades its reference.
_Avoid_: Skill name alone, mutable latest version, unverified package path, dependency snapshot

**SkillPackage Snapshot**:
The immutable, project-owned, content-addressed copy of the complete resolved SkillPackage that the Host persists before that Skill first enters a Skill Selection Set, regardless of Installation Scope. Step Snapshots reference it for audit, recovery, export, and exact replay; equal digests may deduplicate storage, but the snapshot is not an installation, is not discoverable or editable as a live Skill, and executes no package code. If policy, license, or storage constraints prevent persisting the exact package, that Skill is ineligible for the project AgentRun rather than being recorded only by name or provenance.
_Avoid_: Skill Installation Record, ProjectLocal Skill, mutable vendoring, external cache, provenance-only record

**Skill Installation Candidate**:
The isolated, non-executable snapshot produced by inspecting one exact external or local Skill Source before installation, including standard validation, license and compatibility, complete file inventory, digest, scripts, extensions, dependencies, and collision evidence. A Candidate is neither installed nor enabled and grants no capability.
_Avoid_: Installed Skill, executable checkout, Approval, SkillPackage Reference

**Skill Installation Record**:
The immutable result of an author-approved atomic installation, binding the exact Skill Source, Candidate digest, target scope, Approval, and resulting SkillPackage Reference. Update and uninstall append new records, never rewrite references held by existing AgentRuns, and installation itself runs no package code or dependency setup.
_Avoid_: SkillPackage, mutable installation state, Capability Grant, script execution

**Skill Revocation**:
The durable project-policy or security determination that an exact SkillPackage Reference may no longer be used for future work, distinct from ordinary update, disablement, or uninstall. Ordinary lifecycle changes affect discovery for new AgentRuns but never retarget an existing Run's snapshot; a revocation is revalidated before the next RunStep, forcing replanning for an AgentSelected Skill or a Run Wait for a Run Skill Requirement, while preserving all prior Step Snapshots and recording the revocation response.
_Avoid_: Package update, silent upgrade, historical deletion, retroactive Step rewrite

**Skill Draft**:
The isolated, author-reviewable candidate standard SkillPackage produced or revised through Skill creation, including its complete file tree, optional StoryOS Skill Extension, validation results, trigger examples, digest, and intended installation scope. A Draft is neither installed nor Agent-visible; author confirmation publishes it through the project write or Skill installation boundary.
_Avoid_: Installed Skill, mutable live package, Core Proposal, implicit publication

**StoryOS Skill Extension**:
The optional `agents/storyos.yaml` product adapter accompanying a standard SkillPackage when it needs typed StoryOS invocation, Hard Applicability, parameters, Tool Roles, or Outcome Profiles. Its absence never invalidates or prevents use of a standard Skill, other Agent Skills clients may ignore it, and none of its fields grants capability or authority.
_Avoid_: Skill standard fork, required manifest, Tool Registration, Capability Grant

**Skill Parameter Set**:
The immutable, schema-validated values chosen from an optional StoryOS Skill Extension for one SkillPackage Reference and, when present, one Skill Outcome Profile. Parameters may narrow or specialize declared behavior but cannot alter the package digest, Hard Applicability, required evidence, Tool effects, Capability, or authority boundaries.
_Avoid_: Skill override, prompt patch, mutable settings, Capability Grant

**Skill Invocation Policy**:
The optional StoryOS Skill Extension ceiling that makes one enabled SkillPackage either ExplicitOrAgent, visible for author selection and Agent discovery, or ExplicitOnly, visible to the author but absent from the Agent catalog until selected; a standard Skill without the extension defaults to ExplicitOrAgent. Project settings may narrow or disable either policy but never widen ExplicitOnly, while persistent product rules belong to policy or higher-authority instructions rather than an AlwaysOn Skill.
_Avoid_: Implicit invocation, AlwaysOn Skill, Tool Exposure, project enablement

**Skill Invocation**:
Either an author action through the Codex-style `$` Skill picker or an Agent semantic choice from natural-language intent. The picker stores an exact SkillPackage Reference and creates a UserSelected Run Skill Requirement; an unambiguous natural-language instruction to use a named Skill does the same, while ordinary intent matching against standard descriptions produces an AgentSelected Skill Selection Decision. A raw ambiguous name creates a Run Wait before the first RunStep, with no scope, installation-time, or search-path precedence and no automatic installation; later updates never retarget the resolved reference.
_Avoid_: Slash command, keyword router, name-only binding, implicit precedence, approximate-name installation

**Skill Catalog Entry**:
The bounded model-visible discovery projection of one eligible SkillPackage, containing its exact standard name and description plus an opaque source-qualified reference that disambiguates same-name packages. StoryOS Skill Extension fields remain host-side during discovery and neither Installation Scope nor source trust creates ranking, authority, or instruction precedence.
_Avoid_: Full SKILL.md, StoryOS Skill Extension, Skill Eligibility, recommendation score

**Skill Instruction Context**:
The structured, attributable model context containing the exact full `SKILL.md` of a successfully loaded SkillPackage for a subsequent RunStep. The Host does not silently summarize, rewrite, or truncate it; extension-derived parameters and contracts enter separately as typed StoryOS context, while other package resources remain unloaded until explicitly requested.
_Avoid_: Skill Catalog Entry, prompt merge, StoryOS Skill Extension dump, package resource bundle

**Skill Context Composition**:
The immutable model-visible collection of separate Skill Instruction Context items used by one RunStep, canonically ordered by their exact SkillPackage References solely for reproducible serialization. Items remain semantic peers, are never merged into a synthetic Skill or ranked by position, and their compatible Outcome Obligations compose as a union; any incompatible combination follows Skill Conflict handling.
_Avoid_: Concatenated prompt, Primary Skill, list precedence, synthesized SkillPackage

**Skill Applicability Contract**:
The two-part boundary describing where a SkillPackage can be used: its standard description and instructions provide Semantic Applicability for the Agent, while an optional StoryOS Skill Extension may add host-evaluated Hard Applicability over typed StoryOS facts. Hard incompatibility cannot be bypassed by explicit selection, and an extension never replaces the standard description as the portable trigger contract.
_Avoid_: Keyword router, model classifier, arbitrary applicability code, Skill Eligibility

**Skill Tool Role**:
A provider-neutral named dependency slot in an optional StoryOS Skill Extension that constrains acceptable StoryOS ToolSpec identities and versions, required input and output semantics, and the maximum Tool Effect Envelope. A Role and the Agent Skills standard's experimental `allowed-tools` field are never StoryOS execution grants; a Role is required unless it declares a bounded optional fallback and its output consequences.
_Avoid_: Tool name, Tool Registration, provider adapter, Capability Grant

**Skill Dependency Resolution**:
The immutable host result that maps each Skill Tool Role used by one RunStep to an exact active Tool Registration and ToolSpec contract digest, or to its declared optional fallback. An unresolved required Role makes the Skill ineligible, while a successful resolution grants neither Tool Exposure nor Capability.
_Avoid_: Tool installation, Skill selection, Tool Exposure, authorization

**Skill Outcome Profile**:
A named contract in an optional StoryOS Skill Extension that defines the typed Artifacts or Operational Results, provenance, validation evidence, completion criteria, and disclosed partial-result conditions for one way of applying the Skill. A standard Skill without Profiles remains usable under its instructions and AgentRun completion criteria; a Profile may be GuidanceOnly but never grants authority or makes output authoritative.
_Avoid_: Output example, Run Outcome, Core Proposal eligibility, permission

**Skill Outcome Obligation**:
The durable requirement created when a Skill Selection Decision chooses one exact Skill Outcome Profile for an AgentRun. The Run Finalization Gate must settle it as satisfied, explicitly partially satisfied, or failed from typed results and evidence; obligations from multiple Skills compose unless they form a Skill Conflict.
_Avoid_: Run Outcome, model completion claim, Artifact, Acceptance

**Skill Selection Set**:
The immutable, attributable set of exact SkillPackage References consulted for one RunStep, with each member recording UserSelected or AgentSelected provenance and its selection reason. Members are semantic peers; future RunSteps may use a different set, while prior selections never change.
_Avoid_: Active Skill, Primary Skill, Supporting Skill, mutable Run-wide Skill list

**Run Skill Requirement**:
An author-originated, reference-bound requirement that one AgentRun honor an explicitly selected SkillPackage across its remaining work. It persists through RunSteps, Waits, and Holds until prospectively superseded by Steering Input, but grants no capability, need not be consulted by every RunStep, and never carries into a new AgentRun.
_Avoid_: Skill Selection Set, permanent project Skill, Capability Grant, implicit inheritance

**Skill Eligibility**:
The deterministic host-derived determination that an exact SkillPackage Reference may be considered for one RunStep under current project enablement, compatibility, declared Skill conflicts, dependency availability, and policy. Eligibility grants no capability and does not choose the Skill on semantic grounds.
_Avoid_: Skill selection, recommendation score, Capability Grant, Tool Exposure

**Skill Selection Decision**:
The attributable part of an Agent Decision that chooses zero or more eligible SkillPackage References to load for subsequent work and, when declared, their exact StoryOS parameters and Skill Outcome Profiles. It records the semantic reason for each choice while the host retains eligibility, loading, and dependency enforcement.
_Avoid_: Host classifier, fixed workflow routing, Skill Eligibility, active Skill

**Skill Load Request**:
A typed request in one Agent Decision to resolve, validate, and load an eligible SkillPackage Reference and its exact Skill Instruction Context for later work. Successful loading may affect only a subsequent RunStep's Step Snapshot and Skill Selection Set, never the requesting RunStep, and grants no Tool, capability, or data access.
_Avoid_: Mid-step context mutation, ToolCall, Capability Grant, implicit activation

**Skill Resource Load Request**:
A typed request to load one exact package-relative resource from a SkillPackage Snapshot for a subsequent RunStep. The Host confines the path to the package, verifies its digest and supported content type, applies per-item and aggregate context limits, and records attribution; reading a resource grants no script execution, Tool use, capability, or outbound disclosure.
_Avoid_: Skill Load Request, arbitrary file read, automatic resource injection, script execution

**Skill Script Execution**:
An ordinary StoryOS ToolCall to an explicitly registered execution Tool, targeting one exact script path inside an immutable SkillPackage Snapshot and recording the runtime identity, inputs, outputs, effects, and execution evidence. Installing, snapshotting, discovering, loading, or reading a Skill never executes its scripts; execution remains subject to Tool Exposure, Capability Grant, Approval, and effect policy, while standard `allowed-tools` metadata grants nothing. If no compatible Tool or declared non-script fallback exists, the affected Skill requirement cannot be completed.
_Avoid_: Executable SkillPackage, installer hook, direct shell instruction, implicit interpreter, Skill-granted capability

**Skill Conflict**:
The condition in which two or more SkillPackage requirements or instructions cannot be jointly satisfied for the current work under higher-authority product, policy, domain, or author constraints. Declared structural conflicts make a combination ineligible, an AgentSelected semantic conflict requires recorded replanning, and a conflict between Run Skill Requirements creates an exact Run Wait for author resolution.
_Avoid_: List order, automatic override, silent merge, Primary Skill

**Agent Decision**:
The single typed decision durably recorded for a RunStep, such as requesting ToolCalls or Subruns, revising a plan, producing an Artifact, asking the author, or proposing Run termination. It records inspectable inputs, outputs, and rationale without storing hidden chain-of-thought.
_Avoid_: Model response blob, ToolCall, RunPlan, hidden reasoning

**Execution Attempt**:
An immutable record of one concrete try to obtain an Agent Decision or execute an already-defined operation. A retry appends a new Attempt under the same still-valid parent and preserves idempotency and effect evidence; it never rewrites the parent RunStep, Agent Decision, or ToolCall.
_Avoid_: RunStep, ToolCall, retry counter, overwritten execution

**Recovery Decision**:
An immutable, inspectable determination after interruption to resume, retry, replan, reconcile, hold, or terminate exact incomplete work based on durable evidence and live revalidation. It never infers success from missing records or silently resamples an already-persisted Agent Decision.
_Avoid_: Automatic replay, checkpoint, hidden recovery heuristic

**RunPlan**:
The optional first-class Operational Record that organizes the intended work of a nontrivial AgentRun as an immutable chain of RunPlan Revisions. A simple AgentRun may proceed without one, but every RunStep still records its immediate objective and a RunPlan never grants capability, budget, or author authority.
_Avoid_: Mutable checklist, Plan Draft, workflow runtime, authorization

**RunPlan Revision**:
An immutable snapshot of a RunPlan's goal, PlanSteps, dependencies, and replanning rationale at one point in the AgentRun. Each RunStep binds the exact revision it used, while replanning appends a revision instead of rewriting prior intent.
_Avoid_: Current checklist, RunStep, mutable plan state

**PlanStep**:
A stable semantic work item within RunPlan Revisions, describing an objective and its dependencies rather than an execution attempt. Its identity survives replanning only while that semantic work remains the same; RunSteps and their results record actual execution.
_Avoid_: RunStep, ToolCall, checklist row, execution status

**Run Checkpoint**:
A durable, Project Scope-bound PostgreSQL projection of one AgentRun at an exact durable sequence, used only to accelerate recovery of its lanes, plan, waits, child operations, and guardrail counters. It contains no live process state or authority, may be discarded and rebuilt from normalized persistent records, and cannot turn a known compacted-payload gap into byte-level replay or a permanent second history.
_Avoid_: Source of truth, backup, Step Snapshot, live session

**Operational History Compaction**:
A policy-versioned automatic retention transition for an eligible terminal, root-sealed Run or Subrun that makes a Compactable Operational Payload unavailable while retaining its Operational Evidence Floor, digest, checkpoint or snapshot evidence, and explicit availability gap. It is distinct from Operational Archive and never changes Authoritative State, an Artifact, or prior context or disclosure history; Artifact Tombstone and author-initiated deletion are separate.
_Avoid_: History rewrite, Artifact Tombstone, cache eviction, silent log deletion

**Operational Retention Class**:
The policy-versioned classification of one exact Operational Record fact or payload role as either an Operational Evidence Floor or a Compactable Operational Payload, independently of the enclosing Run's lifetime. An unknown or unclassified role fails closed to the evidence floor rather than inheriting a Run-wide TTL.
_Avoid_: Run-wide TTL, Artifact Retention State, cache eviction

**Operational Evidence Floor**:
The compactable-payload-independent minimum durable facts for a Run or Subrun: its event identities and sequences, relevant Attempts, Manifests, Receipts, terminal Result and Outcome, Mailbox Seal and deduplication proof, lifecycle decision, digest, and explicit payload-availability gaps. It preserves what happened and its current inspectability without asserting that every historical raw byte remains available.
_Avoid_: Full raw transcript, complete byte replay, Run-wide blob

**Compactable Operational Payload**:
A high-volume, non-authoritative operational byte payload whose current availability is governed separately from its enclosing Run's Operational Evidence Floor, such as eligible stream fragments or redundant diagnostic material. It can become unavailable only through Operational History Compaction after every applicable settlement and Seal boundary; a cache or projection is not such a payload.
_Avoid_: Run Event fact, Artifact payload, disposable cache, silent cleanup

**Operational Archive**:
A reversible Project Scope-bound cold-retention state for an Operational Payload that preserves its bytes and evidence while excluding it from ordinary retrieval, model context, replay service, and outbound disclosure. An authorized explicit inspection or restoration may make it available under current eligibility, but archive never restores a compacted payload or grants past authority.
_Avoid_: Compaction, Tombstone, cache tier, hidden context source

**Redaction Decision**:
An immutable Project Scope-bound lifecycle decision that immediately makes one exact retained payload, fragment, or read-view scope ineligible for current inspection, cache reuse, Context Assembly, export, and future disclosure while preserving historical identity, provenance, and a safe availability gap. It does not retract a prior confirmed submission or rewrite a prior Manifest, Attempt, Event, or Receipt.
_Avoid_: History rewrite, provider recall, delayed cleanup, Archive

**Redaction Execution**:
The fenced, idempotent asynchronous physical cleanup of payload copies authorized by a Redaction Decision after its logical ineligibility is already committed. Completion may establish cleanup evidence but cannot reopen access, make an erased payload reconstructable, or serve as a substitute for the Decision.
_Avoid_: Delayed redaction, cache invalidation only, destructive history edit

**Diagnostic Projection**:
A bounded, non-authoritative, Project Scope-bound operational projection for local logs, tracing, crash diagnostics, or support correlation, containing only sanitized identifiers, reason categories, timings, counters, and safe availability facts. It contains no default prose, prompt, raw Tool/MCP/Provider payload, Credential, or value digest; it is excluded from Project Export and Restore, has its own short Retention Profile, and loses current readability when its source is redacted.
_Avoid_: Shadow transcript, support archive, canonical Attempt evidence, raw debug log

**Run Budget**:
The multidimensional resource envelope governing one AgentRun's cumulative consumption, reservations, concurrency, and finalization capacity. It is bounded by project policy and the Run's Capability Grant and is narrowed, never copied, for descendant Subruns.
_Avoid_: Cost estimate, provider quota, Capability Grant

**Budget Soft Target**:
The per-dimension planning and fairness target below a Budget Hard Ceiling. Crossing it never silently reduces model reasoning or grants more authority; the Run must use pre-authorized borrowing, replan, finalize, or enter Hold for an author decision.
_Avoid_: Hard limit, automatic truncation, guaranteed allocation

**Budget Hard Ceiling**:
The per-dimension maximum admitted by the current project policy and Capability Grant. No new operation may start unless its enforceable worst-case reservation fits beneath it, and only author Approval within the parent policy ceiling may expand it.
_Avoid_: Soft target, provider estimate, post-hoc alert

**Budget Reservation**:
An atomic, durable admission record that allocates both expected use for soft-target accounting and enforceable worst-case headroom for hard-ceiling safety before work starts. Settlement charges actual consumption and releases only unused reservation; parallel work and Subruns cannot reserve the same remaining capacity.
_Avoid_: Usage record, optimistic estimate, copied child budget

**Budget Borrowing**:
An atomic, attributable allocation of unused parent or project soft capacity to a Run whose existing grant already authorizes burst up to its hard ceiling. It is a Policy Decision within existing authority, never implicit grant expansion; consumed cumulative resources do not return to the pool.
_Avoid_: Approval, permission escalation, reclaiming consumed usage

**Finalization Reserve**:
A ring-fenced portion of each applicable Budget Hard Ceiling reserved for coherent stopping work such as summarizing progress, persisting a partial Artifact, and explaining a Hold. It cannot fund new exploratory work, expand capabilities, or exceed an effect boundary.
_Avoid_: General spare budget, retry allowance, shared burst pool

**Outbound Disclosure**:
The transfer of Project Scope-bound information beyond the StoryOS Controlled Processing Boundary to one named External Processing Destination, including generated queries, excerpts, metadata, or Artifact content. Transformation does not stop information from being a disclosure; every transfer must follow a committed Context Assembly Manifest and Destination Context Manifest under the same Project Scope, fit one exact authorized path under destination policy—an effective Project Destination Grant, an applicable Capability Grant or Tool Approval for a Tool Effect Request, or an exact Destination Disclosure Approval—and retain attributable evidence of its Purpose, data categories, and project sources.
_Avoid_: External tool call, network access, upload

**Context Processing Boundary**:
The Host-owned classification of context handling as StoryOS Host Internal Processing, a StoryOS Controlled Processing Destination, or an External Processing Destination, based on exact Registration, StoryOS operational control, enforced Project Isolation, and controlled data path rather than physical location, hostname, deployment topology, infrastructure ownership, or product name. Every actual destination is minimized and evidenced, while only the external class creates Outbound Disclosure.
_Avoid_: Localhost trust, network-only classification, provider brand

**StoryOS Controlled Processing Boundary**:
The deployment-independent, Host-enforced trust and isolation boundary containing StoryOS Host Internal Processing, StoryOS-controlled PostgreSQL persistence, and explicitly registered StoryOS Controlled Processing Destinations. A locally run development service and a later StoryOS-operated cloud service may both enforce this boundary; physical location, loopback transport, first-party branding, infrastructure ownership, or process ownership alone cannot place a processor inside it.
_Avoid_: Local machine boundary, cloud equals external, localhost trust, same account, trusted provider

**StoryOS Host Internal Processing**:
Context handling entirely within StoryOS Core's controlled assembly and domain boundary, such as source resolution, eligibility, selection, or deterministic Host projection. It introduces no separate processing destination or Outbound Disclosure.
_Avoid_: Local model call, separate Tool process, hidden external service

**StoryOS Controlled Processing Destination**:
A separately identified Tool process or other processor operated inside the StoryOS Controlled Processing Boundary under an exact Registration, enforced Project Isolation, and StoryOS-controlled implementation and data path. Its Destination Attempt receives a Destination Context Manifest and creates no Outbound Disclosure. Any downstream crossing of the controlled boundary is a separately identified External Processing Destination with its own complete Context Assembly, disclosure, and Destination Attempt evidence; the current model and embedding API destinations are not members of this class.
_Avoid_: StoryOS Host internal work, trusted by deployment location, external provider, configured API

**External Processing Destination**:
A named model Provider endpoint, remote or independently controlled MCP server, hosted Tool, embedding service, telemetry system, support system, or other processor outside the StoryOS Controlled Processing Boundary. Every submission to it is an Outbound Disclosure regardless of first-party naming, transport route, apparent localhost address, or whether the StoryOS service itself is deployed locally or in the cloud.
_Avoid_: Network request only, provider alias, localhost exemption

**External Provider Accountability Boundary**:
StoryOS controls whether an external dispatch is admitted and records the exact minimum-necessary prepared payload, Purpose, named destination, durable dispatch claim, owning Destination Attempt, and best-known submission certainty. It claims that information was sent only when immutable confirmation evidence establishes ConfirmedSubmitted; OutcomeUnknown remains conservative potential disclosure. StoryOS does not model, verify, or claim control over a provider's internal retention, training, logging, subprocessors, or later handling after transfer. Such provider-internal behavior remains outside StoryOS durable truth and unknown unless evidenced for some separate purpose, without becoming an execution or disclosure guarantee.
_Avoid_: Destination Data Handling Profile, ZDR as no disclosure, vendor compliance registry, provider promise as enforcement

**Deterministic Verification Boundary**:
The boundary of facts an implementation gate may establish reproducibly: given synthetic scoped inputs, a scripted fake destination, and a deterministic fault schedule, it proves only StoryOS-local admission, state transitions, transaction atomicity, scope and redaction enforcement, durable evidence, and recovery classification. A fake may simulate destination observations such as receipt, delay, duplicate delivery, timeout, or crash cut, but it never proves a Provider's internal handling, a model's semantic understanding or creative quality, or that an opaque external system will rerun identically. A real Provider call may create explicitly scoped advisory evidence, but it is not a deterministic implementation gate.
_Avoid_: Provider-internal proof, creative-quality assertion, real-provider CI gate, simulated receipt as external fact

**Deterministic Verification Oracle**:
A small, independently maintained executable state model that consumes a synthetic operation trace and deterministic fault schedule and defines the permitted durable StoryOS facts after execution or recovery. It models authoritative state, Receipts, Events, Attempts, fences, mailbox obligations, and explicit uncertainty only at their contract boundary; generated state-machine and property traces compare the recovered system against that permitted result set, while curated end-to-end scenarios remain readable examples. It does not mirror production storage, worker, cache, or adapter implementation, and it does not resolve opaque external truth hidden behind OutcomeUnknown.
_Avoid_: Mock call expectation, duplicated production runtime, happy-path-only suite, inferred Provider outcome

**OutcomeUnknown**:
The conservative durable settlement after StoryOS has crossed a local authoritative or external-dispatch boundary but cannot prove whether the corresponding external or authoritative effect occurred. It is neither success nor ordinary failure: timeout, lease expiry, process death, lost connection, and a fake destination's hidden state cannot collapse it. A verified pre-dispatch failure remains distinct; an unknown result may settle only when immutable reconciliation evidence enters through the ordinary applicable StoryOS boundary, and any successor requires the existing live revalidation, fresh Attempt, disclosure, and budget rules.
_Avoid_: Timeout as failure, harness-only fake receipt, inferred non-submission, blind retry, zero usage

**Contract Fault Point**:
A stable named deterministic test boundary at one durable or externally irreversible contract transition. Its declaration identifies the fault cut, the exact recoverable evidence boundary, and the permitted recovered state classification; a new durable commit, dispatch claim, seal or fence, replay handoff, or recovery-visibility boundary cannot become implementation-complete without one. It deliberately does not expose or prescribe individual SQL statements, function calls, queue internals, or storage write order.
_Avoid_: SQL-line crash hook, random process kill, implementation-coupled test, inferred recovery state

**Contract-Faithful Fake Destination**:
A deterministic test destination that replaces only an external destination's nondeterministic transport and scripted observations while remaining subject to the same Host admission, Context Assembly, manifest, Destination Attempt, Adapter wire-mapping, result-validation, and subsequent-context boundaries as its production class. It may expose deterministic delay, duplicate, receipt, drift, failure, and crash-cut behavior only through its declared contract; it cannot directly manufacture an Agent Decision, Tool result, Receipt, authority, or settlement inside Core.
_Avoid_: Core shortcut mock, direct Agent Decision injection, bypassed manifest, fake as authority

**Multi-Scope Adversarial Verification World**:
A deterministic test world containing at least two distinct Users and Project Scopes, with independently scoped sessions, identities, Artifacts, Attempts, cursors, grants, Credential bindings, manifests, and disposable projections. Its generated traces deliberately substitute same-shaped foreign references at every scope-sensitive join and assert non-oracular denial, zero unauthorized effect, and absent cross-scope context or egress. A single-Project test may illustrate normal behavior but cannot prove Project Isolation.
_Avoid_: One-Project isolation proof, UUID uniqueness as scope, client-side filter test, positive-path-only corpus

**Negative Evidence Closure**:
The deterministic security-gate rule that a hostile or cross-Scope input proves more than a rejected request: it leaves no unauthorized authoritative change, external disclosure, Attempt, budget consumption, or other effect; its public result remains non-oracular; and its emitted logs, traces, wire records, archives, projections, and test evidence contain no secret or undeclared foreign-Scope identity. Source inspection alone cannot establish this closure; the gate examines the actual emitted test artifacts.
_Avoid_: Status-code-only denial, debug-log leak, source-scan-only proof, hidden foreign identifier

**Verification Evidence Bundle**:
The non-secret, reproducible output of one deterministic gate, containing its synthetic fixture or seed, exact contract and profile revisions, fake-destination script, Contract Fault Points, expected oracle classification, observed safe durable-fact digests, and sanitized egress and diagnostic summary. Both a pass and a failure validate this shape; a counterexample can be rerun from the Bundle without retaining real novel payload, Credential material, raw transport traffic, product-runtime telemetry, or an undeclared foreign-Scope identity. Declared synthetic fixture identities remain safe test inputs rather than product data.
_Avoid_: Passed-only log line, raw crash dump, unreproducible randomized failure, production telemetry record

**Foundation Contract Walk**:
A small synthetic end-to-end conformance path that crosses selected accepted StoryOS boundaries and compares its durable facts with the Deterministic Verification Oracle. A walk may cover editor input, Run and fake destination use, Proposal and Acceptance, replay, crash recovery, or adversarial denial, but it is not a realistic-product mega-test and does not choose the editor-first release stage, UI scope, or handoff criteria owned by the release baseline.
_Avoid_: Product-slice selection, monolithic UI journey, isolated unit test as end-to-end proof, real Provider dependency

**Fail-Closed Verification Gate**:
A required deterministic implementation gate whose oracle mismatch, missing or unreplayable Verification Evidence Bundle, unrun case, or unclassified Contract Fault Point is an unverified or failed result that blocks the claimed contract implementation. Repeated execution, quarantine, or an observational label cannot turn it green. Real-Provider observations, creative-quality assessment, and empirical performance work may remain advisory evidence, but none may substitute for a failed or missing gate.
_Avoid_: Flaky-test waiver, retry-until-green, silent skip, advisory result as conformance proof

**Deterministic Test Scheduler**:
The controlled virtual monotonic clock and explicit interleaving schedule used by deterministic state-machine, property, fault, mailbox, lease, fence, retry, and timeout gates. Time advances only through the recorded test schedule and is included in the Verification Evidence Bundle, so a seed reproduces the same recovery boundary. Real wall-clock sleeps, thread races, and network timing cannot determine a gate result; separately required recovery drills may measure their declared service profile without becoming synthetic performance thresholds.
_Avoid_: Sleep-based test, scheduler race, accidental timeout, throughput benchmark as semantic gate

**Destination Context Manifest**:
The immutable provider-neutral Operational Record describing one exact minimum-necessary Effective Destination Context under one Context Assembly Manifest, exact requester User and Project Scope, Purpose, Processing Destination Identity and its current evidence revision, processing-boundary class, policy, applicable grant, approval requirement, and any authorization already effective when it commits. It is required for every StoryOS Controlled or External Processing Destination and is neither an actual Destination Attempt nor proof of destination-internal use; a later one-shot Destination Disclosure Approval binds the established Attempt rather than mutating this Manifest.
_Avoid_: Context Assembly Manifest, provider request, Outbound Disclosure Event

**Outbound Disclosure Manifest**:
The immutable Operational Record specializing one Destination Context Manifest for one exact External Processing Destination, additionally binding its Project Scope, applicable outbound data categories, and disclosure policy. Identical currently eligible submissions under the same Project Scope may reference it after revalidation, but it is neither an actual transfer nor evidence that a prior Destination Attempt performed a later operation.
_Avoid_: Destination Context Manifest alone, Outbound Disclosure Event, reusable authorization, provider request log

**Effective Destination Context**:
The complete logical content, instructions, Tool contracts, and other context StoryOS makes newly available or intentionally references for one exact Destination Attempt, including transmitted material and known cache, prior-response, remote-state, or provider projections. Every component is classified as exactly reconstructable, reference-known, or provider-opaque; unknown provider-internal retention, transformation, or use remains unknown and can never be presented as exact fact.
_Avoid_: Wire Payload Projection, request delta, provider cache entry, opaque continuity state

**Wire Payload Projection**:
The exact non-secret provider-, protocol-, and Adapter-specific application payload bytes, frames, fields, or access-controlled payload references prepared for one Destination Attempt, together with opaque Credential References or secret-injection slots, their mapping version, and a digest over non-secret material only. Credential values, credential-value digests, and credential-bearing transport-envelope bytes remain ephemeral and are never persisted as this Projection. It is bound to any local outbound dispatch through its Disclosure Event and is wire-form evidence rather than proof of destination receipt, the canonical semantic request, or the complete Effective Destination Context.
_Avoid_: Context Assembly Manifest, Effective Destination Context, provider payload as truth

**Outbound Disclosure Event**:
The immutable Operational Record transactionally created when an egress worker durably claims one admitted Destination Attempt at StoryOS's local outbound dispatch boundary, binding its exact Project Scope-bound Outbound Disclosure Manifest and already-persisted Wire Payload Projection before any external I/O is permitted. The Event begins as OutcomeUnknown conservative potential-disclosure evidence rather than a claim that the destination received bytes; later immutable confirmation evidence may settle the Destination Attempt as ConfirmedSubmitted without rewriting the Event. Every dispatch claim and redispatch has its own Event even when payload bytes and the Manifest are reused, while a failure proven to occur before the durable claim creates no Event. A crash after the claim remains OutcomeUnknown even if no bytes ultimately left, so an actual disclosure can never lack prior durable evidence.
_Avoid_: Outbound Disclosure Manifest, Destination Attempt, planned transfer, cache hit

**Processing Destination Identity**:
The immutable, Host-owned, exact Project Scope-bound, non-authorizing record of one actual processing and disclosure boundary, established independently from a project-free Registration service surface plus append-only versioned identity evidence and, only when needed to identify the actual account boundary, a Project Credential Binding used solely as non-authorizing evidence; it names the processor, endpoint, account boundary, control classification, and governing intake or disclosure boundary without containing a Project Destination Grant, external-use binding, compatibility Decision, or execution authority. A Registration, Adapter, serialization, model, or Credential revision may reuse the same Identity only when a current immutable evidence revision proves those actual boundaries are unchanged—credential locator similarity alone never proves the account—while any processor, endpoint, account, control, or intake/disclosure-boundary change creates a new Identity before new authorization and use records.
_Avoid_: Provider brand, SDK client, hostname alone, credential, shared vendor account, use binding as identity resolver

**Project Destination Grant**:
An author-owned, versioned project-policy Operational Record that enables one exact Processing Destination Identity for named ordinary Purposes, outbound data categories, and hard disclosure bounds under one Project Scope. Destination Attempts for model and embedding operations that remain inside the effective Grant proceed without individual confirmation but still cross all seven Context Assembly gates and create complete applicable Manifest and Destination Attempt evidence plus an Outbound Disclosure Event when external dispatch is durably claimed. Configuring a Credential Reference, discovering a destination, or having disclosed to it before grants nothing; a new or changed destination, Purpose, data category, or wider bound requires an explicit project-setting change or an exact Destination Disclosure Approval Wait before submission.
_Avoid_: Credential configured, Provider discovered, blanket vendor consent, per-call prompt, prior disclosure as permission

**Destination Attempt**:
The immutable Operational Record and execution evidence for one concrete planned execution or submission attempt to one exact Processing Destination Identity and current Identity evidence revision under one Destination Context Manifest, established durably before destination I/O and settled as pre-dispatch, dispatched, or outcome-uncertain with the applicable submission certainty, wire evidence, outcome, usage, and correlation facts. Its existence alone never proves dispatch. Every physical resend, retry, fallback, or destination change creates a new Destination Attempt; Model Attempt and destination-specific Tool or service attempt records refine this boundary rather than replacing it.
_Avoid_: Logical Invocation, prior Attempt reuse, Disclosure Manifest, SDK hidden retry

**Destination Attempt Admission Decision**:
The immutable fail-closed Host decision at the final pre-I/O boundary for one exact Destination Attempt, revalidating its Project Scope, source and Projection dependencies, Lifecycle, applicable Memory Suppression for memory-derived or ordinary-recall dependencies, Context Exclude, requester permission, grants and exact Tool or Destination Disclosure Approval when required, destination identity and its evidence revision, Registration status, governing intake contract, policy, and budget against current versions. Any changed or unverifiable dependency refuses submission, preserves prior manifests, settles the unsubmitted Attempt, and requires new Context Assembly; only an admitted Decision may cross the destination boundary.
_Avoid_: Context Assembly Manifest, cached authorization, provider retry flag, post-send audit

**Capability Grant**:
A bounded authorization to request named operations over specified project resources, external destinations, data categories, budgets, and time. Effective authority is always the non-escalating intersection of the project policy ceiling, the current Run's Capability Grant, and the exact capability requested by a ToolCall; approval may narrow or extend a lower layer only within its parent boundary.
_Avoid_: Role, permission flag, discovered tool, model-visible tool

**Approval**:
An immutable author decision over one exact typed operational request, input digest, scope, and governing policy. The closed current kinds are Tool Approval and Destination Disclosure Approval; each binds its own complete request shape, grants nothing before the decision, and requires a new decision when a bound input changes. Permanent project policy changes occur only through explicit settings. Approval never performs Acceptance or changes Authoritative State.
_Avoid_: Permission flag, confirmation dialog, Acceptance, permanent project setting

**Tool Approval**:
The Approval kind bound to an exact ToolSpec version, arguments, resolved targets, Tool Effect Request, scope, and governing policy. It may create a grant for only that ToolCall or a bounded remainder of the current Run; high-risk disclosure, external writes, and irreversible effects remain one-shot.
_Avoid_: Destination Disclosure Approval, Tool Exposure, Capability Grant, Acceptance

**Destination Disclosure Approval**:
The Approval kind bound to one exact Operation Requirement, Processing Destination Identity, Purpose, outbound data categories, hard bounds, Destination Context Manifest, source and Projection closure, governing policy, and one already-established but unsubmitted Destination Attempt. It may be created only after that Attempt and its exact Wire Payload Projection exist, and the final Destination Attempt Admission Decision must bind and revalidate the Approval before I/O. It can authorize only that exact disclosure boundary; changing destination, Purpose, category, bound, or logical or wire payload closure requires a new Decision, while any reusable ordinary authorization requires a Project Destination Grant setting. Every Destination Disclosure Approval is one-shot.
_Avoid_: Project Destination Grant, Tool Approval, blanket provider consent, prior disclosure

**Policy Decision**:
An immutable result of StoryOS deterministically evaluating a request against already-effective project policy and Capability Grants. It may authorize an in-scope request or deny it, but it can never create, extend, or replace a Capability Grant; new authority requires author Approval.
_Avoid_: Approval, policy-authored grant, implicit permission

**Model Gateway**:
The sole StoryOS-owned boundary through which any RunStep invokes a configured external model API. It applies only an exact Project Scope-bound Model Route Decision that pins one current Project Model Use Binding and the separate admitting External Contract Compatibility Decision over that binding, requires fallback to produce a new Route Decision and revalidate both records, and never executes model-produced Tool requests; only a validated, persisted Agent Decision may derive ToolCalls for the Tool Gateway.
_Avoid_: Provider client, model SDK, Tool Gateway, direct provider call

**Model Provider Adapter**:
The host-controlled protocol projection used by the Model Gateway to exchange one exact invocation with an external model Provider API and report provider-declared capabilities and failure evidence. It cannot decide retryability, select or substitute a model, initiate fallback, execute ToolCalls, grant authority, or become durable Run truth; Bailian or any other configured Provider is an Adapter choice rather than a kernel requirement.
_Avoid_: Provider Adapter, provider-owned router, silent fallback, Tool executor

**Model Registration**:
The host-owned, versioned, globally reusable non-authorizing contract identity that binds one stable StoryOS model reference to an exact Model Provider Adapter, project-free Provider API or service surface, provider model identifier, and Model Capability Profile revision. It contains no Project data, Project enablement, Credential Reference, actual processor endpoint or account boundary, disclosure destination, grant, compatibility admission, or runtime use state. An opaque provider alias remains explicitly unverifiable, any contract or Adapter binding change creates a new Registration revision, and provider evidence that conflicts with an exact binding creates Model Failure rather than rewriting past Run evidence.
_Avoid_: Model name, provider alias, deployment name, capability tier, global Credential Reference, provider account

**Project Model Use Binding**:
The model-surface use of the single shared `ProjectExternalUseBindingRevision` contract: one immutable, exact Project Scope-bound binding that makes one active Model Registration revision eligible for compatibility evaluation and named project Purposes by composing the current Project Destination Grant or other owning use authorization, one Project Scope-bound Credential Reference binding when required, one already-established Processing Destination Identity with its exact current evidence revision, and non-widening hard bounds. It is not a second model-specific record or shape, does not resolve or create the Identity, and contains no External Contract Compatibility Decision: after the binding exists, a separate immutable Decision evaluates it together with its global Registration and Adapter. Every Model Operational Snapshot, Model Route Decision, Model Invocation, Model Attempt, and fallback pins and revalidates both records. Host defaults and project settings establish the binding without per-call author configuration; Bailian or any other Provider remains only an Adapter choice, and no credential identifier forms a global namespace.
_Avoid_: Model Registration, External Contract Compatibility Decision, global provider credential, Model Route Decision, per-call provider setting

**Model Registration Status**:
The durable Active, Quarantined, or Retired global contract-eligibility state of one exact Model Registration revision: only Active may be considered for a new Project Model Use Binding or Model Route Decision, Quarantined requires explicit Host revalidation, and Retired never returns to service. Status changes preserve past evidence; Project credential availability and dynamic route facts belong to a Project Scope-bound Model Operational Snapshot, while reintroducing a Retired contract requires a new Registration revision.
_Avoid_: Provider health, credential availability, model version, mutable Registration

**Model Capability Profile**:
The immutable, versioned, provider-neutral semantic envelope trusted for one Model Registration, covering supported input and output modalities, context and output bounds, streaming, Tool-request and structured-output semantics and their exact native or Host-compiled projection modes, generation controls, and reportable usage dimensions. Provider claims enter it only through Host mapping or validation, and an unknown required capability makes the Registration ineligible.
_Avoid_: Provider model card, Model Operational Snapshot, benchmark score, availability state

**Model Operational Snapshot**:
An immutable, attributable, exact Project Scope-bound point-in-time observation of one Project Model Use Binding, its global Model Registration, and the separate External Contract Compatibility Decision for that pair, including current Credential Reference binding availability when required, destination eligibility, provider health, rate-limit or quota state, latency, pricing reference, and other dynamic routing facts. Project-free provider observations may be shared only as non-authorizing inputs; the Snapshot repeats the exact binding and Decision before they can affect route eligibility. It may change current eligibility without changing the Registration or Model Capability Profile and never proves semantic capability.
_Avoid_: Model Capability Profile, Model Registration, durable model identity

**Model Routing Policy**:
The immutable, versioned Host rule set that deterministically filters exact Project Model Use Binding and Model Registration pairs only through their separate admitting External Contract Compatibility Decisions and hard Model Route Request requirements, then ranks eligible pairs by declared soft preferences with a stable tie-breaker. Models and providers cannot author it; benchmark or learned evidence must be explicit and versioned, while random or experimental routing requires a separately authorized policy rather than hidden selection.
_Avoid_: Model recommendation, provider router, mutable score, implicit experiment

**Model Route Request**:
The immutable, exact Project Scope-bound pre-sampling statement of hard model capabilities, context bounds, allowed provider and Outbound Disclosure destinations, budgets, and soft quality, latency, and cost preferences for one RunStep, assembled by the Host from its exact plan, Skills, author settings, policy, grants, and inputs. It names no executable model or credential, grants no authority, and exists before that RunStep's Agent Decision.
_Avoid_: Model name, prompt hint, Model Route Decision, model self-selection

**Model Route Decision**:
The immutable exact Project Scope-bound Host result that either selects one exact Model Registration revision together with one current Project Model Use Binding and its separate admitting External Contract Compatibility Decision for a Model Route Request or records that no eligible route exists. It binds the Model Routing Policy revision, complete evaluated candidate binding/compatibility set and reasons, Capability Profiles, Project Scope-bound Operational Snapshots, Credential binding generations when required, grants, budgets, and comparison evidence used. It precedes sampling, cannot be authored by a model, and every fallback requires a new Route Decision over the same hard requirements with a freshly revalidated use binding and compatibility Decision.
_Avoid_: Model suggestion, mutable route, provider fallback, load-balancer choice

**Model Route Override**:
An immutable root-AgentRun-scoped author setting captured as Automatic, Prefer an exact Model Registration revision, or Require that revision, and applied prospectively to every Model Route Request in the whole execution tree; descendant Subruns may only add narrower requirements. It never creates or selects a Project Model Use Binding and never bypasses capability, disclosure, compatibility, grant, budget, credential, or policy eligibility; Prefer may allow another Route Decision, while Require records no eligible route instead of falling back.
_Avoid_: Optional model ID, mutable active model, Capability Grant, provider fallback

**Model Fallback**:
The Host-controlled admission of a successor Model Attempt for the same Model Invocation using a different exact Model Registration revision, its exact Project Model Use Binding, and the separate admitting External Contract Compatibility Decision after a new Model Route Decision re-evaluates the unchanged Model Route Request. It never reuses the prior route's Credential, use binding, or compatibility Decision, never relaxes hard requirements, never repeats a Registration revision within one fallback chain, remains bounded by all Run budgets and overrides, and cannot be delegated to a provider router or SDK.
_Avoid_: Same-route retry, provider substitution, capability downgrade, fallback loop

**Model Invocation**:
The single logical exact Project Scope-bound request by one RunStep to obtain one Agent Decision under an immutable Model Route Request, owning the ordered Model Attempts and their exact Project Model Use Binding plus separate External Contract Compatibility Decision history, with derived aggregate outcome and usage. Provider completion terminates only its Attempt; the Invocation succeeds only when the Host validates and durably records one typed Agent Decision, and a later successful Attempt never erases earlier binding, compatibility, or execution evidence.
_Avoid_: Provider request, Model Attempt, model response blob, retry counter

**Model Attempt**:
The model-specific Execution Attempt durably established before one concrete provider submission under one exact Model Route Decision, repeating the exact Project Scope, Project Model Use Binding, separate External Contract Compatibility Decision, global Model Registration, Credential binding generation when required, actual Processing Destination Identity and current Identity evidence revision, and final destination admission evidence. It binds its request and disclosure evidence to the resulting stream, provider identifiers, partial output, usage, uncertainty, and terminal outcome. Retrying the same Registration appends an Attempt only after revalidating the same use binding and compatibility Decision; fallback requires a new Model Route Decision and the selected route's own binding and subsequent Decision. Outputs from separate Attempts are never silently concatenated.
_Avoid_: Model Invocation, provider retry counter, overwritten request, merged fallback response

**Model Attempt Request**:
The immutable provider-neutral effective request for one Model Attempt, binding its exact Project Scope, Project Model Use Binding, separate External Contract Compatibility Decision, Step Snapshot, Context Assembly Manifest, prompt and output contracts, Tool Exposure and ToolSpec digests, generation controls, streaming mode, output bounds, Model Route Decision, and parameter provenance or default state. Its Adapter projection records the mapping and wire-request digests without silently changing required semantics; ordinary retry preserves the semantic digest, binding, and Decision after live revalidation, repair creates a new Request, and fallback changes the route/binding/Decision and may change only the provider projection while preserving the logical request.
_Avoid_: Model Route Request, mutable prompt, provider payload as canonical contract, silent default

**Model Attempt Cancellation**:
An immutable Host cancellation fence persisted before any best-effort provider abort, permanently preventing its Model Attempt from supplying an Agent Decision. It distinguishes confirmed non-submission or provider-confirmed cancellation from OutcomeUnknown after possible submission, retains partial and late evidence for reconciliation, and permits no successor without a Recovery Decision or after Run Cancellation.
_Avoid_: Closing a stream, confirmed provider stop, discarded output, automatic retry

**Model Repair Attempt**:
A bounded successor Model Attempt within the same Model Invocation whose only semantic addition is Host-generated validation diagnostics for a completed output that could not form an Agent Decision. It retains the exact RunStep objective, Step Snapshot, Model Route Request, Tool Exposure, capability, and authority boundaries while creating fresh request, disclosure, and budget evidence; changing any retained boundary requires a new RunStep and Model Invocation.
_Avoid_: Replan, expanded prompt, hidden context injection, ordinary retry

**Model Stream Event**:
An immutable, provider-neutral observation in one Model Attempt's strictly ordered Host sequence, preserving provider correlation and whether its content is provisional or terminal. The Author UI projects the same sequence; partial text, reasoning summaries, and Tool arguments remain evidence only until a complete Agent Decision is validated, while raw provider traffic is optional diagnostics rather than durable Run truth.
_Avoid_: UI token, raw SSE frame, transcript entry, Agent Decision

**Model Tool Request**:
A provider-neutral candidate inside one complete model output that names an exposed Tool and supplies its business arguments without creating authority or a ToolCall. Native and Host-compiled requests share this form; the Host validates the entire requested batch against the exact Step Snapshot before persisting the Agent Decision and idempotently deriving independently authorized ToolCalls, while any invalid member rejects the whole batch.
_Avoid_: ToolCall, provider function frame, executable request, provider-hosted Tool

**Model Failure**:
An immutable, provider-neutral evidence record bound to the applicable Model Route Request, Model Route Decision, or Model Attempt, identifying failure phase, submission certainty, provider correlation and status, retry hints, partial output, and original diagnostics without granting retry or fallback. Provider refusal is a completed semantic result rather than a Model Failure; only a Host Recovery Decision may choose same-route retry, repair, Hold, termination, or fallback through a new Model Route Decision.
_Avoid_: Provider error string, retryable flag, refusal, Recovery Decision

**Model Attempt Outcome**:
The immutable settlement of one Model Attempt from provider and stream evidence, distinguishing a confirmed result from OutcomeUnknown. An unknown Attempt is never ordinary failure or zero usage; a successor may be admitted only after live revalidation, a new Outbound Disclosure record, and budget reservation for both Attempts, after which the predecessor can supply only late reconciliation evidence rather than an Agent Decision.
_Avoid_: HTTP status, missing terminal event, inferred failure, retry permission

**Model Usage Settlement**:
An immutable per-Attempt accounting record that distinguishes provider-reported, Host-estimated, and unknown usage and cost, while Model Invocation totals remain derived from all Attempts. OutcomeUnknown retains its enforceable worst-case Budget Reservation until later evidence confirms consumption or releases unused headroom, and absence of evidence never settles it as zero.
_Avoid_: Provider invoice, optimistic token estimate, Invocation-only total, cleared reservation

**Model Telemetry Projection**:
A sampleable, droppable, and redactable traces, metrics, or logs view derived from durable Model Invocation, Model Attempt, routing, stream, failure, disclosure, and usage records. It may correlate provider and transport identifiers but never supplies StoryOS identity, authority, recovery, budget settlement, audit truth, or Author UI state.
_Avoid_: Durable Run evidence, provider log as truth, recovery source, audit ledger

**StoryOS ToolSpec**:
The provider-neutral, versioned semantic contract for one Tool, consisting only of its callable input, output, and error contract, Destination Context Intake Contract, Tool Effect Envelope, execution policy, and result and provenance rules. Implementation source and credentials belong to Tool Registration, while project enablement, provider compatibility, Exposure, grants, Approval, pricing, and invocation state remain separate dynamic records.
_Avoid_: Provider function schema, Tool Registration, installed tool, ToolCall

**Destination Context Intake Contract**:
The exact provider-neutral fields, data categories, source classes, Purpose, and hard bounds one non-model Tool, MCP server, embedding service, telemetry sink, or other destination may receive, with no Ambient Context. Required project data is supplied only through explicit inputs or governed StoryOS-controlled references under one Project Scope, and the Contract grants neither source access, Capability, nor Disclosure Eligibility.
_Avoid_: Full prompt forwarding, implicit Transcript, inherited model context, arbitrary project read

**Ambient Context**:
Any Transcript, Project Instruction, Working Target, Agent Memory, project data, or other surrounding content supplied to a non-model destination without an exact Destination Context Intake Contract field and current minimum-necessary decision. Ambient Context is prohibited even when the destination shares a Provider, process, network connection, or AgentRun.
_Avoid_: Convenience metadata, default Tool context, provider session state

**Provider-hosted Tool Destination**:
A Tool executed by or behind a model Provider rather than StoryOS's controlled Tool Gateway implementation boundary. It remains a distinct External Processing Destination with its own Registration, Intake Contract, Capability, Destination Context Manifest, Outbound Disclosure, and Destination Attempt evidence and inherits none of the model destination's context or permission.
_Avoid_: Model Tool Request, StoryOS ToolCall, provider prompt capability, inherited disclosure

**Telemetry Disclosure**:
An Outbound Disclosure to a traces, metrics, logs, debug, crash-reporting, or support destination, defaulting to sanitized operational categories, identifiers, timings, and digests rather than project prose, prompts, research, Tool results, Project Instructions, or credentials. It is independently current-eligibility and Redaction checked, never enters Project Export or Restore, and telemetry's diagnostic purpose never grants ambient access to durable Run or Artifact payloads.
_Avoid_: Model Telemetry Projection, debug upload, support bundle as permission

**Tool Discovery Record**:
An immutable observation of a third-party Tool contract and source identity before StoryOS has assigned trusted Host semantics. It has no Tool Registration identity, project enablement, Exposure, or execution authority; explicit StoryOS-controlled mapping may use it to create a new Tool Registration.
_Avoid_: Tool Registration, installed tool, trusted ToolSpec

**Tool Registration**:
The host-owned, versioned record that binds a built-in implementation or exact Tool Discovery Record to its trusted StoryOS ToolSpec, implementation source, and adapter rules. Its lifecycle is active, quarantined, or retired; discovery and project enablement remain separate records.
_Avoid_: Tool Discovery Record, Project Tool Enablement, Tool Exposure

**Project Tool Enablement**:
A project's explicit enabled or disabled selection of one exact active Tool Registration. It permits the Registration to be considered for Exposure but grants no Run capability or execution authority.
_Avoid_: Tool Registration, Tool Exposure, Capability Grant

**Tool Contract Drift**:
The condition in which a Tool Registration's pinned implementation identity, trusted model-visible callable contract, input or output contract, or trusted adapter mapping no longer matches the currently discovered implementation. Drift quarantines the Registration for new calls, clears derived Exposure, and requires a new StoryOS-controlled mapping; untrusted descriptive provenance alone does not cause Drift, and name equality never carries authority across versions.
_Avoid_: Compatible runtime update, automatic permission inheritance, retryable tool error

**Tool Exposure**:
The disposable Project Scope-bound projection of an enabled Tool for one caller and RunStep, computed from two orthogonal inputs: the Host-allowed caller routes and the current caller's initially-visible, deferred, or hidden discovery state. Exposure also depends on provider compatibility and current policy, but neither grants execution authority nor changes the Tool Registration.
_Avoid_: Tool Registration, project enablement, authorization

**ToolCall**:
An Operational Record for one requested invocation of an exact Tool Registration, including its caller route, validated arguments, resolved targets, Tool Effect Request, authorization state, execution lifecycle, and outcome. A ToolCall may produce Artifacts or other Operational Records but never inherits their lifecycle or authority.
_Avoid_: Tool result, Artifact, Approval, model message

**Tool Gateway**:
The sole StoryOS-owned authorization and execution boundary for every StoryOS-dispatched ToolCall, regardless of whether its caller is a model, generated program, MCP App, or host component. It resolves the Tool Registration, enforces the caller's exact Project Scope, derives effects, enforces grants and Approval, invokes the trusted implementation, validates output, and records the outcome; provider-hosted execution cannot claim this StoryOS-controlled guarantee.
_Avoid_: Tool Registry, provider runtime, direct adapter call

**Credential Reference**:
An opaque, host-owned, Project Scope-bound reference to credential material held by a deployment-specific secret backend and resolved only inside the execution boundary that needs it. PostgreSQL may retain its backend identity, non-secret locator and generation, availability or rebinding state, and Project Scope-bound use-binding metadata, but never the value or a value digest; the Reference, binding, and availability evidence grant no destination use, while Registrations and Operational Records may identify them without containing the secret. The Foundation-local backend is macOS Keychain, later controlled-cloud deployments use the same resolver contract with a managed secret service, and environment variables are development/test inputs only. Ordinary database backups, logs, support material, and Project exports omit secret material; an import whose destination cannot resolve a Reference leaves it explicitly Unbound until an authorized rebind. Models, MCP Apps, generated programs, Tool arguments, outputs, transcripts, and external servers cannot inspect, select, or transport credential material.
_Avoid_: API key field, encrypted database secret, secret-value digest, portable secret export, production environment variable

**Tool Effect Envelope**:
The versioned, host-owned upper bound on the composable effects a registered Tool may request, covering project reads, Artifact writes, Outbound Disclosure, and external reads or writes. Artifact writes distinguish creating an Artifact from appending an Artifact Revision. The Envelope can never include mutation of Authoritative State, and untrusted Tool or MCP annotations cannot expand it.
_Avoid_: Tool category, approval, model-declared effects

**Tool Effect Request**:
The exact effects StoryOS derives for one ToolCall from its registered Tool Effect Envelope, validated arguments, resolved targets, and trusted adapter rules. It must fit both the Envelope and the effective Capability Grant; the model may choose business arguments but its own effect labels never grant authority.
_Avoid_: Model self-classification, Tool Effect Envelope, actual outcome

**Tool Effect Outcome**:
The structured observation of which requested effects were not attempted, confirmed, partially confirmed, or remain unknown after a ToolCall attempt. It binds actual project reads, Artifact Revisions, Outbound Disclosures, external reads, and external writes to evidence; an uncertain external effect remains unknown and cannot be treated as an ordinary failure or automatically retried.
_Avoid_: Tool success flag, Tool Effect Request, inferred side effect

**Artifact**:
A durable, typed output or evidence item produced during author, Agent, or Tool work. An Artifact may propose or support a change, but never becomes Authoritative State in place.
_Avoid_: Result, blob, authoritative artifact

**Artifact Identity**:
The stable identity whose revisions form one linear history guarded by an expected revision. Alternative or merged work creates a new derived Artifact, while the provenance graph across Artifact identities may form a DAG.
_Avoid_: Content hash, revision branch

**Artifact Revision**:
An immutable snapshot of an Artifact's content and provenance. Derivation and Acceptance always reference an exact Artifact Revision rather than only the evolving Artifact identity.
_Avoid_: Current blob, mutable version

**Content Digest**:
An integrity identifier for an immutable payload computed under one exact Digest Profile that may also support physical storage deduplication. It never replaces Artifact or Artifact Revision identity, so causally distinct outputs remain distinct even when their payloads match.
_Avoid_: Artifact ID, semantic identity, unprofiled hash

**Digest Profile**:
The versioned, purpose-specific contract that defines the exact typed fields and canonical bytes covered by a Content Digest or command digest. A Profile is reproducible across StoryOS runtimes, never silently normalizes authoritative prose, and cannot be reused as a different integrity claim merely because the bytes match.
_Avoid_: Runtime serialization, implicit Unicode normalization, universal hash meaning

**Provenance**:
The structured lineage that identifies an Artifact Revision's creator, exact source revisions or snapshots, schema version, creation time, and integrity digest. Provenance belongs to the Artifact Revision itself and does not depend on reconstructing a Run log.
_Avoid_: Metadata blob, inferred history

**Provenance Edge**:
A typed relationship from an Artifact Revision to an exact source or cause. Its role distinguishes direct derivation, evidentiary support, context availability, and the Message or goal being answered.
_Avoid_: Source list, citation text

**Creator**:
The single actor or causal execution step that directly produced an Artifact Revision. Earlier authorship and contributions remain visible through Provenance Edges rather than a mutable contributors list.
_Avoid_: Contributors array, original creator only

**External Source Snapshot**:
A Research Artifact Revision containing an immutable captured version of externally retrieved evidence, including when and where it was obtained and an integrity digest of the captured content. Re-fetching creates a new Snapshot, while annotation or correction creates a derived Research Artifact; a live URL alone is not a Snapshot.
_Avoid_: Bookmark, source URL

**Imported Source Snapshot**:
A Research Artifact Revision containing an immutable capture of evidence supplied from a local file or explicit import. Re-importing creates a new Snapshot, while annotation or correction creates a derived Research Artifact.
_Avoid_: Attachment, untracked file

**Evidence Locator**:
A snapshot-relative, media-specific coordinate that lets an inspector re-find exact content inside one Source Snapshot Revision. It may identify structured web content, a PDF page and region, a media time range with transcript Revision, or a dataset version and replayable record or result; a live location alone is not evidence.
_Avoid_: Live URL fragment, current page position, citation text without a Snapshot

**Evidence Relation**:
A Claim-scoped relationship to an exact Source Snapshot Revision and Evidence Locator that distinguishes supporting, opposing, or qualifying evidence and names the exact Claim or subclaim addressed. `available_as_context` remains independent and proves neither use nor support, while derived conclusions retain their intermediate analysis rather than claiming that a source stated them directly.
_Avoid_: Source list, whole-document citation, context availability as support

**Research Synthesis**:
A Research Artifact that combines or interprets evidence and binds its claims to exact Source Snapshot revisions through supported-by Provenance Edges.
_Avoid_: Source Snapshot, uncited summary

**Research Claim**:
A stable, addressable, independently evaluable conclusion within a Research Synthesis, linked to exact evidence through Evidence Relations. Evidence that addresses only part of a compound conclusion must target an explicit subclaim or cause the Claim to split; a Claim remains non-authoritative regardless of evidence, confidence, or repetition.
_Avoid_: Claim (too generic), Canon fact, paragraph citation

**Finding**:
A stable, addressable conclusion within an Analysis Report, linked to its project targets and supporting evidence. A Finding may suggest Candidates or Proposals but cannot directly change Authoritative State.
_Avoid_: Decision, automatic fix

**Artifact Lifecycle Event**:
An auditable transition in an Artifact's workflow or retention state, tied to an exact Artifact Revision and attributed to an actor and reason. It changes the Artifact's current state projection without creating a content revision.
_Avoid_: Status edit, metadata revision

**Retention State**:
The common disposition of an Artifact independent of its type-specific workflow: retained, archived, or tombstoned. Archived content is excluded from normal retrieval, while tombstoned is a terminal state that removes content from use and retains only per-revision minimum identity and audit relationships.
_Avoid_: Workflow state, authority level

**Artifact Tombstone**:
The minimum non-content records left for an Artifact and each Revision after the author removes their owned payloads, indexes, and derived caches. They preserve artifact and revision IDs, parent link, kind, creation time, integrity digest, deletion provenance, and necessary relationships without deleting separately referenced Artifacts or shared payloads still referenced by another logical Artifact.
_Avoid_: Archived Artifact, soft-deleted payload

**Purged Source Reference**:
A read-time projection shown when an immutable Provenance Edge resolves to an Artifact Revision Tombstone. The original edge never changes; the projection exposes the removed revision identity and digest and makes lost verifiability explicit.
_Avoid_: Broken link, hidden deletion

**Workflow State**:
The type-specific progress of a Core Artifact through its own review or production process. Workflow State is independent of Retention State and never grants authority by itself.
_Avoid_: Authority level, retention status

**Artifact Closure**:
The reversible open or closed disposition used only by Candidates and Drafts, with the closed reason `dismissed`, `superseded`, or `abandoned`. Deriving a Proposal does not close its source Artifact.
_Avoid_: Proposal resolution, archive

**Supersession**:
A provenance relationship stating that a newer Artifact takes the place of an older one for a stated purpose. It preserves both Artifacts and does not rewrite their revision histories.
_Avoid_: Overwrite, implicit latest

**Core Artifact**:
An Artifact type whose semantics and lifecycle are owned by StoryOS. Any Artifact capable of proposing a change to Authoritative State must be a Core Artifact.
_Avoid_: Built-in output, privileged extension

**Extension Artifact**:
A namespaced and versioned Artifact type produced by a Tool or MCP extension for inspectable data or presentation. A known enabled schema may request a Core Proposal through the Host, while an unknown schema is preserve-and-read-only and cannot invoke Tools, source or produce Proposals, validate, or participate in Acceptance; compatible migration appends a revision, while semantic-identity change derives a new Artifact.
_Avoid_: Plugin-owned state, MCP-owned truth

**Proposal**:
A Core Artifact containing inspectable, core-validatable domain changes together with their targets, base versions, and preconditions. It is the only Artifact kind that can become eligible for Acceptance, but Proposal identity alone never grants eligibility.
_Avoid_: Suggestion, direct write, executable extension

**Proposal Operation**:
A stable, independently resolvable domain change within a Proposal; dynamically calculated diff hunks are never operations. Historical applied or rejected incarnations remain frozen, while reopening creates a new Proposal Revision and retains the operation ID only when target and semantic identity are unchanged.
_Avoid_: Diff hunk, visual change marker

**Proposal Generation**:
One durably identified Agent production attempt for a Proposal, owning a strictly ordered and idempotent sequence of candidate batches under one active-writer boundary. A paused or completed Generation never reopens; explicit continuation creates a new Generation identity from an exact Proposal Revision.
_Avoid_: Network stream, resumable socket, mutable generator session

**Proposal Pause Fence**:
The immutable boundary that ends one Proposal Generation at an exact Proposal Revision and digest, admitted-through batch sequence, projection checkpoint, and Editor Input Fence. Later batches remain Run evidence but are permanently ineligible for Proposal or editor replay.
_Avoid_: Network cancellation, UI pause flag, queued late delta

**Editor Input Fence**:
The immutable automatic safety cause identified by `EditorInputFenceId` that binds the first completed author-input signal to one exact Editor Session, writer generation, local intent range, and active Proposal Generation before semantic command admission. It closes the Agent write gate but grants no author-command authority and receives no Author Action Sequence.
_Avoid_: Author Command Admission, browser event as domain command, reusable write permission

**Proposal State Axes**:
The orthogonal generation (`generating | ready_partial | ready`), validation (`pending | valid | invalid | conflicted`), closure (`open | withdrawn | superseded`), and per-Operation resolution (`pending | applied | rejected`) facts of a Proposal. Completion, partial application, and Acceptance Eligibility are derived projections rather than additional states, while Retention State remains separate.
_Avoid_: Proposal status, accepted Proposal state, rejected Proposal state, stale state

**Proposal Transition**:
One exhaustive StoryOS Core command-and-event change to a Proposal State Axis under exact expected Revision and lifecycle preconditions. Unlisted transitions are invalid; content correction, replanning, reopening, and Undo Acceptance append new Proposal Revisions rather than rewriting historical state.
_Avoid_: Status assignment, implicit transition, mutable historical resolution

**Proposal Anchor**:
The durable, versioned, block-relative address by which an InlineEditProposal Operation binds a stable Manuscript Block, exact base Authoritative Revision, coordinate and boundary contract, range, and canonical base-slice digest. A multi-block Operation carries ordered Anchors, while document-wide positions, DOM state, and editor decorations remain reconstructible projections.
_Avoid_: Absolute editor position, DOM range, Decoration identity, visible-text match

**Proposal Boundary Ownership**:
Author input exactly at either edge of an InlineEditProposal Operation belongs to the adjacent Authoritative State as a Direct Author Action; only input strictly inside the Operation edits the Proposal. Extending a Proposal across an edge requires an explicit Proposal edit.
_Avoid_: Inclusive Proposal edge, implicit Proposal growth, cursor affinity as authority

**Proposal Block Exclusivity**:
A stable top-level manuscript block may contain at most one unresolved InlineEditProposal Operation across all Proposals. Another Proposal targeting that block waits until the existing Operation is resolved or withdrawn, while Direct Author Actions outside its exact range remain permitted.
_Avoid_: Same-block Proposal concurrency, interval sharing, implicit Proposal merge

**Proposal Structural Reshaping**:
An author edit inside a pending Proposal Operation that splits, joins, moves, retypes, or changes the block span of its candidate content. StoryOS preserves the edit in a new Proposal Revision but projects a conflict until explicit replanning either proves unchanged semantic identity and retains the Operation ID or replaces it.
_Avoid_: Automatic anchor repair, rejected author input, silent Operation split

**Refused Edit Draft**:
A non-authoritative Draft created when an author edit is atomically refused for crossing Authoritative State and Proposal ownership. It preserves the attempted payload, exact selection snapshot, and edit intent for an explicit narrowed retry, Proposal expansion, or discard without mutating either target.
_Avoid_: Toast-only rejection, partial application, failed Direct Author Action

**Recovery Draft**:
A non-authoritative Draft preserving a complete author-edit intent from the Local Edit Journal or in-memory recovery boundary that has no committed Domain Receipt after reconciliation. It requires an explicit author retry or discard and is never automatically applied to Authoritative State or a Proposal.
_Avoid_: Autosaved truth, automatic crash replay, Refused Edit Draft

**Composition Edit**:
A complete author input intent bounded by one IME composition lifecycle and classified as a single edit only after the input method finishes while Agent document writes remain fenced. A single-owner result commits atomically, while mixed ownership restores the last durable projection and creates a Refused Edit Draft.
_Avoid_: Per-event authoritative write, cancelled IME as correctness, interleaved Agent write

**Proposal Safe Mode**:
A per-editor-session fallback used when the environment cannot uphold lossless Proposal editing, ownership recovery, or unified undo. Authoritative manuscript editing remains available while direct candidate editing and other unproven Proposal interactions are disabled without weakening authority checks.
_Avoid_: Weakened authority mode, blocked manuscript editor, silent compatibility downgrade

**Proposal Recovery Conflict**:
The fail-closed recovery condition in which durable Proposal Heads, stream sequences, Pause Fences, Anchors, digests, or an editor checkpoint cannot prove one unambiguous review projection. Agent replay and Acceptance remain disabled until explicit reconciliation; no cache or network order may fill the uncertainty.
_Avoid_: Best-effort replay, hidden repair, ordinary validation pending

**Editor Support Profile**:
The explicit product promise for manuscript editing environments and author input languages. StoryOS currently supports desktop Chrome with Chinese and English author input; behavior observed in other browsers or input languages is exploratory evidence, not a release gate or an implied support promise.
_Avoid_: Upstream browser matrix as product scope, every available IME, accidental compatibility promise

**Proposal Editing Admission**:
The fail-closed decision that permits one editor session to use full Proposal editing only when it belongs to the Editor Support Profile, its exact editor-contract versions and prior compatibility evidence match, and its live capabilities pass non-destructive checks. Unsupported, unknown, stale, mismatched, or violated evidence selects Proposal Safe Mode rather than weakening an invariant.
_Avoid_: User-Agent allowlist, feature presence as proof, optimistic compatibility

**Proposal Bundle**:
A Proposal subtype whose stable Bundle-level Operations reference exact child Proposal Revisions, selected child Operation IDs, and dependencies without copying child payloads. It declares atomic or ordered-independent execution, and Bundles cannot be nested.
_Avoid_: Mixed-domain Proposal, nested workflow

**Acceptance Eligibility**:
The predicate requiring an exact Proposal Revision to be retained, ready, valid for current targets, open, and selected only over pending Operations. Proposal identity, creator confidence, or a historical Validation Receipt cannot grant eligibility alone.
_Avoid_: Acceptable type, trusted Proposal

**Ready Partial**:
A Proposal generation outcome preserved after production stops before its intended completion. It remains editable but is not eligible for Acceptance until the author explicitly completes the current content or generation finishes.
_Avoid_: Failed Proposal, accepted partial

**Proposal Invalidity**:
The condition in which an exact Proposal Revision violates its own schema, Operation contract, or domain invariants even against its declared base. Invalidity belongs to the Proposal content rather than later target drift.
_Avoid_: Proposal Conflict, creator error message, low confidence

**Proposal Conflict**:
The condition in which an internally well-formed Proposal Revision's exact target, base Revision, Anchor, or preconditions cannot be proven against current Authoritative State. Any referenced target Revision change conflicts even outside the proposed range; recovery appends an explicitly replanned Proposal Revision rather than rebasing or revalidating the old Revision in place.
_Avoid_: Proposal Invalidity, stale warning, automatic merge, silent anchor repair

**Proposal Rejection**:
An author's non-destructive decision not to apply selected pending Proposal Operations. Reopen creates a new pending Proposal Revision, which cannot regain Acceptance Eligibility until current targets and preconditions validate successfully.
_Avoid_: Withdrawal, deletion

**Proposal Withdrawal**:
A non-destructive removal of a Proposal from active review by its current producer or the author. Withdrawal is not represented as an author rejection.
_Avoid_: Rejection, deletion

**Safe Compensation Head**:
The condition in which an Applied Acceptance is the current Author Undo Frontier, has not already been compensated, and every affected target's current Head and payload digest exactly match the resulting Authoritative Revision recorded by its Receipt while the prior evidence remains usable. Any non-exact Head requires a Reversal Proposal or an unavailable outcome rather than range-level inference.
_Avoid_: Non-overlapping guess, inverse patch on a later Head, storage rollback

**Undo Acceptance**:
An author-authorized action that appends compensating Authoritative Revisions only against a Safe Compensation Head and, when retained source content and a safe Proposal lineage allow it, a new Proposal Revision containing the previously applied content against the compensated base. Proposal lineage drift may derive a new Proposal but never blocks otherwise safe authoritative compensation; target Head drift instead requires a Reversal Proposal or unavailable outcome.
_Avoid_: History deletion, editor-only undo

**Acceptance Reapplication**:
An author redo of a successfully undone Acceptance is a new Acceptance attempt against the exact reopened Proposal Revision under current Acceptance Eligibility. It uses new command identity, Commit, and Receipt records, never restores the prior attempt, and becomes unavailable after relevant state drift.
_Avoid_: Redo Acceptance, Receipt replay, status rollback

**Author Action Sequence**:
The Project Scope-local continuous order assigned once to every successfully committed author-owned Core Transition, spanning authoritative changes, Proposal edits, resolutions, lifecycle decisions, and successful compensations. Automatic producer, validation, and input-safety transitions do not become author actions merely because they are visible or causally follow an Author Command Admission. The sequence binds the Transition's canonical Revision, Receipt, or Commit plus either a typed Forward disposition or a Compensation disposition naming the exact earlier action it settled; exact retries reuse it, while refused and no-effect attempts receive none.
_Avoid_: UUID order, wall-clock order, mutable action ledger, editor-history index

**Author Undo Frontier**:
The latest Forward Author Action Sequence that has not been named by a successful Compensation disposition. At most one committed Compensation may name a Forward action; Compensation entries remain in Author Action order for audit but are never themselves undo candidates, while a Reversal Proposal is a new Forward action and does not compensate its source.
_Avoid_: Maximum Author Action Sequence, compensation of a compensation, redo cursor

**Author Undo Order**:
A single newest-first order over uncompensated Forward author-owned actions, regardless of whether they changed Authoritative State or editable Proposal content. The Author Undo Frontier is its exact current candidate; an unsafe Frontier stops undo and requires its explicit reversal or unavailable disposition, and StoryOS never skips it to undo older work.
_Avoid_: Independent undo stacks, editor-first undo, silent history skip

**Author Undo**:
An explicit-editor-command Author Command Admission that requests reversal of the exact Author Undo Frontier through its registered typed Core handler and records one immutable routing Receipt. A successful compensation appends its own Author Action Sequence entry naming that source, but is never a later undo target. Author Undo never skips a Barrier, applies a generic inverse patch, depends on editor history as truth, or creates a durable generic redo.
_Avoid_: Editor-only undo, arbitrary history rollback, universal inverse, redo stack

**Reversal Proposal**:
A Proposal that expresses the inverse of an earlier Acceptance against current Authoritative State when a direct Undo Acceptance would conflict with later changes. It requires ordinary inspection and Acceptance.
_Avoid_: Forced rollback, silent undo

**Candidate**:
A Core Artifact presenting one independently reviewable semantic fact or object without carrying an authoritative change command. It can serve as a source for a Proposal but cannot be accepted directly; independently selectable alternatives remain separate Candidates.
_Avoid_: Proposal, pending truth

**Memory Candidate**:
A typed, source-bearing Candidate derived only from one or more Settled Source Versions for possible admission into Agent Memory. Extraction is replay-safe and preserves exact source lineage, but does not by itself validate the candidate, admit it for retrieval, or grant it authority.
_Avoid_: Memory entry, extracted truth, generic untyped summary, live-stream memory

**Memory Admission**:
The source- and policy-evaluated decision that one exact Memory Candidate is eligible for ordinary project retrieval only within its supported scope. It is separate from extraction and ranking, fails closed on stale, unsupported, conflicting, self-referential, suppressed, or forbidden evidence, and grants no authority.
_Avoid_: Retrieval score, extraction success, truth promotion, model confidence

**Admitted Memory Entry**:
The non-authoritative, source-bearing Agent Memory projection of one exact Memory Candidate under one current Memory Admission. It participates in ordinary retrieval only while its source, scope, permission, and suppression conditions remain valid; ranking may select among admitted entries but cannot admit them.
_Avoid_: Accepted truth, authoritative memory, ranked candidate, permanent memory

**Memory Lifecycle Relation**:
An immutable appended fact that changes current retrieval eligibility or links a successor without rewriting a Memory Candidate, Memory Admission, source, or prior Run context. `corrects` replaces an error at the same scope and effective time, `supersedes` succeeds a once-valid entry from an explicit boundary, and `invalidates` removes eligibility without requiring a replacement; differences explained by story branch, time, or epistemic scope are not errors.
_Avoid_: Mutable memory edit, overwrite, deletion as correction, historical context rewrite

**Memory Suppression**:
An auditable, scope-bounded author or authoritative-policy control that excludes targeted Memory Candidates, Admitted Memory Entries, source sets, or semantic ranges from extraction, admission, current projection, and ordinary retrieval without modifying their sources or prior Run context. It survives replay and rebuild, is normally reversible, and lifting it restores only eligibility for current re-evaluation rather than any historical Admission.
_Avoid_: Archive, Tombstone, negative retrieval score, inferred memory rule

**Inferred Preference**:
A source-bearing Candidate that infers one bounded author preference from prior actions or feedback. Agent Memory may retrieve it as a non-binding hint, but repetition, confidence, author silence, or prior use never turns it into a constraint or lets it override a current author instruction.
_Avoid_: Hidden policy, binding preference, implicit instruction, procedural memory

**Operational Lesson**:
A source-bearing Candidate that generalizes one bounded, reusable execution lesson from multiple comparable, settled, and causally independent Operational Records, including supporting and opposing evidence, applicability and version scope, and a re-evaluation boundary. Memory Admission may let it advise future Runs, but it cannot govern execution or alter a SkillPackage, StoryOS ToolSpec, Capability, or policy; that requires a separate governed change.
_Avoid_: Procedural memory, single-failure rule, executable instruction, promoted Skill

**Draft**:
A Core Artifact containing editable work that has not been expressed as validated domain changes. It can serve as a source for a Proposal but cannot be accepted directly.
_Avoid_: Proposal, authoritative draft

**Message**:
A Core Artifact representing one visible contribution to a project transcript. It references exact Artifact Revisions for embedded results and views rather than copying their payloads or resolving mutable latest versions.
_Avoid_: Run Event, hidden reasoning

**Research Artifact**:
A Core Artifact that captures or synthesizes source-backed research for later inspection and use. It can support a Proposal but cannot directly change Authoritative State.
_Avoid_: Canon, unsourced note

**Analysis Report**:
A Core Artifact containing a derived evaluation or interpretation of project state, Artifacts, or evidence. It remains advisory even when produced by a trusted Skill.
_Avoid_: Decision, authoritative assessment

**Eval Case**:
An explicitly selected, Project Scope-bound advisory evaluation sample that references exact settled StoryOS evidence for later assessment. Ordinary writing and AgentRuns never create one automatically, and an Eval Case grants no authority, authorization, or routine author configuration burden.
_Avoid_: Automatic Run capture, benchmark row, authoritative score, required writing setup

**Evaluation Corpus**:
An optional, Project Scope-bound collection of explicitly selected Eval Cases for comparative or experimental assessment. It is not a hidden copy of ordinary Runs or a prerequisite for using the writing Agent.
_Avoid_: Default Run archive, automatic dataset, author setup requirement

**Eval Evidence View**:
A read-only, Project Scope-bound, redacted projection of exact settled evidence for one Eval Case. Its requester-specific Query/read authorization and visibility/redaction profile govern presentation only; opening or refreshing it is not a Processing Destination, Outbound Disclosure, or model-use proof. It supports observation and explanation without becoming a new truth store or changing normal writing behavior.
_Avoid_: Eval truth store, prompt dump, runtime control panel, model-use proof, Outbound Disclosure

**Eval Surface**:
A standalone, author-facing advisory product surface for observing Project Scope-bound Eval evidence and assessments. It is neither the main writing interface nor a Transcript MCP App, backend monitoring service, hidden telemetry channel, or control plane.
_Avoid_: Admin dashboard, ambient monitoring, authoring surface, MCP App authority

**Eval Evidence Availability**:
An author-visible, view-specific explanation of whether referenced evidence can be inspected and why it is redacted, unavailable, expired, or otherwise limited. It preserves the limitation without disclosing protected content, silently omitting it, or substituting unproven material.
_Avoid_: Hidden gap, raw-data reveal, fabricated completeness, silent substitute

**Eval Author Feedback**:
An author-originated, case-scoped advisory Analysis Report Artifact Revision that qualifies one Eval Case or its assessment. It is not a Message or Transcript contribution, and never automatically becomes an Author Preference, instruction, routing rule, score baseline, or Authoritative State.
_Avoid_: Transcript Message, implicit preference update, chat instruction, global quality rule, acceptance

**Eval Reproducibility Status**:
A Project Scope-bound declaration of whether the exact settled evidence for an Eval Case can be reopened and inspected. It never promises that an external Provider, judge, cache, or opaque mechanism can be rerun to produce the same output; every later assessment is a distinct advisory observation.
_Avoid_: Deterministic model rerun, Provider replay guarantee, overwritten result, hidden missing evidence

**Eval Baseline**:
An explicitly selected, Project Scope-bound reference to exact Eval Cases or Results and their declared metric definition, used only for an advisory comparison. No project-wide score, default baseline, or hidden creative target is inferred from ordinary evaluation observations.
_Avoid_: Automatic benchmark, global quality score, Agent instruction, author goal

**Eval Assessment Attempt**:
An explicitly initiated advisory evaluation operation. Opening an Eval Evidence View causes no outbound call; an external judge or Provider assessment uses the ordinary current-run authorization, minimum disclosure, Attempt, and Receipt boundaries.
_Avoid_: Page-load invocation, invisible upload, unreceipted judge call, ambient authorization

**Tool Artifact**:
A Core Artifact envelope for durable Tool or Service output that has no more specific Core Artifact kind, including permitted namespaced extension schemas. A domain-recognized result uses its specific kind with the ToolCall as Creator rather than adding a duplicate Tool Artifact wrapper.
_Avoid_: Tool result event, direct write

**App UI Resource**:
The stable logical identity of executable MCP App UI content, scoped to one exact MCP server or connector registration identity and one canonical `ui://` URI. Each immutable Revision binds the exact HTML or blob bytes, MIME type, normalized security metadata, and Content Digest; changed bytes create a new Revision, a changed server trust identity creates a new Resource, and equal digests may deduplicate storage without collapsing logical identity.
_Avoid_: URI alone, Content Digest as identity, mutable server response, shared cross-server resource

**App UI Resource Retention**:
The Host-owned availability of an exact App UI Resource Revision's inert bytes and metadata, independent of whether they remain executable. Referenced bytes survive server removal and security revocation for replay evidence, while explicit author-governed deletion may purge them only with a retained digest, provenance, deletion record, and stage-appropriate Prepared Receipt or Terminal Static Fallback; any View that loses its bytes becomes permanently recovery-only.
_Avoid_: Execution Eligibility, live server availability, automatic cache eviction, security revocation

**App UI Resource Execution Eligibility**:
The current versioned Host-owned determination of whether one exact App UI Resource Revision may cross an executable rendering boundary under current trust, security, and project policy. Retention is independent of eligibility, every eligibility change advances a fencing generation, and Loader validation may reject work early but never authorizes later execution.
_Avoid_: Resource retention, Loader success, cached allow flag, Capability Grant

**App UI Execution Admission**:
The one-shot Operational Record created at the immediate sandbox or renderer entry for one exact App View Instance, atomically matching the Resource Revision and digest, current Execution Eligibility generation, policy revision, and enforced sandbox profile before any HTML, object URL, decoded frame, or other executable derivative crosses that boundary. A stale or denied generation fails closed, while revocation after admission forces the active Instance to Terminal and revokes its executable handles.
_Avoid_: Loader validation, long-lived execution token, App View Capability Snapshot, best-effort check

**App UI Derived Data**:
A disposable non-authoritative cache product such as a thumbnail, decoded frame, compiled representation, or object URL, keyed by the exact App UI Resource Revision, Execution Eligibility generation, derivative kind, and transformer version. Every executable or rendering consumer revalidates that generation, and revocation invalidates cached entries and active handles immediately; WeakRef may assist lifetime management but is never the security or correctness guarantee.
_Avoid_: App UI Resource Revision, unversioned cache, WeakRef as revocation, durable truth

**App View Artifact**:
A Core Artifact that preserves a transcript-embedded MCP App as a reproducible View Descriptor: one exact App UI Resource Revision, protocol version and App View Capability Snapshot, exact input revisions, authorized host-context snapshot, optional schema-bound view state, and a mandatory static fallback on every Terminal Revision. It never stores a live iframe runtime or controls authoritative domain data.
_Avoid_: App-owned state, iframe snapshot

**App View Capability Snapshot**:
The immutable Host capability ceiling bound to one exact App View Artifact Revision, covering the supported protocol features and bridge methods plus resource-requested and maximum permitted sandbox permissions. Each Instance records its actual effective subset in App View Instance Negotiation; the Snapshot is compatibility and audit input rather than a Capability Grant, every App request receives live validation against current policy and a new operation scope, and no authority is inherited from the originating Run.
_Avoid_: Capability Grant, reusable permission, inherited Run authority, SDK feature list

**App View Artifact Stage**:
The content stage recorded by each App View Artifact Revision. A Prepared Revision fixes the exact UI, complete input, protocol, capability and Host Context snapshots and requires an App View Prepared Receipt before Instance creation; after the originating ToolCall reaches a durable terminal outcome, a new Terminal Revision of the same Artifact adds its exact result, cancellation, or error projection and App Static Fallback. Live terminal delivery to an Instance pinned to a Prepared Revision is an operational overlay rather than mutation or rebinding, while an explicit Message replacement exposes the Terminal Revision and future replay binds it directly.
_Avoid_: Mutable View Artifact, latest pointer, Instance rebinding, ToolCall replay

**App View Prepared Receipt**:
A minimal immutable Operational Record persisted atomically with one Prepared App View Artifact Revision before Instance creation, binding its exact Resource Revision, originating ToolCall, creation time, exact input reference and digest, and safe pending status. It is recovery and audit truth that a trusted Host renderer may present as a one-line pending or interrupted card; it requires no App or schema renderer, copies no sensitive input summary, and is not rewritten for live progress updates.
_Avoid_: App Static Fallback, free-text log as truth, rich progress card, per-update persistence

**App Static Fallback**:
The mandatory immutable safe representation persisted and validated before a Terminal App View Artifact Revision may be exposed through a Message. It contains terminal status, safe text, exact result, cancellation, or error references, and provenance, with an optional schema-valid typed presentation; StoryOS alone renders it without App HTML, scripts, or external URLs, and an unavailable typed renderer degrades to a bounded generic result, error, or empty-result card rather than leaving a blank transcript slot.
_Avoid_: App View Prepared Receipt, App HTML, on-demand external rendering, model context

**App View Instance**:
A disposable sandboxed rendering of one exact App View Artifact Revision with its own Host-bound identity and bridge. Reloads, reconnects, and concurrent windows create distinct Instances whose lifecycle evidence is recorded as Operational Records; an Instance never revises its Artifact, restores prior runtime state, or re-executes the originating ToolCall.
_Avoid_: App View Artifact, reused iframe session, restored DOM, App runtime as truth

**App View Instance Lifecycle**:
The irreversible progression of one App View Instance from Created to Initializing to Ready to Terminal, with failure permitted directly to Terminal before readiness. Created binds the exact App View Artifact Revision before runtime creation, Initializing rejects View requests until protocol initialization completes, Ready permits ordered replay and mediated requests, and resource-eligibility revocation forces Terminal, which never reopens; teardown progress, clean or unclean shutdown, and terminal reason remain orthogonal facts.
_Avoid_: UI status label, Closing state, reconnect state, App View Artifact lifecycle

**App View Instance Terminal Reason**:
The exhaustive reason recorded when an Instance becomes Terminal: AuthorClosed, Replaced, InitializationFailed, ProtocolViolation, EligibilityRevoked, ResourceLimitExceeded, BridgeLost, HostShutdown, or HostRecovery. Adding a reason is a versioned contract change, and the reason neither implies nor replaces the orthogonal clean or unclean shutdown fact.
_Avoid_: Free-text error, Closing state, ToolCall outcome, clean-shutdown flag

**App View Instance Negotiation**:
The immutable Operational Record of one Instance's actual App and Host protocol exchange, declared and advertised methods, effective sandbox policy, and granted optional capabilities. Replay may produce a safe subset of the Artifact's historical App View Capability Snapshot, but never a superset; the result belongs to the Instance and does not revise its Artifact.
_Avoid_: App View Capability Snapshot, Capability Grant, mutable handshake state, authority

**App Replay Compatibility Decision**:
The persisted Host decision governing whether a new Instance of one exact App View Artifact Revision may become Ready or must yield to its stage-appropriate trusted recovery presentation. Interactive replay requires the stored App UI Resource Revision to pass integrity checks, the recorded protocol to remain supported, current policy to enforce an equal or stricter sandbox, Instance Negotiation to succeed, and a fresh App UI Execution Admission at the rendering boundary; optional capabilities may be removed and recorded in that Negotiation, while any capability expansion, required-capability loss, unsafe resource, stale eligibility generation, or initialization incompatibility fails closed to the Prepared Receipt or Terminal Static Fallback without refetching the resource or re-executing the originating ToolCall.
_Avoid_: Best-effort replay, exact permission equality, resource refetch, ToolCall retry

**App View Delivery**:
A durable per-Instance obligation to dispatch either one exact complete Tool input or one terminal result, cancellation, or error projection from StoryOS-owned records. It has a stable identity and a Pending, Dispatched, or Abandoned disposition without claiming receiver acknowledgement; terminal delivery is ineligible until that Instance's complete-input delivery is Dispatched, while an unclean terminal Instance abandons its remaining deliveries and a replacement Instance creates a fresh ordered pair without re-executing the ToolCall.
_Avoid_: In-memory result buffer, receiver acknowledgement, ToolCall replay, cross-Instance delivery reuse

**App Presentation Signal**:
A bounded, rate-limited, non-authoritative bridge message used only for ephemeral View presentation or lifecycle coordination, such as size, display mode, initialization, or teardown. Ordinary signals are not durable domain history; malformed, abusive, or security-relevant signals produce a durable diagnostic without turning presentation state into an Artifact or App Action Request.
_Avoid_: App Action Request, transcript event, durable layout state, authorization

**App Action Request**:
A durable Operational Record created after bridge validation and before handling any App request with semantic meaning, data access, outbound disclosure, model-context impact, transcript impact, or other effect. Its stable identity is scoped to the exact App View Instance and bridge request id, and it binds the Artifact Revision, method, controlled payload reference or digest, provenance, and current operation scope before recording an accepted, denied, failed, or completed disposition and result reference; an exact duplicate resolves to the same Request, a reused id with a different payload is a protocol violation, and another Instance always creates another Request. It grants no authority, and neither the bridge callback nor a raw MCP client may execute or forward the effect directly.
_Avoid_: App Presentation Signal, Capability Grant, direct bridge effect, independent App runtime

**App Action Routing Decision**:
The immutable Host decision that maps one persisted App Action Request to denial, a typed Host command, a normal transcript or instance-scoped context contribution, or a new causally linked root AgentRun with its own Grant, budget, and Approval boundary. It is an ingress and provenance boundary rather than an executor: it never reuses the originating Run, and accepted work continues under its routed record's own lifecycle after the requesting Instance becomes Terminal; Tool, MCP, external-effect, or creative-state work proceeds only through the existing Tool Gateway and Proposal plus Acceptance rules.
_Avoid_: App workflow runtime, originating Run continuation, direct ToolCall, App authority

**App Action Response Delivery**:
A durable per-Instance obligation to dispatch the settled response of one App Action Request to the same bridge that requested it. It is Pending, Dispatched, or Abandoned without claiming receiver acknowledgement; Instance termination abandons an undelivered response and never cancels or retries the routed work, restores a JSON-RPC promise, or transfers the response to a replacement Instance, while durable results remain visible only through their normal StoryOS Artifact and Message paths.
_Avoid_: App Action result, cross-Instance callback, execution cancellation, receiver acknowledgement

**Derivation**:
The creation of a new Artifact from exact source Artifact Revisions while preserving those sources and their lineage. Derivation never changes a source Artifact's kind in place.
_Avoid_: Conversion, type mutation

**Acceptance**:
An explicit-editor-command Author Command Admission that applies a nonempty selection of pending Operations from an exact eligible Proposal Revision and current Validation Receipt through a StoryOS-owned domain handler. Domain Proposal selections are atomic, while Proposal Bundles obey their explicit atomic or ordered-independent policy.
_Avoid_: Promotion, status flip, overwrite

**Acceptance Attempt**:
The first Core execution of one schema-valid, authorized Acceptance command after idempotency resolution, with all current eligibility, targets, Anchors, preconditions, and domain effects revalidated before commit. Every Attempt produces one immutable Acceptance Receipt, while an exact retry only returns that Receipt and is not another Attempt.
_Avoid_: Acceptance retry, button click, Receipt lookup

**Acceptance Result**:
The exhaustive settlement of one Acceptance Attempt as Applied, Invalid, Conflicted, Refused, or NoEffect. Only Applied changes Authoritative State and resolves selected Operations as applied; infrastructure uncertainty is not a Result and is reconciled through the command's idempotency key.
_Avoid_: Success boolean, exception text, accepted Proposal state, unknown as failure

**Acceptance Receipt**:
The durable outcome of one Acceptance attempt, identifying its command digest and idempotency key, exact Proposal Revision and selected operations, prior and resulting Authoritative Revisions, zero or more Authoritative Commits, child Receipts, and result. Bundle progress may be derived across linked attempt Receipts.
_Avoid_: Success message, accepted Artifact

**Domain Receipt**:
An immutable StoryOS Core record of a validation or domain-command attempt, including success, refusal, redirection, and outcomes with no Authoritative State change. A Domain Receipt is neither an Artifact nor Authoritative State and has no revision, derivation, retention, or Acceptance lifecycle.
_Avoid_: Artifact, log text

**Undo Acceptance Receipt**:
The immutable, idempotent Domain Receipt produced by an Undo Acceptance attempt, identifying the original Acceptance Receipt, command digest, and one outcome: compensated with a Commit, reversal required with a Reversal Proposal, or unavailable with a reason.
_Avoid_: Compensation Receipt, undo message

**Validation Receipt**:
The immutable result of StoryOS Core validating an exact Proposal Revision against exact target versions, domain invariants, and preconditions. It remains true for those historical inputs but ceases to be current or applicable after any relevant Proposal or target change.
_Avoid_: Model confidence, creator-approved flag
