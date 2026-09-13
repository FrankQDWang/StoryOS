# Agent Plan Responses API: verified boundaries

Date: 2026-09-13. Scope: public first-party documentation and first-party CLI reference. No credential access, inference request, account change, or repository contract change was made. This note supports a Stage 3 planning decision. It does not certify the user's exact plan, model, or endpoint capabilities.

Planning baseline: `main@559a3a2e38b88caf4975ab8929fc3c155e00ba8a`. The author-confirmed direction is recorded in [ADR 0033](../adr/0033-use-volcengine-responses-for-the-first-real-model-path.md). This evidence note supports that decision; it does not certify account capability or start implementation.

## Result

Use native Responses as the first protocol and make provider context continuity the normal path. Keep continuity, cache, hosted tools, structured output, context editing, and recovery as separate capabilities. The name "Responses" does not prove that every capability is available in Agent Plan or that all capabilities can be combined. This is a design conclusion from the evidence below.

## Agent Plan is a distinct product route

The current Agent Plan Codex guide explicitly supports Responses, with base URL `https://ark.cn-beijing.volces.com/api/plan/v3`, `wire_api = "responses"`, and a dedicated Agent Plan key. Its current page is **2556054**. Page **2556056** now describes Coding Plan and `/api/coding/v3`; old references must not conflate them. The general Ark API reference uses `/api/v3/responses`. The Agent Plan quickstart also distinguishes dedicated keys from other Ark keys. [Agent Plan Codex guide](https://docs.volcengine.com/docs/82379/2556054?lang=zh), [Coding Plan Codex guide, updated 2026-09-08](https://docs.volcengine.com/docs/82379/2556056?lang=zh), [Agent Plan quickstart, updated 2026-08-28](https://docs.volcengine.com/docs/82379/2373738?lang=zh).

## Personal AI tool use is the relevant scope

The Agent Plan overview addresses personal users and lists AI Agent tools, including self-hosted tools. Its explicit restriction states that text and embedding models cannot be used for API calls outside AI tools; misuse can suspend the plan or account. The same page permits sharing one plan's quota across supported tools. StoryOS's current personal AI Agent scope is relevant to this distinction. The page does **not** identify StoryOS or give blanket approval to every custom tool. Thus eligibility is an explicit confirmation item, not evidence that a personal StoryOS Agent is unavailable. Do not recast the present product as a multiuser SaaS or force a vendor selection detour. The overview now supports optional overage billing; exhausted quota stops use without extra charges only when overage is disabled. [Agent Plan overview, updated 2026-09-12 07:54:42](https://docs.volcengine.com/docs/82379/2366394?lang=zh).

The quickstart expressly addresses users who already have a local Agent and supplies the configuration. It names search, datasets, Agent memory, and application development as separate Harness options. This supports the personal Agent integration direction, while leaving exact custom-tool eligibility unconfirmed. [Quickstart](https://docs.volcengine.com/docs/82379/2373738?lang=zh).

## General Ark Responses contract

The create reference states:

- `previous_response_id` brings prior input and output into the new context. This increases input tokens; it is not a free or unlimited context store.
- `store` defaults to true. `expire_at` applies to stored context and explicit cache. The default is creation plus 259200 seconds; the maximum is 604800 seconds.
- `instructions` does not carry forward with `previous_response_id`. It conflicts with explicit cache writes; a cached prior response with current instructions has zero cache-hit tokens.
- `tools` supports custom functions, built-in tools, and remote MCP. Explicit caching allows custom functions but excludes the other tool categories.
- The API also exposes structured output, metadata, context editing, and encrypted reasoning content. These are separate contracts, not proof of exact Agent Plan support.

These are facts for the **general `/api/v3` reference**, not a tested `/api/plan/v3` capability claim. [Create Response, updated 2026-09-10 20:00:21](https://docs.volcengine.com/docs/82379/1569618?lang=zh).

## Explicit Session cache has real composition limits

The current cache guide distinguishes implicit cache from explicit cache. Implicit cache is automatic on supported models, has no storage charge, and does not guarantee hits. Explicit cache requires storage and explicit configuration, charges storage, and replaces implicit caching for that request.

For a continuous explicit cache chain, prior rounds must have cache writes enabled. `thinking` must stay consistent. `tools` can be supplied only in the first round; supplying it again later conflicts. A deleted first round removes those tool definitions and they cannot be reintroduced into that chain. An enabled cache in the prior chain excludes `json_schema`; `json_object` is supported. Instructions cannot be used for cache reads or writes. The expiry clock does not reset on use; expired context/cache must be recreated.

Implication: do not globally enable explicit Session cache. Define a stable function-tool profile, and a separate profile for hosted tools or changing tools/structured-output requirements. Preserve provider continuity where supported even when explicit caching is unavailable. [Context cache, updated 2026-09-11 12:11:00](https://docs.volcengine.com/docs/82379/1398933?lang=zh). The old page 1602228 currently redirects to the product introduction and is not the current cache authority.

## Provider tools do not transfer StoryOS operation authority

For custom functions, the documented loop returns `function_call`, with `call_id`, name, and arguments. The application executes the function and sends `function_call_output` with the matching `call_id`. Hosted tools are a different path: the provider executes the configured service. [Responses tools](https://www.volcengine.com/docs/82379/1958524?lang=zh).

The official Ark CLI helper reference configures plan Harness services as separate MCP servers. This is evidence that a plan's Harness entitlement is not automatically equivalent to `tools: [{type: "web_search"}]` support within its Responses endpoint. [Official Ark CLI helper reference](https://github.com/volcengine/ark-cli/blob/main/skills/arkcli-helper/references/arkcli-helper.md).

Design conclusion: StoryOS can authorize one bounded hosted operation, then preserve returned lifecycle and result evidence. It need not claim observation of every internal search or reasoning step. Side effects from StoryOS functions remain under StoryOS authorization, execution, and durable outcome control.

## Typed streaming and recovery are different capabilities

The lifecycle reference documents SSE event types `response.created`, `response.in_progress`, `response.completed`, `response.incomplete`, and `response.failed`, with response objects and sequence numbers. The API reference links separate output-item, text, and tool event contracts. Use typed events; a text-delta-only adapter loses meaning. [Response lifecycle, updated 2026-09-10 20:00:22](https://docs.volcengine.com/docs/82379/2644693?lang=zh).

The retrieve endpoint is `GET /api/v3/responses/{responseId}`. A completed response returns its full object; a still-generating response returns an error code. The page does not establish resumable SSE, automatic retry of ambiguous create requests, or exactly-once creation. This investigation found no first-party proof of a create idempotency contract or `/api/plan/v3` retrieval parity. Retrieval is a candidate recovery mechanism after a durable response ID exists, not permission to replay side effects or resend an unknown-outcome create. [Query Response details](https://docs.volcengine.com/docs/82379/1783709?lang=zh).

## Context editing is not proven semantic compaction

The current context-editing guide is beta and lists supported Doubao model versions. It documents `clear_thinking` and `clear_tool_uses`, with retained turns/calls and excluded tools. When combined, thinking cleanup must precede tool cleanup. These operations remove selected history; the guide does not establish a general durable summary or a `/responses/compact` contract.

There is also current documentation shape drift: the guide shows `keep: {type, value}` or `"all"`, and a boolean `clear_tool_inputs`; the expanded create API reference shows wrapper object forms. Verify wire shapes with the selected model and first-party SDK before committing fixtures. [Context editing, updated 2026-09-08 01:17:53](https://docs.volcengine.com/docs/82379/2123215?lang=zh).

## Bounded qualification before claiming acceptance

Keep this as one first-provider qualification slice, not a broad comparison:

| Dimension | Documentation result | Remaining evidence |
| --- | --- | --- |
| Native Responses route | Agent Plan explicitly supports it | Exact model and plan smoke result |
| Stored response continuity | General Ark documented | Plan store, expiry, continuation, invalid ID behavior |
| Cache | General Ark implicit and explicit semantics documented | Plan model support, usage counters, composition, actual quota effect |
| Function loop | General Ark documented | Correlation, parallel calls, interrupted stream, durable outcomes |
| Retrieve recovery | General Ark completed-object retrieval documented | Plan endpoint parity and unknown create outcome policy |
| Hosted search | Plan Harness and general Responses tools both exist | Exact integration path and one bounded operation's evidence |
| Context editing | General Ark beta, model list and wire-shape drift | Exact model wire format; no assumed compaction parity |
| Future providers | No work requested now | Rebuild eligible model context; expose capability differences; no opaque-state migration promise |

Do not turn untested advanced fields into accepted capability flags. The globally reusable Model Capability Profile identifies the Provider product route, model, protocol, supported combinations, and public or exact-model evidence with its date. Current account and credential-binding availability belong to the Project Scope-bound Model Operational Snapshot and its project-use and compatibility records; they do not turn the global Profile into account-specific admission. Keep user-visible conversation, artifact facts, approvals, tool outcomes, and recovery decisions in StoryOS. Provider context is a continuity service with expiry, not the sole durable product record.

## Future protocol extension

OpenAI Responses supports server-held continuation through `previous_response_id` and a separate Conversations API. Its documentation also states that prior input tokens in a response chain remain billable. These are OpenAI facts, not evidence of Agent Plan behavior. [OpenAI conversation state](https://developers.openai.com/api/docs/guides/conversation-state).

Anthropic client-tool use returns `tool_use` blocks and accepts `tool_result` blocks in subsequent Messages requests. Its server tools run a separate internal loop. This supports a common StoryOS result and execution boundary with distinct protocol mappings; it does not support making a Responses handle mandatory for every future Adapter. [Anthropic tool execution](https://platform.claude.com/docs/en/agents-and-tools/tool-use/how-tool-use-works).

Design recommendation: preserve typed items, tool correlation, provisional versus terminal output, usage certainty, and opaque Provider continuation where required. Keep native replay data bound to its original Adapter and destination. A future Provider receives freshly eligible context from StoryOS records; it does not receive another Provider's opaque state.
