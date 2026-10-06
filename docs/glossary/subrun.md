# Glossary: Subrun

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

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
