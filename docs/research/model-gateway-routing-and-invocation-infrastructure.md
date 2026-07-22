# Model Gateway routing and invocation infrastructure: source research and StoryOS implications

- Status: research complete; incorporated into the accepted Wayfinder resolution, not implementation authorization
- Decision: [Specify ModelGateway and Model-Routing Semantics](https://github.com/FrankQDWang/StoryOS/issues/50) — [resolution](https://github.com/FrankQDWang/StoryOS/issues/50#issuecomment-4987328100)
- Reference baseline: [`openai/codex@1f0566d3f59298d1bb88820a0d35294f1eeb07ea`](https://github.com/openai/codex/tree/1f0566d3f59298d1bb88820a0d35294f1eeb07ea), pinned in read-only `.reference/codex`
- Web sources checked: official OpenAI, Anthropic, OpenTelemetry, Temporal, Google Cloud, and AWS documentation or first-party source, 2026-07-15
- Scope: the ticket boundary, model-call control-plane responsibilities, streaming and usage evidence, retries and fallback, and whether one logical invocation must be separated from concrete provider attempts

## Executive conclusion

Yes, this ticket is about **model selection/routing and the Host-owned infrastructure semantics that make a selected model callable, controllable, and inspectable**. It is deliberately broader than an algorithm that picks a model: the issue itself asks for the provider-neutral contract governing capabilities, per-step selection, streaming, tool calls, usage, failure, fallback, and run-level overrides. It is nevertheless a **contract-design ticket**, not authorization to implement provider clients, storage, UI, or a broad routing optimizer.

The evidence also strongly supports separating:

- a logical **Model Invocation**: one RunStep-level intent to obtain one Agent Decision under one Model Route Request; and
- ordered **Model Attempts**: concrete tries to submit that invocation to a provider under an exact Model Route Decision.

Retries, transport fallback, model fallback, provider request IDs, partial streams, usage, and outcome uncertainty all attach to attempts, not safely to the logical invocation as a single overwritten record. OpenAI's pinned Codex source already creates a fresh inference-trace attempt for every concrete request, preserves provider request IDs and partial output on failure or cancellation, and loops those attempts beneath one sampling request. OpenTelemetry independently distinguishes one logical API operation from multiple physical HTTP requests. Temporal formalizes the same durable pattern as one Activity Execution containing a chain of Activity Task Executions.

There is one important constraint. Official OpenAI and Anthropic SDKs retry some errors and timeouts automatically. Anthropic also offers provider-side or SDK-middleware model fallback that can run multiple models and splice them into one response stream. StoryOS cannot preserve its already accepted rule that every model fallback is a new Host decision if such behavior is hidden below the Model Gateway. Production adapters must therefore either disable automatic resubmission and model fallback, or expose every actual resubmission/model iteration to the Host before or as it occurs. Provider-internal model fallback should be ineligible under the current StoryOS boundary because the Host cannot issue a fresh prior Model Route Decision for each hop.

## Evidence labels

- **Source fact** — directly stated or implemented by a primary source.
- **Inference** — a conclusion obtained by combining source facts; not an upstream guarantee.
- **StoryOS recommendation** — an independent design choice for StoryOS.

## Sources checked

### Repository and pinned source

- The ticket question and the [Wayfinder map](https://github.com/FrankQDWang/StoryOS/issues/1), including the destination, adjacent tickets, and the explicit exclusion of production implementation, broad provider catalogs, and advanced routing optimization.
- Pinned Codex model client, sampling loop, retry helper, and rollout trace: [`client.rs`](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/client.rs), [`session/turn.rs`](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn.rs), [`responses_retry.rs`](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/responses_retry.rs), and [`rollout-trace/src/inference.rs`](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout-trace/src/inference.rs).

### Provider and infrastructure specifications

- OpenAI official Python SDK: [request IDs, retries, and timeouts](https://github.com/openai/openai-python#request-ids), plus the [retry implementation](https://github.com/openai/openai-python/blob/main/src/openai/_base_client.py).
- OpenAI Responses API: [streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events#response.completed) and [request debugging](https://developers.openai.com/api/reference/overview#debugging-requests).
- Anthropic: [streaming event order, cumulative usage, partial tool JSON, and stream errors](https://platform.claude.com/docs/en/build-with-claude/streaming), [Python SDK retries and timeouts](https://platform.claude.com/docs/en/cli-sdks-libraries/sdks/python#retries), [request IDs and mid-stream errors](https://platform.claude.com/docs/en/api/errors), and [server-side/client-side fallback semantics and per-attempt usage](https://platform.claude.com/docs/en/build-with-claude/refusals-and-fallback).
- OpenTelemetry: [GenAI client inference spans](https://github.com/open-telemetry/semantic-conventions-genai/blob/2e994c6d59a93bb4fc1752c5378eedb9b8e14d6b/model/gen-ai/spans.yaml#L154-L272), [HTTP retry/resend spans](https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-request-retries-and-redirects), and the [logical-client-operation versus nested protocol-call example](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/api.md#spankind).
- Temporal: [Activity Execution as the full chain of Activity Task Executions](https://docs.temporal.io/activity-execution#what-is-an-activity-execution) and [retry behavior](https://docs.temporal.io/encyclopedia/retry-policies#activity-execution).
- Google Cloud Vertex AI: [`GenerateContentResponse`](https://cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse), including `responseId`, actual `modelVersion`, finish reason, and `usageMetadata`.
- AWS Bedrock: [model invocation logging](https://docs.aws.amazon.com/bedrock/latest/userguide/model-invocation-logging.html), including per-invocation request ID, operation, model ID, account/region identity, request metadata, and input/output token counts.

## Evidence table

| Label | Evidence | Architectural implication |
| --- | --- | --- |
| Source fact | The ticket asks for one provider-neutral contract covering capabilities, per-step selection, streaming, tool calls, usage, failure, fallback, and run-level overrides. | The ticket cannot be reduced to model ranking or a model picker. |
| Source fact | The map excludes production implementation, broad provider catalogs, and advanced routing optimization. | This session should settle semantic boundaries and invariants, not build the gateway or optimize routing. |
| Source fact | Codex keeps stable provider/auth/transport-fallback state in a session-scoped model client, passes model selection and telemetry explicitly per turn, and allows multiple Responses API requests in one turn ([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/client.rs#L1-L24)). | A model call has both longer-lived provider infrastructure and per-step/per-request state; their lifetimes should not be conflated. |
| Source fact | One Codex sampling request loops over retryable failures ([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/session/turn.rs#L1138-L1206)); each concrete HTTP or WebSocket request starts a fresh inference-trace attempt ([HTTP](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/client.rs#L1438-L1507), [WebSocket](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/client.rs#L1618-L1675)). | A logical sampling objective may require several concrete attempts. |
| Source fact | Codex records one unique inference call ID per attempt, the exact request, model/provider, provider request ID, usage, and terminal status; failed or cancelled attempts retain completed partial output items ([trace API](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/rollout-trace/src/inference.rs#L121-L300), [stream mapping](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/client.rs#L1914-L2083)). | Attempt evidence must remain append-only even when a later attempt succeeds. |
| Source fact | Codex retries stream failures and can fall back from WebSocket to HTTP after exhausting a retry budget ([source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/responses_retry.rs#L20-L78)). | Transport retry/fallback is observable state even when model identity does not change. |
| Source fact | OpenAI's official Python SDK automatically retries connection errors, 408, 409, 429, and 5xx twice by default; timeouts are retried as well. It exposes `x-request-id` on successful and failed status responses ([SDK](https://github.com/openai/openai-python#retries), [implementation](https://github.com/openai/openai-python/blob/main/src/openai/_base_client.py#L2969-L3007)). | A Host that records only the outer SDK call can undercount actual submissions and cannot correlate every provider request. |
| Source fact | Anthropic's SDK has the same default two retries, including timeouts; its documentation says a long non-streaming request can terminate and retry without receiving a response ([source](https://platform.claude.com/docs/en/cli-sdks-libraries/sdks/python#retries)). | A timeout is not evidence that no provider-side work occurred. Attempt outcome, usage, and cost can remain unknown. |
| Source fact | Anthropic streams ordered lifecycle events, cumulative usage, partial JSON tool arguments, and error events that can arrive after HTTP 200 ([source](https://platform.claude.com/docs/en/build-with-claude/streaming#event-types)). | A normalized stream needs ordering, explicit terminality, partial-data handling, and provider-specific usage normalization. HTTP status alone is not an attempt outcome. |
| Source fact | Anthropic's beta server-side fallback can run several models inside one API call; `usage.iterations` records every model attempt, mid-stream fallback preserves a model boundary, and already streamed output may be billable ([source](https://platform.claude.com/docs/en/build-with-claude/refusals-and-fallback#what-the-response-contains)). Its SDK middleware can also retry another model and splice events onto the open stream ([source](https://platform.claude.com/docs/en/build-with-claude/refusals-and-fallback#client-side-fallback-with-the-sdk-middleware)). | A provider/SDK can hide model selection and multiple billable attempts below one client call. That conflicts with Host-owned fallback unless disabled or explicitly modeled. |
| Source fact | OpenTelemetry says retries send multiple physical HTTP requests to satisfy the same API call and recommends a distinct span with `http.request.resend_count` for every resend ([source](https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-request-retries-and-redirects)). It also recognizes nested protocol spans within one logical client operation ([source](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/api.md#spankind)). | Operation-level and attempt/transport-level observability are separate layers, not competing names for one record. |
| Source fact | OpenTelemetry's GenAI inference span carries provider, requested model, actual response model, response ID, finish reasons, streaming timing, and usage dimensions ([span source](https://github.com/open-telemetry/semantic-conventions-genai/blob/2e994c6d59a93bb4fc1752c5378eedb9b8e14d6b/model/gen-ai/spans.yaml#L154-L272), [usage source](https://github.com/open-telemetry/semantic-conventions-genai/blob/2e994c6d59a93bb4fc1752c5378eedb9b8e14d6b/model/gen-ai/spans.yaml#L38-L48)). | StoryOS can map attempt telemetry to standard attributes while retaining richer durable domain records. OpenTelemetry spans are observability projections, not authority or persistence truth. |
| Source fact | Temporal defines one Activity Execution as the full chain of Activity Task Executions and creates a new task execution for every retry ([execution](https://docs.temporal.io/activity-execution#what-is-an-activity-execution), [retry](https://docs.temporal.io/encyclopedia/retry-policies#activity-execution)). | Durable systems commonly preserve one logical parent while appending concrete attempts beneath it. |
| Source fact | Google returns a response ID, actual model version, finish reason, and usage metadata; AWS Bedrock invocation logs identify each request by request ID, operation, model ID, account/region, and token counts ([Google](https://cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse), [AWS](https://docs.aws.amazon.com/bedrock/latest/userguide/model-invocation-logging.html#model-invocation-logging-log-entry-format)). | Provider-neutral records need provider-native correlation fields and must distinguish requested registration from actual served model evidence. |

## Ticket scope judgment

### In scope for Specify ModelGateway and Model-Routing Semantics

The ticket should settle the provider-neutral semantics of:

1. model registration, capability trust, operational eligibility, route requests, route decisions, and run-level overrides;
2. the Model Gateway/Model Provider Adapter boundary from an admitted RunStep request to a normalized provider result;
3. logical Model Invocation and ordered Model Attempt identities, lifecycles, correlation, retry/fallback relationships, and terminal outcomes;
4. streaming event normalization, ordering, terminal events, partial text/reasoning/tool-argument evidence, cancellation, and backpressure expectations;
5. provider-produced tool requests as model output handed to the existing Tool Gateway, never adapter-executed local authority;
6. usage and cost evidence, including provider-reported versus estimated versus unknown values, per-attempt aggregation, budget settlement, and reconciliation;
7. retry and fallback eligibility, revalidation, idempotency limitations, ambiguous timeouts, partial output, and prevention of hidden SDK/provider fallback;
8. observability semantics: exact project-free registration, scoped use-binding, subsequent compatibility-decision, and route-decision references; request/response IDs, request digest, actual Processing Destination Identity and provider endpoint/account boundary, attempt ordinal, timings, finish/error categories, stream progress, usage, and disclosure evidence.

These are the semantics of the Host-owned infrastructure even if their eventual implementation spans several crates or services.

### Adjacent but not owned here

- [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54) owns which project information enters a Model Invocation, how it is minimized, and how the author inspects it. This ticket still needs to require an exact context/disclosure reference on every outbound attempt.
- [Specify the Self-Contained Project Storage and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) owns SQLite/filesystem layout, secret storage, migrations, backup, and restore. This ticket should state what must be durable, not choose tables.
- [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) owns exact external envelopes, DTOs, cursors, and compatibility. This ticket supplies their semantic invariants.
- [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) owns the fake-provider, retry, replay, and crash test matrix. This ticket defines the behaviors those tests must prove.
- [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64) owns long-term retention and compaction of the resulting Run evidence.
- Provider SDK implementation, UI layout, production schema, broad provider catalogs, benchmark-driven quality optimization, and pricing arbitrage are not this ticket's deliverable.

## Judgment on Model Invocation and Model Attempt

### Inference

One logical intent and one provider submission cannot share one mutable lifecycle without losing facts:

- an SDK timeout can trigger another request before the Host has a provider response;
- a stream can emit text or a partial Tool request and then fail;
- usage may arrive only in terminal or cumulative late-stream events;
- fallback may change the model, provider, destination, expected price, or disclosure boundary;
- a later success does not erase earlier disclosure, latency, possible cost, partial output, or uncertainty;
- the same user-visible RunStep can therefore have one requested Agent Decision but several provider-side executions.

### StoryOS recommendation

Adopt the separation, with this precise boundary:

```text
Model Invocation
  one RunStep-level intent to obtain one Agent Decision
  one immutable Model Route Request
  ordered Model Attempts
  derived aggregate outcome and usage; never overwrites attempt evidence

Model Attempt
  one admitted concrete try under one exact Model Route Decision and its
  exact Project Scope-bound Project Model Use Binding plus the separate
  subsequent External Contract Compatibility Decision over that binding
  created durably before outbound I/O
  one exact request/context digest and disclosure destination
  one adapter/provider submission lifecycle
  provider request/response IDs and actual served-model evidence when available
  ordered stream evidence, usage, partial output, and one terminal attempt outcome
```

A Model Registration is a globally reusable, non-authorizing contract and Adapter mapping over a project-free Provider API or service surface; it contains no Project data, Credential Reference, actual endpoint/account or disclosure destination, or use admission. The exact Project Model Use Binding—the model use of the single shared `ProjectExternalUseBindingRevision` shape—is created separately with Project Scope, use/Credential authorization, actual Processing Destination Identity, and bounds; only afterward does an immutable External Contract Compatibility Decision evaluate that binding with the Registration and Adapter. A retry that preserves the same exact Registration, binding, compatibility Decision, and request digest appends a new Model Attempt and may reference the same still-valid Model Route Decision after live revalidation. A fallback to another Registration appends both a new Model Route Decision and a new Model Attempt under the same Invocation, with that route's own exact binding and subsequent compatibility Decision, provided the logical RunStep objective and Model Route Request remain unchanged. If the prompt, required capability, authority, disclosure destination set, or intended Agent Decision changes materially, that is a new Invocation rather than a retry.

The existing generic `Execution Attempt` vocabulary can remain the common super-concept; `Model Attempt` is the model-specific contract because it has provider IDs, route-decision binding, streams, model usage, and disclosure evidence that generic attempts do not name.

### Attempt submission and ambiguous outcomes

Create the Model Attempt before network I/O so a crash between admission and response leaves durable evidence. The attempt must distinguish at least:

- definitely not submitted;
- submission started, provider receipt unknown;
- provider acknowledged, with request ID when available;
- stream active with ordered observed events;
- completed;
- failed before output;
- failed or cancelled after partial output;
- timed out or disconnected with provider outcome unknown.

Missing a terminal event must never be converted to ordinary failure or zero usage. Provider-reported usage is evidence for that Attempt; estimated usage is separately labelled; unknown usage stays unknown until reconciliation or a governing conservative settlement rule. Invocation totals are derived from all Attempts, including attempts that did not supply the final accepted output.

Each outbound resubmission is also a separate Outbound Disclosure occurrence. Repeating the same digest to the same endpoint is still another transfer; fallback to another endpoint is a transfer to another named destination. The context payload itself can remain behind a bounded, access-controlled reference, but the attempt needs an attributable disclosure record and digest.

### Retry and fallback enforcement

- Disable provider SDK automatic retries by default. If an SDK or lower layer cannot disable them, the Adapter is eligible only if it exposes each physical resend and its correlation/outcome to StoryOS.
- Do not enable provider-side or SDK-middleware model fallback under the current accepted boundary. It chooses or invokes another model before StoryOS can create the required new Model Route Decision.
- A TCP reconnect with no request resend need not be a new Model Attempt. Any HTTP/WebSocket request resend that can reach model execution is a new Attempt.
- Never splice output from separate Host-owned Attempts into one authoritative model result. Preserve earlier partial output as quarantined evidence; only one explicitly selected successful Attempt can supply the Agent Decision candidate.
- Never assume a generative POST is idempotent merely because the SDK retries it. If a provider offers a documented idempotency token, record its scope and observed behavior, but keep every transport submission inspectable.

## Observability minimum

For every Model Invocation, expose its RunStep, Model Route Request, current state, ordered attempts, selected terminal attempt if any, and derived total usage/cost status.

For every Model Attempt, preserve at minimum:

- exact Project Scope, Model Route Decision, global Model Registration revision,
  Project Model Use Binding, and the separate subsequent compatibility Decision
  with Credential binding generation when required;
- actual Processing Destination Identity, provider endpoint/account boundary,
  requested model identifier, and actual served-model evidence;
- attempt ordinal, causal retry/fallback reason, request digest, and idempotency token if any;
- disclosure record, context-manifest reference, and transmitted data categories;
- provider request/response IDs and Host correlation ID;
- started, first-byte/first-token, last-event, and terminal times;
- normalized ordered stream events or bounded references to them;
- terminal outcome, error class, retryability decision, and uncertainty classification;
- provider-reported usage, estimated usage, cost basis/reference, and reconciliation state;
- partial output and partial Tool-request evidence without treating either as executable authority.

OpenTelemetry GenAI and nested HTTP spans should be emitted as projections of these durable records. They do not replace StoryOS records because telemetry can be sampled, dropped, redacted, or retained under a different policy.

## HITL decisions resolved

1. A Model Invocation succeeds only after the Host validates and durably records one typed Agent Decision; provider completion terminates only its Model Attempt.
2. Retryability is a Host Recovery Decision after live revalidation. Confirmed transient failures may retry within policy and budget, deterministic invalid requests may not retry unchanged, and ambiguous submission follows the OutcomeUnknown rules.
3. A same-Registration retry may reuse a still-valid Model Route Decision only after live revalidation of the same exact Project Model Use Binding and separate compatibility Decision, and creates a new Attempt with the same semantic request digest. Fallback always creates a new Route Decision and uses the newly selected route's own binding plus a compatibility Decision produced after that binding; it never inherits the prior Credential or compatibility evidence.
4. Partial text and Tool arguments remain ordered Attempt evidence projected to the Author UI. They cannot silently enter normal model history, become an Agent Decision, or reach the Tool Gateway.
5. Provider- or SDK-managed model routing and fallback are forbidden in the first slice. Any future composite router requires a separately specified Registration contract.
6. OutcomeUnknown retains enforceable worst-case reservation. A successor requires authorization for another disclosure and budget for both Attempts; late usage reconciles the reservation and never rewrites earlier evidence.

## Direct answer for the current Wayfinder question

The proposed statement — one Model Invocation owns multiple ordered Model Attempts, and every Attempt represents one exact concrete provider submission — is supported, with one refinement: create the Attempt durably **before** outbound I/O and record whether provider receipt was confirmed or remains unknown. This preserves crashes and timeouts that cannot prove whether submission completed.

The accepted resolution adopts this separation together with Host-owned routing, request projection, streaming, Tool-request validation, failure recovery, fallback, usage settlement, cancellation, and telemetry boundaries.
