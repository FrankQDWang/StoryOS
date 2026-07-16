---
status: accepted
---

# Specify Transcript and MCP App Lifecycle Semantics

StoryOS represents a transcript-embedded MCP App as an immutable, reproducible View Descriptor plus disposable sandbox Instances. The App, iframe, MCP server, browser storage, and live bridge are never transcript truth or creative authority. Durable Artifacts and Operational Records preserve exact inputs, results, resources, negotiation, authorization, delivery, recovery, and fallback evidence, while every new App operation re-enters StoryOS through its normal Host, AgentRun, Tool Gateway, Proposal, and Acceptance boundaries.

This decision resolves [issue 53](https://github.com/FrankQDWang/StoryOS/issues/53). It refines the Artifact boundary in [ADR 0001](0001-separate-authoritative-state-artifacts-and-operational-records.md) without creating an App-specific workflow runtime.

## Identity and ownership

### App UI Resource

An `AppUIResourceId` is the pair of:

- one exact MCP server or connector registration identity; and
- one canonical `ui://` URI resolved only against that registration.

An immutable `AppUIResourceRevision` binds:

- exact HTML or blob bytes;
- MIME type;
- normalized security metadata, including requested CSP domains and browser permissions;
- content digest;
- server or connector version and trust identity; and
- fetch and validation evidence.

Changed bytes create a new Resource Revision. A changed server trust identity creates a new Resource identity even when the URI and bytes match. Equal digests may deduplicate physical storage but never collapse logical identity or provenance.

Server removal does not remove a retained Resource Revision. Explicit author-governed deletion may purge inert bytes, but StoryOS retains the digest, provenance, deletion record, and stage-appropriate Prepared Receipt or Terminal Static Fallback; every affected historical View then becomes permanently recovery-only.

### App View Artifact

An `AppViewArtifact` is a Core Artifact whose revisions form one linear history. It is a reproducible descriptor, not an iframe snapshot. Each revision references:

- one exact App UI Resource Revision;
- the selected MCP Apps protocol profile;
- the App View Capability Snapshot and authorized Host Context snapshot;
- exact Tool input references;
- optional schema-bound View state; and
- stage-specific recovery material.

It never contains a live DOM, JavaScript heap, browser storage, pending RPC callback, media handle, or App-owned authoritative state.

An `AppViewInstance` is a disposable rendering of one exact App View Artifact Revision. Reload, reconnect, branch replay, and concurrent windows each create a different Instance. An Instance never changes its bound revision, restores a prior runtime, or re-executes the originating ToolCall.

### Transcript binding

A Message references one exact App View Artifact Revision and never resolves a mutable latest revision. A visible Prepared Message is preserved. When a Terminal Revision becomes publishable, StoryOS creates a new Message that references it and explicitly replaces the Prepared Message in the default transcript projection; the prior Message and revision remain inspectable.

## Artifact stages and publication gates

An App View Artifact Revision has exactly one stage:

| Stage | Required durable content | Publication rule |
| --- | --- | --- |
| `Prepared` | exact UI Resource Revision, complete Tool input reference and digest, protocol and capability ceiling, Host Context snapshot, and App View Prepared Receipt | may create an Instance after the Receipt and revision are atomically durable |
| `Terminal` | all Prepared content plus exact result, cancellation, or error reference and App Static Fallback | may replace the Prepared Message only after the fallback is durable and valid |

The originating ToolCall may become terminal while an Instance remains pinned to the Prepared Revision. The Host delivers the terminal projection as an operational overlay and creates a new Terminal Revision; it never mutates or rebinds the running Instance. Future replay binds the Terminal Revision directly.

### Prepared receipt

The App View Prepared Receipt is a minimal immutable Operational Record, persisted once with the Prepared Revision. It binds the Resource Revision, originating ToolCall, creation time, exact input reference and digest, and safe pending status. It does not require an App renderer, schema renderer, sensitive input summary, or per-progress-update write.

After recovery, a trusted Host renderer may turn the typed Receipt into a one-line pending or interrupted card. Free text is presentation, not the durable contract. If the Receipt and Prepared Revision cannot be persisted, StoryOS does not create the Instance.

### Terminal static fallback

Every Terminal Revision has an immutable App Static Fallback persisted before Message publication. It contains:

- terminal status;
- bounded safe text;
- exact result, cancellation, or error references;
- source and ToolCall provenance; and
- optionally, a schema-valid typed presentation for a registered StoryOS renderer.

Only trusted StoryOS code renders the fallback. It runs no App HTML or script, opens no external URL, and does not promote `_meta` or `structuredContent` into model context. An unavailable typed renderer degrades to a bounded generic result, error, or empty-result card. Publication is blocked only until at least that safe generic representation is durable; a rich renderer is not a liveness dependency.

## Resource retention, eligibility, and execution admission

Storage retention and execution eligibility are orthogonal:

- retained and eligible: interactive replay may be considered;
- retained and ineligible: bytes remain inert evidence and the View is recovery-only;
- deleted: digest and provenance remain, but interactive replay is permanently unavailable.

Execution eligibility is a current, versioned Host determination over one exact Resource Revision. Trust changes, policy changes, security revocation, or author revocation advance an `eligibility_generation`. Loader validation is useful for early rejection and integrity evidence, but it never authorizes execution.

Immediately before any HTML, object URL, decoded frame, compiled representation, or other executable derivative crosses the sandbox or renderer boundary, the Host creates an App UI Execution Admission for one exact Instance. That one-shot admission atomically matches:

- Resource Revision and digest;
- current eligibility generation;
- current policy revision;
- target Instance identity; and
- enforced sandbox profile.

A stale generation fails closed. No browser-held Loader result or prior allow decision may bypass this admission. After a revocation commits, old-generation bridge requests and new derivative consumption fail immediately; active Instances become Terminal and the Host revokes their object URLs, media handles, listeners, and sandbox processes. Physical cache eviction may be asynchronous, but a stale entry cannot cross a consumer gate.

Derived data is disposable and keyed by Resource Revision, eligibility generation, derivative kind, and transformer version. Every executable or rendering consumer revalidates the generation. `WeakRef` may reduce retention or help locate disposable objects, but garbage-collector reachability is never the revocation mechanism or security proof.

The current prototype validates digest presence when projecting render mode and later sends browser-held HTML to the sandbox after `sandbox-proxy-ready`. It intentionally remains a design probe, not the production admission boundary. Production implementation must move the final admission to the trusted sink immediately before resource transfer.

## Instance lifecycle

An Instance progresses irreversibly:

```text
Created -> Initializing -> Ready -> Terminal
    \------------failure---------> Terminal
```

- `Created` binds the exact Artifact Revision and new Instance identity before runtime creation.
- `Initializing` performs sandbox setup and the `ui/initialize` / `ui/notifications/initialized` exchange. App requests are rejected until initialization completes.
- `Ready` permits ordered Tool data delivery and mediated App requests.
- `Terminal` never reopens. Reload or reconnect creates a new Instance.

Termination intent, teardown progress, clean or unclean shutdown, and terminal reason are orthogonal facts, not a `Closing` state. The exhaustive terminal-reason contract is `AuthorClosed | Replaced | InitializationFailed | ProtocolViolation | EligibilityRevoked | ResourceLimitExceeded | BridgeLost | HostShutdown | HostRecovery`; adding a reason is a versioned contract change.

The Host persists teardown intent, gives a cooperative teardown a bounded grace period, then removes listeners, revokes handles, destroys the sandbox, and abandons remaining per-Instance deliveries. Timeout or process loss records an unclean terminal outcome. Instance termination does not cancel an accepted App Action or its routed AgentRun.

## Negotiation and replay compatibility

The App View Capability Snapshot is the immutable Host capability ceiling bound to the Artifact Revision: supported protocol features and bridge methods plus resource-requested and maximum permitted sandbox permissions. The actual effective subset belongs to each Instance Negotiation. The Snapshot is historical compatibility evidence, not a Capability Grant.

Every Instance creates its own immutable Instance Negotiation record. Replay may safely remove optional capabilities but may never add a capability beyond the Artifact snapshot. A removed required capability, unsupported protocol, unsafe Resource Revision, integrity failure, sandbox weakening, stale eligibility generation, or failed initialization prevents the Instance from becoming Ready.

Replay follows this order:

1. resolve the exact Message and App View Artifact Revision;
2. load only the exact retained App UI Resource Revision, never a current server response;
3. evaluate current policy, integrity, protocol support, and an equal or stricter sandbox;
4. create a new Instance and perform a fresh Execution Admission at the rendering boundary;
5. perform a fresh protocol and capability negotiation;
6. persist the App Replay Compatibility Decision;
7. if compatible, create new per-Instance deliveries and enter Ready;
8. otherwise, terminate the Instance and render the Prepared Receipt or Terminal Static Fallback.

Replay never restores DOM, heap, storage, pending RPC promises, or live View state; refetches changed code; or invokes the originating ToolCall. A request for fresh data is a new visible App Action with new execution lineage.

## Ordered Tool data delivery

Every Instance receives its own durable delivery obligations:

- exactly one complete Tool input projection; and
- when the ToolCall is terminal, exactly one result, cancellation, or error projection.

Each delivery is `Pending`, `Dispatched`, or `Abandoned`. `Dispatched` means only that the Host issued the bridge send; the MCP Apps protocol supplies no receiver acknowledgement, so StoryOS never records `Received`.

The terminal delivery is ineligible until the complete-input delivery is `Dispatched`. If the ToolCall settles first, the terminal delivery remains durably Pending; it is not buffered only in Host memory. Dispatching input opens the gate and makes the terminal delivery eligible immediately.

If the Host crashes after bridge send but before recording `Dispatched`, the old Instance becomes unclean Terminal and its remaining deliveries become Abandoned. A replacement Instance gets a fresh ordered pair from durable Tool records. StoryOS does not retry into the old bridge or share a delivery identity across Instances.

## App bridge requests and authority

Bridge traffic has two classes:

- App Presentation Signals cover bounded, rate-limited presentation and lifecycle coordination such as size, display mode, initialization, and teardown. Ordinary signals are ephemeral; malformed, abusive, or security-relevant signals create durable diagnostics.
- App Action Requests cover every request with semantic meaning, data access, outbound disclosure, model-context impact, transcript impact, or other effect. The Host validates the bridge envelope, source window, origin, method, and payload before persisting the Request, and persists it before authorization or execution.

An App Action Request identity is scoped to `(AppViewInstanceId, bridge_request_id)`. An exact duplicate resolves to the same Request. Reusing the id with a different payload is a protocol violation. Another Instance always creates another Request.

The persisted App Action Routing Decision maps the Request to exactly one of:

- denial;
- a typed Host command;
- a normal transcript command or instance-scoped context contribution; or
- a new causally linked root AgentRun with a new Grant, budget, Approval, and Tool Gateway boundary.

The Request and Routing Decision are ingress and provenance records, not an executor. The App never inherits the originating Run's authority, even when that Run is still active. The historical capability snapshot grants nothing. There is no raw MCP client that auto-forwards bridge calls.

`tools/call`, `resources/read`, `ui/open-link`, and other external or effectful operations use new execution lineage. `ui/message` requires an author confirmation before the normal transcript command and is never represented as author speech merely because the App requested it. `ui/update-model-context` creates only a bounded, attributed, instance-scoped contribution; a future Run may include it only through normal ContextManifest policy. Creative-state changes can produce only a Core Proposal and require author Acceptance.

Accepted work continues under the routed record's lifecycle after the requesting Instance becomes Terminal. A separate App Action Response Delivery targets only the requesting Instance and is `Pending`, `Dispatched`, or `Abandoned`. Instance termination abandons an undelivered response without cancelling or retrying work, restoring a promise, or transferring a callback to another Instance. Durable results appear through their normal Artifact and Message paths.

## Durable commands and events

These are normative domain event names and one-to-one semantic transitions. Storage or protocol adapters may namespace their serialization, but they must not rename away, merge, or collapse these events into generic log strings.

| Command or decision | Durable event or record |
| --- | --- |
| prepare an App View | `AppViewPreparedReceiptRecorded`, `AppViewArtifactPreparedRevisionCreated` |
| publish a terminal View | `AppStaticFallbackPersisted`, `AppViewArtifactTerminalRevisionCreated`, `MessageReplacementRecorded` |
| change execution eligibility | `AppUIResourceExecutionEligibilityChanged` with new fencing generation |
| admit bytes or a derivative at the renderer | `AppUIExecutionAdmitted` or `AppUIExecutionDenied` |
| create and initialize an Instance | `AppViewInstanceCreated`, `AppViewInitializationStarted`, `AppViewInstanceNegotiated` |
| decide interactive replay | `AppReplayCompatibilityDecided` |
| enter Ready or Terminal | `AppViewInstanceReady`, `AppViewTerminationRequested`, `AppViewInstanceTerminated` |
| create or settle Tool data delivery | `AppViewDeliveryCreated`, `AppViewDeliveryDispatched`, `AppViewDeliveryAbandoned` |
| receive a semantic bridge request | `AppActionRequested` |
| route and settle an App action | `AppActionRouted`, `AppActionSettled` |
| create or settle an App response delivery | `AppActionResponseDeliveryCreated`, `AppActionResponseDispatched`, `AppActionResponseAbandoned` |
| invalidate resource derivatives | `AppUIDerivedDataInvalidated` with Resource Revision and prior generation |

Every event binds the owning identity, expected prior state or sequence, idempotency key, causation and correlation identities, timestamp, and controlled payload references. Secret-bearing data remains behind encrypted or redacted references rather than raw event payloads.

## Considered options

- Persisting or restoring an iframe, DOM, heap, browser storage, or callback map was rejected because runtime memory is neither durable truth nor safe replay state.
- Binding a Message to `ui://` or mutable latest resource was rejected because server drift would rewrite history and provenance.
- Treating a capability snapshot or originating Run Grant as future App authority was rejected because discovery and historical permission evidence are not authorization.
- Buffering an early terminal result only in Host memory was rejected because crash recovery could violate complete-input-before-terminal ordering.
- Requiring a rich static fallback before every Prepared Instance was rejected because Prepared needs only minimal recovery evidence; a full fallback is a Terminal publication invariant.
- Trusting a Loader eligibility check was rejected because revocation between load and render creates a time-of-check/time-of-use window.
- Using `WeakRef` as revocation was rejected because garbage collection does not fence stale consumption or invalidate strong references.
- Cancelling routed work when the iframe closes was rejected because durable execution ownership belongs to the routed Host command or AgentRun, not the View.

## Consequences

- Transcript history remains exact and inspectable through process loss, server drift, security revocation, and browser teardown.
- Interactive replay is available only while exact stored code remains eligible under an equal or stricter sandbox; static recovery remains usable after executable code becomes unavailable.
- The Host must add explicit Artifact, Instance, delivery, action, eligibility, admission, and fallback records instead of relying on an MCP SDK bridge or browser runtime.
- Resource revocation needs generation-fenced consumers and active-handle teardown, not cache eviction alone.
- A terminal tool outcome may be live in an existing Prepared Instance before its Terminal View Revision is publishable; the durable result remains truth and publication waits only for the mandatory safe fallback.
- App interactivity creates no second Agent runtime and no alternate path to Authoritative State.
