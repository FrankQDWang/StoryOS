---
status: accepted
---

# Continue the Agent Plan Conversation With the Prior Response

On 2026-10-12, the author approved the split of [S4-06: Continue the Qualified Responses Conversation](https://github.com/FrankQDWang/StoryOS/issues/695). This decision records the continuation mapping and the cache evidence of [S4-06a: Continue the Agent Plan Conversation With the Prior Response and Record Cache Evidence](https://github.com/FrankQDWang/StoryOS/issues/1106). On the Volcengine Agent Plan route, a later AgentRun of a Project Conversation sends the response reference of the earlier decision. Thus the model gets the earlier turns of the Conversation from the destination, and StoryOS sends only the current turn.

## Context

At `main` `1fc55a1b`, the Agent Plan profile v2 qualifies only transport, native text, and their Create combination. The Host selects an incremental continuation only when the profile qualifies continuation. Thus each AgentRun sends the full current input with no prior reference (ADR 0049). The model does not get the earlier turns of the Conversation.

[ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md) gives each Project Conversation its own Provider continuation chain. The general Ark reference says that `previous_response_id` brings the earlier input and output into the new context, and that `instructions` do not carry forward. It also says that explicit cache conflicts with `instructions`. These are general Ark facts, not Agent Plan facts.

## Decision

### A Registration names its mapping and the capabilities that the mapping uses

- Each Model Registration names its exact wire mapping revision and its requirement: the capabilities that each Create request of the mapping uses together.
- A Registration is qualified when its profile qualifies its complete requirement. Create admission and the runtime qualification of a new compatibility Decision examine this, not only the Create requirement.
- A new mapping that uses a capability that the profile does not qualify gets a new Registration with a pending profile. The authorized runtime qualification of ADR 0049 admits its requests. After the evidence, a new profile revision and a new Registration qualify it.
- The Host selects an incremental continuation only for a Registration whose requirement names continuation. Create admission refuses that Registration unless it is qualified or the qualification is authorized.

### The continuation mapping

- The mapping `storyos.volcengine-agent-plan.responses.create.v2` sends `previous_response_id` when the Host selects an incremental continuation. The Host selects it only for an eligible prior Model Continuation Binding of the same Project Conversation (ADR 0033). The binding must have the same Project Scope, destination and evidence revision, Registration, mapping, use binding, and compatibility Decision.
- Each request sends the Host instructions, because instructions do not carry forward. Each request also sends the current author request, Working Target, instruction binding, candidate text, and declared targets. A prior reference never replaces current input.
- Without a selected reference, the request sends the same current input with no reference. A changed boundary uses a new transport continuation in the same Conversation.
- The dispatch claim commits a request only when its prior reference is the reference of the binding that the claim decides again.
- The mapping sends `store: true`, no cache field, and no stream. Streaming stays with [S4-06b](https://github.com/FrankQDWang/StoryOS/issues/1107).

### A rejected prior reference

- The Agent Plan route answers a prior reference that it does not have with HTTP 400 and the native code `InvalidParameter.PreviousResponseNotFound` (live observation, 2026-10-12). The code does not tell an expired reference from another missing reference.
- For a request with a prior reference, the adapter maps this rejection to `continuation_unusable`, followed by the native code. Then the delivered recovery of S3-20 applies. The rejected Create keeps its Attempt and its Outbound Disclosure Event, and the binding becomes unusable. The same Run sends the full current input.
- The same code on a request without a prior reference stays an ordinary destination rejection.

### Cache evidence


- Each real Model Attempt records its cache setting: the destination default for implicit cache, and no request for explicit cache. Explicit cache is never requested, so its documented conflicts with instructions and structured output cannot occur.
- The Attempt records the cache use that the destination reports, `usage.input_tokens_details.cached_tokens`, separately from the usage kind. Absent cache use is unknown, never zero.
- StoryOS infers no cache hit and no saving from a setting, a stored response, or a reported count. The `getAgentRun` verification query shows these facts. No author view shows them (ADR 0040).

## Considered options

- Continuation with a pending profile and no new Registration was rejected. A Registration binds an exact mapping, and a changed mapping needs a new Registration.
- A Host-held transcript that each request sends in full was rejected for this route. The destination chain is the normal path of ADR 0033, and the Host keeps the full record.
- An incremental input that omits an unchanged Working Target was rejected. The current target must stay correct with a cache miss or an expired chain, and the Host does not know what the destination keeps.
- Explicit cache was deferred. It conflicts with the instructions that each request sends.

## Consequences

- Registrations `018f0000-0000-7001-8000-00000000fa04` (profile v3, pending continuation, the qualification build) and `018f0000-0000-7001-8000-00000000fa05` (profile v4, qualified continuation) follow `fa03`. An AgentRun that pinned an earlier head refuses with Registration drift. The next AgentRun of the Conversation uses a new transport continuation, because the Registration changed.
- The `getAgentRun` response has a new `cache` field. The generated contracts change with it.
- Confirmed expiry recovery on the real route stays with [S4-04: Recover an Uncertain Real-Model Attempt](https://github.com/FrankQDWang/StoryOS/issues/395).

## Amendment 2026-11-05: the Tool projection

[#1398](https://github.com/FrankQDWang/StoryOS/issues/1398) adds the Tool projection to the requests of the mapping `storyos.volcengine-agent-plan.responses.create.v4`. A non-empty Tool Exposure adds strict function declarations and `tool_choice: "auto"` to the request (ADR 0084). The head Registration `018f0000-0000-7001-8000-00000000fa17` keeps the mapping `v4` and its requirement. This is a recorded exception to the rule that a mapping with a new capability gets a new mapping revision and a new Registration. The author decided it on 2026-11-05, for these reasons:

1. [S5-10](https://github.com/FrankQDWang/StoryOS/issues/696) qualified function calling as one entry of the Model Capability Profile, outside `ModelCapability` (ADR 0049, with the pattern of ADR 0059). No requirement and no combination can name that entry today.
2. A request with an empty Tool Exposure is byte-identical to a request of the mapping `v4`. The mapping `v4` and the recovery requirement therefore stay the pinned mapping and requirement of the Registration, and a prior binding of the same Conversation stays eligible.
3. The qualified function calling entry of the pinned Registration admits the Tool projection. The live request of 2026-11-05 (response `resp_021793825651645295492bf4fae3051b4480808ec74710fc5549c`, logged on #1398, ADR 0052) is the route evidence of the projection.
4. The exception ends when a later Registration puts the entry into the requirement, with a new mapping revision. That happens at the latest with the [S5-09](https://github.com/FrankQDWang/StoryOS/issues/405) journey, which gives the combination evidence of streaming, continuation, compaction, and a function call in one live request. [S5-02b](https://github.com/FrankQDWang/StoryOS/issues/1399) and [S5-03](https://github.com/FrankQDWang/StoryOS/issues/399) do not need it.

A changed Tool Exposure digest starts a new transport continuation in the same Conversation. It does not change the processing boundary, so the expiry recovery of the same boundary continues.

## Amendment 2026-11-12: the tool loop mapping

[#1454](https://github.com/FrankQDWang/StoryOS/issues/1454) adds the mapping `storyos.volcengine-agent-plan.responses.create.v5` for the tool loop profile of [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md). It is the mapping `v4` with other Host instructions: they describe the Tools and the finish rule, and they hold no reply form. Each exposed Tool is a strict function, and `tool_choice` is `required` before a `finish` call that counted and `none` in the closing request. No Model Registration has the mapping `v5` yet, so the head Registration keeps the mapping `v4`.

## Amendment 2026-11-13: the head Registration takes the tool loop mapping

[#1461](https://github.com/FrankQDWang/StoryOS/issues/1461) gives the mapping `v5` to the new head Registration `018f0000-0000-7001-8000-00000000fa19`. It is Registration `018f0000-0000-7001-8000-00000000fa17` with the mapping `v5`, and it keeps the profile and the requirement of `fa17`. Thus the exception of the amendment of 2026-11-05 ends for the head Registration. An AgentRun that pinned `fa17` refuses with Registration drift. A binding with `fa17` does not offer the deployment destination, so the next `createAgentRun` writes new binding records (ADR 0048), and the next AgentRun of the Conversation uses a new transport continuation.

## Amendment 2026-11-14: the replaced input item of the mapping v5

[#1497](https://github.com/FrankQDWang/StoryOS/issues/1497) changes the exact mapping `storyos.volcengine-agent-plan.responses.create.v5`. On a request with a prior response whose current input changed, the mapping `v5` sends one user message item before the function outputs. The item has the replaced input label and then the current input (the amendment of #1497 to [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md) section 3). The item also goes out when the prior response has no function call. A request with an unchanged input has the same bytes as before. The mapping keeps the name `v5`, and the head Registration `018f0000-0000-7001-8000-00000000fa19` keeps the mapping `v5`. No mapping `v6` and no new Registration occur.

This is not an exception to the rule that a changed mapping needs a new Registration. The coordinator decided it on 2026-11-14 and reported it to the author, for these reasons:

1. The mapping `v5` and the Registration `fa19` exist only on the integration branch `agent-tool-loop` of the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451). `main` has neither of them. No release and no database of `main` holds an AgentRun, a binding, or a Model Continuation Binding of `fa19`. The development databases of the branch are disposable.
2. The rule protects an AgentRun, a binding, and evidence that pinned a Registration. No such record exists outside the disposable development databases. The first form of `v5` that `main` gets is the form with this item. A second Registration would give `main` a head Registration and a retired Registration that no deployment used.
3. The decision applies only until the aggregate pull request of #1451 merges into `main`. After that merge, each change of the exact mapping needs a new mapping revision and a new Registration, as this ADR states.

The live run records of 2026-11-14 for [#1464](../research/agent-tool-loop-live-run-2026-11-14.md) and [#1496](../research/unresolved-passage-request-live-run-2026-11-14.md) used `v5` before this item: the request after an opened Proposal had the full input mapping. The [record of #1497](../research/agent-tool-loop-changed-input-live-run-2026-11-14.md) used `v5` with this item. The real route accepted the item in the request with the response `resp_02179462183106202e43eb8440d3b95b4166e4756a96011d8618c`.
