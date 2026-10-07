# Glossary: MCP App

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

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
