# Codex transcript substrate and the StoryOS MCP App host prototype

**Status:** research complete for Wayfinder prototype
[Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52),
as of 2026-07-16.

**Question:** StoryOS is about to validate a transcript-embedded MCP Apps host. What
does the pinned Codex source actually do with transcript persistence, replay,
interrupts, approvals, tool results, compaction, and reconnects—and which of those
patterns should the prototype borrow?

**Short answer:** yes,
[Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52)
is fundamentally a transcript-runtime experiment, not merely an iframe rendering
experiment. Codex's user-visible transcript is a projection over a selectively
durable rollout log. Model-visible history is a different projection over that log.
Live reconnect can combine persisted history with in-memory turn state and in-process
pending callbacks, but this does not provide crash-durable approval or elicitation
recovery. At the pinned commit, Codex carries MCP tool results and app-identifying
context into transcript items, but it does not contain the complete MCP Apps
iframe/bridge host required by the stable specification.

For StoryOS, the useful pattern is therefore **durable facts first, reconstructable
projections second**. The pattern to reject is treating Codex's rollout format,
transcript, live callback maps, iframe state, or an MCP App as authoritative StoryOS
state.

## Scope and source boundary

This report uses three deliberately separated kinds of evidence:

- **Codex source facts** come from StoryOS's read-only, commit-pinned
  `.reference/codex` checkout at
  [`1f0566d3f59298d1bb88820a0d35294f1eeb07ea`](https://github.com/openai/codex/tree/1f0566d3f59298d1bb88820a0d35294f1eeb07ea).
- **MCP Apps specification facts** come from the stable 2026-01-26 specification at
  commit
  [`cf87f2a2c2581b2bc45ff4848aac9fa7e106a576`](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx)
  and the official
  [MCP Apps overview](https://modelcontextprotocol.io/extensions/apps/overview).
- **StoryOS conclusions** are design inferences, not claims about either upstream.
  They are checked against [ADR-0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md),
  the [domain context](../../CONTEXT.md), and the already-audited
  [MCP Apps host obligations report](mcp-apps-host-obligations.md) and its
  [source audit](mcp-apps-host-obligations-source-audit.md).

This is not a general Codex architecture review. Forking and rollback are covered only
where they affect transcript or App replay. The existing host-obligations report
remains the detailed source for iframe security, bridge validation, permission, CSP,
and lifecycle requirements; this report does not duplicate that audit.

## The four layers Codex keeps distinct

The word “transcript” can obscure four different things in the pinned source:

```text
selectively durable RolloutItem JSONL
  ├─ core reconstruction → model-visible ResponseItem history
  └─ ThreadHistoryBuilder → Thread / Turn / ThreadItem → rendered transcript cells

live ActiveTurn + callback maps
  └─ temporary overlay used while the process and turn are still alive
```

1. **Rollout:** an append-only sequence of selected protocol items. It is the durable
   conversation/resume substrate, but its persistence policy intentionally excludes
   many live events.
2. **Model-visible history:** the `ResponseItem` sequence reconstructed for the model.
   Compaction can replace this history without rewriting the user's earlier rendered
   turns.
3. **Thread history:** a reducer projects persisted items into `Turn` and `ThreadItem`
   values for clients.
4. **Live turn state:** active tasks, partial activity, waiters, and server request
   callbacks held in memory while a turn is running.

Codex therefore does **not** have one transcript object that is simultaneously the
database, model context, workflow state machine, and UI. Its UI transcript is a read
projection. Its rollout is durable only for the categories its policy admits. Its
live callback maps fill gaps that disappear on process loss.

StoryOS must keep an additional separation that Codex does not model as its product
domain: Authoritative State, Core Artifacts, and Operational Records. Under ADR-0001,
the transcript can render and link these records, but it cannot collapse them into a
single authority-bearing log.

## Codex source facts

### 1. The rollout persists selected facts, not every event

The protocol's `RolloutItem` union includes session metadata, response items,
compaction checkpoints, turn context, world state, and event messages
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L3130-L3145)).
The persistence policy is explicit rather than accidental
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/policy.rs#L7-L19)).

For response items, persisted categories include messages, reasoning, shell/function
and custom tool calls and outputs, web searches, image generation, and compaction-related
items. Transient trigger/configuration items are excluded
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/policy.rs#L36-L57)).

For events, the important boundary is:

| Category | Durable in the relevant history mode | Normally transient |
|---|---|---|
| Turn boundary | turn started, completed, aborted | incremental item-start and delta events |
| Thread state | goal update, settings, rollback | session configuration chatter |
| Completed activity | terminal MCP/tool/web/image/reasoning activity, depending on history mode | MCP/tool begin and streaming progress |
| Human interaction | a terminal turn may later contain the consequence | approval request, permission request, user-input request, MCP elicitation request |
| Diagnostics | selected terminal records | error/warning and raw response activity |

The exact event policy is visible in
[`should_persist_event_msg`](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/policy.rs#L84-L175).
The default history mode at this pin is `Legacy`
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L693-L700)),
so a reader must not assume identical persisted event shapes across modes.

The session path deliberately persists an event according to this policy before
delivering it to subscribers
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/mod.rs#L1946-L1974)).
Conversation items likewise enter model history, are recorded to the rollout, and
then notify observers with stable IDs and turn association
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/mod.rs#L2766-L2846)).

The recorder retains an unwritten suffix after a failed append and only drains the
confirmed written prefix, allowing later reopen/retry rather than pretending the
whole batch committed
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout/src/recorder.rs#L1579-L1614)).
This is a useful durability technique, but it does not change the fact that excluded
events were never intended to be crash-recoverable from the rollout.

**Finding:** the rollout is authoritative for the retained conversation records it
contains; it is not authoritative for all live execution state. The displayed
transcript is not the rollout itself.

### 2. Model context and user transcript are separate projections

On resume, core reconstruction reverse-scans durable items, identifies turn and
compaction boundaries, applies rollback, and rebuilds model history plus current
context/world-state settings
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/rollout_reconstruction.rs#L112-L295),
[source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/rollout_reconstruction.rs#L317-L438)).
A compacted item can carry replacement history and window lineage
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/protocol.rs#L3165-L3181)).
When compaction occurs, the live model history is replaced and that checkpoint is
persisted
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/mod.rs#L3035-L3068)).

The client-facing path is different. `ThreadHistoryBuilder` is a reducer over
persisted rollout items and is shared by persisted replay and live resume
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/thread_history.rs#L234-L320)).
It creates turns and typed thread items rather than exposing raw rollout JSON. Raw
response items generally do not become visible transcript items by themselves
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/thread_history.rs#L438-L462)).
Compaction marks the affected turn but does not replace the already rendered sequence;
an explicit rollback event, by contrast, removes rolled-back turns from the
projection
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/thread_history.rs#L1197-L1320)).

The TUI names its entry point “Render persisted thread turns” and converts typed
thread items into transcript cells
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tui/src/thread_transcript.rs#L1-L50)).

**Finding:** context compaction is a model-history operation, not permission to
silently rewrite the user's inspectable transcript. Rollback is an explicit durable
semantic event. StoryOS should preserve this distinction.

### 3. Reconnect combines durable replay with ephemeral live state

While a thread is running, Codex keeps a summary of the active turn, pending
interrupts, and current turn history in process memory
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server/src/thread_state.rs#L69-L89)).
Live events update that current-turn history, which is reset after the terminal event
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server/src/thread_state.rs#L144-L154)).

When another connection resumes a still-running thread, the app server composes a
snapshot from persisted history plus the active in-memory turn, attaches the new
connection, emits the snapshot, and then replays pending server requests
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server/src/request_processors/thread_lifecycle.rs#L520-L699)).

Those pending approval/input/elicitation requests are not reconstructed from durable
records. Core turn state stores them as maps of one-shot senders
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/state/turn.rs#L85-L130)).
The app server separately keeps request callbacks in process memory and can resend
them to a newly connected client while that process remains alive
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server/src/outgoing_message.rs#L286-L370)).

**Finding:** Codex supports **client reconnect to a live process**, not complete
**process-crash recovery of pending decisions**. A replayed transcript can show prior
completed activity, but it cannot recreate a lost one-shot sender as an unresolved,
authoritatively answerable wait.

StoryOS already has the stronger concepts: durable Run Wait, Wait Resolution,
Approval, Run Event, and normalized current operational records. The prototype must
exercise those semantics rather than reproduce callback-map recovery.

### 4. Interrupts and terminal boundaries are explicit

Codex cancellation is cooperative first and forceful after a grace period. On a user
interrupt it records a model-visible interrupted marker, flushes it, then emits and
persists the terminal aborted boundary
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tasks/mod.rs#L829-L907)).
Normal completion also flushes the rollout before declaring the task complete
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tasks/mod.rs#L401-L430)).

This makes “the user requested interrupt,” “the call may still have an external
effect,” and “the turn reached an aborted terminal state” distinguishable events.
The transcript can project the terminal outcome without inventing a successful tool
result.

**Finding:** StoryOS should borrow persist-before-notify terminal ordering and an
explicit interrupted/aborted representation. It must additionally preserve an
“external effect unknown” outcome when an MCP server cannot prove cancellation.

### 5. MCP results enter the transcript as typed activity, not as an App runtime

Codex's internal MCP turn item records a stable item ID, server and tool names,
arguments, optional connector and App context, status, result or error, and duration
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/protocol/src/items.rs#L350-L389)).
The app-server protocol exposes a typed `McpToolCall` thread item and an `AppContext`
containing connector/link/resource/App/template/action identity
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/v2/item.rs#L301-L318),
[source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/v2/item.rs#L401-L408)).

The history reducer turns MCP begin/end events into an in-progress then
completed/failed transcript item, retaining content, structured content, metadata,
and App context in the completed result
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/thread_history.rs#L756-L837)).
Metadata is filtered by source; richer connector/App identity is trusted only for the
Codex Apps server path
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/mcp_tool_call.rs#L323-L358)).

What these records do **not** include is just as important: there is no exact HTML
resource snapshot or digest, negotiated bridge/protocol snapshot, effective CSP and
permissions, durable App instance identity, or reconstructable view state. The TUI
fallback renders an MCP call as a static status line, not an interactive View
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tui/src/thread_transcript.rs#L171-L180)).

A repository-wide audit at the pinned commit found no implementation tokens for the
stable MCP Apps iframe bridge and bootstrap lifecycle (`ui/initialize`, `AppBridge`,
`text/html;profile=mcp-app`, `sandbox-proxy-ready`, or `sandbox-resource-ready`).
This is an absence observation about this pin, not a claim about newer Codex clients
or hosted products. The source does include app-initiated MCP tool-call plumbing, but
that is not a complete View host.

**Finding:** pinned Codex is valuable evidence for transcript/activity association.
It is not an implementation to copy for the iframe, bridge, persisted View, and replay
obligations of
[Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52).

### 6. Fork and rollback preserve explicit history boundaries

Rollback appends and flushes a durable rollback marker rather than destructively
editing the existing file first
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/handlers.rs#L531-L563)).
Forking reads persisted history, optionally truncates the copied view at a selected
turn, and materializes a separate rollout for a persistent fork
([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server/src/request_processors/thread_processor.rs#L3434-L3492),
[source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server/src/request_processors/thread_processor.rs#L3542-L3608)).

For StoryOS App replay, the implication is narrow: a branch should reference the
exact pre-existing App View Artifact Revision it inherited. Later App actions create
new Tool Calls and records on that branch; they do not mutate the historical View or
its original tool result.

## Stable MCP Apps specification facts

The stable specification defines the host relationship but intentionally leaves the
durable transcript policy to host implementations:

- A tool declares a `ui://` resource; the host fetches and renders that resource in a
  sandboxed iframe. The App and host communicate over JSON-RPC messages, while the
  host controls which capabilities are actually granted
  ([official overview](https://modelcontextprotocol.io/extensions/apps/overview)).
- Resource discovery and resolution are part of the host protocol, not evidence that
  a View may become the product's data store
  ([stable spec, resource section](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L319-L402)).
- Initialization, tool input/result delivery, teardown, and related bridge lifecycle
  are explicit
  ([stable spec, lifecycle section](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1272-L1490)).
- A host must retain a non-interactive fallback when a View is unavailable or unsafe
  ([stable spec, fallback section](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1492-L1560)).
- Persistence and restoration of App state are explicitly deferred to host policy
  rather than standardized
  ([stable spec, persistence section](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1562-L1576)).

Thus “an App lives inside the conversation” describes placement and interaction. It
does not make the iframe DOM, browser storage, App process, or transcript the
authoritative store.

## StoryOS design conclusions

### Authority and durable identity

StoryOS should project transcript rows from its own durable records:

- A `Message` is a Core Artifact that references exact Artifact Revisions.
- An Agent-authored activity row references its `AgentRun`, `RunStep`, `ToolCall`, and
  terminal or waiting state.
- A transcript-embedded View references an exact `AppViewArtifact` revision.
- The `AppViewArtifact` holds the durable descriptor needed to reconstruct the View:
  fixed resource digest, negotiated protocol and effective capability snapshots,
  exact input/result revision references, host context, optional validated view
  state, and a static fallback.
- Each iframe/render instance is ephemeral and disposable. It is never the
  `AppViewArtifact` and never receives domain authority by being visible.

The prototype may use provisional field names, but it must preserve these identities
and references. A transcript item should never rely on “whatever resource the server
returns now” or “whatever state survived in this browser.”

### Reconnect and replay

Reconstruction should be deterministic from StoryOS-owned facts:

1. Load the Message/Run/Tool/Wait records and exact App View Artifact Revision.
2. Render a fresh sandbox and renegotiate only the allowed host bridge.
3. Deliver the already-recorded complete input, result, cancellation, or error.
4. Never rerun the original MCP tool merely to recreate the View.
5. If the resource snapshot is missing, invalid, or no longer allowed, render the
   stored static fallback and a diagnostic record.
6. If the original run is waiting, reconstruct a durable unresolved Run Wait and
   route any answer through the normal Wait Resolution/Approval contract—not a stale
   callback captured in transcript text.

Live streaming may be an overlay on this durable projection, as in Codex, but a
process reset must expose any unsupported gap instead of silently fabricating a
completed row.

### Bridge requests and author authority

An App-initiated bridge request is a new requested operation. It must enter StoryOS
through the same Tool Gateway, capability, disclosure, budget, Approval, and Run Event
boundaries as any other Tool Call. It cannot reinterpret the original tool result as
continuing authority.

If the request would change authoritative creative state, its outcome can at most
create an inspectable StoryOS Core Proposal. Only the author's explicit Acceptance
through the Core command path crosses the authority boundary. App view state may
change presentation, but it cannot mutate story truth.

### Compaction and branching

Model-context compaction may change what a later model sees. It must not rewrite or
coalesce the author-inspectable Message, ToolCall, App View Artifact, Approval, or Run
Event history. Branches inherit references to exact revisions; new actions create new
records. This prevents a replay from displaying a View assembled from different
inputs than the author originally inspected.

## Borrow, adapt, and reject

| Pattern considered | StoryOS decision | Reason |
|---|---|---|
| Explicit persistence policy | Borrow | Forces a reviewable durable/transient boundary. |
| Append/flush before terminal notification | Borrow | Prevents UI completion from outrunning recorded fact. |
| Typed tool activity with stable item/turn association | Borrow and strengthen | Add RunStep, Artifact Revision, App View Revision, disclosure, and authority links. |
| Separate model reconstruction and user transcript reducers | Borrow | Model context and author-inspectable history have different semantics. |
| In-progress live overlay on durable history | Adapt | Useful for reconnect, but it must degrade honestly after process loss. |
| Callback-map replay for approvals/input | Reject as recovery model | It survives client reconnect only while the process lives. StoryOS waits are durable. |
| Rollout as sufficient operational authority | Reject | It deliberately omits live facts; StoryOS has normalized Operational Records and Run Events. |
| Persisted MCP result plus resource URI as sufficient App replay state | Reject | Exact resource, protocol/capability, input/result, and fallback revisions are also required. |
| Model compaction rewriting inspectable transcript | Reject | It would erase provenance and violate author inspectability. |
| Iframe/App/browser state as product truth | Reject | The App is a sandboxed controller/view over StoryOS-owned typed artifacts. |

## Minimal prototype for [Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52)

The prototype should be a thin experimental harness, not the start of a production
runtime:

- a deterministic fake MCP server exposing one `ui://` resource, one read-only tool,
  and one App-initiated action that requires host mediation;
- a content-addressed resource fixture and static fallback;
- a small StoryOS-owned durable store containing the minimum Message, Run, ToolCall,
  Run Wait/Approval, Run Event, and App View Artifact records needed by the experiment;
- a host page with a sandboxed iframe, strict bridge validator, and a deliberate
  “kill host process / restart / reconnect” control;
- invocation counters and stable IDs so duplicate tool execution is observable;
- a transcript reducer that can rebuild the same author-visible rows from the durable
  store without the original iframe or MCP server.

The harness does not need production schemas, UI polish, multi-project scaling, or a
general plugin catalog. It does need real process-boundary tests; reloading a React
component is not evidence of crash recovery.

### Required experiment matrix

| Case | Setup and action | Success evidence | Failure signal |
|---|---|---|---|
| Completed offline replay | Complete a tool + View, stop the MCP server and host, restart host, open transcript | Fresh iframe receives exact recorded input/result and resource digest; invocation count remains 1; fallback also renders | Server is required, tool reruns, or resource/result changes |
| Live reconnect | Disconnect one client during a running call, reconnect while host lives | Persisted rows plus live overlay converge to one terminal ToolCall/View | Duplicate activity row, lost terminal result, or duplicate call |
| Crash during approval/wait | App action reaches unresolved Approval/Run Wait; kill host process; restart | Exact unresolved wait is reconstructed and can be resolved once through durable ID | Lost prompt, stale callback, or two accepted resolutions |
| Interrupt | Interrupt while tool is active; test cooperative success and unknown external cancellation | Intent, terminal state, and any unknown external effect are distinct; no fake success result | UI reports completion or hides uncertain effect |
| Result projection | Return text content, structured content, metadata, App context, and a View | Transcript links one ToolCall terminal result to one exact App View Artifact revision and fallback | DOM state is the only complete representation |
| Context compaction | Compact model-visible history, then reopen transcript | Author-visible Message/Tool/App/Approval history and exact revision links remain unchanged | Transcript rows disappear or App input changes |
| Branch replay | Fork after a completed App View, then invoke a new App action on the branch | Both branches inherit the same historical View revision; new action has a new ToolCall/Run lineage | Original View mutates or branch action leaks into source |
| Resource drift/missing resource | Change/remove the server's `ui://` response after completion | Stored digest is used; otherwise static fallback plus diagnostic appears | Host silently substitutes the new resource |
| Authoritative edit attempt | App requests a story mutation | Host produces a Proposal candidate only; state changes only after explicit author Acceptance | App or tool writes Authoritative State directly |
| Malformed/unauthorized bridge call | Send wrong-origin, malformed, replayed, or ungranted request | Request is denied, audited, and leaves domain state unchanged | Request reaches Tool Gateway as authorized or changes state |

### Success gate

[Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52)
can be closed as a successful risk-reduction prototype only if:

1. every matrix row has executable evidence, including at least one real host-process
   restart;
2. completed replay, reconnect, and branching produce no duplicate tool side effects;
3. rebuilding from durable records yields the same terminal transcript and exact
   artifact references without browser storage or a live MCP server;
4. an unresolved Approval/Run Wait survives process loss and resolves at most once;
5. model compaction changes model context without changing author-visible history;
6. missing/unsafe resources fail closed to a static, inspectable fallback;
7. every App-initiated operation is capability-checked and provenance-linked; and
8. no test path lets the iframe, MCP server, or transcript write Authoritative State.

The prototype fails its central question if any replay depends on DOM/heap/browser
storage, reruns the original tool, loses a wait after restart, silently fetches a
different resource, or lets the App bypass Proposal and Acceptance.

### Explicit non-goals

- production MCP Apps host architecture or final persisted schemas;
- a reusable browser framework, multi-window UI, or visual design polish;
- exhaustive reproduction of the host-obligations security matrix already specified
  in the prior report, beyond the critical sandbox/bridge cases above;
- adopting, embedding, wrapping, or depending on Codex runtime code;
- editor-native prose proposal/diff/accept-reject UX;
- general event-sourcing infrastructure, migration strategy, retention policy, or
  performance optimization;
- arbitrary third-party App compatibility or a production App marketplace.

## Recommendation for the Wayfinder ticket

Proceed with
[Validate a Transcript-Embedded MCP Apps Host](https://github.com/FrankQDWang/StoryOS/issues/52),
but phrase its implementation target precisely:

> Validate that a StoryOS-owned transcript projection can attach a reconstructable,
> sandboxed App View Artifact to durable Run/Tool/Wait facts; survive reconnect and
> process restart without rerunning the original tool; and mediate every App action
> without granting domain authority.

Do not frame the prototype as “port the Codex transcript” or “add an MCP iframe to a
chat row.” The pinned Codex source answers the substrate questions—selective durable
history, typed activity, separate projections, live reconnect overlays, explicit
terminal boundaries—and exposes the important recovery gap around in-memory waits.
The stable MCP Apps specification and StoryOS's own authority model supply the host
and domain boundaries that Codex at this pin does not implement.
