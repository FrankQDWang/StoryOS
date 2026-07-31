# AI-Independent Editor-First Release Baseline and Handoff Criteria

- Status: current
- Contract revision: `release-baseline-editor-first-2026-07-31-issue62`
- Canonical issue: [Define the AI-Independent Editor-First Release Baseline and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62)
- Exact planning baseline: `main@6c6e99cfcc31fc3cdff592e8a472cffe8b4a4858`
- Exact planning tree: `ea5efeb0e2d6ac82d0aa0309a59bee38ada8ac39`
- Product goal: [GOAL.md](../../GOAL.md)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Issue execution rules: [Issue-Tracker Execution Contract](../agents/issue-tracker.md)

This document owns the Release 1 editor-first delivery baseline, the author
journeys that make each stage meaningful, the explicit exclusions that keep
the stages bounded, and the handoff criteria between planning, implementation,
verification, release, and later deployment. It does not become a second
domain owner for any semantic contract named below.

## 1. Release promise and product boundary

The first author-usable StoryOS release is a high-quality novel editor that
remains useful for daily writing when every model, Agent, Tool, MCP server,
research service, embedding service, and network-dependent AI capability is
unavailable. An author can start from a new or controlled Project
initialization, organize a novel, write and revise manually, understand save
state, recover safely, navigate and search, inspect basic progress, use the
supported Chinese and English input methods, and export readable prose.

AI is adjacent assistance, not the product's authority or editor replacement.
The author owns every authoritative creative state. Direct deterministic
author manipulation uses the existing author-command and Core paths. An
Agent-, Tool-, MCP-, extension-, bulk-, cross-location-, or otherwise
not-fully-previsible prose change remains an editable Core Proposal and
requires the author's explicit Acceptance before it changes Authoritative
State. Rejection is a complete non-destructive author decision and leaves
Authoritative State unchanged.

StoryOS has one general, Project-scoped Agent Loop. Stage-specific behavior
comes from the existing Host, Tools, MCP servers, Services, Skills, Model
Gateway, policy, and domain contracts; this document does not introduce a
second workflow runtime, a task menu that replaces the loop, an Agent-authored
outline, or an automatic authority path.

The initial delivery and validation scope is local. A later controlled-cloud
deployment must use the same User, Project Scope, authority, isolation,
recovery, and disclosure contracts, but that deployment is a validation gate
over the resulting product rather than a fifth implementation stage.

### 1.1 Release invariants

The following stable identifiers are owned by this document and remain valid
across later implementation revisions:

| ID | Release requirement |
| --- | --- |
| REL-001 | With all AI capabilities disabled, the complete AI-independent editor remains a high-quality, daily-usable novel editor. |
| REL-002 | Authoritative creative state is changed only through existing direct-author or accepted Core Proposal paths; browser, Agent, Tool, MCP, model, cache, projection, and transcript state are not authority. |
| REL-003 | Every project-bearing record and operation binds the exact User and Project Scope, and the production path uses StoryOS Core and PostgreSQL rather than an in-memory or prototype authority. |
| REL-004 | Delivery occurs in exactly this order: production-shaped manual-editor risk slice, complete AI-independent editor, contract-faithful fake-model Proposal loop, one real external model with disclosure evidence. |
| REL-005 | A stage is complete only when its author journey and mandatory evidence are current, attributable, replayable where applicable, and passed; failed, unrun, stale, unavailable, or unreplayable mandatory evidence blocks the stage. |
| REL-006 | Planning closure, implementation handoff, stage implementation evidence, stage release, and controlled-cloud deployment are separate gates; no implementation issue or product implementation is created by this document. |

### 1.2 Existing semantic owners

The release baseline consumes the following owners. The owner column is
normative: this document selects when a capability is required, while the
linked contract continues to define what that capability means.

| Concern | Existing sole owner consumed by this baseline |
| --- | --- |
| Domain vocabulary, Artifact classification, and author-owned authority | [Artifact and Authoritative-State Domain Model](artifact-domain-model.md) |
| Author Command Admission and its settlement | [Author Command Admission](author-command-admission.md) |
| Core manuscript revisions, Proposals, Receipts, Acceptance, Rejection, conflict, and undo | [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md) |
| Editor Session, Local Edit Journal, pending projection, synchronization, writer takeover, and browser recovery | [Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md) |
| Public routes, DTOs, Events, acknowledgement, Snapshot, cursor, and same-release compatibility | [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md) |
| PostgreSQL authority, Project Scope integrity, forced RLS, migrations, backup, restore, and portability | [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md) |
| Retention Profiles and Decisions, compaction, archive, replay generations, deletion, and recovery visibility | [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md) |
| Context Assembly, retrieval eligibility, projection, manifests, disclosure, and destination attempts | [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md) |
| Author-facing discovery-writing intent and Proposal request interpretation | [Plain-Language Discovery-Writing Assistance Semantics](plain-language-discovery-writing-assistance-semantics.md) |
| AgentRun, Tool, MCP, Skill, Model Gateway, and external-destination meaning | The existing AgentRun, Tool/MCP, and Model Gateway contracts named by [the current product map](https://github.com/FrankQDWang/StoryOS/issues/1) |
| Threat boundaries, Host/Origin, credential references, and disclosure safety | [StoryOS Service, Client, and External Trust Boundaries](storyos-service-client-external-trust-boundaries-threat-model.md) |
| Deterministic proof selection, executable tests, fault schedules, oracles, and evidence bundles | [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md) |
| Measured latency, scale, and storage-growth observations | [Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76) and the accepting semantic owner |

The release baseline never changes a route, DTO, Event, storage family,
retention value, recovery rule, AgentRun meaning, or verification mechanism.
The current protocol and persistence catalogs remain the unique machine-readable
ledgers for their respective owners.

## 2. Fixed delivery order

Each stage begins only from the resulting current main of the preceding stage.
The stage names below are contract identifiers, not separate workflow runtimes:

1. **Production-shaped manual-editor risk slice.** Prove the smallest
   production boundary for author input, durability, settlement, and recovery
   on controlled Project data.
2. **Complete AI-independent editor.** Deliver the full standalone editor and
   its complete AI-disabled author journey.
3. **Contract-faithful fake-model Proposal loop.** Add the adjacent general
   Agent path with deterministic fake destination behavior while preserving the
   editor and Proposal/Acceptance boundary.
4. **One real external model with disclosure evidence.** Add one separately
   admitted real external-model path and prove StoryOS-owned disclosure and
   uncertainty evidence without claiming opaque Provider internals.

The terminal planning handoff issue, [Create and Lock the First Editor-First
Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77), creates
and locks the first implementation issue only after the preceding contract and
proof work is complete. This document creates no implementation issue and no
product Rust, TypeScript, SQL, UI, runtime, or deployment change.

Controlled-cloud deployment is a later gate that checks deployment identity,
operational recovery, security, cache refresh, same-release activation, and
upgrade evidence for the author's controlled domain. It is not an additional
stage in the four-stage implementation order and does not pull cloud
configuration, collaboration, billing, or service-fleet work into local
Release 1.

## 3. Complete acceptance journey

The complete AI-independent acceptance surface is S2-JRN-001 below. It starts
from new or controlled Project initialization, covers the full author-facing
editor journey with AI disabled, and ends only after recovery, navigation,
search/replace, statistics, long-session writing, and human-readable export
have passed. Stage 1 proves only the production-shaped risk boundary needed to
reach that surface; Stages 3 and 4 add adjacent assistance without replacing
this journey. This section is the stable release-owner anchor consumed by the
protocol and persistence catalogs.

## 4. Stage 1 — production-shaped manual-editor risk slice

Stage 1 is intentionally the smallest production-shaped manual-editor slice.
It is a risk slice for the real authority and recovery boundary, not a
prototype, a complete editor, or a disguised product launch.

### 3.1 Required capabilities and owners

| ID | Required capability | Normative owner |
| --- | --- | --- |
| S1-REQ-001 | Open one controlled Project and a bounded current chapter through the protected StoryOS Web Client and real Server/Core/PostgreSQL boundary. A controlled seed or initialization helper may limit setup, but it cannot replace production authority. | PostgreSQL, Protocol, Web Editor Session, and trust-boundary owners |
| S1-REQ-002 | Capture supported manual author input, including typing, paste, cut, delete, selection replacement, and the supported Chinese/English IME composition boundary, into the real Editor Session and Project Scope-bound Local Edit Journal. | Web Editor Session and Author Command Admission owners |
| S1-REQ-003 | Submit one bounded direct edit through the real Author Command Admission and Core transition, producing the existing authoritative revision, Receipt, Project Activity observation, and visible pending-to-settled save state. | Author Command Admission, Core, Protocol, and PostgreSQL owners |
| S1-REQ-004 | Preserve saved and unsettled author work across a bounded reload or process/restart recovery exercise, including the existing recovery-draft or reconfirmation meaning where the owning contracts require it. | Web Editor Session, Core, Protocol, and retention owners |
| S1-REQ-005 | Enforce exact User and Project Scope, protected Host/Origin/session identity, input bounds, forced-RLS authority, and no credential or cross-Scope leakage on the exercised path. | Trust-boundary, PostgreSQL, Protocol, and Admission owners |
| S1-REQ-006 | Produce attributable evidence that the exercised browser, Server, Core, and PostgreSQL path is the production-shaped path and that no disposable substitute is being treated as authority. | Repository governance and all exercised semantic owners |

### 3.2 Explicitly absent or prohibited

Stage 1 does not claim or require:

- the full Project initialization and management experience;
- complete volume/chapter organization, broad navigation, full-manuscript
  search/replace, statistics, long-session acceptance, or human-readable
  export;
- an Agent, model, Tool, MCP server, research service, embedding service,
  Memory, Skill, Subrun, Eval, or external destination;
- automatic Proposal creation, Agent-authored outline, bulk authoring, or a
  second workflow runtime;
- in-memory authority, browser-session authority, a test Adapter, a mock
  database, a disposable prototype, a local-file convention, or
  .reference/** content in place of Core/PostgreSQL; or
- controlled-cloud deployment or any cloud-only operational promise.

The exclusions are release boundaries, not permissions to weaken the
production-shaped path. A missing Stage 1 capability cannot be hidden by
calling a prototype or test substitute a successful implementation.

### 3.3 Entry, author journey, and completion

**Entry condition.** The exact planning baseline is current; the preceding
design contracts and deterministic-proof handoff are closed as required by the
current Wayfinder chain; and the terminal handoff issue has created and locked
one bounded Stage 1 implementation issue. The implementation starts from that
issue's exact baseline and applicable repository instructions.

**Author journey S1-JRN-001.**

1. Use the protected local Web Client to open a controlled Project and a known
   current chapter.
2. Type and paste a short Chinese and English passage, exercise the supported
   IME composition and ordinary keyboard/clipboard editing, and observe the
   immediate pending projection.
3. Submit the bounded direct edit through the normal path and observe
   author-visible saving followed by durable saved settlement.
4. Interrupt and restart or reload at a contract-owned recovery boundary with
   both settled and unsettled input present.
5. Recover the unsettled input from the existing Editor Session/Local Edit
   Journal path, reconcile the authoritative result, and verify that the
   journal is collected only after convergence or explicit recovery disposition.
6. Inspect the durable Core/PostgreSQL result and verify one author-owned
   effect, one typed Receipt, one Project Activity observation, and no
   duplicate or cross-Scope effect.

**Completion condition.** The journey passes on the real production-shaped
path, every S1 evidence obligation is current and replayable where applicable,
and all required negative boundaries fail closed. Passing Stage 1 does not
authorize any Stage 2 capability or AI work.

### 3.4 Mandatory evidence

| ID | Mandatory evidence category |
| --- | --- |
| S1-EVD-001 | Contract and baseline crosswalk proving the exercised path's semantic owner, exact revision, Project Scope, and production source. |
| S1-EVD-002 | Browser integration evidence for input, IME composition, Local Edit Journal durability, pending projection, and save-state convergence. |
| S1-EVD-003 | Core and persistence evidence for Admission, atomic settlement, Receipt, authoritative revision, Project Activity, and idempotent acknowledgement meaning. |
| S1-EVD-004 | Recovery/replay evidence for reload or restart with settled and unsettled work, including the owning recovery-draft, reconfirmation, fence, or resync result. |
| S1-EVD-005 | Isolation and trust evidence for Host/Origin/session, exact Project Scope, forced RLS, bounds, secret exclusion, and non-oracular refusal. |
| S1-EVD-006 | Reviewable provenance proving no prototype, in-memory authority, test Adapter, local-file authority, or .reference/** substitute entered the production claim. |

Every failed, unrun, stale, unavailable, non-replayable, or otherwise
unverified S1 obligation blocks Stage 1 completion and release. An advisory
observation cannot satisfy a mandatory obligation.

## 5. Stage 2 — complete AI-independent editor

Stage 2 is the complete standalone editor promised by REL-001. It is strictly
larger than Stage 1: it must be useful for ordinary novel work from
initialization through export, not merely prove one write path.

### 4.1 Required capabilities and owners

| ID | Required capability | Normative owner |
| --- | --- | --- |
| S2-REQ-001 | Work from a new or controlled Project initialization with AI, Agent, model, Provider, Tool, MCP, research, embedding, and network-dependent AI capabilities disabled. Manual writing must not require AI configuration. | Project/identity, PostgreSQL, Protocol, and Web Editor Session owners |
| S2-REQ-002 | Create, open, rename, and archive the Project through the existing typed author-command and persistence paths, with exact User and Project Scope binding. | Project domain, Admission, Core, Protocol, and PostgreSQL owners |
| S2-REQ-003 | Create, rename, reorder, remove, and navigate volumes and chapters without losing pending editor work; reopen the Project and display the current chapter and save state from an authorized Snapshot. | Web Editor Session, Protocol, Core, and PostgreSQL owners |
| S2-REQ-004 | Directly write and revise supported manuscript blocks with typing, paste, cut, selection replacement, split, join, move, retype, keyboard navigation, clipboard, undo, and Chinese and English IME behavior. | Web Editor Session, Admission, Core, Artifact, and Protocol owners |
| S2-REQ-005 | Show durable saving, saved, and needs-attention states; preserve input through network delay, acknowledgement/Event ordering, reload, Web Client crash, Server restart, Project Activity replay-floor resynchronization, writer takeover, and recovery-draft/reconfirmation paths. | Web Editor Session, Protocol, Core, PostgreSQL, and retention owners |
| S2-REQ-006 | Search the current chapter and full manuscript with bounded results; perform direct replacement of one visible match; keep selected multi-match or cross-location replacement on the existing Proposal-gated path rather than silently bulk-writing. | Protocol, Web Editor Session, Core/Proposal, and search projection owners |
| S2-REQ-007 | Show basic word, character, chapter, and manuscript progress statistics and export a deterministic, human-readable manuscript in volume/chapter order with explicit representation of unavailable content. Produce the versioned Project Export Archive only through its existing owner contracts. | Protocol, Web Editor Session, PostgreSQL, retention, and Artifact owners |
| S2-REQ-008 | Sustain a long writing session with responsive local input, repeated chapter switches, reload, controlled upgrade, and the measured cold-open, search, journal, Snapshot, replay, storage, and restore envelope adopted by the relevant owner. | Web Editor Session, PostgreSQL, retention, protocol, and measurement owners |

### 4.2 Explicitly absent or prohibited

Stage 2 requires no Agent, model, Provider, Tool, MCP server, Skill,
research/embedding service, Memory, Subrun, Eval, outbound disclosure, or
external network destination for the manual author journey. It also does not
add:

- an Agent-authored outline, mandatory character sheet, professional workflow,
  hidden project plan, or fixed story plan;
- automatic authority from a chat response, model output, MCP App, extension,
  cache, index, transcript, or browser state;
- collaboration, teams, ownership transfer, billing, account management, or
  multi-author editing; or
- controlled-cloud deployment as an implementation stage.

The existing Core Proposal boundary still applies when a manual command is
bulk, cross-location, or otherwise not fully previsible. AI being disabled
does not authorize a second direct-write path.

### 4.3 Entry, author journey, and completion

**Entry condition.** Stage 1 has passed its author journey and mandatory
evidence on one exact main. A later implementation issue may be created only
from that resulting main through the serial Wayfinder process; this document
does not create it.

**Author journey S2-JRN-001.**

1. Bootstrap the local User, create a new Project or perform the allowed
   controlled initialization, and confirm that no AI capability is needed.
2. Create a volume and three chapters, rename and reorder them, close and
   reopen the Project, and navigate the manuscript tree without losing work.
3. Write Chinese and English prose using complete IME composition, keyboard
   navigation, clipboard operations, block operations, and author-visible undo.
4. Observe immediate pending projection and the durable saving, saved, and
   needs-attention states while the service is delayed and acknowledgements or
   activity observations converge.
5. Reload or crash/restart the Web Client with both saved and unsettled edits,
   recover the unsettled text, then restart the Server and PostgreSQL and
   verify the existing recovery and visibility rules before ordinary reading.
6. Open a second tab, observe the writer disposition, perform explicit
   takeover, and preserve the prior tab's unsettled text through the existing
   recovery path.
7. Search the current chapter and full manuscript, replace one visible match
   directly, and verify that any selected multi-match or cross-location
   operation remains inspectable and Proposal-gated.
8. Navigate among chapters, inspect word, character, chapter, and manuscript
   statistics, and continue writing after repeated chapter switches.
9. Exercise the replay-floor boundary through the existing typed Snapshot/resync
   path; an old cursor must not be silently translated into a new generation.
10. Export a human-readable manuscript in deterministic order and create or
    inspect the versioned Project Export Archive through its existing
    lifecycle and portability contract.
11. Continue a long session, reload once more, and verify that no acknowledged
    author work is lost, duplicated, or made authoritative by a disposable
    projection.

**Completion condition.** Every step in S2-JRN-001 passes with AI fully
disabled; all S2 evidence obligations are current, attributable, and
replayable where applicable; and no excluded AI or collaboration capability is
required. Only this condition establishes the complete AI-independent editor
promise.

### 4.4 Mandatory evidence

| ID | Mandatory evidence category |
| --- | --- |
| S2-EVD-001 | Contract-to-journey crosswalk covering every REL and S2 requirement, its sole semantic owner, exact baseline, and current protocol/storage/retention inputs. |
| S2-EVD-002 | Initialization, Project, volume, chapter, navigation, reopen, and authorized Snapshot evidence. |
| S2-EVD-003 | Browser input evidence for IME, keyboard, clipboard, undo, block operations, local continuity, and responsive long-session editing. |
| S2-EVD-004 | Durable save/settlement evidence for pending, saved, needs-attention, Admission, Core, Receipt, Event, and idempotent convergence states. |
| S2-EVD-005 | Reload, crash, restart, writer-takeover, replay-floor, Snapshot/resync, recovery-draft, and Recovery Visibility Proof evidence, including lifecycle gaps where applicable. |
| S2-EVD-006 | Search/replace, statistics, navigation, and explicit Proposal-gated multi-location operation evidence. |
| S2-EVD-007 | Human-readable export and Project Export Archive evidence, including deterministic order, unavailable-content representation, scope, provenance, and lifecycle treatment. |
| S2-EVD-008 | Long-session and storage-growth evidence using only measured values adopted by their named owner; no measurement is silently promoted to a new retention default or SLA. |

Any failed, unrun, stale, unavailable, non-replayable, or unverified S2
obligation blocks the complete AI-independent release. A working editor with
an omitted recovery, IME, export, or long-session obligation is incomplete.

## 6. Stage 3 — contract-faithful fake-model Proposal loop

Stage 3 adds adjacent assistance after the editor is independently complete.
The fake destination is a deterministic implementation of the real model-path
contracts, not a shortcut around them.

### 5.1 Required capabilities and owners

| ID | Required capability | Normative owner |
| --- | --- | --- |
| S3-REQ-001 | Keep the complete Stage 2 editor usable while one general, Project-scoped Agent Loop appears adjacent to the current passage and editor. | Product map, AgentRun, and Web Editor Session owners |
| S3-REQ-002 | Interpret a bounded current author request using the existing discovery-writing intent contract; discussion, explanation, brainstorming, and prose-change scope remain distinct. | Plain-Language Discovery-Writing Assistance owner |
| S3-REQ-003 | Route the fake-model operation through the real Host, Project Scope, Context Assembly, selection/projection, manifest-before-egress, destination Attempt, fence, recovery, and durable AgentRun path. | Context/Disclosure, AgentRun, Model Gateway, Protocol, PostgreSQL, and trust owners |
| S3-REQ-004 | Present generated prose as an editable, anchored Core Proposal in the editor, with validation, refusal, conflict, recovery, explicit Acceptance, and explicit Rejection on the existing owner paths. | Core/Proposal, Artifact, Web Editor Session, Admission, and Protocol owners |
| S3-REQ-005 | Keep chat adjacent to and non-authoritative over the editor; a fake result, transcript message, MCP App, or Agent action cannot write authoritative prose or an outline. | Artifact, Core/Proposal, MCP App, and product-map owners |
| S3-REQ-006 | Record a bounded, inspectable fake-model result and uncertainty/recovery evidence without claiming model understanding, literary quality, Provider behavior, or external retention. | Model Gateway, Context/Disclosure, deterministic verification, and retention owners |
| S3-REQ-007 | Preserve the Stage 2 AI-independent journey as a release requirement even when the fake path is unavailable. | AI-independent editor owners and repository governance |

### 5.2 Explicitly absent or prohibited

Stage 3 does not include a real external model, a second model route, an
unbounded Tool/MCP/research/embedding/Memory workflow, a new task-specific
workflow runtime, an Agent-authored outline, automatic Acceptance, automatic
authoritative write, or a Proposal editor outside the main StoryOS editor.
Any attempted mode that would require those absent capabilities fails closed
without authority or disclosure.

### 5.3 Entry, author journey, and completion

**Entry condition.** Stage 2 is released on one exact main with its
AI-disabled journey and evidence complete. A later Stage 3 implementation issue
must bind to that main and the current owner contracts.

**Author journey S3-JRN-001.**

1. Begin in the complete Stage 2 editor at a current passage in an existing
   Project.
2. Ask the adjacent Agent for bounded help with that passage, and verify that
   the current Working Target and request scope are preserved.
3. Observe the fake-model operation traverse the real Host, Context Assembly,
   manifest, Attempt, fence, recovery, and AgentRun records.
4. Edit the anchored Proposal in the main editor, then explicitly Accept it
   and verify the Core transition, Receipt, Author Action, and authoritative
   revision; repeat with explicit Rejection and verify no authority change.
5. Exercise a refused, conflicted, or interrupted Proposal through its
   existing recovery path and verify that late or stale work cannot publish.
6. Turn the fake destination off and repeat the Stage 2 manual journey; the
   editor remains usable and no fake result is treated as a required source of
   truth.

**Completion condition.** The fake path uses the same semantic boundaries
required of the real path, every S3 evidence obligation passes, Acceptance and
Rejection remain distinct durable outcomes, and the Stage 2 journey still
passes with the fake destination unavailable.

### 5.4 Mandatory evidence

| ID | Mandatory evidence category |
| --- | --- |
| S3-EVD-001 | Adjacent-Agent and intent-scope evidence showing one general loop, current-passage grounding, and no second workflow runtime. |
| S3-EVD-002 | Host, Project Scope, Context Assembly, manifest, destination Attempt, fence, recovery, and AgentRun provenance evidence for the fake operation. |
| S3-EVD-003 | Editable Proposal evidence for anchors, validation, refusal, conflict, recovery, and editor-owned presentation. |
| S3-EVD-004 | Acceptance evidence for the exact Core transition, Receipt, Author Action, and Authoritative Revision, plus Rejection evidence proving non-destructive settlement. |
| S3-EVD-005 | Interruption, late-result, stale-fence, and recovery evidence showing no blind retry, duplicate authority, or hidden disclosure. |
| S3-EVD-006 | Fake-destination limitation evidence that separates StoryOS-owned facts from Provider-internal, model-quality, and literary-quality claims. |
| S3-EVD-007 | AI-independent regression evidence proving that the complete Stage 2 editor remains usable with the fake path unavailable. |

Any failed, unrun, stale, unavailable, non-replayable, or unverified S3
obligation blocks Stage 3 release. A chat transcript or fake response without
the real Proposal path is not a passing substitute.

## 7. Stage 4 — one real external model with disclosure evidence

Stage 4 adds exactly one separately admitted real external-model operation. It
does not make the Provider a StoryOS authority and does not weaken the
AI-independent editor.

### 6.1 Required capabilities and owners

| ID | Required capability | Normative owner |
| --- | --- | --- |
| S4-REQ-001 | Admit one Provider-neutral real external-model route under the existing Registration, Model Use Binding, compatibility, capability, credential-reference, and policy contracts. | Model Gateway, trust-boundary, and Protocol owners |
| S4-REQ-002 | Use the same Host, Context Assembly, bounded projection, manifest-before-egress, destination identity, Attempt, dispatch fence, recovery, and Proposal paths proven by the fake stage. | Context/Disclosure, Model Gateway, PostgreSQL, Protocol, and Core/Proposal owners |
| S4-REQ-003 | Preserve minimum-necessary disclosure evidence: exact Project Scope, source/provenance, destination identity, binding/profile revisions, wire/digest evidence, usage classification, and no credential value in project records or ordinary logs. | Context/Disclosure, trust-boundary, PostgreSQL, and retention owners |
| S4-REQ-004 | Treat a crash, timeout, disconnect, or post-dispatch uncertainty as the existing OutcomeUnknown boundary; late results are fenced, blind resend is forbidden, and any reconciliation or successor is separately admitted. | Model Gateway, Run/Mailbox, Protocol, PostgreSQL, and retention owners |
| S4-REQ-005 | Keep every generated prose change as an editable Core Proposal requiring explicit author Acceptance or Rejection, with the existing refusal, conflict, and recovery semantics. | Core/Proposal, Web Editor Session, Admission, and Artifact owners |
| S4-REQ-006 | Include a real-author session that can write manually, request bounded assistance, inspect/edit a Proposal, Accept and Reject it, and recover the Run, Proposal, disclosure, and authoritative facts. | Web Editor Session, Core/Proposal, Context/Disclosure, and AgentRun owners |
| S4-REQ-007 | Limit the claim to StoryOS-owned disclosure and recovery evidence; do not claim Provider attention, Provider retention/training, hidden SDK behavior, or literary quality. | Context/Disclosure, Model Gateway, trust-boundary, and deterministic verification owners |

### 6.2 Explicitly absent or prohibited

Stage 4 does not add a second Provider, provider-specific authority, local
inference fallback, hidden SDK retry, unbounded external Tool/MCP workflow,
automatic authority, Agent-authored outline, or a cloud implementation stage.
Provider availability is not allowed to become a dependency of the complete
AI-independent editor. A Provider result that cannot satisfy the existing Host,
disclosure, Attempt, fence, recovery, or Proposal contract is rejected or held
without an authority effect.

### 6.3 Entry, author journey, and completion

**Entry condition.** Stage 3 is released on one exact main with its fake path,
Proposal, Acceptance/Rejection, recovery, and AI-independent regression
evidence complete. The selected real destination has a current, separately
admitted binding under the existing owner contracts.

**Author journey S4-JRN-001.**

1. Start with the complete AI-independent editor and its current passage; the
   editor remains usable if the external model is unavailable.
2. Ask the adjacent Agent for one bounded prose change and inspect the exact
   Working Target and Context Assembly decision.
3. Verify that the Host commits the manifest and disclosure evidence before
   the real external attempt, binds the exact destination and credential
   reference without exposing its value, and records the Attempt and fence.
4. Inspect and edit the returned Proposal in the main editor, then explicitly
   Accept it in one run and explicitly Reject it in another.
5. Interrupt one operation after the external dispatch boundary and verify
   OutcomeUnknown, fencing, late-result quarantine, and no blind resend or
   automatic successor.
6. Complete the required reconciliation or separately admitted recovery path,
   then inspect Run, Proposal, disclosure, Receipt, and authoritative facts.
7. Disable the external model and repeat the complete AI-independent journey;
   the editor still passes independently.

**Completion condition.** One real destination completes this journey with
current disclosure and uncertainty evidence, every S4 obligation passes, the
author can Accept or Reject through the editor, and all Provider-opaque claims
remain explicitly out of scope.

### 6.4 Mandatory evidence

| ID | Mandatory evidence category |
| --- | --- |
| S4-EVD-001 | Real-destination identity, Registration, Use Binding, compatibility, capability, policy, and credential-reference evidence. |
| S4-EVD-002 | Ordered Context Assembly, bounded projection, manifest-before-egress, disclosure, wire/digest, and exact Project Scope evidence. |
| S4-EVD-003 | Attempt, dispatch fence, usage classification, OutcomeUnknown, reconciliation, and late-result recovery evidence. |
| S4-EVD-004 | Proposal editing, Acceptance, Rejection, Receipt, Author Action, conflict, and recovery evidence for a real-author session. |
| S4-EVD-005 | Negative evidence for credential exposure, cross-Scope disclosure, hidden retry, stale binding, automatic authority, and Agent-authored outline. |
| S4-EVD-006 | Provider-boundary evidence that labels model quality, Provider attention, retention/training, and literary quality as unclaimed rather than silently passing them. |
| S4-EVD-007 | AI-independent regression and real-destination unavailability evidence proving the complete editor remains independently releasable. |

Any failed, unrun, stale, unavailable, non-replayable, or unverified S4
obligation blocks the real-model stage release. A successful network response
alone is never disclosure, Proposal, acceptance, or release evidence.

## 8. Evidence classes and fail-closed rules

The stage tables use evidence obligations owned by this baseline. The
deterministic-verification owner later chooses the executable proofs and
mechanics that satisfy them. This document intentionally does not choose
deterministic gate identifiers, fixtures, fault schedules, test frameworks,
database hooks, or oracle implementation.

Each mandatory evidence record must identify:

- the exact stage, requirement, journey step, and semantic owner;
- the exact main commit/tree, contract revisions, and applicable profile
  identities;
- the exact User and Project Scope and the protected client/session context
  when the evidence is project-bearing;
- the observed StoryOS-owned facts, expected disposition, and any known
  availability gap;
- safe provenance and disclosure classification without credential values or
  unnecessary prose; and
- replay instructions or a clear reason why replay is not applicable.

The following evidence categories remain distinct:

| Category | What it can establish | What it cannot substitute for |
| --- | --- | --- |
| Contract crosswalk | Owner, requirement, revision, boundary, and scope alignment | An implementation pass or author acceptance |
| Browser author journey | Input, editor continuity, visible state, navigation, and human-facing behavior | Core authority or PostgreSQL durability by itself |
| Core and persistence settlement | Atomic transition, Receipt, Event, Project Scope, and durable authority | IME capture or physical human gesture |
| Recovery and replay | Conservative recovery, fence, resync, lifecycle visibility, and no duplicate effect | A claim that missing evidence means no external submission |
| Isolation and trust | Fail-closed refusal, scope protection, Host/Origin/session, and secret exclusion | Literary quality or Provider-internal behavior |
| Performance and storage growth | Bounded observed workload behavior adopted by the named owner | A new SLA, retention value, cleanup deadline, or capacity promise |
| Proposal and author decision | Editable Proposal, Acceptance, Rejection, conflict, Receipt, and authority boundary | Chat text or model output as authority |
| Disclosure and destination | StoryOS-prepared context, destination identity, manifest, Attempt, and uncertainty | Provider attention, retention, training, or comprehension |

Evidence dispositions are strict:

- **Passed** means the required facts are present, attributable to the exact
  current baseline, and replayable where the obligation requires replay.
- **Failed** means an observed fact contradicts the applicable owner contract;
  the affected stage cannot release.
- **Unrun** means a mandatory obligation has no execution; it blocks release.
- **Stale** means the evidence does not match the exact baseline, contract
  revision, owner, Project Scope, or applicable profile; it must not be reused.
- **Unavailable** means the required proof or source cannot be inspected; it
  is not a pass and does not authorize a weaker claim.
- **Unreplayable** means the evidence cannot reproduce or inspect the claimed
  boundary; it blocks a release claim even if the original run looked green.
- **OutcomeUnknown** remains the owning operational uncertainty, not a failed
  request and not permission to retry blindly.
- **Advisory** evidence, including real-model observations or measurements not
  adopted by their owner, cannot satisfy a mandatory release obligation.

No stage may be marked complete by omitting a failed case, relabelling
unrun/unreplayable evidence, or treating a later stage's evidence as proof of
an earlier stage.

## 9. Planning, implementation, release, and controlled-cloud handoff

The following stable handoff identifiers prevent planning and implementation
ownership from cycling:

| ID | Gate | Passing condition | If not passed |
| --- | --- | --- | --- |
| HND-001 | Planning closure | The tracked contract owners and current design map agree on one exact main, with no unresolved contradiction in the four-stage baseline. | The release baseline remains open; no implementation handoff. |
| HND-002 | Implementation handoff | The terminal handoff owner verifies the complete current contract set, consumes this document's Stage 1 requirements, and creates exactly one bounded Stage 1 implementation issue. | No product implementation issue or product code begins. |
| HND-003 | Stage implementation evidence | The applicable stage issue runs the evidence obligations against the exact implementation baseline and records complete, attributable evidence. | The stage is incomplete; the issue cannot claim release. |
| HND-004 | Stage release | The stage author journey passes, every mandatory obligation passes, and the resulting main is the next stage's exact input. | The current stage remains the active implementation frontier; no next stage starts. |
| HND-005 | Controlled-cloud deployment gate | After the four implementation stages, the same contracts pass deployment identity, security, operational recovery, cache refresh, same-release activation, and upgrade checks for the controlled domain. | Controlled-cloud deployment is blocked; local AI-independent editor claims do not become invalid solely because cloud deployment is later. |
| HND-006 | Serial issue direction | Release requirements flow from this issue to [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60), and the resulting proof contract flows to [Create and Lock the First Editor-First Implementation Issue](https://github.com/FrankQDWang/StoryOS/issues/77). | This issue must not select executable proof mechanics, and no downstream issue may infer missing product scope. |

Planning closure is not stage implementation evidence. A current contract can
be ready for the implementation handoff while every future stage test remains
unrun. Conversely, passing a test or a fake destination cannot close a
planning contradiction or redefine an owner. The current issue stays focused
on the release contract; it does not update the map, the proof owner, or the
terminal implementation issue.

## 10. Explicit non-scope of this contract

This contract does not:

- implement Rust, TypeScript, SQL, migrations, generated routes, UI, runtime,
  backup jobs, deterministic harnesses, deployment, or cloud infrastructure;
- create or lock an implementation issue;
- select deterministic gate IDs, fixtures, fault schedules, test adapters,
  fake-server mechanics, schedulers, or oracle internals;
- define public routes, DTOs, Events, settlement, physical table families,
  RLS, backup/restore mechanics, Context Assembly semantics, AgentRun meaning,
  Proposal state transitions, or retention/lifecycle meaning;
- adopt any new Retention Profile value, cleanup duration, RPO/RTO, SLA,
  performance threshold, storage capacity target, or Provider guarantee;
- make an MCP App, transcript, cache, index, browser journal, fake result, or
  external Provider an authoritative data store;
- add a required outline, Agent-authored plan, collaboration, billing,
  ownership transfer, multi-author editing, or separate workflow runtime; or
- treat .reference/** as product input, dependency, build/test/package input,
  evidence authority, or implementation substitute.

The only planned primary document change for this ticket is this tracked
editor-first release baseline and handoff contract.
