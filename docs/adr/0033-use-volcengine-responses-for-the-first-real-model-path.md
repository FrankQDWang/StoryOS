---
status: accepted
---

# Use Volcengine Responses for the First Real Model Path

On 2026-09-13, the author selected Volcengine Agent Plan Responses as the
first real-model direction. The author confirmed the decisions below.
They aim to use Provider services for ordinary model-session work and keep
StoryOS responsible for the novel and the author's decisions.

This ADR records the accepted vendor direction and Model Gateway contract.
The author also confirmed Codex CLI as the primary design reference for
ordinary user steering and context compaction. The Tool boundary is accepted
in ADR 0034 and background Project Memory in ADR 0035. Context, trust, protocol,
storage, retention, release, and proof alignment remains open. This decision
does not complete the ADR 0005 revision or start Stage 3 implementation.

## Confirmed decisions

1. Use the Provider for ordinary model-session continuation and caching.
   StoryOS retains Authoritative State, Proposal review, and durable business
   recovery records. Use native Responses capabilities in the first real
   path after validation at the selected endpoint and model.
2. Add Provider-hosted tools in Stage 5. StoryOS authorizes a bounded operation
   before submission, including its permitted outbound content and tool scope,
   and checks the returned result. StoryOS does not claim to inspect each
   Provider-internal step before that step consumes a result.
3. Retain future support for OpenAI Responses and Anthropic-format APIs.
   Their implementations are deferred. On a Provider change, keep StoryOS
   conversation and Project records and build eligible model context for the
   new destination. Do not promise lossless transfer of Provider-held state.
   Make capability differences inspectable.
4. After confirmed continuation expiry or unavailability, automatically
   rebuild eligible context when all prior submissions are settled and the
   existing authorization and budget cover the new submission. Keep the same
   author-visible conversation and the same Provider and processing boundary.
   Use a new Context Assembly for the current instructions, Working Target,
   and required retained history. Show a lightweight recovery status without
   a routine confirmation prompt. Missing required content, changed
   permissions, or insufficient budget stops this automatic path with a clear
   reason. Preserve the prior evidence; do not claim lossless recovery of
   Provider-held state or treat an unknown create outcome as expired context.

   A changed Effective Model Context requires a new RunStep and Model
   Invocation. The next RunStep may occur within the same active AgentRun and
   conversation turn. A terminal AgentRun stays terminal; further work uses
   a new causally linked AgentRun. Source eligibility remains with the Context
   owner.
5. After an unknown create outcome, first try to retrieve the original result
   when a retained response reference and the validated capability permit it.
   If the outcome remains unknown, permit at most one automatic additional
   Model Attempt for that interrupted Invocation. The same route, request,
   and current authorization must permit another submission, the budget must
   cover both Attempts, and there must be no unresolved Tool or hosted-operation
   effect. Record a Recovery Decision and fresh Attempt and disclosure evidence
   before submission. Run Cancellation prohibits this automatic successor.

   Persist a fence before the successor starts so the predecessor can no
   longer supply an Agent Decision or advance the active continuation chain.
   Its late result may reconcile evidence and usage, but cannot execute a Tool,
   replace the successor's answer, or enter normal conversation context.
   Keep the predecessor's outcome and usage unknown until evidence settles
   them; retain its required worst-case Budget Reservation. Restart or repeated
   recovery cannot reset the one-successor allowance. If the conditions fail
   or that allowance is spent, pause with an inspectable reason instead of
   making another automatic submission.
6. Give each Project Conversation its own Provider continuation chain. Later
   AgentRuns in the same conversation may reuse an eligible reference; Run
   completion alone does not reset that chain. A new conversation starts a
   new chain from currently eligible project sources and never attaches to
   another conversation's Provider reference. Project Agent identity stays
   unchanged. Cross-conversation continuity uses StoryOS project sources and
   background-generated Project Memory under ADR 0035, not a shared Provider
   session.

## Model Gateway contract

This section applies the author-confirmed decisions to the existing Model
owner. ADR 0034 owns hosted execution; the Context owner retains its pending
source, manifest, and result-intake revisions.

### Conversation history and active context

StoryOS owns the ordered conversation records and the application-held input
used for each decision. Append ordinary author corrections and changes of mind
as Messages. The Agent interprets them with its available context. New source
reads use currently available, authorized sources; a source edit or ordinary
deletion does not rewrite content already recorded in the conversation.
Do not classify ordinary natural language into a semantic exclusion registry,
derived-removal graph, or automatic continuation reset. No complete formal
intent or routine confirmation is required for ordinary assistance.

The active model context is a bounded selection of retained conversation
items, current instructions, current Working Target, and retrieved material.
General window limits, tool-output bounds, and compaction may change this
selection between model calls, including calls within one conversation turn.
Compaction uses general task, progress, decision, constraint, and pending-work
guidance. It does not promise exact semantic preservation, precise forgetting,
or removal of a past source's influence. Keep lossy and unknown content facts
inspectable; do not present a generated summary as original history.

Install a compaction result only for a later request. Preserve the submitted
Attempt, its Step Snapshot, input and wire evidence, and the original Messages,
Tool results, and Run Events under their owning retention contracts. This is
active-window management, not Operational History Compaction or deletion of
durable records. Any extra model request used for compaction must pass the
existing model and Context admission and evidence boundaries; it cannot be an
unrecorded Adapter request or hidden retry. The Context owner defines the
compaction record and projection details.

Each new decision boundary supplies the required current instructions and
exact Working Target under the validated profile. Compaction or a continuation
reference cannot silently replace them with stale summary text. Preserve valid
native tool-call/result correlation in the selected input. Do not drop or
reorder pending work in a way that invents a settled Tool result. Background
Memory extraction and consolidation are separate from this active context.

### Continuation identity and current admission

Use a Model Continuation Binding to retain the association between one
original Model Attempt and its Provider reference. Bind the exact Project
Scope, Project Conversation, Processing Destination Identity and its evidence
revision, Model Registration and Adapter mapping, and original Project Model
Use Binding and External Contract Compatibility Decision. Keep each original
association immutable when a successor response supplies a new reference.
Project Conversation names the stable grouping of author-visible Messages;
it may span AgentRuns, and its identity does not come from a Provider reference.

Only the complete output selected for its Model Invocation may advance the
ordinary continuation chain, and only after its Agent Decision passes full
validation and is durable. A response identifier or Provider completion alone
cannot advance the chain. Incomplete, rejected, unselected, cancelled, or
otherwise fenced results retain their references for evidence and permitted
retrieval only. Model Repair Attempt may add its existing bounded Host-generated
validation diagnostics; it cannot use a rejected response reference to admit
that response's whole Provider-held context into ordinary continuation.

Each new Attempt records the prior association it uses and obtains current
admission under its own exact use binding and compatibility Decision. The
reference grants no authority, disclosure permission, budget, or eligibility.
A new top-level AgentRun binds its own Project Instruction and current grant,
target, and budget facts. It does not inherit these from a Provider session.
Keep Project Agent identity, conversation identity, Run lifecycle, Model
Invocation, and Provider continuation distinct.

Normal continuation submits the new input required by the current decision
and validated profile. StoryOS retains its application-held input or compaction
record, actual sent content, source associations, received items, and known
prior references. A request delta is not the sole conversation or replay record.
Record Provider-reported processing separately from Host observations and opaque
internal state. Known references do not prove exact internal content, attention,
or lossless reconstruction. Current source access and disclosure admission
remain the Context owner's boundary; a retained reference bypasses neither.

Use a delta and prior reference only when the validated Adapter mapping can
represent the current request correctly. Otherwise submit the permitted full
input or establish a new transport continuation under current admission.
Compaction or a changed request shape can require this transport change without
resetting the author-visible conversation. Do not impose Codex's exact prefix
or parameter rules on another Provider; validate its own continuation contract.

A changed processing destination, account boundary, model Registration, or
Adapter mapping cannot silently reuse a prior binding. Establish the current
use and compatibility records and rebuild eligible context under the accepted
Provider-change or recovery boundary. A credential change alone does not
prove an unchanged account boundary. Actual access revocation, Project
Isolation, destination authorization, and deletion or redaction rules that
apply to retained copies must be enforced by StoryOS. Stop use of an opaque
reference when those restrictions cannot be enforced, and admit any rebuilt
input separately. These operational controls do not depend on the Agent
recognizing a natural-language intent. They require no claim that all semantic
influence can be traced or removed.

Ordinary non-use guidance, source edits, or loss of a source for new reads do
not by themselves invalidate the conversation chain. A missing required current
input can still prevent a new decision. An explicit conversation reset stops
reuse of its prior continuation; it does not erase retained history or recall
past disclosure. An expired or unusable handle follows the confirmed recovery
policy, not a semantic exclusion policy.

### Native output and complete validation

Preserve typed output items and their order, native item identifiers, roles
and phases where present, text, declared reasoning summaries, function names
and arguments, tool-call identifiers and their result correlation, refusals,
Provider-hosted result items, and usage evidence. Do not flatten these into
one text string or confuse an item identifier with a tool-call identifier.
Retain required opaque replay data only under its original destination and
Adapter mapping. It is not inspectable reasoning, authorization, or portable
cross-Provider history. Do not store hidden chain-of-thought as Agent rationale.

Normalize observed events into the ordered Model Stream Event sequence while
preserving native correlation and whether an event is provisional or terminal.
Keep completed, incomplete, failed, cancelled where supported, and unknown
outcomes distinct. A dropped stream is not a completed answer or confirmed
Provider cancellation. Retrieval of a full response is not resumed streaming.

Ordinary author discussion may use native text output. A Provider need not
emit one large JSON object for every interaction. The Host maps a complete
result to the existing typed Agent Decision and validates the whole candidate
before it becomes durable decision truth. A Provider completion ends its
Attempt, not its Invocation or AgentRun. Refusal is a completed semantic
result; it is not automatic retry permission.

Provisional text can appear as in-progress output. Partial function arguments
cannot create ToolCalls, and incomplete output cannot silently enter normal
model history. The complete requested business-tool batch must pass current
validation; an invalid member rejects that batch. Only then may the Host
derive independently authorized StoryOS ToolCalls. A Provider-hosted item is
evidence of the separately authorized hosted operation, not proof that the
Host admitted or inspected each invisible internal step. The Tool owner owns
that operation's scope and authorization; the Context owner owns result intake.

### Capability evidence and combinations

Keep Responses transport, stored continuation, implicit cache, explicit cache,
function calling, hosted tools, structured output, context editing, native
compaction, response retrieval, and cancellation as separate capabilities.
Record their supported combinations,
limits, native or Host-compiled mapping, and attributable evidence. Streaming,
modalities, generation controls, and reportable usage retain their existing
capability duties.

A public API claim is not exact-model validation. Exact-model validation is
not proof of current account or product-route availability. Globally reusable
Registration and Capability Profile evidence stays free of Project data,
credentials, and account-specific admission. Current scoped availability
belongs to the Model Operational Snapshot over the exact project-use and
compatibility records. Unknown required behavior blocks the affected route.

The Agent Plan Responses route and the general Ark Responses reference are
distinct evidence sources. In particular, do not assume a prior-response
reference carries current instructions, a stored response yields a cache hit,
or a Harness entitlement proves an in-Response hosted Tool capability.
Enable explicit cache only for a validated combination that preserves the
required instructions, Tool definitions, and output contract. A cache miss or
incompatible optional cache must not silently change those requirements.
Responses transport proves neither stored continuation nor native compaction.
Context editing does not prove a summary or compaction capability. A validated
Host-managed summary path may support general compaction without native support;
its model use still requires ordinary admission and bounded evidence. Unknown
required behavior blocks that path. Unknown usage is not zero usage, and cache
configuration alone proves no saving.

### Recovery and future Adapters

Use the confirmed expiry and unknown-outcome policies above. Each physical
resubmission has its own Attempt and applicable disclosure evidence. Disable
hidden SDK retries, Provider model fallback, and output splicing. If a client
cannot expose and obtain Host admission for every actual submission, it is
ineligible. Do not assume create idempotency from an API format or retry option.

Persist Model Attempt Cancellation before best-effort Provider abort. The
fence prevents the cancelled Attempt from supplying a decision or advancing
the active continuation chain even if a complete response arrives later.
Keep late results and usage as reconciliation evidence. Provider abort and
response retrieval require their own validated capabilities; absence of an
abort capability does not weaken the Host fence.

A future authorized Provider change keeps StoryOS Project and conversation
records and rebuilds eligible context at the new destination. It creates a
new Invocation at a lawful Run boundary rather than disguising changed
requirements as fallback. Fallback within one Invocation keeps its unchanged
hard Model Route Request and effective semantic request. Neither path imports
another Provider's opaque replay data or conceals capability differences.
OpenAI Responses, Anthropic-format Adapters, generalized routing, and their
implementations remain deferred.

## Pending downstream alignment

The Context owner must revise ADR 0005 and its normative source-closure,
compaction, cache, inspection, and result-intake rules to describe the actual
control boundary. It must distinguish new source reads from recorded history
and remove requirements for a semantic influence closure or ordinary-correction
chain reset. ADR 0034 already defines bounded hosted authorization and effect
evidence beside StoryOS ToolCalls. A hosted operation receives no implicit
Project context or manuscript authority.

The trust, protocol, storage, and retention owners then validate their affected
boundaries, including real revocation, deletion, and retained-copy controls.
The release and proof owners must align the existing stage specifications and
approved child graph before product implementation may resume.

## Plan placement

| Position | Required planning change |
| --- | --- |
| Before Stage 3 | Revise the original Model Gateway, Context Assembly, and Tool/MCP owners. Record endpoint capability evidence and unresolved account/model checks. Align affected release, proof, specification, and child-ticket contracts. |
| Stage 3 | Keep the real Host and fake destination. Exercise typed result mapping, ordinary steering, incremental and full-input continuation, target updates, compaction between calls, lost-response recovery, and cancellation fences. Fake proof makes no Provider capability or semantic-correctness claim. |
| Stage 4 | Implement one Agent Plan Responses path with validated continuation, streaming, caching behavior, response retrieval, and truthful usage evidence. Keep general Tool/MCP execution in Stage 5. |
| Stage 5 | Implement the accepted hosted-operation boundary beside StoryOS-executed tools. Validate function-call continuation, external content intake, research sources, approval, and uncertain effects. |
| Stage 7 | Use the background-generated Project Memory contract in ADR 0035. Project continuity and authoritative fiction facts remain separate from active context compaction and ordinary Provider continuation. |
| Later Provider work | Add OpenAI Responses and Anthropic-format Adapters with their own capability and recovery contracts. Rebuild eligible context at a newly authorized destination. |

## Ownership and evidence gate

The existing [Model Gateway owner](https://github.com/FrankQDWang/StoryOS/issues/50)
owns the model seam and continuation contract. The
[Context Assembly owner](https://github.com/FrankQDWang/StoryOS/issues/54) owns
the source and disclosure changes, including
[ADR 0005](0005-require-ordered-context-assembly-before-destination-disclosure.md).
The [Tool/MCP owner](https://github.com/FrankQDWang/StoryOS/issues/48) owns the
hosted execution boundary through ADR 0034. Complete pending revisions through
the existing serial tracker flow. This ADR is not a second specification for
their domains.

The inspected Codex snapshot is `c9ef7eff005c3299a5a5f0004c34c6a3eedf2564`.
Its [turn loop](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/core/src/session/turn.rs)
records pending user input and permits compaction between calls. Its
[client](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/core/src/client.rs)
checks request compatibility before delta continuation and keeps logical
request evidence distinct from the transport delta. Its
[session](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/core/src/session/mod.rs)
installs compacted active history and records that transition. These are design
references, not copied code, runtime dependencies, or Agent Plan capability
evidence.

Official API documentation is design evidence, not account acceptance.
The dated [capability preflight](../research/agent-plan-responses-capability-preflight.md)
separates Agent Plan evidence from the general Ark API reference. In particular,
explicit cache has restrictions on instructions, later tool definitions, and
structured output. Enable it only for a validated combination; session
continuation remains a separate capability.
Before the real path is enabled, select and validate the exact Agent Plan
endpoint, account, model, permitted use, and spending bounds. Check combined
behavior for continuation, cache, instructions, structured output, tool use,
response expiry, and recovery. Cache configuration alone proves no cost saving.
Unknown required capability blocks that route; it does not authorize a silent
protocol downgrade. Current personal validation does not establish that the
same subscription may supply a future multi-user service.

The current [Stage 3 specification](https://github.com/FrankQDWang/StoryOS/issues/361)
and [Stage 4 specification](https://github.com/FrankQDWang/StoryOS/issues/362)
remain on EXECUTION HOLD. This decision resolves the Model owner only; it does
not claim completed downstream alignment, API acceptance, or a stage release.
