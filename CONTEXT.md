# StoryOS

StoryOS is a novel-project workspace in which the author retains authority over creative truth while Agents, Tools, Skills, and MCP Apps produce inspectable assistance around it.

## Language

**Authoritative State**:
The author-approved current truth of a novel project, including prose, canon, characters, timeline, outline, structure, and author plans. Authority is a binary boundary reached only through an explicit author-authorized domain action; lifecycle, confidence, and lock status do not form authority levels.
_Avoid_: Canon (too narrow), accepted artifact, Agent memory

**Authoritative Revision**:
An immutable version of one authoritative domain object, created through a Direct Author Action, Acceptance, or safe compensation and guarded by an expected prior revision.
_Avoid_: Artifact Revision, mutable row

**Authoritative Commit**:
The project-ordered atomic record of one author-authorized domain transaction, identifying its actor, cause, and all prior and resulting Authoritative Revisions. It provides a global sequence without copying a full project snapshot.
_Avoid_: Project snapshot, Run Event

**Direct Author Action**:
A deterministic, immediately visible change caused through the author's own editor input path against one exact authoritative target under direct manipulation, including manual paste. Bulk, cross-location, not-fully-previsible, Agent-, Tool-, MCP-, or extension-produced changes remain Proposal-gated even when an author click initiates them.
_Avoid_: Author-triggered automation, silent bulk edit

**Operational Record**:
A durable record of execution, context, authorization, usage, validation, or a state transition, such as an AgentRun, RunStep, RunPlan, ContextManifest, ToolCall, Approval, Artifact Lifecycle Event, Domain Receipt, or Run Event. It can reference and produce Artifacts but does not inherit Artifact lifecycle or authority.
_Avoid_: Artifact, temporary log

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
An immutable, causally attributable, monotonically sequenced fact recording one committed Run Transition for inspection and recovery history. Run Events and normalized current records are written atomically without requiring pure event sourcing; Checkpoints, caches, and read models are derived, while external effects follow persisted intent through the Tool Gateway and append their outcomes afterward.
_Avoid_: Mutable status row, model transcript, telemetry log, cache entry

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
The durable terminal boundary proving that one root AgentRun has no unsettled Subrun deliveries and that every sender generation is closed at recorded directional high-watermarks. Message payload retention is independent, but Message ID deduplication evidence cannot be discarded by age before the Seal and may be compacted afterward only without losing the proof needed to reject replay.
_Avoid_: TTL expiry, Inbox deletion, delivery acknowledgement, payload retention policy

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
A replaceable, derived projection of one AgentRun at an exact durable sequence, used only to accelerate recovery of its lanes, plan, waits, child operations, and guardrail counters. It contains no live process state or authority and may be discarded and rebuilt from normalized persistent records.
_Avoid_: Source of truth, backup, Step Snapshot, live session

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
The transfer of project-derived information beyond the local project boundary to a named external destination, including generated queries, excerpts, metadata, or Artifact content. Transformation does not stop information from being a disclosure; every transfer must fit an explicit grant and retain attributable evidence of its destination, purpose, data categories, and project sources.
_Avoid_: External tool call, network access, upload

**Capability Grant**:
A bounded authorization to request named operations over specified project resources, external destinations, data categories, budgets, and time. Effective authority is always the non-escalating intersection of the project policy ceiling, the current Run's Capability Grant, and the exact capability requested by a ToolCall; approval may narrow or extend a lower layer only within its parent boundary.
_Avoid_: Role, permission flag, discovered tool, model-visible tool

**Approval**:
An immutable author decision over one exact operational request, bound to its ToolSpec version, arguments, resolved targets, Tool Effect Request, scope, and governing policy. It may create a grant for only that call or a bounded remainder of the current Run; changed inputs require a new decision, high-risk disclosure, external writes, and irreversible effects remain one-shot, and permanent project policy changes occur only through explicit settings. Approval never performs Acceptance or changes Authoritative State.
_Avoid_: Permission flag, confirmation dialog, Acceptance, permanent project setting

**Policy Decision**:
An immutable result of StoryOS deterministically evaluating a request against already-effective project policy and Capability Grants. It may authorize an in-scope request or deny it, but it can never create, extend, or replace a Capability Grant; new authority requires author Approval.
_Avoid_: Approval, policy-authored grant, implicit permission

**Model Gateway**:
The sole StoryOS-owned boundary through which any RunStep invokes a local or external model. It applies only an exact Model Route Decision, requires fallback to produce a new Decision, and never executes model-produced Tool requests; only a validated, persisted Agent Decision may derive ToolCalls for the Tool Gateway.
_Avoid_: Provider client, model SDK, Tool Gateway, direct provider call

**Model Provider Adapter**:
The host-controlled protocol projection used by the Model Gateway to exchange one exact invocation with a local or external model provider and report provider-declared capabilities and failure evidence. It cannot decide retryability, select or substitute a model, initiate fallback, execute ToolCalls, grant authority, or become durable Run truth.
_Avoid_: Provider Adapter, provider-owned router, silent fallback, Tool executor

**Model Registration**:
The host-owned, versioned routable identity that binds one stable StoryOS model reference to an exact Model Provider Adapter, provider endpoint and account boundary, provider model identifier, Credential Reference, and Model Capability Profile revision. An opaque provider alias remains explicitly unverifiable, any binding change creates a new Registration revision, and provider evidence that conflicts with an exact binding creates Model Failure rather than rewriting past Run evidence.
_Avoid_: Model name, provider alias, deployment name, capability tier

**Model Registration Status**:
The durable Active, Quarantined, or Retired routing eligibility state of one exact Model Registration revision: only Active may enter a new Model Route Decision, Quarantined requires explicit Host revalidation, and Retired never returns to service. Status changes preserve past evidence; transient credential, quota, health, latency, and pricing facts belong to Model Operational Snapshot, while reintroducing a Retired binding requires a new Registration revision.
_Avoid_: Provider health, credential availability, model version, mutable Registration

**Model Capability Profile**:
The immutable, versioned, provider-neutral semantic envelope trusted for one Model Registration, covering supported input and output modalities, context and output bounds, streaming, Tool-request and structured-output semantics and their exact native or Host-compiled projection modes, generation controls, and reportable usage dimensions. Provider claims enter it only through Host mapping or validation, and an unknown required capability makes the Registration ineligible.
_Avoid_: Provider model card, Model Operational Snapshot, benchmark score, availability state

**Model Operational Snapshot**:
An immutable, attributable point-in-time observation of one Model Registration's current credential-reference availability, provider health, rate-limit or quota state, latency, pricing reference, and other dynamic routing facts. It may change current eligibility without changing the Registration or Model Capability Profile and never proves semantic capability.
_Avoid_: Model Capability Profile, Model Registration, durable model identity

**Model Routing Policy**:
The immutable, versioned Host rule set that deterministically filters Model Registrations by hard Model Route Request requirements, then ranks eligible candidates by declared soft preferences with a stable tie-breaker. Models and providers cannot author it; benchmark or learned evidence must be explicit and versioned, while random or experimental routing requires a separately authorized policy rather than hidden selection.
_Avoid_: Model recommendation, provider router, mutable score, implicit experiment

**Model Route Request**:
The immutable pre-sampling statement of hard model capabilities, context bounds, allowed provider and Outbound Disclosure destinations, budgets, and soft quality, latency, and cost preferences for one RunStep, assembled by the Host from its exact plan, Skills, author settings, policy, grants, and inputs. It names no executable model, grants no authority, and exists before that RunStep's Agent Decision.
_Avoid_: Model name, prompt hint, Model Route Decision, model self-selection

**Model Route Decision**:
The immutable Host result that either selects one exact Model Registration revision for a Model Route Request or records that no eligible route exists, binding the Model Routing Policy revision, complete evaluated candidate set and reasons, Capability Profiles, Operational Snapshots, grants, budgets, and comparison evidence used. It precedes sampling, cannot be authored by a model, and every fallback requires a new Decision over the same hard requirements.
_Avoid_: Model suggestion, mutable route, provider fallback, load-balancer choice

**Model Route Override**:
An immutable root-AgentRun-scoped author setting captured as Automatic, Prefer an exact Model Registration revision, or Require that revision, and applied prospectively to every Model Route Request in the whole execution tree; descendant Subruns may only add narrower requirements. It never bypasses capability, disclosure, grant, budget, credential, or policy eligibility; Prefer may allow another Route Decision, while Require records no eligible route instead of falling back.
_Avoid_: Optional model ID, mutable active model, Capability Grant, provider fallback

**Model Fallback**:
The Host-controlled admission of a successor Model Attempt for the same Model Invocation using a different exact Model Registration revision after a new Model Route Decision re-evaluates the unchanged Model Route Request. It never relaxes hard requirements, never repeats a Registration revision within one fallback chain, remains bounded by all Run budgets and overrides, and cannot be delegated to a provider router or SDK.
_Avoid_: Same-route retry, provider substitution, capability downgrade, fallback loop

**Model Invocation**:
The single logical request by one RunStep to obtain one Agent Decision under an immutable Model Route Request, owning the ordered Model Attempts and their derived aggregate outcome and usage. Provider completion terminates only its Attempt; the Invocation succeeds only when the Host validates and durably records one typed Agent Decision, and a later successful Attempt never erases earlier evidence.
_Avoid_: Provider request, Model Attempt, model response blob, retry counter

**Model Attempt**:
The model-specific Execution Attempt durably established before one concrete provider submission under one exact Model Route Decision, binding its request and disclosure evidence to the resulting stream, provider identifiers, partial output, usage, uncertainty, and terminal outcome. Retrying the same Registration appends an Attempt, while fallback also requires a new Model Route Decision; outputs from separate Attempts are never silently concatenated.
_Avoid_: Model Invocation, provider retry counter, overwritten request, merged fallback response

**Model Attempt Request**:
The immutable provider-neutral effective request for one Model Attempt, binding its exact Step Snapshot, ContextManifest, prompt and output contracts, Tool Exposure and ToolSpec digests, generation controls, streaming mode, output bounds, Model Route Decision, and parameter provenance or default state. Its Adapter projection records the mapping and wire-request digests without silently changing required semantics; ordinary retry preserves the semantic digest, repair creates a new Request, and fallback may change only the provider projection.
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
The provider-neutral, versioned semantic contract for one Tool, consisting only of its callable input, output, and error contract, Tool Effect Envelope, execution policy, and result and provenance rules. Implementation source and credentials belong to Tool Registration, while project enablement, provider compatibility, Exposure, grants, Approval, pricing, and invocation state remain separate dynamic records.
_Avoid_: Provider function schema, Tool Registration, installed tool, ToolCall

**Tool Discovery Record**:
An immutable observation of a third-party Tool contract and source identity before StoryOS has assigned trusted local semantics. It has no Tool Registration identity, project enablement, Exposure, or execution authority; explicit local mapping may use it to create a new Tool Registration.
_Avoid_: Tool Registration, installed tool, trusted ToolSpec

**Tool Registration**:
The host-owned, versioned record that binds a built-in implementation or exact Tool Discovery Record to its trusted StoryOS ToolSpec, implementation source, and adapter rules. Its lifecycle is active, quarantined, or retired; discovery and project enablement remain separate records.
_Avoid_: Tool Discovery Record, Project Tool Enablement, Tool Exposure

**Project Tool Enablement**:
A project's explicit enabled or disabled selection of one exact active Tool Registration. It permits the Registration to be considered for Exposure but grants no Run capability or execution authority.
_Avoid_: Tool Registration, Tool Exposure, Capability Grant

**Tool Contract Drift**:
The condition in which a Tool Registration's pinned implementation identity, trusted model-visible callable contract, input or output contract, or trusted adapter mapping no longer matches the currently discovered implementation. Drift quarantines the Registration for new calls, clears derived Exposure, and requires a new local mapping; untrusted descriptive provenance alone does not cause Drift, and name equality never carries authority across versions.
_Avoid_: Compatible runtime update, automatic permission inheritance, retryable tool error

**Tool Exposure**:
The disposable projection of an enabled Tool for one caller and RunStep, computed from two orthogonal inputs: the locally allowed caller routes and the current caller's initially-visible, deferred, or hidden discovery state. Exposure also depends on provider compatibility and current policy, but neither grants execution authority nor changes the Tool Registration.
_Avoid_: Tool Registration, project enablement, authorization

**ToolCall**:
An Operational Record for one requested invocation of an exact Tool Registration, including its caller route, validated arguments, resolved targets, Tool Effect Request, authorization state, execution lifecycle, and outcome. A ToolCall may produce Artifacts or other Operational Records but never inherits their lifecycle or authority.
_Avoid_: Tool result, Artifact, Approval, model message

**Tool Gateway**:
The sole StoryOS-owned authorization and execution boundary for every StoryOS-dispatched ToolCall, regardless of whether its caller is a model, generated program, MCP App, or host component. It resolves the Tool Registration, derives effects, enforces grants and Approval, invokes the trusted implementation, validates output, and records the outcome; provider-hosted execution cannot claim this local guarantee.
_Avoid_: Tool Registry, provider runtime, direct adapter call

**Credential Reference**:
An opaque, host-owned reference to credential material resolved only inside the execution boundary that needs it. Tool Registrations and Operational Records may identify the reference and its availability but never contain its value; models, MCP Apps, generated programs, Tool arguments, outputs, transcripts, and external servers cannot inspect, select, or transport it.
_Avoid_: API key field, secret value, model parameter, logged credential

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
An integrity identifier for an immutable payload that may also support physical storage deduplication. It never replaces Artifact or Artifact Revision identity, so causally distinct outputs remain distinct even when their payloads match.
_Avoid_: Artifact ID, semantic identity

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

**Research Synthesis**:
A Research Artifact that combines or interprets evidence and binds its claims to exact Source Snapshot revisions through supported-by Provenance Edges.
_Avoid_: Source Snapshot, uncited summary

**Claim**:
A stable, addressable conclusion within a Research Synthesis, linked to the exact evidence that supports it. A Claim remains non-authoritative regardless of confidence or repetition.
_Avoid_: Canon fact, paragraph citation

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

**Proposal Bundle**:
A Proposal subtype whose stable Bundle-level Operations reference exact child Proposal Revisions, selected child Operation IDs, and dependencies without copying child payloads. It declares atomic or ordered-independent execution, and Bundles cannot be nested.
_Avoid_: Mixed-domain Proposal, nested workflow

**Acceptance Eligibility**:
The predicate requiring an exact Proposal Revision to be retained, ready, valid for current targets, open, and selected only over pending Operations. Proposal identity, creator confidence, or a historical Validation Receipt cannot grant eligibility alone.
_Avoid_: Acceptable type, trusted Proposal

**Ready Partial**:
A Proposal generation outcome preserved after production stops before its intended completion. It remains editable but is not eligible for Acceptance until the author explicitly completes the current content or generation finishes.
_Avoid_: Failed Proposal, accepted partial

**Proposal Conflict**:
The condition in which a Proposal's target, base version, or preconditions no longer hold. A conflicted Proposal cannot be accepted or silently rebased and must instead be replaced or explicitly replanned.
_Avoid_: Stale warning, automatic merge

**Proposal Rejection**:
An author's non-destructive decision not to apply selected pending Proposal Operations. Reopen creates a new pending Proposal Revision, which cannot regain Acceptance Eligibility until current targets and preconditions validate successfully.
_Avoid_: Withdrawal, deletion

**Proposal Withdrawal**:
A non-destructive removal of a Proposal from active review by its current producer or the author. Withdrawal is not represented as an author rejection.
_Avoid_: Rejection, deletion

**Undo Acceptance**:
An author-authorized action that safely creates compensating authoritative versions and, when retained source content and a safe linear head allow it, a new Proposal Revision containing the previously applied content against the compensated base. Otherwise it creates a new derived Proposal or Reversal Proposal and never overwrites later conflicting author changes.
_Avoid_: History deletion, editor-only undo

**Reversal Proposal**:
A Proposal that expresses the inverse of an earlier Acceptance against current Authoritative State when a direct Undo Acceptance would conflict with later changes. It requires ordinary inspection and Acceptance.
_Avoid_: Forced rollback, silent undo

**Candidate**:
A Core Artifact presenting one independently reviewable semantic fact or object without carrying an authoritative change command. It can serve as a source for a Proposal but cannot be accepted directly; independently selectable alternatives remain separate Candidates.
_Avoid_: Proposal, pending truth

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
An author-authorized action that applies selected operations from an exact eligible Proposal Revision through a StoryOS-owned domain handler. Domain Proposal selections are atomic, while Proposal Bundles obey their explicit atomic or ordered-independent policy.
_Avoid_: Promotion, status flip, overwrite

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
