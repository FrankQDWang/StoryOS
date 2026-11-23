---
status: accepted
---

# Deliver a Smaller Stage 9 With a Plan List of a Run and a Subrun of One Level That the Parent Waits For

The author decided this on 2026-11-23 in conversation. [ADR 0101](0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 1 gives the order of the work after Stage 11, and its item 3 is a smaller Stage 9: the list of the plan first, then the Subruns, and the triggers last or not at all. Item 2 of that order has its first part on `main` ([ADR 0102](0102-send-the-earlier-turns-of-a-project-conversation-from-the-host-records-when-a-request-has-no-prior-response.md)), and its second part waits for the decision of the author on the first new Model Provider. The author started item 3 on 2026-11-23. This ADR decides that item. It releases the plan list of Stage 9 from the EXECUTION HOLD. The Subrun of Stage 9 stays on that hold until the amendment of section 3. That amendment is section 3A, of 2026-11-23.

## Context

The Stage 9 of the [delivery continuation](../foundation/storyos-product-delivery-continuation.md) has six requirements and nine tickets ([#367](https://github.com/FrankQDWang/StoryOS/issues/367)): plans, Subruns with nested children, Advisory joins, a mailbox, follow-up tasks, the interrupt of a child, proactive triggers with a schedule, and a model policy. [ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) says that these are abilities of a general agent.

At `main` `5e043ab7`, the code has no plan record, no Subrun (only the refused capability name `subrun` of the fake profile), no Proactive Trigger, and no model policy. A Run of the author has at most 24 RunSteps (ADR 0091). The author can send a Steering Input, pause the Run, and cancel it. The Worker is one loop that works on one Run at a time, and the Tool Gateway dispatches the ToolCalls of one Agent Decision in their order.

The coordinator made a survey of three coding agents on 2026-11-23 ([record](../research/subagent-design-survey-2026-11-23.md)):

- Grok Build and Claude Code give a child a new context: its own instructions, the project instructions, and the brief that the parent writes. The current Codex generation copies the full history of the parent by default, and the generation before it gave a new context.
- In each one, the result of a child is only its last message. In Grok Build and Claude Code, a child cannot ask the user.
- Codex is asynchronous: the call returns at once, and the result comes as mail before the next model request of the parent. Claude Code waits by default. A mailbox is the base of Codex only; Grok Build has it off by default, and Claude Code has it only for teams.

## Decision

### 1. The scope

Stage 9 delivers two things, in this order:

1. **The plan list of a Run** (section 2).
2. **A Subrun of one level that the parent waits for** (section 3).

Stage 9 does not deliver these parts of its earlier contract, and their tickets close as not planned:

- a Subrun of a Subrun, and an Advisory join ([S9-03](https://github.com/FrankQDWang/StoryOS/issues/437));
- Subrun Messages, a Subrun Follow-up, the Subrun Mailbox, its Backpressure, and its Seal ([S9-04](https://github.com/FrankQDWang/StoryOS/issues/438));
- the Subrun Interrupt ([S9-05](https://github.com/FrankQDWang/StoryOS/issues/439));
- the Proactive Trigger and its schedule ([S9-06](https://github.com/FrankQDWang/StoryOS/issues/440), [S9-07](https://github.com/FrankQDWang/StoryOS/issues/441));
- the model policy ([S9-08](https://github.com/FrankQDWang/StoryOS/issues/442)). The ADR that selects the first new Model Provider decides a model policy. This amends [ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) section 6, which says that the ADR of the vendor fits the destination to the model policy of Stage 9;
- the budget reservation of a Subrun, the Resource Hold, and the Safety Hold of the earlier contract. The limits of a Run that exist apply, and section 3 gives the limits of a Subrun.

The glossary keeps the terms of the Subrun Messages, of the Subrun Mailbox, of the Subrun Interrupt, and of the Proactive Trigger, the Model Routing Policy, and the Model Route Override, and each one gets the line that this ADR defers it. A term that this stage delivers in part gets a line with the delivered part.

### 2. The plan list

- **The Tool.** A Run of the author has the Host Tool `update_plan`. Its argument is the complete list: 1 through 12 steps, and each step has a text of one line with at most 100 characters and one state of `pending`, `in_progress`, and `completed`. At most one step is `in_progress`. Each accepted call replaces the list. A refused call gives its reason as its result. A background Run and a Subrun do not have the Tool.
- **Who decides.** The assistant judges by itself if the work is complex enough for a plan. The length of the request is not a rule, and the Host has no rule. A simple task has no plan.
- **No approval.** The work continues after the call. The author uses the Steering Input and the cancellation that exist.
- **The record.** No table is new. The plan of a Run is the arguments of its newest accepted `update_plan` ToolCall. Each accepted call is one RunPlan Revision: a list of steps, with no goal, no dependencies, and no rationale, and no RunStep binds it. A step has a text and a state, and it has no identity across two revisions: the PlanStep of the glossary is deferred.
- **The read.** `getAgentRun` gives the plan of the Run, when it has one.
- **The panel.** The author chose this form on 2026-11-23. The turn of the Run shows the complete list, with a mark for each step: done, in work, or not started. While the Run works, the list is below the status line. After the Run ends, the list stays in the turn above the results. The list has no control. A Run with no plan shows as before. The panel shows the newest list.
- **The end of a Run.** The Host changes no step. When a Run ends with a step that is not `completed`, the panel shows the list as the assistant recorded it.
- **Not a part of it.** A plan gives no capability, no budget, and no authority (glossary, RunPlan). The limit of 24 RunSteps does not change. Stage 9 delivers no layered timeline, and the Step Snapshot of a RunStep binds no plan revision.

[#435](https://github.com/FrankQDWang/StoryOS/issues/435) gives the exact texts and the revisions:

- **The texts of the Tool.** The description is `DESCRIPTION` of `crates/storyos-core/src/update_plan.rs`, and `parameters()` of that file gives the argument. The ToolSpec is `UPDATE_PLAN_TOOL_SPEC` (`storyos.tool.update-plan.v1`).
- **The results.** An accepted call gets `PLAN_RECORDED`. A refused call gets `PlanRefusal::result_text` of the first rule that applies, in the order of the step count (`plan_step_count`), the step text (`plan_step_text`), and the count of the steps `in_progress` (`plan_in_progress_count`). A call with arguments of another form gets the refusal of the intake that each other Host Tool gets.
- **The instruction.** The paragraph is `UPDATE_PLAN_INSTRUCTION` of `crates/storyos-adapter-volcengine-responses/src/wire.rs`. A request that declares `update_plan` has it directly after the instructions of the tool loop.
- **The revisions.** The Host Tool Registration is `018f0000-0000-7001-8000-0000000e0106`. The mapping is `storyos.volcengine-agent-plan.responses.create.v13` (`AGENT_PLAN_RUN_PLAN_MAPPING_REVISION`), and the head Model Registration is `018f0000-0000-7001-8000-00000000fa27` ([ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md), amendment of 2026-11-23 for the mapping `v13`).

### 3. The Subrun

This section gives the rules that the author decided. It is not a contract that a ticket can implement: it does not give the records of a Subrun, and it does not give the settlement of the ToolCall of the parent. The Subrun stays on EXECUTION HOLD until an amendment of this ADR gives them (see "Open" below). No Subrun ticket is ready for an agent before that amendment: the tickets [#436](https://github.com/FrankQDWang/StoryOS/issues/436) and [#443](https://github.com/FrankQDWang/StoryOS/issues/443) keep the hold, and their bodies of the earlier contract do not apply.

- **One level.** A Run of the author can start a Subrun with a Host Tool. A Subrun cannot start a Subrun.
- **The parent waits.** The ToolCall of the parent gets its result when the Subrun ends. The parent Run sends no model request while the Subrun works. The Tool surface must stay the same when a later ADR adds an asynchronous form.
- **A new context.** A Subrun gets a short instruction text of its own, the Project Instruction of the author, and the brief that the assistant writes in the call. It gets no message and no ToolCall of the Project Conversation. This is the Subrun Context Bundle of the glossary: the context of the parent does not go into it implicitly.
- **Fewer Tools.** A Subrun has a fixed set of Tools: Tools that read the manuscript and Agent Memory, and `finish`. A Subrun cannot open a Proposal, record a Research Artifact, write Agent Memory, or record a plan, and it cannot ask the author. This ADR gives a Subrun no research Tool. The amendment can add `web_search` and `web_fetch` when the first use needs them, and then it gives the capture rule of a Source Snapshot for a Subrun. Each Tool of a Subrun is a Tool that its parent Run has, and no Grant Revision record is new.
- **The end and the result.** A Subrun ends by the finish rule of a Run ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md)) or at its own limit of RunSteps. The assistant gets no ToolCall of the Subrun. When the parent Run can continue, the assistant gets the final answer text of the Subrun as the result of its ToolCall, or a fixed sentence when the Subrun ended with no final answer. The amendment gives each case in which the parent Run cannot continue. The outcome PartiallySucceeded of the glossary is not delivered. No Mailbox delivers the result, and no Subrun Result Disposition is recorded: the next Agent Decision of the parent uses the result or does not use it.
- **Limits and usage.** A Subrun has its own limit of RunSteps, and a Run of the author has a limit of Subruns. The usage of a Subrun is recorded with the Subrun.
- **Control.** The pause and the cancellation of the parent Run apply to its Subrun: a Subrun starts no work while its parent Run is paused, and it does no work after its parent Run was cancelled or ended. The author gets no control for a Subrun alone.

  > Amended on 2026-11-23: section 3A gives one exception. The Worker does the duties of a cancelled Run for a Model Attempt of a cancelled Subrun that was in flight.
- **Open.** An amendment of this ADR records these points before the Subrun ticket is published:
  - the two answers of the author: the first use of a Subrun, and how the panel shows a Subrun;
  - the record of a Subrun and of its Subrun Result;
  - the settlement of the ToolCall of the parent in each case: a final answer, no final answer, a Subrun that the Host refuses, a cancelled parent Run, a paused parent Run, and a lost lease. The amendment gives it with the rules of ADR 0091 for a Host Tool and of [ADR 0086](0086-fence-an-interrupted-tool-call-and-classify-it-from-retained-records.md) for an interrupted ToolCall, it says if the ToolCall of the parent can become `unknown` and what the assistant gets after that, and it says what occurs to a ToolCall and to a Model Attempt of the Subrun that are in work then;
  - the exact list of the Tools of a Subrun, the two limits, the exact sentences, the request form, the source class of the brief, the mapping revision, and the Registrations.

  > Amended on 2026-11-23: section 3A records the two answers, the record, the settlement in each case, the list of the Tools, and the two limits. As for the plan list, the implementing tickets record the exact sentences, the request form, the name of the source class, the mapping revision, and the Registrations.

### 3A. The records of a Subrun (amendment of 2026-11-23)

> This section amends section 3. It replaces the "Open" list of section 3, and it ends the EXECUTION HOLD of the Subrun. The author gave the two answers on 2026-11-23, and the coordinator made a survey of the code at `main` `5cecb944`.

**The first use.** A Subrun reads the earlier Chapters and answers one question about the manuscript, for example the earlier history of a character. Its result is the answer with the Chapters that it comes from. The assistant decides by itself when to start a Subrun, and the Host has no rule.

**The Tool.** A Run of the author has the Host Tool `start_subrun`. Its arguments are a `title` (one line, 1 through 40 characters; the panel shows it) and a `brief` (1 through 4,000 characters; the complete task). A Run of the author can have at most 4 Subrun Requests. The Host examines this limit only for a call that has no Subrun Request: a second execution of an accepted call, after a pause or a lost lease, continues its Subrun. The Tool needs no approval. A refused call gives its reason as its result. A background Run and a Subrun do not have the Tool.

**The record.**

- A Subrun is one row of `agent_runs` with the new kind `subrun`. It has its own RunSteps, Model Attempts, ToolCalls, and usage, as each Run has them. As each kind that is not `author`, it has no Conversation, no Chapter, and no author message. It binds the grant, the use binding, the Model Registration revision, and the Project Instruction revision of its parent Run.
- A new table has one row for each accepted call: the Subrun, its parent Run, the ToolCall of the parent, the title, and the brief. This row is the Subrun Request, and the ToolCall of the parent is its key: a second execution of the same call continues the same Subrun.
- The Subrun Result has no record of its own. It is the end status and the final answer of the Subrun. The Host copies the text to the result of the ToolCall of the parent in the transaction that settles that ToolCall.

**The execution.** The Worker does not select a Subrun as it selects a Run. The work of the ToolCall of the parent starts the Subrun and runs it to its end inside the claim of the parent Run, and the lease of the parent is renewed during that time. The Host stops the work of the Subrun when the claim of the parent Run is not valid. Thus the parent Run sends no model request while the Subrun works, a Subrun does not give way to another Run, and a Subrun does work only while the claim of its parent Run is valid. While a Subrun works, and while its parent Run is paused, the status of the Subrun is `claimed`; the selection of the Worker does not take it, because the selection does not have its kind. The one exception: the Worker selects a cancelled Subrun for the duties that a cancelled Run has for a Model Attempt in flight.

**The context.** A request of a Subrun has an instruction text of its own, the Project Instruction, the brief as one Context item of a new source class, the Chapter list that each Run has ([ADR 0095](0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md) section 1), the Memory Index of the Memory revision that its parent Run pinned, when its parent Run has Memory use ([ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)), and the Tool results of the Subrun. The Memory Index of a Subrun has the lines of the Index of its parent Run with no Character Intent, as the Index of a capture has them, and the read of a character entry in a Subrun gives no intent: only a Run of the author gets an intent ([ADR 0101](0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 5). The Tools of a Subrun take their arguments from the Chapter list and from the Memory Index. The grant scope of a Run of the author gets a new version, which names the source class of the brief and the Purpose of a Subrun request. No Grant Revision record of a Subrun is new, because a Subrun uses the grant of its parent. A Subrun does not compact, as a background Run: a request that is too long ends the Subrun.

**The Tools and the end.** A Subrun has `read_chapter`, `search_manuscript`, `read_memory_entry`, `search_memory`, and `hybrid_search_manuscript`, each one only when its parent Run has it, and `finish`. The finish rule of ADR 0091 section 4 applies: each request requires a call until a `finish` call counts, and the text of the closing response is the final answer. A Subrun has at most 12 RunSteps, and the request of the last RunStep has `tool_choice: "none"`. A Subrun knows this limit (section 5).

**The settlement of the ToolCall of the parent.** A Host ToolCall has no Destination Attempt (ADR 0091 section 2). For such a ToolCall, the database checks of `tool_calls` permit the states `pending`, `awaiting_approval`, `settled`, and `refused`, and they do not permit `dispatched`, `failed`, and `unknown`. A `start_subrun` call needs no approval, so its states are `pending`, `settled`, and `refused`. Its result is a `tool_result` item of the parent with the trust `data_only`, as the result of each Host Tool. A Subrun ends when its status is not `queued` and not `claimed`.

| Case | The ToolCall of the parent | What the assistant gets |
| --- | --- | --- |
| The Subrun ends `completed` with a final answer | `settled` | The final answer text |
| The Subrun ends with no final answer: `completed` with no text, `refused`, `waiting` after a Model Attempt with an unknown outcome, or `paused` with the successor reason ([ADR 0088](0088-recover-an-unknown-continuation-attempt-by-retrieval-and-one-bounded-successor.md) section 3) | `settled` | A fixed sentence that says that the Subrun ended with no result. When the cause is a request that was too long, the sentence gives that cause, so that the assistant can give a smaller task |
| The call is not valid, or the call has no Subrun Request and the Run has 4 Subrun Requests | `refused` | The reason, as a sentence |
| The author cancels the parent Run | `refused` with `run_cancelled`, as each `pending` ToolCall (ADR 0086) | Nothing: the Run is cancelled |
| The author pauses the parent Run | Stays `pending` | The result, after the Run continues |
| The claim of the parent Run loses its lease | Stays `pending` | The result, after the next claim |

- **Cancellation.** The cancel transaction of the parent Run also cancels each Subrun of the Run that did not end. The work of the Subrun in flight ends by the rules of a cancelled Run: a `pending` ToolCall of the Subrun becomes `refused` (ADR 0086), and a Model Attempt in flight gets the duties of a cancelled Run.
- **Pause.** The pause changes no record of the Subrun. The Subrun does no work, because only the claim of the parent Run runs it. The stopped work records no response that arrives after the pause. When the Run continues, the next claim of the parent continues the same Subrun, and a Model Attempt of the Subrun that was in flight is recovered by the rules of a Run.
- **Lost lease.** The next claim of the parent Run finds the `pending` ToolCall and its Subrun Request, takes the Subrun with a new fence, and continues it. A Model Attempt of the Subrun that was in flight is recovered by the rules of a Run.
- **A stopped recovery.** The recovery rules of a Run can stop a Run as `waiting` or as `paused` with the successor reason (ADR 0088 section 3). No command continues a Subrun, so each of the two is an end of the Subrun with no final answer.

**The reads.**

- `getAgentRun` gives the Subruns of the Run in the order of their calls, when the Run has one: the identity, the title, and one state: `working` (the Subrun is `queued` or `claimed`), `completed` (it is `completed` with a final answer), `stopped` (it is `cancelled`), and `no_result` (each other status).
- A new read gives the record of one Subrun: the title, the brief, the state, its steps in order, and its final answer. A step is a text that the Subrun wrote or one ToolCall with the name of the Tool, its target (the Chapter, the Memory Entry, or the search text), and the count of the results of a search. The read gives no Chapter text and no Tool result text.
- No command for a Subrun exists.

**The panel.** The author chose this form on 2026-11-23. The turn of the Run shows one card for each Subrun, below the status line and below the plan list. The first line of the card is the state, and the second line is the title. The card opens a drawer from the right side, also while the Subrun works. The drawer shows the brief, the steps, and the final answer, and it has only a close control.

**Not chosen.**

- *The Model Attempts of the Subrun under the Run of the parent.* The records have one Model Attempt, one manifest, and one requirement for each role and position of a Run, and the finish rule, the limit of RunSteps, and each read select by the Run. A Subrun with its own row uses all of them with no change.
- *The parent Run gives its claim back, and the Worker selects the Subrun.* No status of a Run fits: the Worker never claims a `waiting` Run again, and it claims a `queued` Run at once. A new status changes the selection, the commands, and the panel.

The implementing tickets record the exact texts of the Tool and of the two instruction texts, the exact sentences of a refused call, the request form, the name of the source class, the new mapping revision, the new Model Registration revision, and the new Host Tool Registration revision.

### 4. The release of the stage

The [delivery continuation](../foundation/storyos-product-delivery-continuation.md), the [release baseline](../foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md), the [proof selection](../foundation/product-delivery-proof-selection.md), and the [verification gates](../foundation/deterministic-verification-and-failure-recovery-gates.md) still say that Stage 9 is on EXECUTION HOLD, and the delivery continuation and the proof selection have the Requirements, the journey `S9-JRN-001`, and the evidence rows of the earlier contract. A second documents change gives the four documents the scope of this ADR and binds the proof documents again. It merges before the journey ticket of the stage starts. Until then, this ADR is correct where it and one of the four documents are different. The EXECUTION HOLD ends for the plan list with this ADR, and it ends for the Subrun with the amendment of section 3 (section 3A). Stage 8 keeps its hold, and a later ADR decides it (ADR 0101 section 1 item 4).

### 5. The model knows its limit of RunSteps

The author decided this on 2026-11-23: a limit must not stop the work of the model with no notice. The model must know its limit, and it must know when its work comes near to the limit. At `main` `5cecb944`, no instruction text gives the limit of 24 RunSteps of a Run of the author, and the request of the last RunStep has `tool_choice: "none"` with no sentence that gives the cause.

- **The limit.** The instruction text of a Run gives its limit of RunSteps, for each kind that has more than 2 RunSteps: a Run of the author (24), a Subrun (12), and a Memory capture (8). The instruction text of a Memory capture gives its limit today.
- **Near the limit.** The decision request of each of the last 4 RunSteps of such a Run has one more sentence at the end of its instruction text. The sentence gives the number of RunSteps that are left. In the last RunStep, it says which calls the Host executes: none for a Run of the author and for a Subrun, whose response must be the final answer, and only the output call for a Memory capture.
- **Not in each request.** A sentence that changes in each request would change the instruction text of each request. The record of 2026-10-13 shows one request with a different instruction text that got no cached input on this route ([record](../research/agent-plan-compaction-threshold-qualification-2026-10-13.md)). Thus only the requests of the last 4 RunSteps have the sentence.
- **Other limits.** The declaration of a Tool gives the limit of that Tool, in its description or in the description of an argument, and a call above the limit is refused with a sentence that gives the limit: the 12 steps of a plan, which the description of the argument `steps` gives today (section 2), and the 4 Subruns of a Run (section 3A). A Subrun that ends because its request is too long gives the assistant a sentence with that cause (section 3A).

The implementing ticket records the exact sentences, the new mapping revision, and the new Model Registration revision: each change of the exact mapping needs the two ([ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) section 3). This section amends [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 5.

## Considered options

- **Deliver the Stage 9 of the earlier contract.** Refused by the author. Nested children, a mailbox, triggers, and a model policy are abilities of a general agent, and no present need of the author asks for them.
- **A plan table with revisions, PlanSteps with identities, and a layered timeline.** Refused. The ToolCall record already holds each version of the list, and the author asked for one list in the panel.
- **The assistant waits for an approval of the plan.** Refused. The author refused an approval prompt for the research Tools before ([ADR 0083](0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md) section 4), and the Steering Input can change the direction at each RunStep.
- **One status line with only the current step.** Refused by the author. The complete list shows what the assistant intends before it does the work.
- **The child gets the complete Project Conversation (the default of the current Codex generation).** Refused. The author sees it as not clean, the copy makes each request of the child large, and two of the three agents of the survey give a new context.
- **An asynchronous Subrun now.** Refused for this stage; the author chose the waiting form first on 2026-11-23. It needs concurrent Runs in the Worker, a wait of the parent with a rule for its end, and the insertion of a late result. The assistant needs the result of a Subrun for its next step in most work.

## Consequences

- The author sees what the assistant intends to do in complex work, and can change the direction early.
- The assistant can give reading work to a Subrun, and its own context stays small.
- A new Host Tool changes the Tool Exposure. The next request of each open Project Conversation has no prior response, and it sends the earlier turns (ADR 0102).
- Six of the nine tickets of Stage 9 close as not planned. A later ADR is necessary for each withdrawn part.
