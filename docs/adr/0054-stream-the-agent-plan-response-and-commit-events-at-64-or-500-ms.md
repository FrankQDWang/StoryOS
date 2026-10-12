---
status: accepted
---

# Stream the Agent Plan Response and Commit Events at 64 or 500 ms

On 2026-10-12, the author selected the commit bound of provisional Model Stream Events from a live measurement. The selection was made at [S4-06b: Stream the Agent Plan Response as Typed Model Stream Events](https://github.com/FrankQDWang/StoryOS/issues/1107). [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md) gave this selection to S4-06. The Volcengine Agent Plan route now streams each response as typed server-sent events.

## Context

ADR 0039 says that the sink commits Model Stream Events in short fenced transactions. It commits at each terminal item event and after a bounded number of provisional events. It also says that the sink tells the adapter to stop when the Run Lease is stale or a Model Attempt Cancellation is durable. The adapter learns a stop only at a commit.

A measurement build committed each provisional event (bound 1). Three admitted live requests on 2026-10-12 gave these facts ([evidence record](../research/agent-plan-streaming-qualification-2026-10-12.md)):

| Output tokens | Event commits | Mean commit | Exchange |
| --- | --- | --- | --- |
| 113 | 77 | 4.9 ms | 5.6 s |
| 262 | 216 | 4.9 ms | 8.5 s |
| 751 | 413 | 3.9 ms | 20.3 s |

Each commit is one serializable Project transaction. At bound 1, a response at the 8192 output token bound needs approximately 6,700 commits. These transactions also compete with Author Edit transactions of the same Project. The author surface does not show provisional text (ADR 0040), so provisional events are evidence only.

## Decision

- The mapping `storyos.volcengine-agent-plan.responses.create.v3` sends `stream: true`. It keeps the continuation mapping of [ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md).
- The adapter reads the server-sent events in order. Each output item and each text, refusal, reasoning summary, or function argument delta becomes one provisional Model Stream Event. The event keeps its native item identifier. An `output_item.done` event is a terminal item event.
- A terminal item event commits at once, with the provisional events before it.
- Provisional events commit when 64 events wait, or 500 ms after the last commit, whichever comes first. The time condition also applies when no event arrives: the adapter then commits an empty batch. Thus a stale fence or a durable cancellation stops the exchange within approximately 500 ms. The author selected these values.
- The terminal response event (`response.completed`, `response.incomplete`, or `response.failed`) maps to the terminal observation, with the same mapping as a response that is not streamed.
- A stream that ends before its terminal response event, a read failure, a timeout, or a stop from the sink is OutcomeUnknown. The observation keeps the response reference of `response.created`, if the stream reported it. A dropped stream is never a completed answer. Retrieval and recovery stay with [S4-04](https://github.com/FrankQDWang/StoryOS/issues/395).
- Partial text and partial function arguments supply no Agent Decision and no ToolCall.
- Registrations `018f0000-0000-7001-8000-00000000fa06` (profile v5, pending streaming) and `018f0000-0000-7001-8000-00000000fa07` (profile v6, qualified streaming) follow `fa05`.

## Considered options

- Bound 1 was rejected. It needs thousands of serializable transactions for a long response.
- A count bound only, as ADR 0039 first said, was rejected. A model can produce no event for a long time, for example during reasoning. Then the exchange does not learn a cancellation.
- Bounds 16 and 256 were considered. The author selected 64 events or 500 ms, the common form of a size or time flush.

## Consequences

- The measured rate is 0.55 to 0.82 events for each output token. Thus a response at the 8192 token bound needs approximately 100 commits for its provisional events. The time condition adds at most two commits each second.
- A crash can lose at most the provisional events after the last commit. The response stays OutcomeUnknown in that case.
- The Contract-Faithful Fake Destination keeps its own stream batches.
