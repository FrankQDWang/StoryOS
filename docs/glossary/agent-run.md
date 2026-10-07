# Glossary: AgentRun, plan, retention, and budget

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Retention Profile**:
A versioned policy contract that supplies the bounded time, capacity, replay, checkpoint, archive, compaction, and generated Memory payload retention values for one Project Scope. Every lifecycle action binds its exact Profile Revision and frozen effective values; a new revision applies prospectively, while affecting an existing record requires a new inspectable Retention Decision rather than a silent retroactive expiry.
_Avoid_: Global mutable TTL, host configuration switch, per-Run author setting

**Retention Decision**:
The immutable Project Scope-bound lifecycle determination that applies one Retention Profile Revision to an exact record or payload role, recording its class, eligibility, source and settlement evidence, effective bounds, due condition, and resulting availability transition or refusal. A Profile update alone is not a Retention Decision and cannot delete, compact, archive, or revive prior payloads.
_Avoid_: Background cleanup log, mutable expiry column, profile update

**Operational Record**:
A durable record of execution, context, authorization, usage, validation, or a state transition, including Author Command Admission evidence, Pre-Admission Refusal Records, Editor Input Fences, Author Actions, typed Receipts, AgentRuns, ToolCalls, Approvals, Artifact Lifecycle Events, and Run Events. It can reference or produce Artifacts but has no Artifact revision, derivation, retention, Acceptance, or authority lifecycle.
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
An immutable, causally attributable, monotonically sequenced fact recording one committed Run Transition for inspection and recovery history. Run Events and normalized current records are written atomically without requiring pure event sourcing; the event fact remains historical even when an associated eligible operational payload later becomes unavailable through Operational History Compaction, while Checkpoints, caches, and read models are derived and external effects follow persisted intent through the Tool Gateway and append their outcomes afterward.
_Avoid_: Mutable status row, model transcript, telemetry log, cache entry

**Run Event Segment**:
A Project Scope-bound, losslessly encoded physical grouping of contiguous immutable Run Events for storage or cold Archive. It may be compressed, moved, or have its replay service bounded, but it never semantically deletes, reorders, rewrites, or replaces any committed Event or its causal meaning.
_Avoid_: Semantic event deletion, lossy transcript summary, mutable event batch, new truth stream

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

**Step Snapshot**:
The immutable, attributable view of the exact plan revision, Skill Selection Set and SkillPackage Snapshots, context sources, contract versions, capabilities, budget remainder, guardrail counters, and project revisions used for one RunStep. It preserves decision evidence but grants no lasting authority, so effects still require live revalidation before execution.
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
A durable, Project Scope-bound PostgreSQL projection of one AgentRun at an exact durable sequence, used only to accelerate recovery of its lanes, plan, waits, child operations, and guardrail counters. It contains no live process state or authority, may be discarded and rebuilt from normalized persistent records, and cannot turn a known compacted-payload gap into byte-level replay or a permanent second history.
_Avoid_: Source of truth, backup, Step Snapshot, live session

**Operational History Compaction**:
A policy-versioned automatic retention transition for an eligible terminal, root-sealed Run or Subrun that makes a Compactable Operational Payload unavailable while retaining its Operational Evidence Floor, digest, checkpoint or snapshot evidence, and explicit availability gap. It is distinct from Operational Archive and never changes Authoritative State, an Artifact, or prior context or disclosure history; Artifact Tombstone and author-initiated deletion are separate.
_Avoid_: History rewrite, Artifact Tombstone, cache eviction, silent log deletion

**Generated Memory Payload Cleanup**:
A policy-versioned retention purge of a superseded generated Memory revision's bytes after current publication, Artifact Head, author-edit, active-work, recovery, and shared-payload protections pass. It preserves revision identity, provenance, digest, recorded uses, the Retention Decision, and an explicit availability gap without changing the Artifact's common Retention State.
_Avoid_: Artifact Tombstone, index eviction, Memory Note completion, precise semantic forgetting

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
