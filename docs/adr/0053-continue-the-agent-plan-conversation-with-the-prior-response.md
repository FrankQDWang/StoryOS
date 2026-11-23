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
- A Host-held transcript that each request sends in full was rejected for this route. The destination chain is the normal path of ADR 0033, and the Host keeps the full record. A request with a full input and no prior response sends the earlier turns of the Conversation from the Host records ([ADR 0102](0102-send-the-earlier-turns-of-a-project-conversation-from-the-host-records-when-a-request-has-no-prior-response.md)).
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

[#1497](https://github.com/FrankQDWang/StoryOS/issues/1497) changes the exact mapping `storyos.volcengine-agent-plan.responses.create.v5`. On a request with a prior response whose current input changed, the mapping `v5` sends one user message item before the function outputs. The item has the replaced input label and then the current input (the amendment of #1497 to [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md) section 3). The item also goes out when the prior response has no function call. A request with an unchanged input has the same bytes as before. The mapping keeps the name `v5`, and the head Registration `018f0000-0000-7001-8000-00000000fa19` keeps the mapping `v5`. No mapping `v6` and no new Registration occur. The tool loop instructions of the mapping `v5` also changed before the merge into `main`: [#1502](https://github.com/FrankQDWang/StoryOS/issues/1502) added the Proposal Growth rule and the Selection Reference rule, and the output of an unexecuted call of the last RunStep ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 5). The same decision applies to these changes.

This is not an exception to the rule that a changed mapping needs a new Registration. The coordinator decided it on 2026-11-14 and reported it to the author, for these reasons:

1. The mapping `v5` and the Registration `fa19` exist only on the integration branch `agent-tool-loop` of the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451). `main` has neither of them. No release and no database of `main` holds an AgentRun, a binding, or a Model Continuation Binding of `fa19`. The development databases of the branch are disposable.
2. The rule protects an AgentRun, a binding, and evidence that pinned a Registration. No such record exists outside the disposable development databases. The first form of `v5` that `main` gets is the form with this item. A second Registration would give `main` a head Registration and a retired Registration that no deployment used.
3. The decision applies only until the aggregate pull request of #1451 merges into `main`. After that merge, each change of the exact mapping needs a new mapping revision and a new Registration, as this ADR states.

The live run records of 2026-11-14 for [#1464](../research/agent-tool-loop-live-run-2026-11-14.md) and [#1496](../research/unresolved-passage-request-live-run-2026-11-14.md) used `v5` before this item: the request after an opened Proposal had the full input mapping. The [record of #1497](../research/agent-tool-loop-changed-input-live-run-2026-11-14.md) used `v5` with this item. The real route accepted the item in the request with the response `resp_02179462183106202e43eb8440d3b95b4166e4756a96011d8618c`.

## Amendment 2026-11-14: the head Registration takes the mapping v6

[ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) ([#1509](https://github.com/FrankQDWang/StoryOS/issues/1509)) gives the mapping `storyos.volcengine-agent-plan.responses.create.v6` to the new head Registration `018f0000-0000-7001-8000-00000000fa20`. It is Registration `fa19` with the mapping `v6`. An AgentRun that pinned `fa19` keeps the mapping `v5` and refuses with Registration drift after the deployment offers `fa20`.

## Amendment 2026-11-15: the head Registration takes the mapping v7

[ADR 0089](0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md) section 3 ([#1528](https://github.com/FrankQDWang/StoryOS/issues/1528)) gives the mapping `storyos.volcengine-agent-plan.responses.create.v7` to the new head Registration `018f0000-0000-7001-8000-00000000fa21`. It is Registration `fa20` with the mapping `v7`, and it keeps the profile, the requirement, and the request bounds of `fa20`. The mapping `v7` is the mapping `v6` with the Skill section of a request and the Skill paragraph of the Host instructions. A request of `v7` with no Skill item has the bytes of the `v6` request. An AgentRun that pinned `fa20` keeps the mapping `v6`. It refuses with Registration drift after the deployment offers `fa21`, and with `model_use_binding_stale` after its own Project binds `fa21`. The Stage 6 tickets change the mapping `v7` in place until the aggregate pull request of [#364](https://github.com/FrankQDWang/StoryOS/issues/364) merges into `main`, for the reasons of the amendment of 2026-11-14 above (ADR 0093 section 3).

## Amendment 2026-11-16: the head Registration takes the mapping v8

[ADR 0095](0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md) section 6 ([S7-B0](https://github.com/FrankQDWang/StoryOS/issues/1567)) gives the mapping `storyos.volcengine-agent-plan.responses.create.v8` to the new head Registration `018f0000-0000-7001-8000-00000000fa22`. It is Registration `fa21` with the mapping `v8`, and it keeps the profile, the requirement, and the request bounds of `fa21`, so it needs no new qualification. The mapping `v8` keeps each section, each instruction paragraph, and the summary instruction of `v7`. The mapping `v8` and the fake tool loop mapping declare the Host Tool Registration `018f0000-0000-7001-8000-0000000e0104`, which has the six Host Tools of `…0e0103`; the mapping `v7` keeps `…0e0103`. An AgentRun that pinned `fa21` keeps the mapping `v7`. It refuses with Registration drift after the deployment offers `fa22`, and with `model_use_binding_stale` after its own Project binds `fa22`. The Stage 7 tickets change the mapping `v8` and the Host Tool Registration `…0e0104` in place until the aggregate pull request of [#365](https://github.com/FrankQDWang/StoryOS/issues/365) merges into `main`, for the reasons of the amendment of 2026-11-14 above (ADR 0093 section 3).

## Amendment 2026-11-18: the head Registration takes the mapping v9

[ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) sections 5 and 6 ([#1700](https://github.com/FrankQDWang/StoryOS/issues/1700)) give the mapping `storyos.volcengine-agent-plan.responses.create.v9` to the new head Registration `018f0000-0000-7001-8000-00000000fa23`. It is Registration `fa22` with the mapping `v9`, and it keeps the profile, the requirement, and the request bounds of `fa22`, so it needs no new qualification. The mapping `v9` is the mapping `v8` with three changes of the Host instructions: the paragraph of `write_memory`, the instructions of a capture, and the instructions of a pass. The declared schema of each function does not change, so the mapping `v9` declares the Host Tool Registration `018f0000-0000-7001-8000-0000000e0104` of the mapping `v8`. An AgentRun that pinned `fa22` keeps the mapping `v8`. It refuses with Registration drift after the deployment offers `fa23`, and with `model_use_binding_stale` after its own Project binds `fa23`.

## Amendment 2026-11-20: the code keeps only the current Registration

[ADR 0097](0097-keep-only-the-current-model-registrations-and-request-mapping-in-the-code.md) removes the earlier Registrations and mapping branches from the code, because no stored Run names them. The rule for later is in that ADR.

## Amendment 2026-11-21: the head Model Registration takes the mapping v10

[ADR 0100](0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) section 2 ([#1761](https://github.com/FrankQDWang/StoryOS/issues/1761)) gives the mapping `storyos.volcengine-agent-plan.responses.create.v10` to a new head Model Registration. That Registration is `018f0000-0000-7001-8000-00000000fa24`.

- `fa24` is Registration `fa23` with the mapping `v10`. It keeps the adapter, the API surface, the provider model, and the requirement of `fa23`.
- It also keeps the capability profile and the request bounds of `fa23`. Thus it needs no new qualification.
- No capability of the requirement changes. The request form changes only in the text of the Host instructions and of one label.

The mapping `v10` is the mapping `v9` with three changes of the request text:

1. **The block of the writing rules.** Each tool-loop request has the block of the default writing rules for prose. It follows the first paragraphs of the tool loop and comes before the paragraph of `read_tool_result`. [#1774](https://github.com/FrankQDWang/StoryOS/issues/1774) changes three places of the block, by the findings of the live run of [#1762](https://github.com/FrankQDWang/StoryOS/issues/1762).
2. **The paragraph on the one `propose_edit` call** ([#1774](https://github.com/FrankQDWang/StoryOS/issues/1774)). The Host instructions of the tool loop say that StoryOS accepts one `propose_edit` call for the author request. The paragraph follows the paragraph on the joinable blocks, and it comes before the paragraph on the Author references. It states the refusal `after_proposal` of ADR 0062. The refusal stays until the Run gets a new message of the author.
3. **The label of the replaced input** ([#1774](https://github.com/FrankQDWang/StoryOS/issues/1774)). The label of ADR 0085 gets two sentences. StoryOS can remove a block with a pending candidate of a Proposal of the Run from the target blocks. When the text under "Author request" does not change, the author sent no new message and made no new selection.

Other facts of the mapping `v10`:

- The requests of a Conversation capture, of a consolidation pass, and of a summary do not change.
- The declared schema of each function does not change. Thus the mapping `v10` declares the Host Tool Registration `018f0000-0000-7001-8000-0000000e0104` of the mapping `v9`.
- The code keeps only `fa24` and the mapping `v10` (ADR 0097).
- The Stage 10 tickets change the mapping `v10` in place until the aggregate pull request of [#1760](https://github.com/FrankQDWang/StoryOS/issues/1760) merges into `main`. The reasons are those of the amendment of 2026-11-14 above (ADR 0093 section 3).

## Amendment 2026-11-22: the head Model Registration takes the mapping v11

[ADR 0101](0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 3 ([#1787](https://github.com/FrankQDWang/StoryOS/issues/1787)) gives the mapping `storyos.volcengine-agent-plan.responses.create.v11` to a new head Model Registration. That Registration is `018f0000-0000-7001-8000-00000000fa25`.

- `fa25` is Registration `fa24` with the mapping `v11`. It keeps the adapter, the API surface, the provider model, and the requirement of `fa24`.
- It also keeps the capability profile and the request bounds of `fa24`. Thus it needs no new qualification.
- No capability of the requirement changes. The request form changes in the text of the Host instructions, in the limit that the texts and the schema descriptions give, in the intent of a Run of the author (item 4), and in the new request of an intent pass (item 5).

The mapping `v11` is the mapping `v10` with five changes of the request form, for the two parts of a character file and its intent:

1. **The paragraph of `write_memory`.** A character can have two entries with the same title. The model writes what the author tells it to remember about a character and that the character knows into the entry with `"author_part": true`, and it creates that entry when the Memory Index does not list it. A fact that the character does not know goes into an entry of the type other. The model cannot change the other entry of the character.
2. **The Host instructions of a capture.** A paragraph gives the form of an observation of the type character: the view of the character and only what the character can know. The paragraph of the Memory Index says that the line with `"author_part": true` is the entry of the author. The paragraph of the delete marks says that a deleted entry with `"author_part": true` does not stop an observation of that character.
3. **The Host instructions of a consolidation pass.** The pass keeps one entry of the background work for each character beside the entry of the author, and a paragraph gives the form of the file of a character from the manuscript. The paragraph of the delete marks says that a deleted entry with `"author_part": true` does not stop a write of the entry of that character that comes from the manuscript.
4. **The intent of a character** ([S11-05](https://github.com/FrankQDWang/StoryOS/issues/1791), ADR 0101 section 5). In a Run of the author, the Memory Index line of a character entry of the background work with a current result has the field `"intent"`, and the result of `read_memory_entry` for that entry has the line `intent` after `changed_at`. A paragraph directly after the paragraph of the Memory Index and its read and search sentences says that an intent is a suggestion for the plot and not a fact or an instruction. The Host instructions of a capture have one sentence after the rule of a capture of a Project Conversation: an intent in an answer of the assistant is a suggestion of background work, and the capture records it only when a message of the author makes it a decision. A capture, a consolidation pass, and an intent pass get no intent and no question.
5. **The request of an intent pass** ([S11-03](https://github.com/FrankQDWang/StoryOS/issues/1789), ADR 0101 section 4). A Run of the kind `intent_pass` sends a request with the Purpose `intent_pass`, the one strict function `record_character_intent`, and `tool_choice: "required"`. Its only source is one user message with the section `Character file`. That section has the part of the author when that part has text, and the part of the background work. The Run sends at most two requests: the second one only after a refused call, with that call and its refusal as a Tool result. A valid call gives an intent of at most 150 characters and at most 3 questions of at most 100 characters each, and the Host refuses a question with a control character.

Other facts of the mapping `v11`:

- The limit of a character text is 4,000 characters. The texts of a capture and of a pass and the schema descriptions of `write_memory`, `record_memory_observations`, and `apply_memory_operations` give this number.
- The Memory Index line of a character entry of the author has the field `"author_part": true`, in a Run of the author and in a capture. The line of a deleted character entry of the author in the delete marks of a capture and of a pass has the same field.
- Each declared schema keeps its keys and its types, and only the descriptions give the new limit.
- [S11-03](https://github.com/FrankQDWang/StoryOS/issues/1789) adds the output Tool `record_character_intent` of an intent pass (ADR 0101 section 4). Thus the mapping `v11` declares the new Host Tool Registration `018f0000-0000-7001-8000-0000000e0105`: the Host Tools of `…0e0104` and `record_character_intent`. Only a Run of the kind `intent_pass` declares the new Tool. The code keeps only `…0e0105` (ADR 0097), and the Stage 11 tickets change it in place by the rule of the next item.
- The code keeps only `fa25` and the mapping `v11` (ADR 0097).
- The Stage 11 tickets change the mapping `v11` in place until the aggregate pull request of [#1786](https://github.com/FrankQDWang/StoryOS/issues/1786) merges into `main`. The reasons are those of the amendment of 2026-11-14 above (ADR 0093 section 3).
- [S11-12](https://github.com/FrankQDWang/StoryOS/issues/1818) changes the mapping `v11` in place by the rule of the previous item, after the live run of [#1792](https://github.com/FrankQDWang/StoryOS/issues/1792). The paragraph of `write_memory` says that a fact that the character knows goes into the entry of the author also when the author says that a later Chapter uses it, and that it is not foreshadowing. The sentences that end that paragraph say that an entry has no label, no explanation, and no title of another entry. The Host instructions of a capture and of a consolidation pass say that a detail of the narration is not what a character said. The Registration `fa25` and the Host Tool Registration keep their revisions.

## Amendment 2026-11-23: the head Model Registration takes the mapping v12

[ADR 0102](0102-send-the-earlier-turns-of-a-project-conversation-from-the-host-records-when-a-request-has-no-prior-response.md) section 5 ([#1826](https://github.com/FrankQDWang/StoryOS/issues/1826)) gives the mapping `storyos.volcengine-agent-plan.responses.create.v12` to a new head Model Registration. That Registration is `018f0000-0000-7001-8000-00000000fa26`.

- `fa26` is Registration `fa25` with the mapping `v12`. It keeps the adapter, the API surface, the provider model, and the requirement of `fa25`.
- It also keeps the capability profile and the request bounds of `fa25`. Thus it needs no new qualification.
- No capability of the requirement changes. The request form changes only in a decision request of a Run of the author with a full input and no prior response.

The mapping `v12` is the mapping `v11` with one change of the request form. Such a request sends the earlier turns of the Project Conversation from the Host records, in one section of the user message directly before the section `Author request` (ADR 0102 section 5). A request with no earlier-turn item has the bytes of the mapping `v11`, so a request that continues a prior response does not change. The code keeps only `fa26` and the mapping `v12` (ADR 0097).
