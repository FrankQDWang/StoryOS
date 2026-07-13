# Codex agent-kernel patterns for StoryOS

- Status: research complete; architecture input, not implementation authorization
- StoryOS ticket: [#41](https://github.com/FrankQDWang/StoryOS/issues/41)
- Destination map: [#1](https://github.com/FrankQDWang/StoryOS/issues/1)
- Reference baseline: [`openai/codex@1f0566d3f59298d1bb88820a0d35294f1eeb07ea`](https://github.com/openai/codex/tree/1f0566d3f59298d1bb88820a0d35294f1eeb07ea)

## Executive conclusion

StoryOS should adopt Codex's *orchestration disciplines*, not its coding-agent runtime.

The most valuable adaptable patterns are:

1. a model/tool loop with explicit thread, turn, and per-sampling-step boundaries;
2. an immutable step snapshot shared by context assembly and advertised tools, then attached to Tool execution as decision evidence while live authorization is revalidated;
3. a registry that keeps tool declaration and executable implementation together while separating registration, model exposure, and dispatch;
4. an explicit approval decision followed by a separate enforcement/sandbox decision;
5. one MCP manager and adapter feeding the ordinary tool registry, with lifecycle, provenance, deterministic names, timeouts, and bounded output;
6. progressive-disclosure Skills with a bounded catalog and full instructions loaded only after selection;
7. bounded child-agent trees with explicit parentage, context projection, messaging, cancellation, and resource limits;
8. typed lifecycle events, durable terminal boundaries, replay tests, and scoped `AGENTS.md` instructions with provenance.

StoryOS must redesign the semantic center around authorship. Its kernel needs durable `AgentRun`, `Plan`, `RunStep`, `ToolCall`, `ApprovalRequest`, `Artifact`, and `RunEvent` records; domain capabilities instead of shell permissions; typed Core Proposals whose selected pending Operations may affect Authoritative State only through StoryOS Core Acceptance; narrow Core-owned Direct Author Actions; and crash recovery from StoryOS-owned state rather than a model process or transcript. Those requirements follow the repository's product invariants and accepted domain vocabulary: author-owned creative state, discovery distinct from authorization, minimum external disclosure, durable inspectability, and MCP Apps that remain non-authoritative views/controllers ([StoryOS `AGENTS.md`](../../AGENTS.md#product-invariants), [canonical glossary](../../CONTEXT.md#language), [authority-changing commands](../foundation/artifact-domain-model.md#8-authority-changing-commands)).

The recommendation is therefore **independent implementation with selective pattern adaptation**. A few isolated utility units may later be evaluated for code reuse, but no Codex runtime crate or source tree should enter the StoryOS workspace or dependency graph.

## Decision vocabulary

Every area below is classified four ways:

- **Adaptable pattern** — a design invariant worth carrying into StoryOS.
- **StoryOS redesign** — the same concern expressed through StoryOS domain types and authority rules.
- **Reject / do not copy** — a Codex assumption that would distort the product.
- **Code candidate** — a small implementation unit that might be reusable only after architectural, transitive-dependency, license, and provenance review.

“Adapt” never means “fork Codex.” The pinned tree is evidence and a read-only reference, not a runtime dependency.

## 1. General agent loop and execution boundaries

### What Codex demonstrates

Codex documents a simple general loop: sample the model; execute requested function calls; append their results; sample again; complete when the model returns only an assistant message ([turn loop][codex-turn-loop]). Its implementation adds several important boundaries:

- `TurnContext` holds the effective model, environments, approval policy, permission profile, dynamic tools, skills, and timing for one turn ([turn context][codex-turn-context]).
- Each sampling iteration captures one `StepContext`, specifically so context, advertised tools, and tool calls use the same request view ([step snapshot][codex-step-snapshot]).
- Input sent while a turn is active is queued and drained at controlled points between samples ([steering][codex-steering]).
- A session owns at most one running task at a time; the task has a cancellation token and abort handle ([active turn][codex-active-turn]).
- Stop hooks can request a bounded continuation instead of forcing the loop to end on the first final-looking message ([stop hooks][codex-stop-hooks]).
- Task completion and interruption emit explicit terminal events. Codex flushes ordinary items before finishing a turn ([pre-terminal barrier][codex-pre-terminal-flush]) and flushes again after the terminal event; interruption also flushes its marker before emitting the abort ([task completion][codex-task-completion], [task abort][codex-task-abort]).

This is a general agent loop even though Codex fills it with coding tools. The separation between loop mechanics and tool specialization directly supports StoryOS's decision to have one novel-project Agent Loop with task-specific Tools, MCP, Services, Skills, and policy.

### Adaptable pattern

Use four distinct execution scopes:

| Scope | Responsibility | Lifetime |
|---|---|---|
| Project agent | Stable identity and project-level defaults; never a running model process | Project |
| `AgentRun` | One durable intent, trigger, authority envelope, budget, and outcome | User/event/schedule invocation |
| Turn | One user-visible exchange or autonomous continuation boundary | Part of a run |
| `RunStep` | One immutable model/tool decision snapshot and its effects | One sampling or deterministic action |

At the beginning of every `RunStep`, freeze a `StepSnapshot` containing at least:

- run/turn/step IDs and causal parent;
- model/provider identity and prompt-contract version;
- context artifact IDs plus content digests;
- Tool Registry snapshot and exact ToolSpec digests;
- active Skill versions/digests;
- MCP server/tool versions and trust metadata;
- effective capabilities, disclosure allowance, budget remainder, and guardrail counters;
- project commit sequence and exact Authoritative Revisions read by the step.

The same snapshot must provide the attributable evidence for prompt construction, tool declarations, policy inputs, audit records, and deterministic recovery decisions. It is not a long-lived authorization token. Immediately before Tool execution, external disclosure, or Acceptance, StoryOS must live-revalidate current revocation state, target revisions, policy, and remaining budget, then atomically reserve the required budget and conflict locks with the transition into execution.

### StoryOS redesign

The recommended loop is:

1. Load the durable run, current plan revision, authority envelope, budgets, and project revision.
2. Select or revise the next `PlanStep`; persist the decision and rationale.
3. Materialize and persist a `StepSnapshot` before calling a model or external service.
4. Build bounded context from typed StoryOS artifacts, provenance, explicit user input, selected Skills, and prior step results.
5. Resolve registered tools, then independently filter them by exposure, capability, disclosure, trust, and remaining budget.
6. Ask the model for a typed decision: tool call(s), plan revision, proposal/final artifact, question, or stop.
7. Persist the decision before side effects. For each tool call, perform policy evaluation and approval, execute under enforcement, persist result/artifacts, and loop.
8. Treat prose or structural edits as Core Proposals. The Agent/Tool path stops at a Proposal Revision; only the author's explicit Acceptance of selected pending Operations lets a StoryOS Core handler construct and atomically execute the internal domain commands. Narrow Direct Author Actions use their separate Core-owned boundary.
9. End only on a typed terminal condition or guardrail outcome; persist the terminal event before notifying clients.

The run guardrail is multidimensional: model calls, tool calls, elapsed time, token/cost budget, repeated failures, repeated no-progress decisions, external disclosure, subrun depth/concurrency, and user-configured limits. Codex primarily relies on context compaction for loop longevity and says it should avoid an infinite loop when compaction works ([compaction comment][codex-compaction-comment]); StoryOS needs explicit no-progress and budget termination independent of context size.

Recommended run states are `queued`, `running`, `waiting_for_user`, `waiting_for_approval`, `waiting_for_external`, `paused`, and terminal `succeeded`, `failed`, or `cancelled`. A `RunStep` should additionally distinguish `planned`, `prepared`, `waiting_for_user`, `waiting_for_approval`, `waiting_for_external`, `executing`, `succeeded`, `failed`, `cancelled`, `skipped`, and `outcome_unknown`. State-transition methods should be exhaustive and reject illegal transitions.

### Reject / do not copy

- Do not make a live thread/session, WebSocket, model response ID, or process the source of truth.
- Do not equate a Codex “task” or “turn” with a durable StoryOS run.
- Do not copy coding task kinds such as review, compaction, or user shell as kernel concepts.
- Do not expose arbitrary shell execution as the universal escape hatch.
- Do not let context compaction rewrite or replace the durable audit trail; compacted model context is a derived view.
- Do not allow hooks to create unbounded invisible continuations. Every continuation consumes guardrail budget and produces a visible event.

### Code candidate

No loop implementation is a direct reuse candidate. `run_turn` is deeply coupled to Responses API items, Codex history, code-oriented hooks, compaction, terminal tools, and session services. Reuse the state-boundary ideas and write the StoryOS loop against StoryOS contracts.

## 2. Plan, run, turn, and step semantics

### What Codex demonstrates

Codex's `update_plan` schema is a small list of `pending`, `in_progress`, or `completed` checklist items ([plan types][codex-plan-types]). The handler explicitly calls it a TODO/checklist tool, disallows it in Plan mode, and merely emits a `PlanUpdate` event ([plan handler][codex-plan-handler]). This is useful UI feedback, but it is not a durable execution plan or recovery model.

### Adaptable pattern

- Keep plan representation separate from the loop implementation.
- Let the agent revise a plan during execution.
- Emit plan changes as typed events clients can render.
- Require a single current in-progress frontier, unless the plan explicitly represents safe parallel branches.

### StoryOS redesign

Make plans first-class records:

```text
Plan {
  plan_id, run_id, revision, goal, created_by,
  steps: [PlanStep], rationale, supersedes_revision
}

PlanStep {
  step_id, objective, dependencies, expected_artifact_kind,
  required_capabilities, disclosure_ceiling, budget_slice,
  status, attempt, result_artifact_refs, failure, replan_reason
}
```

The plan is adaptive, but each revision is immutable. Replanning appends a new revision and explains which facts invalidated the old plan. A run may proceed without a long visible plan for trivial work, but it still records its next-step decision and authority envelope.

“Plan,” “turn,” and “step” must not be aliases:

- the plan says what the run currently intends to do;
- the turn is a conversational/autonomous interaction boundary;
- the step is the smallest recoverable decision/effect unit;
- the run owns all of them and carries the terminal outcome.

### Reject / do not copy

- Do not persist only the latest checklist presentation.
- Do not infer execution completion from a model changing a checklist item.
- Do not let plan text grant tool access or author permission.
- Do not use an open-ended thread goal as the execution authority. A run begins only from an approved user, event, or schedule trigger and remains bounded.

### Code candidate

The three-value `StepStatus` enum is too small for StoryOS recovery and has no identity, dependency, revision, attempt, artifact, or authority semantics. It is evidence for a UI adapter only, not a code-reuse candidate.

## 3. Tool Registry, router, and specifications

### What Codex demonstrates

Codex has a valuable separation of concerns:

- A reusable `ToolExecutor` keeps the tool name, model-visible spec, exposure, discovery metadata, parallelism declaration, and handler together ([tool executor][codex-tool-executor]).
- `ToolExposure` distinguishes direct, deferred, model-only, and hidden-but-dispatchable tools ([tool exposure][codex-tool-exposure]).
- A per-step `ToolRouter` contains the registry and the exact model-visible specs; it parses different provider response items into one internal `ToolCall` and dispatches it with the same step snapshot ([tool router][codex-tool-router]).
- Tool planning assembles built-ins, MCP, extensions, and dynamic tools into model-visible declarations plus a registry ([tool plan][codex-tool-plan]).
- The registry rejects duplicate names, rejects incompatible payload kinds, runs pre/post hooks, and records lifecycle/telemetry around execution ([registry][codex-tool-registry]).

The weak point is that Codex's `ToolSpec` is explicitly a Responses API wire representation ([Responses ToolSpec][codex-tool-spec]). StoryOS should not make a provider schema its canonical domain contract.

### Adaptable pattern

Use three separate concepts:

1. **Registration** — what implementations exist and are valid.
2. **Exposure/discovery** — what the model can see now or load later.
3. **Authorization** — what this run/step may actually call with these arguments and data.

Exposure is never authorization. A hidden tool may still require authorization; a model-visible tool may still be denied at invocation time.

Keep the declaration and executor coupled enough that a declared tool cannot exist without an implementation, but compile provider-specific declarations through an adapter. Reject duplicate canonical names and ambiguous aliases at registry construction.

### StoryOS redesign

Define a provider-neutral `StoryOSToolSpec` with at least:

| Field group | Required content |
|---|---|
| Identity | canonical name, semantic version, implementation version, source/provenance, spec digest |
| Contract | description, typed input schema, typed output schema, error schema |
| Effects | `read`, `propose`, or `external_effect`; expected external side effects and data classes; authoritative-state impact is always none |
| Authority | required capabilities, approval policy, scope rules, data classifications |
| Disclosure | allowed outbound fields/classes, destination class, redaction policy |
| Execution | timeout, cancellation behavior, retry policy, idempotency class/key strategy |
| Budget | token/cost/time/tool-call accounting dimensions |
| Concurrency | serial/parallel declaration plus conflict keys or resource locks |
| Result | artifact kinds, provenance requirements, user-visible summary policy |

Compile that spec into Responses API, another model provider, MCP, and HTTP/API schemas. Provider adapters may lose capabilities but may not invent authority.

Every tool execution should create a durable record before execution with the exact spec digest, canonicalized input digest, run/step ID, live authorization decision, approval reference, disclosure record, idempotency key, and attempt number. The execution transition must atomically reserve budget and applicable conflict locks after revalidating revocations, policy, target revisions, and the external-disclosure envelope. The result records output/artifact digests, observed side effects, cost, timing, and terminal classification.

The first StoryOS tool surface should be narrow and domain-oriented: fine-grained read/query tools, Proposal creation, typed Artifact production, and explicitly approved external effects. A model-visible Tool, MCP Tool, Skill, Service, AgentRun, or Subrun never exposes or invokes an authoritative domain command. StoryOS Core constructs those commands internally only while handling Acceptance of selected pending Proposal Operations; narrow Direct Author Actions also remain inside the Core boundary. General-purpose filesystem and shell tools should not be in the product loop.

### Reject / do not copy

- Responses API `ToolSpec` as the canonical model.
- Shell, patch, git diff, working-directory, and terminal process assumptions.
- Code-mode-specific nested tools or provider-specific freeform payloads in kernel types.
- A boolean “supports parallel” without conflict keys, authority checks, and authoritative-state serialization.
- Pre/post hooks that can silently rewrite semantically significant tool input without persisting both original and effective input.

### Code candidates

After review, the small `ToolName` value type from `codex-rs/protocol` (re-exported by `codex-rs/tools`), JSON-schema helpers, exposure enum, and executor/spec coupling may be candidates ([tool identity][codex-tool-name]). The core registry/router are not: they depend on Codex sessions, Responses items, telemetry, code hooks, and turn diff tracking. Even for isolated candidates, StoryOS must add effect, capability, disclosure, idempotency, budget, and artifact semantics.

## 4. Approval, capability, and sandbox boundaries

### What Codex demonstrates

Codex distinguishes an effective `PermissionProfile` from its display identity and supports managed, disabled, or externally enforced sandboxes ([permission profile][codex-permission-profile]). It represents per-command grants as partial permission overlays ([permission overlay][codex-permission-overlay]). Its policy transformation code normalizes and intersects a user grant with the original request while retaining constraining deny rules, so a grant cannot silently exceed the request ([permission intersection][codex-permission-intersection]).

Execution follows an explicit sequence: classify a call as skip, forbidden, or approval-required; resolve approval; select and run the sandbox; then consider a separately gated escalation after sandbox denial ([approval orchestrator][codex-approval-orchestrator]). The implementation also refuses an escalation path that would erase deny-read restrictions ([sandbox escalation][codex-sandbox-escalation]).

### Adaptable pattern

Use this decision pipeline:

```text
declared requirement
  -> requested capability set
  -> intersect with run capability ceiling
  -> policy decision (allow / deny / ask)
  -> durable approval request and response when needed
  -> effective one-shot or scoped grant
  -> enforcement/sandbox selection
  -> execution
  -> observed-effect verification and audit
```

Approval and enforcement solve different problems. Approval records human/policy authorization; a sandbox constrains what executable code can physically do. Neither substitutes for the other.

An approval or grant is revocable policy state, not a bearer capability captured forever in `StepSnapshot`. Every use must live-revalidate the current grant, policy, target revisions, disclosure scope, and budget. Budget and conflict-lock reservations must be atomic so concurrent steps or Subruns cannot each spend the same remaining allowance or begin incompatible effects after independently passing a stale check.

### StoryOS redesign

Model domain capabilities, not primarily paths and network booleans. Candidate capability families include:

- read a specified project/artifact/revision;
- query local indexes or graph projections;
- create a typed candidate artifact or prose proposal;
- request StoryOS Core validation of an exact Proposal Revision;
- use a named external model/service/MCP server/tool;
- disclose specified data classes to a specified destination;
- start a subrun with a bounded budget and capability ceiling;
- respond to an event or schedule under an explicit trigger policy.

Every grant needs scope, issuer, subject run/step/tool, capability parameters, expiry, reuse policy, provenance, and revocation state. Child runs and tool calls receive the intersection of their request with the parent's ceiling; they can never enlarge it.

Run capabilities stop at reading, producing Artifacts or Proposals, requesting Core validation, and invoking separately approved external effects. They never authorize Acceptance or direct execution of authority-changing commands.

Authoritative creative mutation has only two Core-owned entry paths:

1. the author explicitly accepts selected pending Operations from one exact eligible Proposal Revision; the Acceptance handler live-revalidates eligibility, current target revisions, policy, permission, idempotency, and budget, atomically reserves applicable budget and target locks, then internally constructs and atomically executes the domain commands; or
2. the author performs a narrow, deterministic, immediately visible Direct Author Action against one exact target through the Core editor boundary.

Use an OS/process sandbox for untrusted MCP stdio servers, Skill scripts, converters, and other executable extensions. The sandbox should receive a derived least-privilege filesystem/network view; it should not be the representation of author authority.

### Reject / do not copy

- Codex's shell-centered approval modes and “known safe command” heuristics ([approval policy][codex-approval-policy]).
- Workspace-write or danger-full-access as product-level authority presets.
- The model deciding that approval is unnecessary merely by selecting an “on request” mode.
- Session-wide remembered approval without destination, argument, data-classification, revision, and expiry constraints.
- Treating denial as a reason to retry with broader ambient access.

### Code candidates

The permission-profile intersection algorithm is a possible *algorithmic reference*, especially its retention of constraining denies. The current code is filesystem-specific and should not become StoryOS's capability model. A clean StoryOS capability-lattice implementation is preferable; code reuse should be considered only for a small, isolated generic set/intersection utility after tests prove non-escalation.

## 5. MCP integration and MCP Apps

### What Codex demonstrates

`McpConnectionManager` owns clients by server, origin metadata, startup events, required-server validation, aggregated tools/resources, call routing, and shutdown ([manager role][codex-mcp-manager]). Server startup is concurrent and emits `starting`, `ready`, `cancelled`, or `failed`, followed by an aggregate completion event; required servers are validated together ([startup lifecycle][codex-mcp-startup]). Tool calls are routed by `(server, raw tool name)`, checked against the configured filter, and use a per-server timeout ([MCP call routing][codex-mcp-call]).

Codex preserves raw server/tool identities for protocol calls while generating deterministic, sanitized, collision-resistant model names under a length limit ([MCP names][codex-mcp-names]). It also snapshots the manager/config needed by a step, separates direct and deferred model exposure, and applies an extra visibility policy to Apps ([MCP exposure][codex-mcp-exposure]). MCP calls have start/end lifecycle events, an approval path, output bounds, and provenance metadata; only a reserved first-party Apps server is trusted for certain connector fields ([MCP approval and trust][codex-mcp-approval]).

### Adaptable pattern

- One lifecycle-owning manager per configured runtime boundary.
- Required versus optional servers.
- Explicit startup state and failure isolation.
- Stable raw identity plus deterministic model-facing aliases.
- A normal Tool Registry adapter rather than a second agent loop.
- Separate configured, connected, enabled, model-visible, and call-authorized states.
- Per-call timeout, cancellation, output caps, provenance, and lifecycle events.
- Immutable per-step snapshots of the exact server/tool/schema/config version used.

### StoryOS redesign

Treat every third-party MCP server, tool annotation, resource, and App payload as untrusted input. An annotation may inform policy but cannot grant capability or prove a tool is read-only. StoryOS policy is authoritative.

Each MCP-backed ToolSpec should record:

- server identity and configuration digest;
- transport and execution environment;
- raw and canonical tool names;
- schema digest and discovered-at revision;
- trust tier and publisher provenance;
- StoryOS effect classification and capability mapping;
- disclosure destination/data classes;
- timeout, output cap, retry/idempotency policy;
- whether the tool is direct, deferred, or disabled for this step.

Before a call, StoryOS should produce an outbound disclosure manifest containing only the approved fields/artifact slices. After a call, it should validate and bound results, store raw external evidence separately when necessary, and convert usable results into StoryOS-owned typed artifacts with provenance.

MCP Apps remain transcript-native views/controllers. They may render a typed artifact, collect structured input, or request a StoryOS command. They do not own canon, prose, plans, approvals, artifacts, or run state, and they do not replace editor-native proposal review.

### Reject / do not copy

- Reserved Codex Apps/ChatGPT connector identities as a trust shortcut.
- Model-visible-by-default behavior for tools without visibility metadata; Codex currently does this ([visibility default][codex-mcp-visibility]).
- Approval decisions based primarily on MCP `readOnlyHint`, `destructiveHint`, or `openWorldHint`.
- MCP resources as the authoritative novel store.
- Ambient cwd, host filesystem, host credentials, or broad transcript disclosure.
- Codex Apps OAuth/cache/policy code and connector-specific metadata.

### Code candidates

The strongest isolated candidate is the deterministic MCP name normalization/collision utility. Generic RMCP transport wrappers and manager lifecycle pieces may also be evaluated, but only if their dependency graph can be isolated from Codex Apps, ChatGPT authentication, Codex config, and Responses API types. The MCP approval and Apps layers should be redesigned, not copied.

## 6. Skills

### What Codex demonstrates

Codex bounds Skill traversal and metadata sizes ([discovery limits][codex-skill-discovery]), gathers roots from explicit scopes and the root-to-cwd project path ([Skill roots][codex-skill-roots]), skips hidden directories, records parse errors, and parses `SKILL.md` frontmatter plus optional interface/dependency/policy metadata ([discovery walker][codex-skill-walker], [skill parsing][codex-skill-parsing]). It renders a compact catalog under a budget of roughly 2% of the context window, with an 8,000-character fallback ([skill budget][codex-skill-budget]). The model sees name/description/source first, then must load the selected `SKILL.md` fully and only follow relevant references—an explicit progressive-disclosure protocol ([skill disclosure][codex-skill-disclosure]).

Explicit structured/path mentions are resolved before textual `$skill` mentions, with ambiguity checks ([skill selection][codex-skill-selection]). On invocation, the complete Skill body is loaded and injected as a structured fragment containing name, path, and content ([skill injection][codex-skill-injection]). Codex also prevents project config layers from enabling/disabling Skills; only user/session layers can do so ([skill config authority][codex-skill-config]).

### Adaptable pattern

- Bounded catalog metadata before full body loading.
- Explicit and semantic selection with an inspectable selection event.
- Full-body loading only after selection; targeted reference loading afterward.
- Validation, errors, and provenance surfaced instead of silently ignoring malformed user/project Skills.
- Dependencies declared as requirements, never treated as grants.
- User/session authority controls activation of project-provided instructions.

### StoryOS redesign

A Skill is a versioned instruction package, not a mutable file path. Its manifest should include:

- stable ID, semantic version, content digest, publisher/provenance, and trust tier;
- name, description, triggers, and compatible kernel versions;
- required and optional tools/capabilities;
- permitted context/artifact classes and disclosure constraints;
- expected typed outputs and failure contract;
- referenced templates/assets/scripts with individual digests;
- evaluation fixtures and safety/privacy declarations;
- implicit-invocation policy, defaulting to **off** for project and third-party Skills.

Installation, activation, selection, and invocation are separate durable events. A Skill may narrow behavior and request capabilities, but it cannot grant them, bypass author approval, register Acceptance handlers or Core-only authority-changing commands, or change the run's disclosure ceiling. Executable Skill scripts run as tools under the same policy and sandbox pipeline.

Record the exact Skill version/digest and every loaded resource in `StepSnapshot`; never identify the effective behavior only by a mutable local path.

### Reject / do not copy

- Implicit invocation defaulting to true; Codex does so when policy is absent ([implicit default][codex-skill-model]).
- Prompt text as the entire enforceable contract.
- Optional metadata that fails open for authority-relevant fields.
- Plugin/ChatGPT installation semantics as kernel concepts.
- Project-local Skill code gaining ambient project filesystem or network access.
- A Skill instructing the model to treat discovery as permission.

### Code candidates

Frontmatter parsing/validation, bounded discovery, catalog rendering, and explicit mention parsing are plausible isolated candidates. They still need a new StoryOS manifest, immutable package identity, digest verification, trust policy, capability mapping, and stricter failure behavior for security-relevant metadata.

## 7. Subagents as bounded Subruns

### What Codex demonstrates

Codex scopes one shared `AgentControl` and registry to a root thread tree, rather than globally, and tracks parent metadata, status, resource limits, and a shared rollout budget ([agent control][codex-agent-control]). The registry records canonical agent paths, parent depth, total spawn reservations, and depth limits ([agent registry][codex-agent-registry]). Spawn is a real child thread with inherited environment/policy state, an explicit parent edge, an initial message, and optional full or last-N-turn context fork ([agent spawn][codex-agent-spawn], [agent fork][codex-agent-fork]).

Its v2 messaging distinguishes queue-only messages from follow-up tasks that trigger a turn ([agent messaging][codex-agent-messaging]). Status derives from lifecycle events, and execution/residency limiters bound concurrently active or loaded children ([agent status][codex-agent-status], [agent limits][codex-agent-limits]). Parent-child edges are persisted, enabling later resume of child graphs ([agent graph][codex-agent-graph]).

### Adaptable pattern

- Explicit parent/child IDs and canonical tree paths.
- Atomic resource reservations that release on failed spawn.
- Maximum depth, total children, active concurrency, and budget slices.
- Narrow task messages and typed result delivery.
- Queue-only versus wake-and-run delivery semantics.
- Cancellation propagation and explicit terminal status.
- Durable parent-child edges even when the child model process is ephemeral.
- Context projection choices, never accidental inheritance.

### StoryOS redesign

Represent subagents as hierarchical `Subrun` records under an `AgentRun`. Each Subrun receives:

- one concrete objective and expected typed artifact/result;
- an explicit context bundle of artifact IDs/revisions, not a cloned transcript by default;
- a capability/disclosure ceiling equal to the intersection of its request and its parent grant;
- a budget slice and depth/concurrency limits;
- a conflict scope for any resources it may read or propose against;
- a parent mailbox for typed progress, questions, artifacts, and terminal results.

Subruns may research, analyze, evaluate, or generate candidates. They may not independently commit authoritative creative changes. A child result becomes evidence or a candidate Artifact; the parent may use it to derive a Proposal, while only the author may resolve selected pending Proposal Operations through Acceptance or Rejection.

Default context should be **none plus an explicit task bundle**, not full history. Last-N-turn or full-history projection may exist as deliberate policies, but the snapshot must record exactly what was inherited. Sensitive context and capabilities only narrow down the tree.

### Reject / do not copy

- Shared writable workspace as the coordination mechanism.
- Child authority inherited merely because the parent had ambient filesystem access.
- Forking the parent's provider transcript as the normal domain-state transfer.
- Model/nickname/service-tier options as kernel semantics.
- A child's final prose message as the only result record.
- Concurrency based only on a thread count; StoryOS must also constrain cost, disclosure, conflict keys, and authoritative effects.

### Code candidates

Canonical tree-path parsing, atomic spawn-slot reservation, status watching, and generic concurrency guards could be evaluated as small utilities. `AgentControl`, thread spawning, forking, residency, and Codex inter-agent messages are too coupled to Codex threads and rollouts for direct reuse.

## 8. Events, persistence, and crash recovery

### What Codex demonstrates

Codex exposes typed submissions (`Op`) and typed outbound `EventMsg` variants with correlation IDs, including turn start/complete/abort, approval requests, MCP/tool lifecycle, plan updates, and errors ([operations][codex-ops], [events][codex-events]). Its rollout is append-only JSONL containing session metadata, provider response items, inter-agent messages, compaction checkpoints, turn context, world state, and selected events ([rollout item][codex-rollout-item]).

The recorder exposes explicit `persist`, `flush`, and `shutdown` barriers ([rollout recorder][codex-rollout-recorder]); its writer buffers items in order, retains unwritten suffixes after I/O failure, and retries after reopening the file ([rollout writer][codex-rollout-writer]). Resume reconstructs model history, context baselines, world state, compaction windows, rollback, and incomplete/aborted turn boundaries by replaying rollout items ([reconstruction][codex-reconstruction]). Codex tests incomplete and interrupted histories explicitly ([recovery tests][codex-recovery-tests]).

The critical limitation for StoryOS is visible in `TurnState`: pending approvals, permission requests, user questions, MCP elicitations, and dynamic tool responses are in-memory one-shot senders and are cleared as waiters ([pending waiters][codex-pending-waiters]). The rollout can reconstruct model context, but it is not a durable workflow engine that can resume every awaiting external interaction exactly where it stopped.

### Adaptable pattern

- Typed command/event protocols with stable IDs and explicit lifecycle boundaries.
- Append-only evidence and versioned schemas.
- Persist-before-notify barriers for terminal or approval-sensitive events.
- Correlation and causation across model calls, tool calls, approvals, artifacts, and subruns.
- Recovery tests for every crash cut point, not only happy-path replay.
- Context/world-state snapshots as derived replay accelerators, never the authority themselves.

### StoryOS redesign

Use StoryOS-owned transactional persistence—consistent with the planned project-local SQLite plus content-addressed artifact store—as the authority. A recommended event envelope is:

```text
RunEventEnvelope {
  event_id, schema_version, project_id, run_id,
  turn_id?, step_id?, sequence,
  correlation_id, causation_id,
  occurred_at, actor, payload
}
```

Persist normalized event families such as:

- run created/started/paused/resumed/cancelled/completed/failed;
- plan created/revised and step status changed;
- step snapshot created;
- model request started/completed/failed with provider metadata and usage;
- tool proposed/authorized/approval-requested/started/completed/failed/outcome-unknown;
- approval requested/decided/expired/revoked;
- disclosure proposed/approved/performed;
- Artifact Revision created and Artifact Lifecycle Event recorded, including retention changes, with Supersession recorded as an explicit provenance relationship;
- Proposal Revision created/withdrawn/superseded/reopened and Proposal Operation resolution projected as `applied` or `rejected` against the exact resolving revision;
- `ValidationReceipt` recorded and the current Proposal validation projection updated; `AcceptanceReceipt` and `UndoAcceptanceReceipt` recorded for their exact Core attempts;
- `AuthoritativeCommit` recorded with its prior and resulting Authoritative Revisions and its Acceptance, Direct Author Action, or safe-compensation cause;
- subrun spawned/messaged/completed/cancelled;
- guardrail warning/tripped and recovery decision made.

Use a transactional outbox for replayable SSE: the state transition and outbound event commit together; clients acknowledge/continue from event sequence. Raw provider traffic may be stored as private diagnostic evidence under retention controls, but kernel recovery must depend on normalized StoryOS records.

Recovery policy must be effect-aware:

- if a step never reached execution, use its persisted snapshot to reconstruct the decision evidence, then live-revalidate and record a new execution attempt before doing work;
- retry safe/idempotent reads using the same idempotency key and attempt lineage;
- never blindly retry an external or authoritative write after an ambiguous crash;
- classify it `outcome_unknown`, reconcile with the destination or ask the author;
- restore pending approval/user-input states as durable records, not in-memory channels;
- validate that referenced Artifact Revisions, ToolSpecs, Skills, MCP schemas, capability grants, revocations, target revisions, policy, and budgets still match; otherwise create a visible replan/reapproval event;
- before Tool execution, disclosure, or Acceptance, atomically reserve current budget and conflict locks so concurrent recovery or Subruns cannot double-spend or race the same target.

### Reject / do not copy

- JSONL transcript as the only source of truth for runs and creative state.
- A giant provider-oriented event enum as the domain model.
- In-memory waiters for approvals or author questions.
- Replaying a model transcript as proof that an external effect did or did not occur.
- Storing hidden chain-of-thought as required audit data; persist decisions, inputs, outputs, rationale summaries, provenance, and effects instead.
- Silent history replacement during compaction. StoryOS's model context may be rebuilt, but the audit/event log remains append-only.

### Code candidates

The recorder's ordered buffering, retry-after-write-failure behavior, and flush-barrier tests are useful implementation references. Direct reuse is not recommended because StoryOS needs SQLite transactions, artifact references, an outbox, workflow projections, schema migration, and effect-aware recovery. Generic JSONL import/export may later reuse small helpers, but not as the authoritative store.

## 9. Test architecture to adapt

Codex's root guidance requires integration tests for agent-logic changes and snapshot coverage for user-visible UI changes ([Codex test guidance][codex-test-guidance]). The source contains representative public-boundary tests for:

- approval matrices and preservation of deny-read restrictions on escalation ([approval matrix][codex-approval-tests], [deny-preserving escalation][codex-deny-read-test]);
- Tool Registry aliases/hooks/lifecycle and router exposure/parallelism ([registry tests][codex-registry-tests], [router tests][codex-router-tests]);
- MCP replayable call correlation, approval annotations, trust boundaries, output truncation, and environment selection ([call correlation][codex-mcp-tests], [MCP approval tests][codex-mcp-annotation-tests], [MCP boundary tests][codex-mcp-boundary-tests]);
- root-to-cwd `AGENTS.md` composition, overrides, resume, and fork behavior ([AGENTS composition tests][codex-agents-tests], [AGENTS replay tests][codex-agents-replay-tests]);
- subagent limits, relative paths, messaging, depth, and resume ([path/message tests][codex-subagent-message-tests], [depth tests][codex-subagent-tests], [resume tests][codex-subagent-resume-tests]);
- rollout materialization, buffered-write retry, reconstruction, rollback, compaction, world state, and incomplete/aborted turns ([recorder tests][codex-recorder-tests], [reconstruction tests][codex-reconstruction-tests]).

StoryOS should reproduce the *test dimensions* through its own public contracts:

| Boundary | Required tests |
|---|---|
| General loop | tool-follow-up, final completion, steering, interrupt, pause/resume, no-progress guardrail, every budget limit, stale `StepSnapshot` revalidation |
| Plan/run/step | legal transition table, revision history, dependency frontier, replan rationale, restart at every state |
| Tool Registry | duplicate identity, schema compatibility, provider compilation, exposure != authorization, input rewrite audit, cancellation, conflict keys |
| Capability/approval | property tests for non-escalating intersection; grant scope/expiry/revocation; approval persistence; changed-risk reapproval; TOCTOU revocation/policy/target changes between decision and execution |
| Author authority | Agent/Tool/MCP cannot invoke authority-changing commands; Acceptance selects only pending Operations from one exact eligible Proposal Revision; TOCTOU changes are revalidated at Acceptance; Core commands, Receipts, and Authoritative Commit are atomic and idempotent; Direct Author Action stays narrow and Core-owned |
| MCP | required/optional startup, malicious annotations, name collisions, timeout/cancel, schema drift, minimal disclosure, bounded output, untrusted App requests |
| Skills | manifest/digest validation, catalog budget, explicit/implicit policy, dependency-without-grant, referenced-resource provenance, sandboxed script |
| Subruns | parentage, capability/budget narrowing, depth/concurrency, mailbox ordering, cancellation, crash/reload, no authoritative child writes |
| Persistence | fault injection before/after every side effect and event commit; deterministic projection rebuild; outbox replay; ambiguous-effect reconciliation; atomic budget/conflict-lock reservation under concurrent steps and Subruns |
| APIs/SSE | schema golden tests, cursor replay, duplicate delivery, reconnect, event ordering, migration/backward compatibility |
| `AGENTS.md` | root/nested scope, override precedence, budgets, provenance, source changes, separation from runtime project policy |

Add model-provider contract fakes rather than depending on live inference for deterministic kernel tests. Then add a smaller evaluation suite for probabilistic plan/tool-choice quality. State-machine and capability-lattice property tests should complement integration tests, not replace them.

## 10. `AGENTS.md` pattern and its boundary

### What Codex demonstrates

Codex finds the project root, walks root to cwd, loads one instruction file per directory, prefers `AGENTS.override.md`, stops at the root, caps total bytes, and records source path/environment/cwd provenance ([AGENTS discovery][codex-agents-discovery]). The prompt defines directory-tree scope, nested precedence, and direct system/developer/user precedence over repository instructions ([AGENTS semantics][codex-agents-semantics]). A small nested file in the TUI demonstrates the intended use: only subtree-specific state-machine documentation and checks, without duplicating the root contract ([nested example][codex-nested-agents]).

Codex also persists world-state/context baselines so changed instructions can be reinjected on resume rather than blindly duplicating them ([AGENTS replay tests][codex-agents-replay-tests]). The general pattern is valuable: effective instructions are scoped, bounded, attributable, and snapshot-aware.

### Adaptable pattern

- One concise root engineering contract.
- Nested files only for subtree-specific invariants, commands, or review requirements.
- Deterministic root-to-leaf scope and override precedence.
- Hard size limits and explicit source provenance.
- Persist effective instruction digests in build/test/agent-run evidence where they affect behavior.
- Integration tests for discovery, precedence, change, deletion, resume, and fork/replay.

### StoryOS redesign

Keep two instruction systems separate:

1. **Repository `AGENTS.md`** — contributor/engineering instructions for agents modifying the StoryOS source tree.
2. **StoryOS runtime project policy** — typed, versioned author/project policy governing novel-agent behavior, tools, disclosures, commands, and schedules.

Runtime agents must not interpret repository `AGENTS.md` as novel-domain authority. Conversely, prose inside a novel project must not act as engineering instructions or silently override kernel policy. Runtime project policy should be a StoryOS artifact with schema, revision, provenance, author ownership, and explicit activation.

The current StoryOS root `AGENTS.md` already follows the right pattern: it states global product invariants and engineering rules while requiring nested files to add only subtree-specific content ([StoryOS `AGENTS.md`](../../AGENTS.md#verification)).

### Reject / do not copy

- Hidden override files as a security boundary.
- Configurable fallback filenames for runtime authority.
- Unbounded or frequently changing instruction blobs in model context.
- Coding-agent autonomy language applied to novel-state mutation.
- Repository instructions, Skills, or project prose overriding system policy, capability grants, or author approval.

### Code candidates

Root-to-cwd discovery and provenance structs are small potential utilities, but StoryOS already has simple repository needs. Runtime policy must be implemented separately; copying `agents_md.rs` into the product would blur a crucial boundary.

## 11. Concrete code-reuse candidates and license gate

The pinned source is Apache-2.0. The license grants copyright/patent permissions subject to conditions, including carrying the license, marking modified files, retaining relevant notices, and propagating NOTICE attribution when applicable ([Apache grant][codex-license-grant], [redistribution conditions][codex-license-redistribution]). The upstream tree includes a NOTICE file identifying OpenAI Codex and third-party Ratatui-derived code ([NOTICE][codex-notice]). This section is an engineering gate, not legal advice.

| Candidate unit | Initial disposition | Required redesign/check before reuse |
|---|---|---|
| `ToolName` from `codex-rs/protocol`, basic JSON-schema helpers from `codex-rs/tools` | Evaluate separately | `ToolName` is only re-exported by `codex-rs/tools`; remove remaining Codex/provider coupling, add StoryOS naming/version rules, and record provenance |
| `ToolExposure` and executor/spec coupling | Evaluate as tiny isolated unit | Ensure exposure is never authorization; add effect/capability/disclosure metadata outside or within contract |
| MCP deterministic name normalization | Strongest candidate | Replace SHA-1 if project policy requires; property-test Unicode/byte limit/collisions; retain raw identity and provenance |
| Skill frontmatter parser and bounded walker | Evaluate | New strict manifest/version/digest/trust rules; no authority-relevant fail-open metadata |
| Skill mention parser/catalog budgeting | Evaluate | StoryOS trigger semantics, i18n, ambiguity, and explicit-invocation policy |
| Permission intersection helpers | Prefer clean-room redesign | Existing representation is filesystem-specific; prove lattice laws and deny preservation |
| Atomic spawn reservation/concurrency guard | Evaluate as generic utility | Add run budgets, disclosure ceiling, conflict keys, durable reservations |
| Rollout writer buffering/retry | Pattern only | StoryOS uses transactional DB/outbox; JSONL is export/diagnostic, not authority |
| Full loop, session, registry/router, MCP Apps, subagent control | Reject direct reuse | Deep Codex/provider/coding coupling and wrong domain authority model |

Before copying any implementation code:

1. identify the exact upstream file, line range, commit, authorship/NOTICE implications, and transitive dependencies;
2. show that the unit is architecturally isolated and smaller/safer than an independent implementation;
3. record a provenance note and license obligations in the StoryOS source/package;
4. rename and reshape the API around StoryOS concepts rather than preserving Codex semantics accidentally;
5. add characterization tests from upstream plus StoryOS contract, security, and boundary tests;
6. verify `.reference/**` remains outside Cargo workspace, builds, tests, packages, releases, and runtime;
7. obtain the project's explicit license/boundary review before landing the copy.

Until that checklist is complete, all implementation work should be cleanly derived from the documented patterns, not copied source.

## 12. Recommended foundation sequence

1. **Write kernel contracts first.** Define provider-neutral IDs, `AgentRun`, plan/step state machines, event envelopes, ToolSpec, capability grants, approvals, Artifacts, and the Core-only Acceptance and Direct Author Action boundaries.
2. **Build persistence and replay before a powerful loop.** SQLite transactions, content-addressed artifacts, outbox/SSE cursoring, schema migration, and crash-cut tests establish the source of truth.
3. **Implement the smallest general loop.** One model adapter, bounded context builder, immutable step snapshot, Tool Registry, no shell, and read/proposal tools only.
4. **Add authorization before extensions.** Capability intersection, disclosure manifests, durable approvals, effect-aware idempotency, and sandbox adapter.
5. **Add MCP through the Tool Registry.** Start with untrusted read-only research tools, deterministic identities, snapshots, timeouts, and output bounds; keep Apps non-authoritative.
6. **Add versioned Skills.** Immutable manifests/digests, progressive disclosure, explicit activation, and dependency-without-grant behavior.
7. **Add Subruns last among kernel primitives.** They multiply every persistence, budget, authority, disclosure, and recovery risk; require the same run machinery with a narrowed ceiling.
8. **Evaluate code reuse only after boundaries stabilize.** Start with MCP name normalization or small schema/name utilities, never the loop/runtime.

This sequence preserves the requested general-agent experience while making the first implementation a trustworthy substrate for later large-scale expansion.

## Source index

[codex-turn-loop]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn.rs#L128-L149
[codex-turn-context]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn_context.rs#L101-L146
[codex-step-snapshot]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn.rs#L224-L294
[codex-steering]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/codex_thread.rs#L284-L332
[codex-active-turn]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/state/turn.rs#L29-L83
[codex-stop-hooks]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn.rs#L372-L415
[codex-pre-terminal-flush]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tasks/mod.rs#L401-L430
[codex-task-completion]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tasks/mod.rs#L724-L803
[codex-task-abort]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tasks/mod.rs#L829-L907
[codex-compaction-comment]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn.rs#L338-L369
[codex-plan-types]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/plan_tool.rs#L6-L29
[codex-plan-handler]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/handlers/plan.rs#L62-L96
[codex-tool-executor]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tools/src/tool_executor.rs#L44-L69
[codex-tool-exposure]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tools/src/tool_executor.rs#L13-L42
[codex-tool-name]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/tool_name.rs#L1-L57
[codex-tool-router]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/router.rs#L28-L244
[codex-tool-plan]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/spec_plan.rs#L101-L201
[codex-tool-registry]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/registry.rs#L322-L590
[codex-tool-spec]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tools/src/tool_spec.rs#L13-L63
[codex-permission-profile]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/models.rs#L404-L476
[codex-permission-overlay]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/models.rs#L253-L276
[codex-permission-intersection]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/sandboxing/src/policy_transforms.rs#L125-L195
[codex-approval-orchestrator]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/orchestrator.rs#L143-L305
[codex-sandbox-escalation]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/sandboxing.rs#L250-L307
[codex-approval-policy]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L895-L954
[codex-mcp-manager]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/codex-mcp/src/connection_manager.rs#L1-L7
[codex-mcp-startup]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/codex-mcp/src/connection_manager.rs#L150-L366
[codex-mcp-call]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/codex-mcp/src/connection_manager.rs#L760-L796
[codex-mcp-names]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/codex-mcp/src/tools.rs#L108-L215
[codex-mcp-exposure]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/mcp_tool_exposure.rs#L13-L95
[codex-mcp-approval]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/mcp_tool_call.rs#L101-L357
[codex-mcp-visibility]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/codex-mcp/src/connection_manager.rs#L85-L111
[codex-skill-discovery]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/loader.rs#L130-L148
[codex-skill-roots]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/loader.rs#L242-L413
[codex-skill-walker]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/loader.rs#L503-L625
[codex-skill-parsing]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/loader.rs#L628-L724
[codex-skill-budget]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/render.rs#L18-L24
[codex-skill-disclosure]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/render.rs#L30-L46
[codex-skill-selection]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/injection.rs#L138-L205
[codex-skill-injection]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/injection.rs#L63-L116
[codex-skill-config]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/config_rules.rs#L30-L68
[codex-skill-model]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core-skills/src/model.rs#L14-L58
[codex-agent-control]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/control.rs#L88-L108
[codex-agent-registry]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/registry.rs#L16-L97
[codex-agent-spawn]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/control/spawn.rs#L230-L425
[codex-agent-fork]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/control/spawn.rs#L428-L582
[codex-agent-messaging]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/handlers/multi_agents_v2/message_tool.rs#L12-L129
[codex-agent-status]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/status.rs#L4-L28
[codex-agent-limits]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/control/execution.rs#L13-L115
[codex-agent-graph]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agent/control.rs#L682-L710
[codex-ops]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L523-L684
[codex-events]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L1264-L1436
[codex-rollout-item]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L3130-L3153
[codex-rollout-recorder]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/recorder.rs#L877-L931
[codex-rollout-writer]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/recorder.rs#L1545-L1654
[codex-reconstruction]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/rollout_reconstruction.rs#L112-L195
[codex-recovery-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/thread_manager_tests.rs#L1372-L1491
[codex-pending-waiters]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/state/turn.rs#L85-L130
[codex-test-guidance]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/AGENTS.md#L112-L130
[codex-approval-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/tests/suite/approvals.rs#L1801-L2009
[codex-deny-read-test]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/tests/suite/approvals.rs#L3521-L3617
[codex-registry-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/registry_tests.rs#L144-L451
[codex-router-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/router_tests.rs#L107-L458
[codex-mcp-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/mcp_tool_call_tests.rs#L138-L190
[codex-mcp-annotation-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/mcp_tool_call_tests.rs#L365-L420
[codex-mcp-boundary-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/mcp_tool_call_tests.rs#L1024-L1264
[codex-agents-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/tests/suite/agents_md.rs#L188-L318
[codex-agents-replay-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/tests/suite/agents_md.rs#L807-L1040
[codex-subagent-message-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/handlers/multi_agents_tests.rs#L1135-L1513
[codex-subagent-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/handlers/multi_agents_tests.rs#L2320-L2528
[codex-subagent-resume-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/handlers/multi_agents_tests.rs#L2802-L2881
[codex-recorder-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/recorder_tests.rs#L414-L594
[codex-reconstruction-tests]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/rollout_reconstruction_tests.rs#L1364-L1609
[codex-agents-discovery]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/agents_md.rs#L1-L235
[codex-agents-semantics]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/gpt_5_2_prompt.md#L17-L27
[codex-nested-agents]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tui/src/bottom_pane/AGENTS.md#L1-L12
[codex-license-grant]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/LICENSE#L66-L87
[codex-license-redistribution]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/LICENSE#L89-L128
[codex-notice]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/NOTICE#L1-L6
