# Glossary: Model Gateway

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Model Gateway**:
The sole StoryOS-owned boundary through which any RunStep invokes a configured external model API. It applies only an exact Project Scope-bound Model Route Decision that pins one current Project Model Use Binding and the separate admitting External Contract Compatibility Decision over that binding, requires fallback to produce a new Route Decision and revalidate both records, and never executes model-produced Tool requests; only a validated, persisted Agent Decision may derive ToolCalls for the Tool Gateway.
_Avoid_: Provider client, model SDK, Tool Gateway, direct provider call

**Model Provider Adapter**:
The Host-controlled protocol projection that preserves a Model Attempt's typed native items, item order, call correlation, provisional and terminal events, complete output, usage, failure evidence, and any required opaque replay data bound to its original destination and mapping. It cannot decide retryability, select or substitute a model, initiate fallback, execute StoryOS ToolCalls, grant authority, or become durable Run truth; a Provider is an Adapter choice rather than a kernel requirement. A Contract-Faithful Fake Destination has its own Model Provider Adapter, but the destination is not a Provider.
_Avoid_: Provider Adapter, provider-owned router, silent fallback, Tool executor

**Model Registration**:
The host-owned, versioned, globally reusable non-authorizing contract identity that binds one stable StoryOS model reference to an exact Model Provider Adapter, project-free Provider API or service surface, provider model identifier, and Model Capability Profile revision. It contains no Project data, Project enablement, Credential Reference, actual processor endpoint or account boundary, disclosure destination, grant, compatibility admission, or runtime use state. An opaque provider alias remains explicitly unverifiable, any contract or Adapter binding change creates a new Registration revision, and provider evidence that conflicts with an exact binding creates Model Failure rather than rewriting past Run evidence.
_Avoid_: Model name, provider alias, deployment name, capability tier, global Credential Reference, provider account

**Project Model Use Binding**:
The model-surface use of the single shared `ProjectExternalUseBindingRevision` contract: one immutable, exact Project Scope-bound binding that makes one active Model Registration revision eligible for compatibility evaluation and named project Purposes by composing the current Project Destination Grant or other owning use authorization, one Project Scope-bound Credential Reference binding when required, one already-established Processing Destination Identity with its exact current evidence revision, and non-widening hard bounds. It is not a second model-specific record or shape, does not resolve or create the Identity, and contains no External Contract Compatibility Decision: after the binding exists, a separate immutable Decision evaluates it together with its global Registration and Adapter. Every Model Operational Snapshot, Model Route Decision, Model Invocation, Model Attempt, and fallback pins and revalidates both records. Host defaults and project settings establish the binding without per-call author configuration; Bailian or any other Provider remains only an Adapter choice, and no credential identifier forms a global namespace.
_Avoid_: Model Registration, External Contract Compatibility Decision, global provider credential, Model Route Decision, per-call provider setting

**Model Registration Status**:
The durable Active, Quarantined, or Retired global contract-eligibility state of one exact Model Registration revision: only Active may be considered for a new Project Model Use Binding or Model Route Decision, Quarantined requires explicit Host revalidation, and Retired never returns to service. Status changes preserve past evidence; Project credential availability and dynamic route facts belong to a Project Scope-bound Model Operational Snapshot, while reintroducing a Retired contract requires a new Registration revision.
_Avoid_: Provider health, credential availability, model version, mutable Registration

**Model Capability Profile**:
The immutable, versioned, provider-neutral semantic envelope for one Model Registration, distinguishing modalities, context and output bounds, Responses transport, typed streaming, stored continuation, implicit and explicit cache, function calling, hosted tools, structured output, context editing, native compaction, response retrieval, cancellation, generation controls, usage, and their supported combinations and native or Host-compiled mappings. Public claims and exact-model validation remain distinct evidence; an unknown required capability makes the route ineligible, while current account availability belongs to the scoped Model Operational Snapshot.
_Avoid_: Provider model card, Model Operational Snapshot, benchmark score, availability state

**Model Continuation Binding**:
The immutable, non-authorizing Operational Record that associates a Provider continuation reference with its original Model Attempt, exact Project Scope and Project Conversation, Processing Destination Identity and evidence revision, Model Registration, Adapter mapping, and original project-use and compatibility evidence. Eligible reuse may span AgentRuns only within that Project Conversation after current admission; replacing the transport continuation does not itself replace conversation identity or durable history.
_Avoid_: Project Model Use Binding, Project Agent identity, AgentRun, conversation identity, authorization, shared project-wide Provider session

**Model Operational Snapshot**:
An immutable, attributable, exact Project Scope-bound point-in-time observation of one Project Model Use Binding, its global Model Registration, and the separate External Contract Compatibility Decision for that pair, including current Credential Reference binding availability when required, destination eligibility, provider health, rate-limit or quota state, latency, pricing reference, and other dynamic routing facts. Project-free provider observations may be shared only as non-authorizing inputs; the Snapshot repeats the exact binding and Decision before they can affect route eligibility. It may change current eligibility without changing the Registration or Model Capability Profile and never proves semantic capability.
_Avoid_: Model Capability Profile, Model Registration, durable model identity

**Model Routing Policy**:
The immutable, versioned Host rule set that deterministically filters exact Project Model Use Binding and Model Registration pairs only through their separate admitting External Contract Compatibility Decisions and hard Model Route Request requirements, then ranks eligible pairs by declared soft preferences with a stable tie-breaker. Models and providers cannot author it; benchmark or learned evidence must be explicit and versioned, while random or experimental routing requires a separately authorized policy rather than hidden selection.
_Avoid_: Model recommendation, provider router, mutable score, implicit experiment

**Model Route Request**:
The immutable, exact Project Scope-bound pre-sampling statement of hard model capabilities, context bounds, allowed provider and Outbound Disclosure destinations, budgets, and soft quality, latency, and cost preferences for one RunStep, assembled by the Host from its exact plan, Skills, author settings, policy, grants, and inputs. It names no executable model or credential, grants no authority, and exists before that RunStep's Agent Decision.
_Avoid_: Model name, prompt hint, Model Route Decision, model self-selection

**Model Route Decision**:
The immutable exact Project Scope-bound Host result that either selects one exact Model Registration revision together with one current Project Model Use Binding and its separate admitting External Contract Compatibility Decision for a Model Route Request or records that no eligible route exists. It binds the Model Routing Policy revision, complete evaluated candidate binding/compatibility set and reasons, Capability Profiles, Project Scope-bound Operational Snapshots, Credential binding generations when required, grants, budgets, and comparison evidence used. It precedes sampling, cannot be authored by a model, and every fallback requires a new Route Decision over the same hard requirements with a freshly revalidated use binding and compatibility Decision.
_Avoid_: Model suggestion, mutable route, provider fallback, load-balancer choice

**Model Route Override**:
An immutable root-AgentRun-scoped author setting captured as Automatic, Prefer an exact Model Registration revision, or Require that revision, and applied prospectively to every Model Route Request in the whole execution tree; descendant Subruns may only add narrower requirements. It never creates or selects a Project Model Use Binding and never bypasses capability, disclosure, compatibility, grant, budget, credential, or policy eligibility; Prefer may allow another Route Decision, while Require records no eligible route instead of falling back.
_Avoid_: Optional model ID, mutable active model, Capability Grant, provider fallback

**Model Fallback**:
The Host-controlled admission of a successor Model Attempt for the same Model Invocation using a different exact Model Registration revision, its exact Project Model Use Binding, and the separate admitting External Contract Compatibility Decision after a new Model Route Decision re-evaluates the unchanged Model Route Request. It never reuses the prior route's Credential, use binding, or compatibility Decision, never relaxes hard requirements, never repeats a Registration revision within one fallback chain, remains bounded by all Run budgets and overrides, and cannot be delegated to a provider router or SDK.
_Avoid_: Same-route retry, provider substitution, capability downgrade, fallback loop

**Model Invocation**:
The single logical exact Project Scope-bound request by one RunStep to obtain one Agent Decision under an immutable Model Route Request, owning the ordered Model Attempts and their exact Project Model Use Binding plus separate External Contract Compatibility Decision history, with derived aggregate outcome and usage. Provider completion terminates only its Attempt; the Invocation succeeds only when the Host validates and durably records one typed Agent Decision, and a later successful Attempt never erases earlier binding, compatibility, or execution evidence.
_Avoid_: Provider request, Model Attempt, model response blob, retry counter

**Model Attempt**:
The model-specific Execution Attempt durably established before one concrete provider submission under one exact Model Route Decision, repeating the exact Project Scope, Project Model Use Binding, separate External Contract Compatibility Decision, global Model Registration, Credential binding generation when required, actual Processing Destination Identity and current Identity evidence revision, and final destination admission evidence. It binds its request and disclosure evidence to the resulting stream, provider identifiers, partial output, usage, uncertainty, and terminal outcome. Retrying the same Registration appends an Attempt only after revalidating the same use binding and compatibility Decision; fallback requires a new Model Route Decision and the selected route's own binding and subsequent Decision. Outputs from separate Attempts are never silently concatenated.
_Avoid_: Model Invocation, provider retry counter, overwritten request, merged fallback response

**Model Attempt Request**:
The immutable provider-neutral effective request for one Model Attempt, binding its exact Project Scope, Project Model Use Binding, separate External Contract Compatibility Decision, Step Snapshot, Context Assembly Manifest, prompt and output contracts, Tool Exposure and ToolSpec digests, generation controls, streaming mode, output bounds, Model Route Decision, and parameter provenance or default state. Its Adapter projection records the mapping and wire-request digests without silently changing required semantics; ordinary retry preserves the semantic digest, binding, and Decision after live revalidation, repair creates a new Request, and fallback changes the route/binding/Decision and may change only the provider projection while preserving the logical request.
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
