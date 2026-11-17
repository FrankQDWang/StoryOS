---
status: accepted
---

# Admit an Author-Authorized Runtime Qualification

On 2026-10-10, at the refresh gate of [S4-02: Complete One Admitted Real-Model Request](https://github.com/FrankQDWang/StoryOS/issues/393), the author accepted this decision. While the Model Capability Profile of a real route is pending, a separate author authorization can admit its runtime qualification requests. The deployment configuration gives the link to that authorization. The External Contract Compatibility Decision of the Project records it. The requests go through the normal AgentRun and Model Gateway path.

## Context

At `main` `cf8c2d11`, the Agent Plan profile of [ADR 0048](0048-use-the-deployment-model-destination-by-default.md) is pending, and Create admission refuses each Agent Plan request with `model_runtime_qualification_pending`. S4-02 owns the first runtime qualification and the transition to qualified. A qualified profile needs attributable evidence from the exact route and model. That evidence needs admitted requests, and the pending profile refuses them.

[ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md) keeps account-specific admission out of the global Registration and Capability Profile. A raw probe outside the Model Gateway is not permitted, because durable admission and dispatch evidence must precede egress.

The author authorized live requests for S4-02 on 2026-10-10 ([authorization](https://github.com/FrankQDWang/StoryOS/issues/393#issuecomment-6098554693)), with a log of each request and no unbounded series.

## Decision

- The Server reads `STORYOS_MODEL_QUALIFICATION_AUTHORIZATION`. Its value is an `https` link to the author authorization. Only the `volcengine_agent_plan` destination accepts it, and the Server refuses to start with another value.
- The profile of the offered Registration can be pending while the deployment gives the link. Then the Host writes the compatibility Decision with `runtime_qualification = qualification_authorized` and the link in `qualification_authorization`. Without the link, the Decision is `pending`. A qualified profile writes `qualified` and no link.
- A change of the link, the Credential Reference, the request bounds, the Registration, or the destination makes new binding records at the next `createAgentRun`. Earlier records stay unchanged, and an AgentRun that pinned an earlier binding refuses with `model_use_binding_stale`.
- Create admission dispatches a pending profile only when the current Decision is `qualification_authorized`. Each such Model Attempt records `runtime_qualification = qualification_authorized`. The pending profile is never reported as qualified.
- Each Agent Plan binding records the request bounds `agent_plan_request_v1`: at most 8192 output tokens, reasoning tokens included, and a request timeout of 180 seconds. The author adopted these bounds on 2026-10-10 for the qualification. Create admission refuses a binding without known bounds with `model_request_bounds_unknown`.
- An unqualified continuation sends the full current input and no prior response reference.
- The executor writes the planned number of live requests before each run and logs each request. The tracked evidence record of the qualification cites the Model Attempts of these requests.

## Considered options

- A static authorization constant in the Agent Plan Capability Profile was rejected. It puts account-specific admission into the global profile, against ADR 0033.
- A new author or operator command that records the authorization was rejected. It needs the command sequence, generated contracts, and an author surface, and the qualification needs none of them.
- A request outside the Model Gateway was rejected. It is a raw probe with no admission or disclosure evidence.

## Consequences

- Migration 0088 adds `qualification_authorized` and the `qualification_authorization` column to the compatibility Decisions, and `agent_plan_request_v1` to the binding bounds. The assistance query can answer `qualification_authorized`.
- After S4-02c qualifies the profile, a Decision is `qualified`, and the link has no effect.
- This decision authorizes no external account, spend, or deployment change. The author keeps the Agent Plan quota and overage under control in the Volcengine console.

## Amendment: function calling (2026-11-03)

[S5-10: Probe the Tool Capabilities of the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/696) adds one Tool entry to the Model Capability Profile: function calling. An entry can also record `unsupported` evidence from the exact route and model. The author accepted these changes on 2026-11-03 through the coordinator of Stage 5. [ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md) removes the hosted entries of the first plan.

- The link also authorizes a qualified Registration while its function calling entry is pending. A pending entry has documentation as its only evidence. Then the Decision is `qualification_authorized`, and ordinary requests dispatch as before.
- A function calling probe is a Create request of an AgentRun whose author message is the exact constant text of the probe. Only an authorized Decision on a pending entry selects it. The first probe sends that constant text and one function tool. The second probe sends the constant output of the earlier function call and the prior response reference. The earlier call must have the same Conversation, Registration, and destination. Without an earlier call, the adapter refuses the second probe before dispatch. A probe sends no Project content and no Provider-side tool. Its response makes no Agent Decision on any path, and a probe has no recovery. A rejection refuses the AgentRun. An unknown outcome keeps the Model Attempt uncertain and leaves the AgentRun waiting. The Model Attempt records the native Tool items.
- Registration `018f0000-0000-7001-8000-00000000fa16` has the entry pending. The probe of 2026-11-03 qualifies it ([evidence](../research/agent-plan-function-calling-probe-2026-11-03.md)). Registration `018f0000-0000-7001-8000-00000000fa17` has the qualified entry, and the head Registration `018f0000-0000-7001-8000-00000000fa19` keeps it with the tool loop mapping (ADR 0053), so the link authorizes no more probes.
- No ordinary AgentRun gets a Tool from this amendment.

## Amendment: one exception for the `tool_choice` probe (2026-11-12)

The author approved this exception on 2026-11-12 through the coordinator of [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451). It also covers [ADR 0050](0050-load-server-and-worker-settings-through-one-typed-settings-layer.md).

- The 9 live requests of [#1452](https://github.com/FrankQDWang/StoryOS/issues/1452) on 2026-11-12 are the evidence of the `tool_choice` probe ([record](../research/agent-plan-tool-choice-probe-2026-11-12.md)).
- They did not go through the Model Gateway. A test of the model adapter sent each body directly to the `exchange` of the adapter, so no AgentRun, no Model Attempt, and no Outbound Disclosure Event records them. A test process, and not the Worker, loaded the key through the settings loader.
- These facts stay true: no key value is in a file, a log, or an output; the record keeps the request bodies; and the tests that sent the requests are not in the repository.
- The exception permits no later request of this kind. Each later qualification request goes through the admitted path of this ADR.

## Amendment: the embedding probe path (2026-11-16)

The author approved this path on 2026-11-16 through the coordinator of [#365](https://github.com/FrankQDWang/StoryOS/issues/365), for [S7-C1](https://github.com/FrankQDWang/StoryOS/issues/1572). It also covers [ADR 0050](0050-load-server-and-worker-settings-through-one-typed-settings-layer.md).

- The admitted path of this ADR sends only the fixed bodies of a Responses Create request, so it cannot send an embedding request. The Worker command `storyos-worker --probe-embedding <case>` sends one embedding request of the probe of [ADR 0095](0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md) section 5. It has the form of `storyos-worker --discover-exa`.
- Each request is one `prepare` and one `exchange` of the embedding function of the Volcengine adapter: one POST, no retry, and no redirect. The request has constant text and no Project content. The command reads no database, and it writes the observation as JSON to stdout.
- The key reaches the request only through the typed Worker settings: the Worker resolves `settings:STORYOS_AGENT_PLAN_API_KEY` (ADR 0050). No test process loads the key. No AgentRun, Model Attempt, or Outbound Disclosure Event records the requests. The [record](../research/embedding-and-pgvector-probe-2026-11-16.md) keeps the request bodies.
- The command permits only the requests of that probe. S7-C2a2 removes it, and each later embedding request goes through the admitted embedding path of Stage 7.
- [S7-C2a2](https://github.com/FrankQDWang/StoryOS/issues/1596) removed the command on 2026-11-17. Each later embedding request goes through the Gateway function of [S7-C2a1b](https://github.com/FrankQDWang/StoryOS/issues/1612): the admission, the committed dispatch claim, and the record.
