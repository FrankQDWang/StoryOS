---
status: accepted
---

# Compact at 80 Percent of the Model Window After the Output Bound

On 2026-10-12 and 2026-10-13, the author selected the rule that starts Host-managed compaction automatically, and the behavior near the window when no compaction installs. The decisions were made on [S4-06c: Compact the Agent Plan Conversation With a Host-Managed Summary](https://github.com/FrankQDWang/StoryOS/issues/1108) and delivered by [S4-06c3](https://github.com/FrankQDWang/StoryOS/issues/1116).

## Context

[ADR 0055](0055-install-a-host-managed-compaction-summary-for-the-decision-request.md) installs a Host-managed summary for the decision request. It did not decide when compaction starts. The window of a model is a fact of the model, and different models have different windows. The route has no documented query for the window.

## Decision

- The Model Capability Profile records the context window of its model, which input and output share, as one fact with its evidence. For `doubao-seed-2.1-pro`, the documented window is 262,144 tokens.
- The threshold is a rule, not a stored number: 80 percent of the window after the output bound of the use binding. For `doubao-seed-2.1-pro` with `agent_plan_request_v2` (32,768 output tokens), the threshold is 183,500 tokens. A new Registration with another window changes the threshold with no code change and no new ADR.
- Before the first decision request of an AgentRun, the Host calculates the expected input. It is the reported input and output tokens of the last response of the chain, plus the counted tokens of the new turn. Compaction starts when the expected input reaches the threshold.
- A profile with no window, an unknown output bound, or unreported usage gives no automatic compaction.
- With a known window, a requested compaction can fail to install: the summary request refuses, no complete summary comes, or the install check refuses. When the expected input then reaches the window after the output bound, the AgentRun refuses before dispatch with `conversation_too_long`. The author sees one sentence: this conversation is too long, so start a new conversation ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)).
- With no known window, StoryOS does not refuse before dispatch. The mapping of a destination rejection for an input that is too long waits until the native rejection shape is observed ([#1110](https://github.com/FrankQDWang/StoryOS/issues/1110)).
- The author message prefix of the Host fake compacts only on the Host fake. It is a test request, not a product trigger.
- Registrations `018f0000-0000-7001-8000-00000000fa10` (profile v9, documented window, pending trigger) and `018f0000-0000-7001-8000-00000000fa11` (profile v10, documented window, qualified trigger) follow `fa09`.

## Considered options

- A stored threshold of 400,000 tokens (80 percent of a window of 1,000,000 tokens) was rejected. This model has a window of 262,144 tokens.
- A threshold that counts the output bound in the window was rejected. A request near the window then has no room for its output.
- A runtime query for the window was rejected. The route documents no such query.

## Consequences

- The live qualification sent 6 requests. The automatic trigger started a compaction, and its summary request had 190,516 input tokens. The route accepted it. The window stays a documented value, because no request examined the full window ([evidence record](../research/agent-plan-compaction-threshold-qualification-2026-10-13.md)).
- The expected input counts the new turn with the Token Counting Profile of StoryOS, which counts Unicode scalars. The provider can count more tokens for the same text. In the measured run, the margin of 20 percent covered this difference.
