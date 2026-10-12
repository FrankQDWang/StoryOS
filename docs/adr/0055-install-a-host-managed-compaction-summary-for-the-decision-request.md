---
status: accepted
---

# Install a Host-Managed Compaction Summary for the Decision Request

On 2026-10-12, [S4-06c2b2: Qualify Host-Managed Compaction on the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/1123) qualified Host-managed compaction on the Volcengine Agent Plan route. This ADR records the mapping, the order of the requests, and the grant scope revision that the compaction uses. The author approved the grant scope revision on 2026-10-12 in [S4-06c2a](https://github.com/FrankQDWang/StoryOS/issues/1119).

## Context

[ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md) continues a Conversation with the prior response reference. The stored chain of the destination grows with each turn, and the destination window limits it. [ADR 0005](0005-require-ordered-context-assembly-before-destination-disclosure.md) lets general compaction prepare a later request within one active Run and turn. [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md) says that a compaction result installs only for a later request, and that a compaction keeps the submitted history.

S3-21 staged a compaction after the decision of a Run and installed it for a placeholder Model Attempt that sent nothing. The Agent Plan route has no native compaction that the profile can qualify, so StoryOS makes the summary.

## Decision

- Compaction occurs before the first decision request of an AgentRun, when the mapping of the pinned Registration names Host-managed compaction. The decision request is the later request.
- The summary request is a Create with the Purpose `active_context_compaction`. It sends one Host instruction and the prior response reference of the Conversation, and no project content. It has its own Context Assembly, manifests, Model Attempt, Outbound Disclosure Event, and usage.
- With no prior response, nothing is compacted. A refused summary request, a summary request with no complete summary, and a refused install keep the ordinary continuation of the decision request.
- The install check refuses a restricted source and missing Exact Required input. It also refuses a changed author message or Working Target, and a summary over the Context item limit. A summary cannot satisfy Exact Required input.
- An install commits a new Step Snapshot and Context Assembly for the later request. The summary is its first selected item, with the source class `conversation_summary` and the new projection mode `derived_summary`. The current instructions and the exact target follow it.
- The later request uses a new transport: it sends no `previous_response_id`. Its dispatch claim records the install with a new RunStep and the Model Invocation and Model Attempt of the claim. Its response starts the chain of the next turn.
- The grant scope `project_writing_assistance_v2` names the compaction Purpose and the summary source class. A grant with the first scope refuses the summary request, and the decision request keeps the ordinary continuation.
- The mapping `storyos.volcengine-agent-plan.responses.create.v4` keeps the streaming mapping of [ADR 0054](0054-stream-the-agent-plan-response-and-commit-events-at-64-or-500-ms.md) and adds the summary request and the later request. Only this mapping sends a summary request on the route.
- Registrations `018f0000-0000-7001-8000-00000000fa08` (profile v7, pending compaction) and `018f0000-0000-7001-8000-00000000fa09` (profile v8, qualified compaction) follow `fa07`.
- No author view shows a summary or a compaction fact ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)). The inspection response `getAgentRun` shows the compaction record.

## Considered options

- Compaction after the decision, as S3-21 did, was rejected. The installed summary then had no real request, and the next Run started from the full chain again.
- A summary that continues the chain was rejected. The later request then sends the full chain and the summary together.
- A new decision position for the later request was rejected. Steering Input uses the decision position, and a new position for compaction changes the position of the next Steering Input.

## Consequences

- The live qualification sent 3 requests with 1,616 input and 1,756 output tokens ([evidence record](../research/agent-plan-compaction-qualification-2026-10-12.md)).
- The semantic preservation of a summary is unknown, and each compaction record says so.
- Until [S4-06c3](https://github.com/FrankQDWang/StoryOS/issues/1116) adds the automatic threshold, only a request with the Host-fake compaction prefix compacts.
