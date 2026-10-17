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
- With no known window, StoryOS does not refuse before dispatch. When the destination rejects an input that is too long, the AgentRun refuses with `conversation_too_long`, and the author sees the same sentence. The section below gives the mapping.
- The author message prefix of the Host fake compacts only on the Host fake. It is a test request, not a product trigger.
- Registrations `018f0000-0000-7001-8000-00000000fa10` (profile v9, documented window, pending trigger) and `018f0000-0000-7001-8000-00000000fa11` (profile v10, documented window, qualified trigger) follow `fa09`.

## Considered options

- A stored threshold of 400,000 tokens (80 percent of a window of 1,000,000 tokens) was rejected. This model has a window of 262,144 tokens.
- A threshold that counts the output bound in the window was rejected. A request near the window then has no room for its output.
- A runtime query for the window was rejected. The route documents no such query.

## Consequences

- The live qualification sent 6 requests. The automatic trigger started a compaction, and its summary request had 190,516 input tokens. The route accepted it. The window stays a documented value, because no request examined the full window ([evidence record](../research/agent-plan-compaction-threshold-qualification-2026-10-13.md)).
- The expected input counts the new turn with the Token Counting Profile of StoryOS, which counts Unicode scalars. The provider can count more tokens for the same text. In the measured run, the margin of 20 percent covered this difference.
- The new turn counts each text that the request sends: each selected Context item, each selection reference text, and each declared target text. The fixed headings of the request text are not counted. The margin of 20 percent covers them. [Count Each Sent Selection Reference Text in the Compaction Estimate](https://github.com/FrankQDWang/StoryOS/issues/1193) added this statement.

## Destination rejection of an input that is too long

On 2026-10-14, [Show a Result Sentence When the Real Destination Refuses a Request](https://github.com/FrankQDWang/StoryOS/issues/1110) added this section. The author decided on 2026-10-13 that a Registration with no known window does not compact and does not refuse before dispatch.

- A live request above the window got HTTP 400 with the native code `InvalidParameter` and a message that starts with `Total tokens of image and text exceed max message tokens` ([evidence record](../research/agent-plan-over-long-input-rejection-2026-10-14.md)). Other parameter errors have the same code, so the Agent Plan adapter reads the start of the message. It reports the rejection as `input_too_long:http_400:InvalidParameter` and keeps no message text.
- The Host refuses an AgentRun whose Create gets the `input_too_long` condition with `conversation_too_long`. It refuses an AgentRun whose Create gets another rejection with `destination_refused`. The author sees one sentence for each ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)). A rejection of a prior response reference keeps its recovery.
- The same live run found that the route served `doubao-seed-2-1-pro-260915`. The Ark model list gives this version a 1024k window. The documented 262,144-token window above is the window of the earlier version. This section does not change the window fact or the threshold.

## Window of the dated version

On 2026-10-14, [Pin the Served Doubao Model Version and Record Its Context Window](https://github.com/FrankQDWang/StoryOS/issues/1143) added this section. [ADR 0059](0059-pin-a-dated-provider-model-version-in-each-model-registration.md) pins the dated identifier `doubao-seed-2-1-pro-260915`.

- The profile of the dated version records the documented window of 1024K, 1,048,576 tokens, from the Ark model list (updated 2026-10-09). The window keeps documented evidence.
- With `agent_plan_request_v2`, the threshold of the head Registration `fa15` is (1,048,576 - 32,768) x 80 percent = 812,646 tokens. The rule did not change, so no code changed.
- An AgentRun that pinned `fa13` or an earlier Registration with a window keeps the window of 262,144 tokens and the threshold of 183,500 tokens.
- A live probe of approximately 827,800 provider tokens, above the new threshold, was accepted on the dated version ([evidence record](../research/agent-plan-pinned-version-qualification-2026-10-14.md)). No automatic compaction at the new threshold was examined.
- The statements above with 262,144 tokens and 183,500 tokens apply to the undated identifier `doubao-seed-2.1-pro`.
