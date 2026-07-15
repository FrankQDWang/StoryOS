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
A durable, uniquely addressable unresolved dependency, such as author input, Approval, or an external result, that blocks only the work depending on it. Only a typed response bound to that exact Wait may resolve it; an AgentRun can remain active while another branch continues despite one or more Waits.
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

**Run Pause**:
An author control that immediately creates a Run Hold, prevents new work from starting, and requests cooperative interruption of work that remains safe to cancel. The Hold propagates to descendant Subruns and preserves confirmed or uncertain effects; resuming the parent clears only the inherited Hold, not a child's own Hold or Wait.
_Avoid_: Run Wait, process kill, Run Cancellation

**Run Cancellation**:
An irreversible intent to stop an AgentRun that immediately prevents new work and propagates cancellation through its in-flight work and descendant Subruns. The AgentRun records a cancelled Run Outcome only after every affected operation has reached a confirmed terminal or outcome-unknown boundary; cancellation never rolls back an effect.
_Avoid_: Run Pause, process kill, rollback

**Run Finalization Gate**:
The automatic, deterministic, idempotent host check that turns a persisted Agent Finalize Intent into one terminal Run Outcome only after all in-flight operations, required joins, Waits, Holds, reservations, effect uncertainties, final Artifacts, Proposals, provenance, and unfinished-work dispositions are durably settled. It requires no routine author confirmation, cannot be bypassed by a model completion claim, and can resume safely after a crash without duplicating output or terminal events.
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

**Subrun**:
A durable hierarchical child execution within one root AgentRun, bound to an immutable direct parent and owning its own Run Lane, narrowed context and capabilities, budget slice, Waits, Holds, and typed outcome. It is not a top-level AgentRun, has no independent project-level grant, cannot outlive its root, and can never commit Authoritative State.
_Avoid_: Child AgentRun, background process, cloned conversation, permanent Agent

**Subrun Mailbox**:
The durable, ordered, bounded channel for typed progress, observation, Artifact, question, plan-change proposal, task, cancellation, and terminal-result messages between one Subrun and its direct parent. It grants no shared writable state or direct author access; any author question is escalated by the root lane, while Tool Gateway may independently surface an exact Approval Wait for a child ToolCall.
_Avoid_: Shared memory, cloned transcript, direct author chat, permission channel

**Subrun Result**:
The immutable typed terminal output of one Subrun, containing its outcome, produced Artifact references, observations, usage settlement, and unresolved effect evidence. It is delivered through the Subrun Mailbox and changes parent execution only after an explicit parent consumption record.
_Avoid_: Final chat message, parent outcome, shared state mutation

**Subrun Join**:
The parent-declared Required or Advisory dependency governing how a parent RunStep consumes one Subrun Result. Ordinary child failure never automatically fails the parent, while any outcome-unknown effect must escalate to the root AgentRun and prevent termination until reconciled or explicitly adjudicated.
_Avoid_: Failure propagation, thread join, implicit result merge

**Step Snapshot**:
The immutable, attributable view of the exact plan revision, context sources, contract versions, capabilities, budget remainder, guardrail counters, and project revisions used for one RunStep. It preserves decision evidence but grants no lasting authority, so effects still require live revalidation before execution.
_Avoid_: Prompt text, checkpoint, authorization token, current project state

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

**App View Artifact**:
A Core Artifact that preserves a transcript-embedded MCP App as a reproducible View Descriptor: fixed UI resource digest, protocol version, exact input revisions, authorized host-context snapshot, optional schema-bound view state, and static fallback. It never stores a live iframe runtime or controls authoritative domain data.
_Avoid_: App-owned state, iframe snapshot

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
