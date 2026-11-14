---
status: accepted
---

# Compact a Run More Than One Time, Keep Tool Results as Stubs, and Keep the Recent Author Messages

The author approved the specification [#1507](https://github.com/FrankQDWang/StoryOS/issues/1507) on 2026-11-14. This ADR records its decisions. Each ticket of the specification adds its section. This version has the sections of [#1509](https://github.com/FrankQDWang/StoryOS/issues/1509): the Host Tool `read_tool_result`, the mapping `v6`, and the new Registrations. It amends [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 2 with a fourth Host Tool. It also has the section of [#1510](https://github.com/FrankQDWang/StoryOS/issues/1510): a compaction at each RunStep. That section amends [ADR 0055](0055-install-a-host-managed-compaction-summary-for-the-decision-request.md) (the amendment of 2026-11-13: one compaction for each Run, and the input limit after a compaction). It also has the section of [#1511](https://github.com/FrankQDWang/StoryOS/issues/1511): the stub of each Tool result that the model read before a compaction. That section amends [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md) section 3 and the rule "Tool results keep their form" of the amendment of 2026-11-13 to ADR 0055.

## Context

A later ticket of the specification makes each Tool result that the model read a short stub after a compaction. The stored result stays in PostgreSQL. The model needs a way to read that result again without a second search, which is a second external read with a second disclosure (ADR 0085, Considered options).

## Decision

### 1. The Host Tool `read_tool_result`

- It is a fourth Host Tool of ADR 0091. Its one argument `call_id` is the identity of a call: the `source` attribute of the `tool_result` tag of that result.
- The Host finds the call by this identity in the Project. A call of the same Run goes first. When more than one call of the Run has the identity, the Host takes the latest one in item order.
- It reads only a settled `web_search` or `web_fetch` call of the same Run. Its result is the bounded form of the first read of that call: the `tool_result` boundary with the source tag of that call, the bound of the Context item limit (ADR 0052), and the `tool_sources` tag of its citable sources (ADR 0087). The request after the read sends this text as it is, as the output of the read call. Thus `submit_research` can cite a source of that result.
- It refuses with a reason, and the Run continues (ADR 0091 section 3):

  | Refusal | Cause |
  | --- | --- |
  | `unknown_call` | No call of the Project has the identity. |
  | `call_of_other_run` | Only a call of another Run has the identity. |
  | `host_tool_call` | The call is a call of a Host Tool. |
  | `no_stored_result` | The call did not settle with a result, or it failed its validation before it got a Tool. |

- A Tool result has no restriction state, so the read has no refusal for a restricted source: the grant admission of the next request is the guard of the read result, as for each Tool result (ADR 0085 section 4). The coordinator decided this on 2026-11-14. The owner of a later restriction state for sources adds the refusal.
- The rules of ADR 0091 apply: no external destination, no Destination Attempt, no Outbound Disclosure Event, and no Source Snapshot of its own. The call uses a RunStep, has a ToolCall record, and is not in the public ToolCall list. Its result goes to the model as a `tool_result` source under the grant admission of each Tool result (ADR 0085 section 4).
- The Host instructions of the mapping `v6` add one paragraph that says when to call the Tool.

### 2. The mapping `v6` and the Registrations

- The request forms change, so the mapping gets the revision `storyos.volcengine-agent-plan.responses.create.v6` (ADR 0053). It is the mapping `v5` with the fourth function declaration and the paragraph of section 1.
- The head Registration `018f0000-0000-7001-8000-00000000fa20` is Registration `018f0000-0000-7001-8000-00000000fa19` with the mapping `v6`. It keeps the profile, the requirement, and the request bounds of `fa19`. No requirement capability changes, so it needs no new qualification before it becomes the head. The live ticket of the specification qualifies the forms of `v6` on the real route (ADR 0049).
- The Host Tool Registration `018f0000-0000-7001-8000-0000000e0102` has the four Host Tools. The Registration `018f0000-0000-7001-8000-0000000e0101` keeps the first three.
- A Model Registration declares the Host Tools of its mapping. The mapping `v6` and the fake tool loop mapping declare the four Host Tools. Each earlier mapping declares the first three. Thus an AgentRun that pinned `fa19` keeps the mapping `v5`, three Host Tools, and its exact request bytes. The adapter forms the requests of the mappings `v5` and `v6`, and it refuses a decision request of each other mapping.
- The fake tool loop Registration `018f0000-0000-7001-8000-00000000fa18` declares the four Host Tools with no new revision. It has no external destination, and no record outside a test database pinned it.
- The Tool Exposure digest changes with the fourth Tool, so the next AgentRun of a Conversation on `fa19` starts a new transport continuation (ADR 0053, amendment of 2026-11-05).

### 3. Later changes of `v6` on the integration branch

The later tickets of #1507 change the request forms of `v6` again: the stub outputs, the section of the recent author messages, the summary instruction, and the Host instructions. The mapping keeps the name `v6`, and the head Registration keeps `fa20`. The coordinator decided this for the reasons of the amendment of 2026-11-14 to ADR 0053: `main` has no `v6` and no `fa20` before the aggregate pull request of #1507, so no release and no database of `main` holds a record that pinned them. After that merge, each change of the exact mapping needs a new mapping revision and a new Registration.

### 4. Compaction at each RunStep

[#1510](https://github.com/FrankQDWang/StoryOS/issues/1510) adds this section.

- **Each RunStep.** The Host evaluates the threshold of ADR 0056 at each RunStep of a Run on a Registration with Host-managed compaction, also after a compaction of the Run. The expected input is the rule of the amendment of 2026-11-14 to ADR 0056. The rule has no gate for an older Registration: an AgentRun that pinned an older Registration refuses with Registration drift when the deployment offers `fa20` (ADR 0053). The coordinator decided this on 2026-11-14.
- **One compaction for each position.** One RunStep position has at most one compaction. The summary request, its Context Assembly, its Model Attempt, the compaction record, and the later request have the Decision position of the RunStep that compacts. The Host finds the summary claim, the record, and a refused install by the Run and the position.
- **The next summary.** The RunStep after a later request continues the response of the later request. Thus the summary request of a later position continues the Provider conversation that started at the earlier compaction. That conversation holds the earlier summary, so the new summary continues it. The Host keeps no separate chain of summaries, and the later request sends only the new summary. The summary request sends the output of each call of its prior response before its instruction (ADR 0055, amendment of 2026-11-13; ADR 0085; ADR 0091 section 5). The source of the new record is the later request of the earlier compaction.
- **The expected input after a later request.** The later request is a full input with no prior response, so the reported usage of its response has the summary, the current input, and each Tool result of the Run. The input digest has no summary item, so the response read the current input. Thus the next RunStep sends only the outputs of the calls of that response, and its expected input is that usage plus those outputs. The check for `conversation_too_long` uses the same value. A test with the scripted stream endpoint proves it: with an author message of 4,800 tokens, a reported usage of 810,546 input and 100 output tokens does not compact again, and 812,546 input tokens compact again.
- **Too long.** The AgentRun refuses before dispatch with `execution_refused` and `conversation_too_long` only in two cases:
  1. A necessary compaction does not install, and the expected input reaches the window after the output bound (ADR 0056). The summary request refused, no complete summary came, or the install check refused.
  2. The install check passes, but the full input of the later request reaches the window after the output bound. The full input counts each selected item of the later request (the summary, the current input, and each Tool result), each selection reference text, and each declared target text. Then the record stays `staged`, and the later request does not go out.

  The rejection of the destination for an over-long input (`input_too_long`) stays as the last guard (ADR 0056).
- **The install check.** The check of ADR 0055 applies to each compaction: a restricted source, missing Exact Required input, a changed input, and a summary over the item limit refuse the install.
- **Records and the public read.** The table `active_context_compactions` has the column `decision_position`. Its unique rule goes from one record for each Run to one record for each Run and position, in the release baseline. `getAgentRun` has the optional field `compactions`: each record of the Run in RunStep order, in the form of `active_compaction`. The field is absent when the Run has no record. The field `active_compaction` stays and holds the last record. No field goes. No author view shows a summary or a compaction fact (ADR 0040).
- **The Host fake.** The author message prefix `Compact active context between calls.` alone asks for one compaction, at the first RunStep that compacts, as before. The prefix with `At RunSteps <position> ...`, for example `At RunSteps 0 1.`, asks for a compaction at each named position. It is a test request (ADR 0056).
- **Tool results.** The request after a compaction still sends each Tool result of the Run in its exact form. A later ticket of the specification changes that.

  > Superseded by section 5 ([#1511](https://github.com/FrankQDWang/StoryOS/issues/1511)).

### 5. Tool result stubs

[#1511](https://github.com/FrankQDWang/StoryOS/issues/1511) adds this section.

- **Which results.** The later request of a compaction at the position P sends each function call of the Run, as before. A call is a stub when a decision request of the Run sent its output before the summary request of P. The summary request of P continues a prior response, and it sends the outputs of the calls of that response. These outputs wait for the later request, so they keep their bounded exact form. The Host finds them by the Decision of that response (the coordinator decided this rule on 2026-11-14). Each other call of the Run settled at an earlier RunStep, and the request after that RunStep sent its output, because each native function call needs its output (ADR 0085).
- **Which Tools.** Only the result of a `web_search` or `web_fetch` call and the result of a `read_tool_result` call can be a stub, and only with the state `ok`. `read_tool_result` cannot read the result of a Host Tool, so the result of `finish`, `propose_edit`, `submit_research`, and a refused read keeps its exact form.
- **Size.** A stub has at most 2,000 counted tokens (the count of ADR 0052). A result that is not larger than this limit keeps its exact form.
- **The stub.** The stub keeps the `tool_result` boundary with the identity of its call and the attribute `form="stub"`. Its text gives the Tool name, the canonical arguments of the call, and one sentence that tells the model to call `read_tool_result` with the identity of the call. The `tool_sources` tag of ADR 0087 follows, with one line for each Snapshot (its identity, its title when the result gave one, and its locator) and one line for each Gap (its identity, its locator, and its reason). Thus the model can cite a source of a stub with `submit_research`.
  - When all source lines do not fit, the tag keeps the first lines. The text gives the number of the sources and says that the model must read the result again before it cites one of the other sources.
  - When the arguments do not fit with the first source line, the Host cuts the arguments and ends them with a mark.
  - The stub of a `read_tool_result` call names the call that it read, and its `read_tool_result` sentence gives the identity of that call. It has no `tool_sources` tag, because the stub of that call has the sources.
- **Later full inputs.** The compaction record names each ToolCall whose result its later request sends as a stub. Each later RunStep of the Run selects the result of a ToolCall that an installed record names as a stub. Thus a full input after the compaction, for example after an expired prior reference, sends the stub again. An incremental request sends only the outputs of the calls of its prior response. These calls are newer than each installed compaction, so they are never stubs.
- **Records.** A stub is a Context item of the source class `tool_result` with the projection mode `tool_result_stub`. Its considered source keeps the token count of the exact result. The grant scope does not change, and the stub goes through the grant admission of each Tool result. A Tool result has no restriction state (section 1), so the stub has no restriction check.
- **The input limit.** The check for `conversation_too_long` after the install (section 4) counts the stubs, because it counts the selected items of the later request.
- **The summary request.** It gets no stub. It sends the waiting outputs in their exact form, as before.
- **Host instructions.** The mapping `v6` adds one paragraph after the paragraph of section 1: it says what a stub is, that the model can cite a source of a stub, and that the model calls `read_tool_result` for the text of the result or for a source that the stub does not give. The mapping stays `v6`, and the head Registration stays `fa20` (section 3).
- **The public read.** The compaction record in `getAgentRun` gets the optional field `stub_tool_call_ids`, absent when the later request sent no stub. The public `ProjectionMode` of a selected item gets the value `tool_result_stub`, because the decision Context Assembly of a RunStep after a compaction selects the stubs.
- **The Host fake.** The fake destination refuses a request with the reason `stub_call_missing` when a stub that the request sends names a call that the request does not send: as its own call, or as the call that a read stub read. A research script and a read script can follow the compaction prefix.

## Considered options

- **The read result is the raw stored text, and the next request projects it again with the identity of the read call.** Rejected. The model would see a second source tag for the same result, and the `tool_sources` tag would have no Snapshot, so `submit_research` could not cite the sources.
- **Execute the research Tool again.** Rejected for the reason of ADR 0085: a second exchange is a second external read with a second disclosure.
- **A separate chain of summaries, or one summary request for each earlier summary.** Rejected. The Provider conversation of the later request already holds the earlier summary, so the next summary request continues it.
- **A stub for each result, also for a short result or the result of a Host Tool.** Rejected. A stub of a short result is not smaller, and `read_tool_result` cannot read the result of a Host Tool.
- **A stub for the waiting outputs of the summary request.** Rejected. No decision request sent them, so the model did not read them.

## Consequences

- An AgentRun that pinned `fa19` refuses with Registration drift after the deployment offers `fa20`, as for each earlier head (ADR 0053).
- Section 1 and section 2 need no migration and no public schema change. The row of the Host Tool Registration `e0102` appears at its first ToolCall.
- Section 4 changes the release baseline and adds one optional field to `getAgentRun`. The local database has no data to keep.
- Section 5 adds the column `stub_tool_call_ids` to `active_context_compactions` in the release baseline, one optional field to the compaction record of `getAgentRun`, and the projection mode `tool_result_stub`.
