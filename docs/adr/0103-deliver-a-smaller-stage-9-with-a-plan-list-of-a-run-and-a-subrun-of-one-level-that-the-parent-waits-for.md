---
status: accepted
---

# Deliver a Smaller Stage 9 With a Plan List of a Run and a Subrun of One Level That the Parent Waits For

The author decided this on 2026-11-23 in conversation. [ADR 0101](0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 1 gives the order of the work after Stage 11, and its item 3 is a smaller Stage 9: the list of the plan first, then the Subruns, and the triggers last or not at all. This ADR decides that item. It releases Stage 9 from its EXECUTION HOLD with the scope of this ADR.

## Context

The Stage 9 of the [delivery continuation](../foundation/storyos-product-delivery-continuation.md) has six requirements and nine tickets ([#367](https://github.com/FrankQDWang/StoryOS/issues/367)): plans, Subruns with nested children, Advisory joins, a mailbox, follow-up tasks, the interrupt of a child, proactive triggers with a schedule, and a model policy. [ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) says that these are abilities of a general agent.

At `main` `5e043ab7`, the code has no plan record, no Subrun (only the refused capability name `subrun` of the fake profile), no trigger, and no model policy. A Run of the author has at most 24 RunSteps (ADR 0091). The author can send a steering input, pause the Run, and cancel it. The Worker is one loop: it holds one claim at a time, and the Tool Gateway dispatches the ToolCalls of one Agent Decision in their order.

The coordinator made a survey of three coding agents on 2026-11-23 ([record](../research/subagent-design-survey-2026-11-23.md)):

- Grok Build and Claude Code give a child a new context: its own instructions, the project instructions, and the brief that the parent writes. The current Codex generation copies the full history of the parent by default, and the generation before it gave a new context.
- In each one, the result of a child is only its last message, and a child cannot ask the user.
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
- the model policy ([S9-08](https://github.com/FrankQDWang/StoryOS/issues/442)). It waits for the decision on the first new Model Provider ([ADR 0102](0102-send-the-earlier-turns-of-a-project-conversation-from-the-host-records-when-a-request-has-no-prior-response.md) section 7).

The glossary keeps the terms of the Subrun Messages, of the Subrun Mailbox, of the Subrun Interrupt, and of the Proactive Trigger, and each one gets the line that this ADR defers it.

### 2. The plan list

- **The Tool.** A Run of the author has the Host Tool `update_plan`. Its argument is the complete list: 1 through 12 steps, and each step has a text of one line with at most 100 characters and one state of `pending`, `in_progress`, and `completed`. At most one step is `in_progress`. Each accepted call replaces the list. A refused call gives its reason as its result. A background Run and a Subrun do not have the Tool.
- **Who decides.** The assistant judges by itself if the work is complex enough for a plan. The length of the request is not a rule, and the Host has no rule. A simple task has no plan.
- **No approval.** The work continues after the call. The author uses the steering input and the cancellation that exist.
- **The record.** No table is new. The plan of a Run is the arguments of its newest accepted `update_plan` ToolCall. Each accepted call is one RunPlan Revision: a list of steps, with no goal, no dependencies, and no rationale, and no RunStep binds it. A step has a text and a state, and it has no identity across two revisions: the PlanStep of the glossary is deferred.
- **The read.** `getAgentRun` gives the plan of the Run, when it has one.
- **The panel.** The author chose this form on 2026-11-23. The turn of the Run shows the complete list, with a mark for each step: done, in work, or not started. While the Run works, the list is below the status line. After the Run ends, the list stays in the turn above the results. The list has no control. A Run with no plan shows as before. The panel shows the newest list.
- **The end of a Run.** The Host changes no step. When a Run ends with a step that is not `completed`, the panel shows the list as the assistant recorded it.
- **Not a part of it.** A plan gives no capability, no budget, and no authority (glossary, RunPlan). The limit of 24 RunSteps does not change. Stage 9 delivers no layered timeline and no binding of a plan revision to a Step Snapshot.

The implementing ticket records the exact texts of the Tool and of the instruction, the mapping revision, and the Registrations.

### 3. The Subrun

- **One level.** A Run of the author can start a Subrun with a Host Tool. A Subrun cannot start a Subrun.
- **The parent waits.** The ToolCall of the parent settles with the result of the Subrun. The parent sends no request while the Subrun works. The Tool surface must stay the same when a later ADR adds an asynchronous form.
- **A new context.** A Subrun gets a short instruction text of its own, the Project Instruction of the author, and the brief that the assistant writes in the call. It gets no message and no ToolCall of the Project Conversation. This is the Subrun Context Bundle of the glossary: the context of the parent does not go into it implicitly.
- **Fewer Tools.** A Subrun has the Tools that read: the manuscript, Agent Memory, and the research Tools when the Run of the author has them. It cannot open a Proposal, write Agent Memory, record a plan, or ask the author.
- **The result.** The result is the text of the final answer of the Subrun, and nothing more. The assistant gets no ToolCall of the Subrun.
- **Control.** A cancelled or paused parent cancels or holds its Subrun. The author gets no control for a Subrun alone.
- **Open, and the author decides each one before the Subrun ticket starts:** the first use of a Subrun, and how the panel shows a Subrun. An amendment of this ADR records the two answers and the limits (the number of Subruns of one Run and the RunSteps of a Subrun).

### 4. The release of the stage

A second documents change gives the Requirements, the journey `S9-JRN-001`, and the evidence rows of Stage 9 the scope of this ADR in the delivery continuation, and it binds the proof documents to them. It merges before the journey ticket of the stage starts. Until then, this ADR is correct where it and the delivery continuation are different. Stage 8 keeps its hold, and a later ADR decides it (ADR 0101 section 1 item 4).

## Considered options

- **Deliver the Stage 9 of the earlier contract.** Refused by the author. Nested children, a mailbox, triggers, and a model policy are abilities of a general agent, and no present need of the author asks for them.
- **A plan table with revisions, PlanSteps with identities, and a layered timeline.** Refused. The ToolCall record already holds each version of the list, and the author asked for one list in the panel.
- **The assistant waits for an approval of the plan.** Refused. The author refused an approval prompt for the research Tools before ([ADR 0083](0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md) section 4), and the steering input can change the direction at each RunStep.
- **One status line with only the current step.** Refused by the author. The complete list shows what the assistant intends before it does the work.
- **The child gets the complete Project Conversation (the default of the current Codex generation).** Refused. The author sees it as not clean, the copy makes each request of the child large, and two of the three agents of the survey give a new context.
- **An asynchronous Subrun now.** Refused for this stage; the author chose the waiting form first on 2026-11-23. It needs concurrent Runs in the Worker, a wait of the parent with a rule for its end, and the insertion of a late result. The assistant needs the result of a Subrun for its next step in most work.

## Consequences

- The author sees what the assistant intends to do in complex work, and can change the direction early.
- The assistant can give reading work to a Subrun, and its own context stays small.
- A new Host Tool changes the Tool Exposure. The next request of each open Project Conversation has no prior response, and it sends the earlier turns (ADR 0102).
- Six of the nine tickets of Stage 9 close as not planned. A later ADR is necessary for each withdrawn part.
