---
status: accepted
---

# Set the Context Item Limit From the Measured Model Input

On 2026-10-12 the author accepted this decision for [S4-07: Measure the Model Input Limit and Raise the Context Item Limit](https://github.com/FrankQDWang/StoryOS/issues/1104). One model-visible Context item can have 32,000 tokens under the admitted Token Counting Profile. The Agent Plan route uses the new request bounds `agent_plan_request_v2`: at most 32,768 output tokens and a request timeout of 600 seconds. The author confirmed both numbers after the measurement.

## Context

At `main` `1fc55a1b`, the absolute profile `storyos.foundation.absolute.v1` limits one model-visible Context item to 10,000 tokens, and the protocol makes that ceiling a repository invariant. The Token Counting Profile `storyos.token-counting.unicode-scalar.v1` counts Unicode scalars. A long Chapter can have more than 10,000 scalars, so the Host refuses a request for it before dispatch.

The Agent Plan binding uses the bounds `agent_plan_request_v1` of ADR 0049: 8192 output tokens and 180 seconds. The author adopted them for the qualification, not for the rewrite of a long Chapter.

The [measurement record](../research/agent-plan-input-limit-measurement-2026-10-12.md) of 2026-10-12 sent 8 planned live requests to the qualified route:

- The route accepted 128,186 input tokens in 10.7 seconds. The coordinator capped the ladder there. The documented window of the model is 262,144 tokens, which input and output share.
- Ordinary Chinese prose used 0.69 to 0.92 Provider tokens for each scalar. Emoji used 2.13, and CJK Extension B scalars used 3.97.
- A full rewrite of a 12,000-scalar Chapter used 29,577 output tokens, of which 20,421 were reasoning tokens. It took 451.5 seconds, approximately 65 output tokens for each second.

## Decision

### The per-item limit

- The per-item limit is 32,000 tokens under the admitted Token Counting Profile. It replaces 10,000 in the absolute profile row and in the repository invariant of the protocol.
- Method: the measured accepted input (128,186 tokens), minus the output reserve (32,768 tokens), minus the fixed request overhead (approximately 241 tokens), leaves approximately 95,000 tokens. One request can carry three large items: the author message, the Working Target, and the target blocks, which repeat part of the Working Target. At 0.92 Provider tokens for each scalar, the share of one item is approximately 34,000 scalars. The limit rounds this down to 32,000.
- The Core and adapter checks use the one limit `CONTEXT_ITEM_TOKEN_LIMIT`. An item at the limit is selected. An item above it is rejected with `over_item_token_limit`, and the Context Assembly blocks the AgentRun before dispatch. The run writes no Model Attempt and no Outbound Disclosure Event.
- Each new Context Assembly record keeps the limit of its assembly. A record from before this decision has no limit field. Its assembly ran under 10,000 tokens, so the query answers 10,000 for it.
- The Token Counting Profile does not change. No unspecified tokenizer or character estimate counts a token.
- The public array bounds of a passage collection stay at 10,001 items. They are shape bounds of one JSON array, not the item limit.

### The request bounds

- The new bounds `agent_plan_request_v2` are 32,768 output tokens, reasoning tokens included, and a request timeout of 600 seconds.
- At the measured output rate, 32,768 tokens take approximately 504 seconds. Thus the token bound stops a long reply before the timeout. The route then returns an incomplete response that supplies no candidate, which is a known outcome, not `outcome_unknown`. The 600-second timeout is the absolute ceiling of one external network Attempt.
- New Agent Plan bindings record `agent_plan_request_v2`. A current binding with `agent_plan_request_v1` does not offer the deployment destination, so the next `createAgentRun` writes new binding records (ADR 0048). An AgentRun that pinned a binding keeps the bounds of that binding.
- `agent_plan_request_v1` keeps its values, because the qualification records cite them. Migration 0091 adds `agent_plan_request_v2` to the binding bounds.

### Superseded statements

This record supersedes the 10,000-token per-item ceiling of the protocol, and the statement of ADR 0049 that each Agent Plan binding records `agent_plan_request_v1`.

## Considered options

- A per-item limit of 48,000. Rejected: it fits only two large items in the measured input, and three large items can occur in one request.
- A per-item limit of 16,000. Rejected: it refuses a Chapter of approximately 20,000 scalars.
- 49,152 output tokens with 600 seconds. Rejected: at the measured rate the timeout stops the wait before the token bound, and a timeout after dispatch gives `outcome_unknown`.
- A change of the values of `agent_plan_request_v1`. Rejected: the records of the qualification then state bounds that they did not use.
- A Token Counting Profile that uses the Provider tokenizer. Deferred: the route publishes no tokenizer, and this decision keeps the current profile.

## Consequences

- An author can send a Chapter of up to 32,000 scalars as one Working Target.
- One reply can rewrite approximately 12,000 to 13,000 scalars in full. A longer rewrite needs more than one request, each with a part of the Chapter as its target blocks.
- A text of mostly emoji or rare CJK scalars can need up to approximately 4 Provider tokens for each counted token. Such a request can exceed the window of the route. The route then refuses it after dispatch. A later Token Counting Profile revision can close this gap.
- The AgentRun claim lease (30 seconds by default) is shorter than the request timeout. The bounds of ADR 0049 already had this gap. The Worker loop does one claim at a time. Thus with one Worker that has the Agent Plan adapter, no other claim takes the run while its request waits. A deployment with more than one such Worker needs a lease renewal during the request first ([#1112](https://github.com/FrankQDWang/StoryOS/issues/1112)).
- Streaming and continuation (S4-06) can change the timeout rule of a long reply in a later decision.

## Amendment

On 2026-10-13, [#1112](https://github.com/FrankQDWang/StoryOS/issues/1112) closed the lease gap of the Consequences. While a destination request is in progress, the Model Gateway renews the Run Lease at once, and then three times in each lease period. The renewal stops when the request settles, when the Worker stops, or when the claim has lost its lease. A renewal is a read committed update that requires the current fence (ADR 0058).
