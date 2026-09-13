---
status: accepted
---

# Bound Provider-hosted Tool Operations

This contract belongs to
[Specify ToolSpec, Capability, Approval, and MCP Trust Semantics](https://github.com/FrankQDWang/StoryOS/issues/48).
It applies the accepted Model Gateway direction in
[ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md).
On 2026-09-13, the author confirmed search, reading, and temporary computation
as the first hosted scope, direct execution inside the current Run Grant, and
delivery of complete validated partial research results. Actual Tool behavior
determines approval configuration; a hypothetical approval-resume flow is not
a separate product restriction. This decision does not start implementation.

## Execution boundary

A StoryOS-dispatched ToolCall passes through the Tool Gateway. The Gateway
owns argument validation, effect derivation, current authorization, dispatch,
result validation, and durable outcomes. This remains true for a model,
generated program, MCP App, or Host caller. Caller route and execution carrier
remain independent axes.

A Provider-hosted Operation is the separately admitted, bounded hosted work
that one exact Model Attempt may cause. It covers the complete hosted Tool
set enabled in that Attempt, including the case in which the Provider uses
none of those Tools. It is an Operational Record, not a StoryOS ToolCall,
AgentRun, Agent Decision, or independent execution runtime. Internal Provider
steps do not create invented StoryOS ToolCalls or dispatch records.

The first hosted scope permits external search and reading, and temporary
computation with bounded scratch files inside the admitted Provider sandbox.
It excludes external business writes, messages, publication, irreversible
business effects, and direct writes to StoryOS Artifacts or Authoritative
State. Scratch files are not durable StoryOS results. Importing a returned
file remains a separate validated Host operation. Provider-internal retention
and billing remain subject to the existing accountability and budget contracts.

An external business write continues through a separately authorized
StoryOS ToolCall and its existing one-shot Approval boundary. Creative changes
continue through Proposal and author Acceptance. No approval of a hosted
operation can enable an excluded effect.

## Registration and exposure

Keep the existing separation between Tool Discovery Record, Tool Registration,
Project Tool Enablement, Tool Exposure, Capability Grant, and Approval.
Discovery and MCP annotations are untrusted observations. A Host-owned mapping
pins each permitted hosted capability to an exact Registration and StoryOS
ToolSpec. The ToolSpec owns its callable, intake, effect, execution, and result
contract. It does not contain mutable authorization or account state.

Global Registration revisions contain no Project data or Credential Reference.
Scoped Project External Use Bindings own credential and project-use bindings;
the separate External Contract Compatibility Decision owns compatibility
admission. A Provider alias, Tool name, model capability, enabled setting,
or previous successful call grants no hosted execution permission.

Exposure declares the exact hosted capability set available to one Model
Attempt. StoryOS admits that entire set before submitting the request because
hosted work may start before a model result returns. A later tool-use event
cannot retroactively authorize it. An unregistered, disabled, quarantined,
incompatible, or unauthorized hosted entry prevents that request from being
submitted with hosted execution enabled.

Current Agent Plan account and model support must be validated separately
from public general Ark documentation. The
[dated capability preflight](../research/agent-plan-responses-capability-preflight.md)
is public evidence only. This contract does not certify a Provider capability.
The [Tool authorization preflight](../research/volcengine-tool-authorization-preflight.md)
records the examined pre-submission controls and their evidence limits.

## Exact operation and admission

Before submission, record the operation's semantic request and input digest,
then bind at least these facts through existing typed records:

- trusted User and Project Scope, AgentRun, RunStep, Model Invocation, and
  exact Model Attempt;
- Model Registration, Adapter mapping, exact hosted Tool Registrations and
  ToolSpecs, project-use bindings, and separate compatibility Decisions;
- exact Processing Destination Identities and account boundaries, with the
  permitted hosted processors and external Tool destinations;
- the operation's purpose, explicit input or controlled references, source
  associations, data categories, and applicable intake contracts;
- the allowed Tool set, search or read resource scope, sandbox scope,
  permitted outward processing, and effect ceiling;
- policy and current Capability Grant revisions, exact required Approval,
  caller route, and current cancellation or revocation fence;
- applicable manifests, exact non-secret wire projection, Destination Attempt,
  and final Destination Attempt Admission Decision;
- budget reservations, execution bounds, and expected result, error, and
  provenance contracts.

These are semantic obligations, not new DTOs or database tables. The protocol
owner defines the versioned representation. The hosted operation refines its
owning Model Attempt and Destination Attempt; it does not add a second network
submission or double-count the same disclosure or usage. Each actual later
model submission has a fresh Model Attempt and its own hosted operation when
hosted execution is enabled. Retrieval for reconciliation has its own admitted
Destination Attempt and never re-executes the original operation.
A changed Effective Model Context requires a new RunStep and Model Invocation
under ADR 0033; a new Attempt cannot disguise that change as a retry.

Effective authority is the non-expanding intersection of project policy, the
current Run Grant, current use bindings, the registered effect ceilings, and
the exact operation. Revalidate it at the final local dispatch boundary.
An unchanged, covered operation requires no further author confirmation.
A Policy Decision cannot create permission, and prior exposure or a Provider
continuation reference cannot replace current admission.

## Input, outward processing, and bounds

Hosted execution receives no Ambient Context. Tool access is limited to the
explicitly admitted intake, including any deliberately referenced prior state.
Sharing a Provider with the main conversation gives a hosted Tool no right to
its Transcript, Project Instruction, Working Target, or other Project data.
If the selected Provider mode cannot restrict Tool access to the admitted
intake, that mode is ineligible for this operation. A separate bounded request
may be used when it meets the same policy and capability requirements.

The operation may let the Provider derive queries or intermediate results
inside the admitted boundary. StoryOS does not require advance knowledge of
every internal query value. It requires a validated way to bound the permitted
input, Tool set, outward processing, and effects. A prompt instruction or
untrusted read-only annotation alone is insufficient. Unobserved internal
query values and transfers remain unknown; final output proves neither their
exact content nor per-step Host admission.

Reserve finite worst-case headroom before work starts. The admitted profile
identifies its relevant cost, output, Tool-use, time, and resource ceilings,
which controls enforce them, and any finite maximum used for reservation.
Do not invent a Provider control or require a counter the Provider cannot
report. A local timeout bounds Host waiting, not remote execution or cost.
If a required effect or resource ceiling cannot be bounded, refuse that route.
Reconcile actual usage without charging one physical use to both the Model
Attempt and hosted operation; missing usage remains unknown.

The Context owner retains source eligibility, minimum-necessary projections,
manifest contents and order, affected continuation chains, and returned-result
intake. This contract supplies the operation boundary to that owner. It does
not revise ADR 0005 or permit a StoryOS-controlled processor to bypass the
existing separate admission of its own nested external dispatch.

## Approval contract change

Retain the two Approval kinds: Tool Approval and Destination Disclosure
Approval. Extend the semantic target of Tool Approval to an explicit choice
between one StoryOS ToolCall and one Provider-hosted Operation. Each choice
has its own complete request shape. A hosted target binds the exact operation,
all its permitted registrations, input and references, requested effects,
destinations, bounds, governing policy, and request digest.

The existing ToolCall-only shape cannot approve a hosted operation. The two
target forms cannot be substituted, inferred from missing fields, or treated
as a blanket Provider approval. This is a versioned contract change for the
protocol owner, not a third approval product or a permanent project setting.

For an operation outside the current Run Grant but inside project policy,
request author Tool Approval before submission. An explicit low-risk approval
may cover only that operation or a bounded remainder of the current Run,
as in the existing Tool Approval contract. High-risk disclosure stays one-shot.
Destination Disclosure Approval authorizes only its exact disclosure, never
the hosted execution. Check every applicable model and Tool disclosure path;
one user interaction may present required decisions together but cannot merge
their distinct authority or identities.

A changed bound input requires a new request and current admission. An already
effective grant may cover the new request; a previously bound one-shot approval
does not. Rejection or expiry grants nothing and does not trigger repeated
prompts for the same need. Permanent project policy changes use explicit
settings. The existing Proactive Trigger escalation policy remains in force.

Select the approval behavior from the actual Tool and Provider contract.
When the required Tool scope and authorization can be configured before
submission, use that mode after StoryOS admission. Do not impose a separate
approval-resume product restriction or build a generic continuation workflow
without a concrete selected Tool that needs it.

A Provider-issued approval request is a protocol request, not an author
decision. It grants nothing and cannot cause an automatic expansion of scope.
If a selected Tool requires such a continuation, validate its exact request,
correlation, current authority, submission evidence, and cancellation behavior
before enabling that path. Covered protocol decisions do not require a new
author prompt; new authority uses the applicable Tool Approval. Any concrete
gap in the Model or operation continuation contract returns to its existing
owner. A hidden Adapter callback cannot bypass admission or the complete
selected-result rule.

## Result and effect evidence

Keep three evidence classes distinct:

| Class | Facts it can establish |
| --- | --- |
| StoryOS-controlled | Exact admitted request, frozen bounds, local dispatch claim, bytes received, validation decisions, fences, and Host settlements. |
| Provider-reported | Reported tool names, item and call identities, execution events, sources, status, and usage, with their original association. |
| Unknown | Unobserved internal calls, query payloads, intermediate consumption, unreported effects, and unsettled usage or outcome. |

Record which permitted effects were not attempted, reported, confirmed by
applicable evidence, partially confirmed, or remain unknown. A completed model
response is not proof that every internal Tool succeeded or stayed inside the
intended boundary. Observed scope violations block further work and ordinary
result intake, retain evidence, and require the existing safety and drift
handling. A Provider report must not be relabelled as a Host observation.

Validate returned output, source association, size, and declared result shape.
Preserve native item and call correlation without inventing internal ToolCall
identities. Returned content is data, not instructions or authorization.
Only the selected complete Model Attempt whose whole Agent Decision validates
and becomes durable may supply ordinary continuation or subsequent business
ToolCalls. Each StoryOS ToolCall still receives its own admission. One invalid
member rejects the complete business-tool batch; partial arguments execute
nothing. A function-call output comes from the settled matching ToolCall and
uses its original native call correlation under the pinned Adapter mapping.
Delivering that output creates no second execution of the Tool.
Batch rejection does not undo hosted work that already occurred. Preserve its
disclosure, reported effects, unknown effects, and budget settlement separately.

An operation that reaches a known limit may
return an explicitly incomplete research scope. If the outer Model Attempt
still completes and its entire Agent Decision validates, preserve the complete,
validated sub-results with their sources and the unmet objective. This can be
an inspectable partial deliverable under the existing Artifact and Run rules;
it is not an assertion that the full research task succeeded.

Incomplete model streams and cancelled or fenced Attempts remain evidence only.
Do not promote late fragments into a new Artifact, splice them into a later
answer, or use a renamed partial result to bypass the selected-result rule.
An Artifact durably completed before cancellation retains its existing
lifecycle. The Context owner determines eligibility for any later use.

## Stop, cancellation, and recovery

Before dispatch, refusal, withdrawal of permission, drift, a changed input, or
missing required approval prevents submission. Preserve the refusal and any
prepared records. Proven failure before the local dispatch claim establishes
that the operation was not dispatched.

After dispatch, revocation, cancellation, drift, or a breached bound fences
new Host work and requests best-effort Provider stop when supported. StoryOS
does not promise immediate remote termination or rollback. Late events may
settle disclosure, effects, and usage; they cannot revive execution, provide a
new Agent Decision, or advance a fenced continuation chain.

A lost response, stream failure, timeout, or unconfirmed stop preserves
OutcomeUnknown and the required worst-case reservation. First use permitted
retrieval or other immutable reconciliation evidence. Do not automatically
rerun a hosted operation merely because its intended business scope was read-only.
ADR 0033's one automatic successor requires no unresolved Tool or hosted effect,
fresh current admission, and budget for both Attempts; cancellation prohibits
that successor. Provider retention or continuation expiry does not prove that
an unknown operation never ran.

Keep refusal, known failure, completed result with a limited objective, and
OutcomeUnknown distinct. Existing Run Finalization and recovery rules decide
the Run outcome. Provider completion alone never completes the AgentRun.

## Ownership and delivery

The operation, authority, effect, and recovery evidence fit the existing
Operational Record and retention families. Preserve pending operations and
OutcomeUnknown evidence under the current settlement and retention rules.
There is no new scheduler, workflow engine, Artifact kind, Receipt producer,
or Provider session as the only durable business record.

The Context owner aligns ADR 0005, manifests, continuation eligibility, and
result intake. The trust owner assesses the admitted boundary and residual
Provider uncertainty. The protocol owner versions the approval target and
operation representation. Release and proof owners align the existing Stage
specifications and verification gates. Exact Provider account qualification
remains separate; Stage 3 and later product implementation stays on EXECUTION
HOLD. This decision neither claims downstream completion nor releases Stage 5.
