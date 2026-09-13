---
status: accepted
---

# Use Volcengine Responses for the First Real Model Path

On 2026-09-13, the author selected Volcengine Agent Plan Responses as the
first real-model direction. The author confirmed the decisions below.
They aim to use Provider services for ordinary model-session work and keep
StoryOS responsible for the novel and the author's decisions.

This ADR records the accepted vendor direction and Model Gateway contract.
The related Tool, Context, trust, protocol, release, and proof owners still
require their own revisions. This decision does not override ADR 0005 or
start Stage 3 implementation.

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
   Invocation. A terminal AgentRun stays terminal; further work uses a new
   causally linked AgentRun. Source eligibility remains with the Context owner.
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
   the later Project Memory contract, not a shared Provider session.

## Model Gateway contract

This section applies the author-confirmed decisions to the existing Model
owner. The Tool and Context owners retain their pending contract revisions.

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

Normal continuation submits new eligible input and the current instructions,
controls, and exact Working Target content required by the validated profile.
It need not resend the whole conversation or recreate Provider-internal state.
StoryOS retains sent content, source associations, received items, and known
prior references. It labels Provider-reported and unknown internal facts
separately. Current source eligibility and the required manifest sequence
remain the Context owner's boundary; a retained reference bypasses neither.

A changed processing destination, account boundary, model Registration, or
Adapter mapping cannot silently reuse a prior binding. Establish the current
use and compatibility records and rebuild eligible context under the accepted
Provider-change or recovery boundary. A credential change alone does not
prove an unchanged account boundary. Exclusion, revocation, deletion, or an
unverifiable dependency prevents use of the affected chain under the Context
owner's rules. Ordinary prose edits instead supply the current target version
at the next admitted decision boundary.

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

Keep stored continuation, implicit cache, explicit cache, function calling,
hosted tools, structured output, context editing, response retrieval, and
cancellation as separate capabilities. Record their supported combinations,
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
Context editing does not prove durable semantic compaction. Unknown usage
is not zero usage, and cache configuration alone proves no saving.

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

The Context owner must revise ADR 0005 and its normative source-closure and
result-intake rules to describe the actual control boundary. The Tool owner
must define bounded hosted authorization and effect evidence beside StoryOS
ToolCalls. A hosted operation receives no implicit Project context or manuscript
authority. Neither downstream decision is settled by this Model contract.

The trust and protocol owners then validate their affected boundaries. The
release and proof owners must align the existing stage specifications and
approved child graph before product implementation may resume.

## Plan placement

| Position | Required planning change |
| --- | --- |
| Before Stage 3 | Revise the original Model Gateway, Context Assembly, and Tool/MCP owners. Record endpoint capability evidence and unresolved account/model checks. Align affected release, proof, specification, and child-ticket contracts. |
| Stage 3 | Keep the real Host and fake destination. Exercise typed result mapping, incremental continuation, target updates, lost-response recovery, and cancellation fences in deterministic tests. Fake proof makes no Provider capability claim. |
| Stage 4 | Implement one Agent Plan Responses path with validated continuation, streaming, caching behavior, response retrieval, and truthful usage evidence. Keep general Tool/MCP execution in Stage 5. |
| Stage 5 | Implement the accepted hosted-operation boundary beside StoryOS-executed tools. Validate function-call continuation, external content intake, research sources, approval, and uncertain effects. |
| Stage 7 | Retain cross-thread Project continuity, authoritative fiction facts, and source-bearing Memory. These are separate from ordinary Provider session continuation. |
| Later Provider work | Add OpenAI Responses and Anthropic-format Adapters with their own capability and recovery contracts. Rebuild eligible context at a newly authorized destination. |

## Ownership and evidence gate

The existing [Model Gateway owner](https://github.com/FrankQDWang/StoryOS/issues/50)
owns the model seam and continuation contract. The
[Context Assembly owner](https://github.com/FrankQDWang/StoryOS/issues/54) owns
the source and disclosure changes, including
[ADR 0005](0005-require-ordered-context-assembly-before-destination-disclosure.md).
The [Tool/MCP owner](https://github.com/FrankQDWang/StoryOS/issues/48) owns the
hosted execution boundary. Complete their revisions through the existing
serial tracker flow. This ADR is not a second specification for their domains.

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
