# Codex subagent orchestration: source research and StoryOS implications

- Status: research complete; architecture input, not implementation authorization
- Decision: [Specify Subrun Control-Plane, Mailbox, and Observability Semantics](https://github.com/FrankQDWang/StoryOS/issues/63)
- Reference baseline: [`openai/codex@1f0566d3f59298d1bb88820a0d35294f1eeb07ea`](https://github.com/openai/codex/tree/1f0566d3f59298d1bb88820a0d35294f1eeb07ea), pinned in read-only `.reference/codex`
- Web sources checked: official OpenAI documentation and engineering posts, 2026-07-15
- Scope: subagent identity, lifecycle, progress, messaging, waiting, interruption, completion delivery, persistence, UI visibility, and comparison with an external `codex exec` wrapper

## Executive conclusion

The user's 2025 experience was not mainly a weakness of Skills or of the child model. It was an **orchestrator/control-plane gap**.

A Claude Code Skill that starts `codex exec` creates an operating-system process outside the parent agent's native lifecycle. Unless the wrapper separately implements event ingestion, stable identities, status subscriptions, message routing, idempotent spawning, cancellation semantics, persistence, and recovery, the parent can only poll process output and guess whether the child is alive, finished, or stuck. Polling timeout is then easily confused with task timeout; a retry can create a duplicate child; and multiple children have no shared coordination protocol.

Codex subagents work better because they are first-class threads in one long-lived runtime:

1. one root-scoped `AgentControl` owns a typed agent tree, registry, execution limiter, residency manager, and communication paths;
2. every child has a thread ID, parent, depth, and canonical task path;
3. spawn is atomically reserved, so failed or duplicate starts do not leave ghost agents;
4. messages are typed and queued; “queue only” and “trigger another turn” are separate operations;
5. waiting is event-driven and non-destructive; a wait timeout does not interrupt, close, or restart a child;
6. interruption is a distinct explicit operation;
7. the UI can subscribe to the child's complete event stream, while the parent model receives only deliberate messages and a bounded completion envelope;
8. child threads and tree edges can be persisted and cold-loaded instead of treating a process as the source of truth.

The architectural lesson for StoryOS is therefore: **subagents are not a Skill feature; they are a durable runtime primitive with a control plane and two progress channels**. StoryOS should adapt that discipline, but not copy Codex's transient status semantics, best-effort completion forwarding, prose-only results, or shared-write workspace model.

## Evidence labels

- **Source fact** — directly implemented or tested in the pinned Rust source.
- **Official documentation** — current public OpenAI documentation or first-party engineering description.
- **Inference** — a conclusion from the composition or absence of mechanisms; not an explicit upstream promise.
- **StoryOS recommendation** — independent design guidance derived from the evidence and StoryOS product invariants.

## 1. What the external `codex exec` arrangement was missing

`codex exec` is designed as a non-interactive entry point for scripts and CI. Current official documentation says ordinary mode streams progress to `stderr` and leaves the final result on `stdout`; `--json` emits JSONL events, and a session can be resumed by ID ([non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode)). Therefore the subprocess is not intrinsically incapable of exposing progress.

The problem is that a parent Skill invoking the executable does not automatically receive the rest of the orchestration contract:

| Missing runtime primitive | Failure observed by the user |
| --- | --- |
| Typed child identity and parent/child registry | The parent cannot reliably distinguish an existing child from a new retry. |
| Event subscription and race-free wait | The parent polls, sees no new text, and guesses that the child has stalled. |
| Separate observe, message, continue, interrupt, and close operations | A polling timeout or wrapper cleanup can terminate healthy work. |
| Idempotent/atomic spawn | A retry can launch the same logical task twice. |
| Mailbox and delivery protocol | Peer agents coordinate through prose relayed by the parent or not at all. |
| Durable thread/result state | Process exit, terminal detachment, or parent restart loses authoritative state. |
| Shared scheduler and resource limits | Concurrent children oversubscribe resources or conflict in the workspace. |
| UI attachment to child event streams | The user sees only whatever the wrapper chooses to scrape and forward. |

**Inference:** a robust external-exec solution is possible, but it must consume the JSONL stream, persist correlation and lifecycle state, route later input through the correct resumed session, separate cancellation from observation timeout, and reconcile workspace effects. That recreates much of a subagent runtime outside the host. A Skill prompt alone cannot supply these guarantees.

## 2. Codex's control plane

### 2.1 One root-scoped runtime owns the whole tree

`AgentControl` is created once per root thread tree and cloned into its subagents. It owns a shared session identifier, agent registry, residency manager, execution limiter, and rollout budget ([`AgentControl`](../../.reference/codex/codex-rs/core/src/agent/control.rs#L88-L107)). This is an in-process control plane, not a convention that each agent must reconstruct from conversation text.

Each child carries a structured `SubAgentSource::ThreadSpawn` containing `parent_thread_id`, `depth`, `agent_path`, nickname, and role ([spawn source](../../.reference/codex/codex-rs/protocol/src/protocol.rs#L2786-L2804)). `AgentPath` gives every V2 agent a canonical `/root/...` structural identity, validates task-name segments, and resolves relative references against the caller's path ([`AgentPath`](../../.reference/codex/codex-rs/protocol/src/agent_path.rs#L15-L72), [relative resolution](../../.reference/codex/codex-rs/protocol/src/agent_path.rs#L125-L180)).

The V2 spawn handler builds this source metadata and calls `AgentControl::spawn_agent_with_communication` directly inside the runtime ([V2 spawn](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs#L40-L170)). This is the critical difference from shelling out: the child is registered in the same coordinator before ordinary collaboration starts.

### 2.2 Atomic spawn prevents ghosts and path collisions

The registry reserves a spawn slot before construction and commits metadata only after the child is successfully created. A `SpawnReservation` rolls back the path and count if construction fails ([reservation and commit](../../.reference/codex/codex-rs/core/src/agent/registry.rs#L80-L119), [duplicate-path rejection](../../.reference/codex/codex-rs/core/src/agent/registry.rs#L256-L305), [RAII rollback](../../.reference/codex/codex-rs/core/src/agent/registry.rs#L308-L353)).

This does not by itself make arbitrary user retries semantically idempotent, but it does prevent two live V2 agents from occupying the same canonical task path and avoids partially registered “ghost” children. StoryOS still needs an explicit durable idempotency key because a model may choose a different task name for the same logical work.

### 2.3 Context is forked deliberately, not inherited as a live process

V2 spawn supports full, none, or the last N turns of context. The fork path retains a limited whitelist—system, developer, user, and final assistant messages—rather than blindly copying tool traffic, reasoning, or inter-agent communication ([fork arguments](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs#L178-L225), [history sanitization](../../.reference/codex/codex-rs/core/src/agent/control/spawn.rs#L45-L78)). The child receives the caller's effective model, provider, reasoning settings, developer instructions, approval policy, sandbox, and working directory from the live turn context ([child configuration](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_common.rs#L154-L231)).

This is useful context projection, but StoryOS should prefer a typed, bounded `SubrunContextBundle` with artifact revisions and capability grants rather than treating a transcript fork as its canonical input.

## 3. Progress has two different channels

This distinction explains why Codex feels observable without flooding the parent model's context.

### 3.1 Human/UI channel: the full child thread event stream

The runtime announces a child thread to clients before submitting its initial task and persists the spawn edge ([spawn publication](../../.reference/codex/codex-rs/core/src/agent/control/spawn.rs#L380-L390)). The app server subscribes to thread creation, installs a listener for the initialized connection, and attaches the child thread's events to that connection ([app-server thread listener](../../.reference/codex/codex-rs/app-server/src/request_processors/thread_processor.rs#L2626-L2661)). Its protocol exposes per-thread turn and item lifecycle notifications, including collaborative tool calls ([app-server event stream](../../.reference/codex/codex-rs/app-server/README.md#L1351-L1389)).

Current official subagent documentation describes the product behavior: activity appears in the app, CLI, and IDE; the app surfaces each child as a separate thread; the CLI can switch agents; and the IDE provides a thread panel ([Subagents: monitor and steer](https://learn.chatgpt.com/docs/agent-configuration/subagents)). OpenAI's App Server engineering description likewise explains that one request can produce many events and that a long-lived `ThreadManager` owns one session per thread ([Unlocking the Codex harness](https://openai.com/index/unlocking-the-codex-harness/)).

Thus a user can inspect detailed child progress without asking the parent model to poll and restate every child event.

### 3.2 Parent-model channel: deliberate messages and bounded completion

The parent model does **not** continuously receive every child model item, tool call, or log line. Inter-agent communication is a typed record containing an ID, author and recipient paths, content, and whether it should trigger a turn ([`InterAgentCommunication`](../../.reference/codex/codex-rs/protocol/src/protocol.rs#L735-L790)). It is rendered into a compact `NEW_TASK` or `MESSAGE` envelope when delivered to a model ([model envelope](../../.reference/codex/codex-rs/protocol/src/protocol.rs#L800-L845)). Terminal delivery uses a separate bounded `FINAL_ANSWER` envelope ([completion envelope](../../.reference/codex/codex-rs/core/src/context/inter_agent_completion_message.rs#L5-L39)).

Official documentation explicitly says child work is kept off the main thread to reduce context pollution and that subagents return summaries rather than raw logs ([Subagents: context and results](https://learn.chatgpt.com/docs/agent-configuration/subagents)).

**Inference:** Codex's “real-time progress” is primarily a UI/client capability. The parent model learns fine-grained progress only when a child sends an explicit message or when the parent is woken by mailbox/completion activity. This is a feature: it separates observability from model context consumption.

## 4. Messaging and scheduling are explicit, typed operations

### 4.1 Queueing a message is different from starting work

The V2 tool surface makes two operations explicit:

- `send_message` queues a message and does not trigger a new turn;
- `followup_task` triggers a turn if the child is idle, or delivers the task at a safe message boundary when it is already running.

Their model-visible contracts state that distinction directly ([tool specifications](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_spec.rs#L153-L212)). Both use the same typed message path, differing only in `QueueOnly` versus `TriggerTurn` delivery mode ([shared message handler](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/message_tool.rs#L12-L31), [message dispatch](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/message_tool.rs#L59-L130)).

The session owns a FIFO mailbox plus a watch channel. Subscription checks already-pending mailbox/steering input after subscribing, which closes the common “completion arrived just before I began waiting” race ([input queue](../../.reference/codex/codex-rs/core/src/session/input_queue.rs#L34-L70), [FIFO delivery](../../.reference/codex/codex-rs/core/src/session/input_queue.rs#L72-L101)). Mail is drained into model input before the next sample and may preempt a running turn only at controlled model-message boundaries, not by tearing down an arbitrary in-flight tool call ([turn delivery](../../.reference/codex/codex-rs/core/src/session/turn.rs#L219-L235), [safe-boundary preemption](../../.reference/codex/codex-rs/core/src/session/turn.rs#L2099-L2142)).

### 4.2 One active turn per child, bounded concurrency across children

The execution guard refuses to start a second active turn for the same agent and acquires shared concurrency capacity before scheduling a triggered turn ([execution guard](../../.reference/codex/codex-rs/core/src/agent/control/execution.rs#L29-L82)). V2 residency only unloads an agent when it is not running and has no pending mailbox; completed, errored, or interrupted threads can be cold-loaded later ([unload eligibility](../../.reference/codex/codex-rs/core/src/agent/control/residency.rs#L216-L236), [cold reload](../../.reference/codex/codex-rs/core/src/agent/control/spawn.rs#L150-L227)).

This is why the system need not kill a healthy agent merely to control the number of resident sessions.

## 5. Wait is observation; interrupt is control

The V2 `wait` tool subscribes to parent input-queue activity, first checks for an already pending mailbox item, and then waits on a watch channel with a timeout ([wait implementation](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs#L36-L118), [race-free wait helper](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs#L171-L196)). Its result reports only a summary and whether the wait timed out ([wait result](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs#L127-L149)).

Crucially, the wait handler never calls a child operation. A timeout ends the caller's observation period; it does not interrupt, close, evict, or respawn any child. The model-visible contract also says the wait may end because of an agent update or steered user input ([wait specification](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_spec.rs#L252-L261)).

Interruption is a different tool. It resolves the target, captures the prior status, sends an explicit `Op::Interrupt`, emits an activity event, and leaves the agent available for future tasks ([interrupt handler](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_v2/interrupt_agent.rs#L26-L88)).

This separation directly prevents the user's earlier failure mode: “no progress observed yet” cannot silently become “the child should be terminated.”

## 6. Completion and lifecycle: useful Codex behavior, insufficient StoryOS contract

### 6.1 Current V2 completion is direct and best-effort

Every session event is persisted and sent to clients before terminal-turn parent notification is considered ([event ordering](../../.reference/codex/codex-rs/core/src/session/mod.rs#L1766-L1795)). For V2, a terminal `TurnComplete` or qualifying abort derives an `AgentStatus`, builds a completion message, and sends it directly to the immediate parent with `trigger_turn = false` ([terminal notification](../../.reference/codex/codex-rs/core/src/session/mod.rs#L1812-L1908)). A send error is logged and then ignored; there is no durable retry or reroute in this path.

Tests confirm the boundary: when the direct parent is dead, a grandchild's completion is neither delivered to that parent nor rerouted to root ([dead-parent test](../../.reference/codex/codex-rs/core/src/agent/control_tests.rs#L1965-L2073)).

An older detached completion watcher exists in `AgentControl`, but the V2 spawn path deliberately starts it only when the selected multi-agent version is **not** V2 ([legacy watcher](../../.reference/codex/codex-rs/core/src/agent/control.rs#L454-L541), [version gate](../../.reference/codex/codex-rs/core/src/agent/control/spawn.rs#L407-L419)). It must not be mistaken for the production V2 completion mechanism.

### 6.2 “Completed” means a turn completed, not an immutable subrun result

`AgentStatus::Completed` is derived from `EventMsg::TurnComplete`; it carries the last assistant prose. `Interrupted` is explicitly non-final, while completed, errored, shutdown, and not-found states are treated as final for status watching ([status derivation](../../.reference/codex/codex-rs/core/src/agent/status.rs#L4-L28)). Yet the same child thread can complete one turn, receive `followup_task`, and complete another turn; tests expect one parent completion message per turn ([follow-up completion test](../../.reference/codex/codex-rs/core/src/tools/handlers/multi_agents_tests.rs#L1960-L2111)).

Therefore Codex status is thread/turn operational state. It is not equivalent to a StoryOS `Subrun` with one immutable terminal outcome.

### 6.3 Completion is mainly prose

The completion prefix caps content at 1,000 tokens. A successful completion is the child's final assistant text; failures are abbreviated with a suggested next action; an interrupted turn produces no completion payload ([completion formatting](../../.reference/codex/codex-rs/core/src/session_prefix.rs#L10-L43)).

That is appropriate for conversational coding delegation, but StoryOS needs a typed result with artifact references, evidence, proposals, effects, conflicts, budgets, and provenance—not only a prose summary.

## 7. Persistence and recovery

Codex persists non-ephemeral spawn edges in an agent graph and flushes the parent rollout before taking a history snapshot for a fork ([edge persistence](../../.reference/codex/codex-rs/core/src/agent/control.rs#L682-L710), [fork snapshot](../../.reference/codex/codex-rs/core/src/agent/control/spawn.rs#L428-L582)). V2 can reconstruct a nonresident child from stored thread history and its saved path/metadata ([cold load](../../.reference/codex/codex-rs/core/src/agent/control/spawn.rs#L150-L227)).

However, the mailbox itself is a session-local in-memory queue. Communication becomes part of the durable history when it is drained and recorded into a model turn ([mail recording](../../.reference/codex/codex-rs/core/src/session/mod.rs#L2929-L2957)). Combined with best-effort direct-parent completion, this means Codex does not provide the durable inbox/outbox and exactly-once join semantics StoryOS needs.

## 8. Maturity and version boundary

At the pinned commit, the original `multi_agent` feature is stable and enabled by default, while `multi_agent_v2` is marked under development and disabled by default ([feature declarations](../../.reference/codex/codex-rs/features/src/lib.rs#L1031-L1042)). Current public documentation describes subagents as a supported product capability and says Codex orchestrates spawning, follow-up, waiting, and closing; it also documents configurable thread and depth limits ([official Subagents documentation](https://learn.chatgpt.com/docs/agent-configuration/subagents)).

The V2 source is especially relevant because its tools—task-path identity, `send_message`, `followup_task`, `wait`, `interrupt_agent`, and `list_agents`—match the interaction being investigated. But StoryOS should treat those internals as design evidence, not a stable upstream API contract. Public documentation may describe the stable product behavior while implementation details continue to change.

## 9. What StoryOS should adapt

### 9.1 Recommended durable domain model

```text
Subrun {
  subrun_id,
  parent_run_id,
  parent_subrun_id?,
  task_key,
  idempotency_key,
  objective,
  context_bundle_ref,
  authority_envelope_ref,
  join_policy: Required | Advisory,
  lifecycle,
  attempt,
  created_at,
  terminal_result_ref?
}

SubrunResult {
  subrun_id,
  outcome,
  summary,
  artifact_refs,
  proposal_refs,
  evidence_refs,
  effects,
  unresolved_effect_evidence,
  conflicts,
  disclosure_records,
  budget_usage,
  provenance
}
```

Keep two state machines separate:

- **Recommended Subrun lifecycle projection:** apply the same durable three-state discipline already chosen for StoryOS Runs: `Queued -> Active -> Terminal`. Waits, Holds, cancellation intent, recovery, and finalization remain orthogonal records while the Subrun is Queued or Active; its terminal result settles as `Succeeded | PartiallySucceeded | Failed | Cancelled`. Unknown external effects remain evidence that blocks successful finalization, not another top-level outcome. The downstream protocol and storage decisions must still settle the exact persisted fields and transitions.
- **Child turn state:** `idle | sampling | executing_tool | interrupted | completed_turn | errored`.

A completed child turn may be followed up. A finalized `SubrunResult` may not be silently reopened or overwritten.

### 9.2 Required control-plane contracts

1. **Atomic idempotent spawn.** Persist one `Subrun Request` with a stable request key in its parent Run Lane and Agent Decision before execution. Duplicate delivery of that Request returns the existing Subrun; an intentional retry or repetition requires a new causally linked Request.
2. **Durable inbox/outbox.** Messages have stable IDs, sender/recipient, causal references, delivery state, acknowledgement, and deduplication. Completion/result persistence and outbox enqueue happen in one transaction.
3. **Non-destructive observation.** A parent observes child state through a durable Run Wait and Run Wakeup, with an atomic initial state check that closes the subscription race. An observation deadline ends only that Wait and leaves the child and its Join unchanged. Subrun Interrupt, Run Pause, and Run Cancellation remain separate controls.
4. **Declarative join and disposition.** Every Subrun Request declares an immutable `Required` or `Advisory` dependency. Required prevents the creating RunStep from settling until any terminal Subrun Result exists; Advisory does not. Neither kind propagates child success or failure, and every Result receives one explicit Subrun Result Disposition in Request declaration order. Aggregate `any` or `quorum` policies would be a future design decision, not an assumption of this research.
5. **Two progress channels.** Clients may inspect the detailed child event timeline. The parent model receives bounded, attributable Subrun Progress Reports and typed Results only when useful.
6. **Bounded context projection.** A `SubrunContextBundle` names exact artifact/revision references, selected transcript fragments, source provenance, hard size caps, authority, disclosure allowance, and budget. Do not default to a full transcript clone.
7. **Attenuated authority.** A child receives no more capabilities than the parent and only the subset required by the delegated task. Tool discovery is not authorization.
8. **Recovery-owned scheduling.** Leases/heartbeats, concurrency reservations, and restart recovery are host responsibilities. The model never decides that “silence means dead.”
9. **Durable direct-parent delivery.** Worker or application loss leaves messages in the exact direct parent's durable Inbox for recovery; StoryOS does not reroute them. Only a missing, corrupt, or lifecycle-invalid persistent parent creates an Undeliverable Subrun Message and root Safety Hold, while preserving the original message and reason.

### 9.3 StoryOS authority and workspace boundary

Codex's agents share the same directory and see each other's edits; its own generated guidance warns that all agents share a workspace, and official documentation cautions against parallel write-heavy tasks ([shared-workspace guidance](../../.reference/codex/codex-rs/core/src/config/mod.rs#L251-L258), [official Subagents documentation](https://learn.chatgpt.com/docs/agent-configuration/subagents)).

StoryOS must not copy that as an authority model. Under the repository invariants:

- one execution owner controls any authoritative deliverable;
- investigative subruns should start read-only;
- child output becomes typed artifacts or inspectable Core Proposals;
- parallel children must not directly mutate Authoritative Creative State;
- Acceptance remains the only path from Agent/Tool/MCP-produced changes into authoritative state.

## 10. Recommended first vertical slice

Implement one **read-only research subrun with a required join**:

1. The parent Run persists a Subrun Request with a stable request key, typed Subrun Context Bundle, attenuated Subrun Capability Grant, budget slice, and Required Join.
2. The scheduler atomically reserves capacity and transitions the Subrun to `Active`.
3. The child emits detailed lifecycle and progress events to its own stream; optional Subrun Progress Reports go through the durable Outbox.
4. On completion, the Subrun Result, Terminal Run Event, and direct-parent delivery intent are committed atomically.
5. The parent's durable Wait wakes when the Required Join condition becomes satisfiable, loads the typed Result, records one Subrun Result Disposition, and continues.
6. An observation deadline ends only the parent's Wait and leaves the child and Join unchanged; interruption, pause, and cancellation require their explicit controls.
7. A process or application restart reconstructs the subrun, inbox/outbox, result, and outstanding join from StoryOS-owned state.

Keep the first slice at depth one and read-only. Exclude direct project writes, nested delegation, author approval delegation, and unbounded peer-to-peer chatter until persistence and recovery semantics are proven.

## 11. Verification matrix

| Area | Required test |
| --- | --- |
| Spawn identity | Two requests with the same idempotency key create one subrun; different logical tasks cannot collide. |
| Spawn failure | Failure before/after worker creation leaves no ghost reservation and can recover safely. |
| Wait race | Completion before subscription, during subscription, and exactly at timeout is never lost. |
| Wait semantics | An observation deadline ends only the exact parent Wait and leaves the child and Join unchanged; Steering Input cannot resolve it. |
| Message scheduling | Queue-only does not start an idle turn; follow-up does; active delivery occurs only at a safe boundary. |
| Delivery | Inbox/outbox replay is deduplicated and ordered by causal/message ID. |
| Completion atomicity | Crashes before Result, between Result and delivery, and after delivery yield one durable Result, Terminal Event, and effective direct-parent delivery. |
| Parent loss | Worker or application loss preserves the exact direct-parent Inbox; an invalid persistent parent creates an Undeliverable Subrun Message and Safety Hold without rerouting. |
| Cancellation | Parent, child, and descendant cancellation propagation follows the declared policy; uncertain external effects produce unresolved evidence that blocks successful finalization. |
| Concurrency | In-flight work is never evicted merely because an observer deadline elapsed; reservations respect global and per-run limits. |
| UI/model split | UI receives full child events; parent prompt receives only bounded Subrun Progress Reports and Results. |
| Authority | Child cannot exceed the delegated capability/disclosure envelope or mutate authoritative creative state. |
| Join | Required versus Advisory settlement, non-propagating outcomes, one explicit Result Disposition, and deterministic Request-order presentation behave correctly; no unapproved aggregate policy is inferred. |
| Recovery | Restart restores live Subruns, unresolved Waits and Holds, completed results, and undelivered messages from durable records. |

## 12. Final assessment

Codex demonstrates that usable subagents require a host-owned collaboration substrate:

```text
typed identity + event streams + mailbox + scheduler + non-destructive wait
+ explicit interrupt + bounded completion + persistence/reload + UI attachment
```

Skills can tell the main agent **when and why to delegate**. They cannot safely implement the lifecycle of delegation by themselves. StoryOS should therefore place subruns in the Agent Loop's durable execution model and let Skills merely influence planning and specialization.

The strongest patterns to adopt are the two-channel progress model, explicit separation of queue/follow-up/wait/interrupt, atomic child registration, safe-boundary message delivery, and host-owned concurrency. The strongest boundaries not to copy are best-effort direct-parent completion, mutable turn-derived “Completed” status, prose-only results, transcript-centric context inheritance, and shared authoritative writes.
