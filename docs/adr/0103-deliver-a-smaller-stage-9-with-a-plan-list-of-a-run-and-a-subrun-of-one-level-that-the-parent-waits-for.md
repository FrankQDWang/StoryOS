---
status: accepted
---

# Deliver a Smaller Stage 9 With a Plan List of a Run and a Subrun of One Level That the Parent Waits For

The author decided this on 2026-11-23 in conversation. [ADR 0101](0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 1 gives the order of the work after Stage 11, and its item 3 is a smaller Stage 9: the list of the plan first, then the Subruns, and the triggers last or not at all. Item 2 of that order has its first part on `main` ([ADR 0102](0102-send-the-earlier-turns-of-a-project-conversation-from-the-host-records-when-a-request-has-no-prior-response.md)), and its second part waits for the decision of the author on the first new Model Provider. The author started item 3 on 2026-11-23. This ADR decides that item. It releases the plan list of Stage 9 from the EXECUTION HOLD. The Subrun of Stage 9 stays on that hold until the amendment of section 3.

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

The implementing ticket records the exact texts of the Tool and of the instruction, the mapping revision, and the Registrations.

### 3. The Subrun

This section gives the rules that the author decided. It is not a contract that a ticket can implement: it does not give the records of a Subrun, and it does not give the settlement of the ToolCall of the parent. The Subrun stays on EXECUTION HOLD until an amendment of this ADR gives them (see "Open" below). No Subrun ticket is ready for an agent before that amendment: the tickets [#436](https://github.com/FrankQDWang/StoryOS/issues/436) and [#443](https://github.com/FrankQDWang/StoryOS/issues/443) keep the hold, and their bodies of the earlier contract do not apply.

- **One level.** A Run of the author can start a Subrun with a Host Tool. A Subrun cannot start a Subrun.
- **The parent waits.** The ToolCall of the parent gets its result when the Subrun ends. The parent Run sends no model request while the Subrun works. The Tool surface must stay the same when a later ADR adds an asynchronous form.
- **A new context.** A Subrun gets a short instruction text of its own, the Project Instruction of the author, and the brief that the assistant writes in the call. It gets no message and no ToolCall of the Project Conversation. This is the Subrun Context Bundle of the glossary: the context of the parent does not go into it implicitly.
- **Fewer Tools.** A Subrun has a fixed set of Tools: Tools that read the manuscript and Agent Memory, and `finish`. A Subrun cannot open a Proposal, record a Research Artifact, write Agent Memory, or record a plan, and it cannot ask the author. This ADR gives a Subrun no research Tool. The amendment can add `web_search` and `web_fetch` when the first use needs them, and then it gives the capture rule of a Source Snapshot for a Subrun. Each Tool of a Subrun is a Tool that its parent Run has, and no Grant Revision record is new.
- **The end and the result.** A Subrun ends by the finish rule of a Run ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md)) or at its own limit of RunSteps. The assistant gets no ToolCall of the Subrun. When the parent Run can continue, the assistant gets the final answer text of the Subrun as the result of its ToolCall, or a fixed sentence when the Subrun ended with no final answer. The amendment gives each case in which the parent Run cannot continue. The outcome PartiallySucceeded of the glossary is not delivered. No Mailbox delivers the result, and no Subrun Result Disposition is recorded: the next Agent Decision of the parent uses the result or does not use it.
- **Limits and usage.** A Subrun has its own limit of RunSteps, and a Run of the author has a limit of Subruns. The usage of a Subrun is recorded with the Subrun.
- **Control.** The pause and the cancellation of the parent Run apply to its Subrun: a Subrun starts no work while its parent Run is paused, and it does no work after its parent Run was cancelled or ended. The author gets no control for a Subrun alone.
- **Open.** An amendment of this ADR records these points before the Subrun ticket is published:
  - the two answers of the author: the first use of a Subrun, and how the panel shows a Subrun;
  - the record of a Subrun and of its Subrun Result;
  - the settlement of the ToolCall of the parent in each case: a final answer, no final answer, a Subrun that the Host refuses, a cancelled parent Run, a paused parent Run, and a lost lease. The amendment gives it with the rules of ADR 0091 for a Host Tool and of [ADR 0086](0086-fence-an-interrupted-tool-call-and-classify-it-from-retained-records.md) for an interrupted ToolCall, it says if the ToolCall of the parent can become `unknown` and what the assistant gets after that, and it says what occurs to a ToolCall and to a Model Attempt of the Subrun that are in work then;
  - the exact list of the Tools of a Subrun, the two limits, the exact sentences, the request form, the source class of the brief, the mapping revision, and the Registrations.

### 4. The release of the stage

The [delivery continuation](../foundation/storyos-product-delivery-continuation.md), the [release baseline](../foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md), the [proof selection](../foundation/product-delivery-proof-selection.md), and the [verification gates](../foundation/deterministic-verification-and-failure-recovery-gates.md) still say that Stage 9 is on EXECUTION HOLD, and the delivery continuation and the proof selection have the Requirements, the journey `S9-JRN-001`, and the evidence rows of the earlier contract. A second documents change gives the four documents the scope of this ADR and binds the proof documents again. It merges before the journey ticket of the stage starts. Until then, this ADR is correct where it and one of the four documents are different. The EXECUTION HOLD ends for the plan list with this ADR, and it ends for the Subrun with the amendment of section 3. Stage 8 keeps its hold, and a later ADR decides it (ADR 0101 section 1 item 4).

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
