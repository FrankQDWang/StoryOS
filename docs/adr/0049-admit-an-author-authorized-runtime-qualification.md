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
