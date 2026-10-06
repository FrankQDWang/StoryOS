# Glossary: Outbound disclosure, destination, and approval

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Outbound Disclosure**:
The transfer of Project Scope-bound information beyond the StoryOS Controlled Processing Boundary to a named External Processing Destination, including generated queries, excerpts, metadata, or Artifact content. Each StoryOS-controlled dispatch requires prior manifests and exact destination authority; bounded hosted outward processing uses the owning operation's admitted scope, with unobserved internal transfers remaining unknown rather than invented Host dispatch evidence.
_Avoid_: External tool call, network access, upload

**Context Processing Boundary**:
The Host-owned classification of context handling as StoryOS Host Internal Processing, a StoryOS Controlled Processing Destination, or an External Processing Destination, based on exact Registration, StoryOS operational control, enforced Project Isolation, and controlled data path rather than physical location, hostname, deployment topology, infrastructure ownership, or product name. Every actual destination is minimized and evidenced, while only the external class creates Outbound Disclosure.
_Avoid_: Localhost trust, network-only classification, provider brand

**StoryOS Controlled Processing Boundary**:
The deployment-independent, Host-enforced trust and isolation boundary containing StoryOS Host Internal Processing, StoryOS-controlled PostgreSQL persistence, and explicitly registered StoryOS Controlled Processing Destinations. A locally run development service and a later StoryOS-operated cloud service may both enforce this boundary; physical location, loopback transport, first-party branding, infrastructure ownership, or process ownership alone cannot place a processor inside it.
_Avoid_: Local machine boundary, cloud equals external, localhost trust, same account, trusted provider

**StoryOS Host Internal Processing**:
Context handling entirely within StoryOS Core's controlled assembly and domain boundary, such as source resolution, eligibility, selection, or deterministic Host projection. It introduces no separate processing destination or Outbound Disclosure.
_Avoid_: Local model call, separate Tool process, hidden external service

**StoryOS Controlled Processing Destination**:
A separately identified Tool process or other processor operated inside the StoryOS Controlled Processing Boundary under an exact Registration, enforced Project Isolation, and StoryOS-controlled implementation and data path. Its Destination Attempt receives a Destination Context Manifest and creates no Outbound Disclosure. Any downstream crossing of the controlled boundary is a separately identified External Processing Destination with its own complete Context Assembly, disclosure, and Destination Attempt evidence; the current model and embedding API destinations are not members of this class.
_Avoid_: StoryOS Host internal work, trusted by deployment location, external provider, configured API

**External Processing Destination**:
A named model Provider endpoint, remote or independently controlled MCP server, hosted Tool, embedding service, telemetry system, support system, or other processor outside the StoryOS Controlled Processing Boundary. Every submission to it is an Outbound Disclosure regardless of first-party naming, transport route, apparent localhost address, or whether the StoryOS service itself is deployed locally or in the cloud.
_Avoid_: Network request only, provider alias, localhost exemption

**External Provider Accountability Boundary**:
StoryOS controls whether an external dispatch is admitted and records the exact minimum-necessary prepared payload, Purpose, named destination, durable dispatch claim, owning Destination Attempt, and best-known submission certainty. It claims that information was sent only when immutable confirmation evidence establishes ConfirmedSubmitted; OutcomeUnknown remains conservative potential disclosure. StoryOS does not model, verify, or claim control over a provider's internal retention, training, logging, subprocessors, or later handling after transfer. Such provider-internal behavior remains outside StoryOS durable truth and unknown unless evidenced for some separate purpose, without becoming an execution or disclosure guarantee.
_Avoid_: Destination Data Handling Profile, ZDR as no disclosure, vendor compliance registry, provider promise as enforcement

**Destination Context Manifest**:
The immutable provider-neutral Operational Record describing the minimum-necessary input and referenced context for one admitted operation, with its Context Assembly Manifest, exact Scope, Purpose, destination identity/evidence, processing boundary, policy, and authorization requirements. It binds permitted hosted processing when applicable but proves neither internal Provider consumption nor actual submission; a later one-shot Approval binds the established Attempt without changing the Manifest.
_Avoid_: Context Assembly Manifest, provider request, Outbound Disclosure Event

**Outbound Disclosure Manifest**:
The immutable Operational Record specializing one Destination Context Manifest for one exact External Processing Destination, additionally binding its Project Scope, applicable outbound data categories, and disclosure policy. Identical currently eligible submissions under the same Project Scope may reference it after revalidation, but it is neither an actual transfer nor evidence that a prior Destination Attempt performed a later operation.
_Avoid_: Destination Context Manifest alone, Outbound Disclosure Event, reusable authorization, provider request log

**Effective Destination Context**:
The logical content, instructions, Tool contracts, and known prior state StoryOS prepares or intentionally references for one exact Destination Attempt. Evidence distinguishes exact input, known references, Provider reports, and opaque internal state without asserting complete internal reconstruction or model attention.
_Avoid_: Wire Payload Projection, request delta alone, Provider-internal replica

**Wire Payload Projection**:
The exact non-secret provider-, protocol-, and Adapter-specific application payload bytes, frames, fields, or access-controlled payload references prepared for one Destination Attempt, together with opaque Credential References or secret-injection slots, their mapping version, and a digest over non-secret material only. Credential values, credential-value digests, and credential-bearing transport-envelope bytes remain ephemeral and are never persisted as this Projection. It is bound to any local outbound dispatch through its Disclosure Event and is wire-form evidence rather than proof of destination receipt, the canonical semantic request, or the complete Effective Destination Context.
_Avoid_: Context Assembly Manifest, Effective Destination Context, provider payload as truth

**Outbound Disclosure Event**:
The immutable Operational Record transactionally created when an egress worker durably claims one admitted Destination Attempt at StoryOS's local outbound dispatch boundary, binding its exact Project Scope-bound Outbound Disclosure Manifest and already-persisted Wire Payload Projection before any external I/O is permitted. The Event begins as OutcomeUnknown conservative potential-disclosure evidence rather than a claim that the destination received bytes; later immutable confirmation evidence may settle the Destination Attempt as ConfirmedSubmitted without rewriting the Event. Every dispatch claim and redispatch has its own Event even when payload bytes and the Manifest are reused, while a failure proven to occur before the durable claim creates no Event. A crash after the claim remains OutcomeUnknown even if no bytes ultimately left, so an actual disclosure can never lack prior durable evidence.
_Avoid_: Outbound Disclosure Manifest, Destination Attempt, planned transfer, cache hit

**Processing Destination Identity**:
The immutable, Host-owned, exact Project Scope-bound, non-authorizing record of one actual processing and disclosure boundary, established independently from a project-free Registration service surface plus append-only versioned identity evidence and, only when needed to identify the actual account boundary, a Project Credential Binding used solely as non-authorizing evidence; it names the processor, endpoint, account boundary, control classification, and governing intake or disclosure boundary without containing a Project Destination Grant, external-use binding, compatibility Decision, or execution authority. A Registration, Adapter, serialization, model, or Credential revision may reuse the same Identity only when a current immutable evidence revision proves those actual boundaries are unchanged—credential locator similarity alone never proves the account—while any processor, endpoint, account, control, or intake/disclosure-boundary change creates a new Identity before new authorization and use records.
_Avoid_: Provider brand, SDK client, hostname alone, credential, shared vendor account, use binding as identity resolver

**Project Destination Grant**:
An author-owned, versioned project-policy Operational Record that enables one exact Processing Destination Identity for named ordinary Purposes, outbound data categories, and hard disclosure bounds under one Project Scope. Destination Attempts for model and embedding operations that remain inside the effective Grant proceed without individual confirmation but still cross all seven Context Assembly gates and create complete applicable Manifest and Destination Attempt evidence plus an Outbound Disclosure Event when external dispatch is durably claimed. Configuring a Credential Reference, discovering a destination, or having disclosed to it before grants nothing; a new or changed destination, Purpose, data category, or wider bound requires an explicit project-setting change or an exact Destination Disclosure Approval Wait before submission.
_Avoid_: Credential configured, Provider discovered, blanket vendor consent, per-call prompt, prior disclosure as permission

**Destination Attempt**:
The immutable Operational Record and execution evidence for one concrete planned execution or submission attempt to one exact Processing Destination Identity and current Identity evidence revision under one Destination Context Manifest, established durably before destination I/O and settled as pre-dispatch, dispatched, or outcome-uncertain with the applicable submission certainty, wire evidence, outcome, usage, and correlation facts. Its existence alone never proves dispatch. Every physical resend, retry, fallback, or destination change creates a new Destination Attempt; Model Attempt and destination-specific Tool or service attempt records refine this boundary rather than replacing it.
_Avoid_: Logical Invocation, prior Attempt reuse, Disclosure Manifest, SDK hidden retry

**Destination Attempt Admission Decision**:
The immutable fail-closed Host decision immediately before I/O, revalidating exact Scope, actual input and reference dependencies, retained-copy restrictions, new-source read permissions and applicable Memory settings, grants, required Approvals, destination identity/evidence, Registration, intake contract, policy, and budget. An invalid required dependency refuses submission and preserves prior evidence; ordinary source changes do not recursively invalidate recorded conversation history.
_Avoid_: Context Assembly Manifest, cached authorization, provider retry flag, post-send audit

**Capability Grant**:
A bounded authorization to request named operations over specified project resources, external destinations, data categories, budgets, and time. Effective authority is always the non-escalating intersection of the project policy ceiling, the current Run's Capability Grant, and the exact capability requested by a StoryOS ToolCall or Provider-hosted Operation; approval may narrow or extend a lower layer only within its parent boundary.
_Avoid_: Role, permission flag, discovered tool, model-visible tool

**Approval**:
An immutable author decision over one exact typed operational request, input digest, scope, and governing policy. The closed current kinds are Tool Approval and Destination Disclosure Approval; each binds its own complete request shape, grants nothing before the decision, and requires a new decision when a bound input changes. Permanent project policy changes occur only through explicit settings. Approval never performs Acceptance or changes Authoritative State.
_Avoid_: Permission flag, confirmation dialog, Acceptance, permanent project setting

**Tool Approval**:
The Approval kind with two distinct exact request targets: a StoryOS ToolCall bound to its ToolSpec, arguments, resolved targets, and Tool Effect Request, or a Provider-hosted Operation bound to its complete registered Tool set, intake, effects, destinations, and bounds under the governing policy and Project Scope. It may grant only that operation or a bounded remainder of the current Run; high-risk disclosure, external writes, and irreversible effects remain one-shot, and neither target authorizes the other.
_Avoid_: Destination Disclosure Approval, Tool Exposure, Capability Grant, Acceptance

**Destination Disclosure Approval**:
The one-shot Approval bound to an exact unsubmitted Destination Attempt, its Operation Requirement, destination, Purpose, outbound categories, bounds, manifests, actual input/reference dependencies, non-secret Wire Payload Projection, and governing policy. Final admission revalidates it; a changed approved request requires a new Decision, and no approval proves a semantic influence closure or grants hosted execution authority.
_Avoid_: Project Destination Grant, Tool Approval, blanket provider consent, prior disclosure

**Policy Decision**:
An immutable result of StoryOS deterministically evaluating a request against already-effective project policy and Capability Grants. It may authorize an in-scope request or deny it, but it can never create, extend, or replace a Capability Grant; new authority requires author Approval.
_Avoid_: Approval, policy-authored grant, implicit permission
