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
