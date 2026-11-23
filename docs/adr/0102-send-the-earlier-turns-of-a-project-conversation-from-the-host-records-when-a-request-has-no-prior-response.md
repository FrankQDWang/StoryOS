---
status: accepted
---

# Send the Earlier Turns of a Project Conversation From the Host Records When a Request Has No Prior Response

The author decided this on 2026-11-23 in conversation. It is the first part of the answer to [the decision ticket of the Host-held conversation record](https://github.com/FrankQDWang/StoryOS/issues/1406). [The ticket of the earlier turns](https://github.com/FrankQDWang/StoryOS/issues/1826) delivers it. The second part of that ticket stays open (section 7).

## Context

At `main` `6da701fb`, the model gets the earlier turns of a Project Conversation only from the Provider conversation that `previous_response_id` continues ([ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md)). A request with a full input and no prior response sends only the current turn. Such a request occurs in these cases:

1. The destination does not have the prior response. The Agent Plan route keeps a stored response for 72 hours.
2. The Model Registration, the mapping, the destination, or the use binding changed. Each release that changes the mapping gives a new Registration (ADR 0053). Thus the next Run of each open Conversation after such a release is such a request.
3. The Tool Exposure changed.

The panel shows a notice only for case 1 ([ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) section 10). In each case, the model does not know what the author and the assistant said before.

The Host records already hold each turn. A Run has its author message, each steering input is a retained record, and the Model Attempt of the last Decision of a completed Run has the final answer. The Conversation capture of Agent Memory reads these records (ADR 0094 section 5). The later request of a compaction sends the earlier messages of the author from them ([ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) section 6).

The author will use another model for production, and rates the Claude series and the Gemini series as the best for Chinese prose ([ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) section 6). A Provider that keeps no conversation on its server needs the earlier turns in each request. The rule of this ADR is a part of each such route.

## Decision

### 1. The Host records are the conversation record

The Host sends the earlier turns of a Project Conversation from the records that it has. No table is new, and no record is a copy of another record.

### 2. The earlier turns

- An earlier turn is one earlier completed Run of the author in the same Project Conversation. A Run that is refused, cancelled, or in progress gives no turn. These are the Runs that the Provider conversation of ADR 0053 holds.
- A turn has these messages, in this order: the author message of the Run, each steering input that a RunStep of the Run read, and the final answer text of the Run. A Run with no final answer text gives only the messages of the author.
- Each message has its exact text. The Tool calls and the Tool results of an earlier turn are not a part of it. The model reads the manuscript and Agent Memory with its Tools, and the current input has the current Working Target.
- A message of the author has the source class `author_instruction`, as in ADR 0093 section 6. A final answer has a new source class for an earlier answer of the assistant. It is Data-only Context: it is model output, and it has no Instruction Authority.

### 3. Which request sends them

- A decision request of a Run of the author sends the earlier turns when it has a full input and no prior response.
- A request that continues a prior response does not send them, because the Provider conversation holds them. The rule is the rule of the Skill of a Conversation ([ADR 0089](0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md) section 5): the items are not a part of the current input digest (ADR 0085), and the expected input of a request that continues a prior response does not count them (ADR 0056). The expected input of a full input counts them.
- After a compaction of the Run is installed, no request of the Run sends them. The summary and the earlier messages of the author of ADR 0093 section 6 are in their place.
- A summary request, a request of a background Run, and a qualification probe do not send them.

The Agent Plan route keeps the prior response as its normal path. ADR 0053 refused a Host-held transcript that each request sends in full, and that stays.

### 4. The limit

The earlier turns of one request have at most 100,000 counted tokens (the count of ADR 0052). The Host takes whole turns from the newest. The first turn that does not fit ends the selection, and the request says that older turns are left out. A turn with a message above the Context item limit of ADR 0052 does not fit. The coordinator set the number on 2026-11-23: it is about one tenth of the context window of the pinned model, and a turn has no Tool result, so the limit holds many turns.

### 5. The request form

The implementing ticket records the exact texts, the mapping revision, the Model Registration, and the grant scope revision here and in ADR 0053 and [ADR 0051](0051-make-the-project-destination-grant-a-standing-grant.md).

- **The source class.** A final answer has the source class `earlier_answer`.
- **The Operation Requirement.** The field `earlier_turns` of the Operation Requirement of a RunStep records the earlier turns that its record selected. It has `sources`, in the order of the items, and `older_turns_left_out`. Each source has `run_id`, `message_id`, and `message_kind` (`author` or `answer`). The `message_id` is the identity of the Run for its author message, the identity of the steering input, or the identity of the Model Attempt of the final answer. It is also the Context Source Version of the item.
- **The section of the Agent Plan route.** In the user message, one section comes directly before the section `Author request`. Its heading is this exact text, with a colon after it:

  ```text
  Earlier turns of this conversation, oldest first. Each turn has the exact words of the author and your final answer. The Tool calls and the Tool results of those turns are not here
  ```

  When the limit left out one or more turns, the next line is `Older turns are left out.`, and one empty line follows it. Then each turn follows, with one empty line between two turns and between two messages. The first line of a turn is `[turn <n>]`, where the oldest turn that the request sends has the number 1. Each message of the author has the line `Author:` before its text, and the final answer has the line `Assistant:` before its text. A request with no earlier-turn item has the bytes of the mapping `storyos.volcengine-agent-plan.responses.create.v11`.
- **The mapping and the Model Registration.** The mapping is `storyos.volcengine-agent-plan.responses.create.v12`, and the head Model Registration is `018f0000-0000-7001-8000-00000000fa26` (ADR 0053, amendment of 2026-11-23). The fake tool loop Registration `fa18` keeps its revision, and the wire of the fake destination has the earlier turns.
- **The grant scope.** The scope revision `project_writing_assistance_v7` names the source class `earlier_answer` (ADR 0051, amendment of 2026-11-23).

### 6. The notice of the panel goes

The sentence "这段对话已超过 72 小时，助手可能不记得之前的内容。" is not true after this change, so the panel does not show it. The field `earlier_turns_expired` of the Conversation read, its Core function, and the retention constant go with it. This amends ADR 0094 section 10.

### 7. What stays open

The decision ticket stays open for a Provider that keeps no conversation on its server:

- Inside one turn with Tool calls, such a Provider needs each step of the turn in each request, with its private data in the exact form that it returned. Examples are a `thinking` block with its `signature` of the Anthropic Messages API and a thought signature of the Gemini API. The Host keeps no such data today.
- The author selects the first such Provider, its account, and its cost (ADR 0096 section 6). The decision on that record and the adapter of that Provider come together, after a probe of the route.

## Considered options

- **Send the Host-held turns in each request of the Agent Plan route, and remove the prior response.** Refused. The route works, the prior response keeps each request small, and the recovery of ADR 0088 needs the stored response.
- **A new ordered item table for each Conversation now.** Refused. The records that exist give the turns of this decision. An item table has a reader only when a Provider needs the steps of a turn, and that decision waits for the Provider (section 7).
- **Send the Tool calls and the Tool results of the earlier turns.** Refused. They are large, and a result of the manuscript or of Agent Memory can be out of date. The model reads the current state with its Tools.
- **Make a summary of the older turns.** Refused. A summary request continues a prior response (ADR 0093), and this request has none. The limit of section 4 is sufficient.
- **Keep the notice and change its text.** Refused. The model gets the earlier turns, so the author needs no notice.

## Consequences

- After 72 hours, after a release with a new Registration, and after a changed Tool Exposure, the model gets what the author and the assistant said before in the Conversation.
- The first Run of each open Conversation after the deployment of the new Registration sends its earlier turns.
- A request with earlier turns is larger. The limit of section 4 bounds it.
- A route of a Provider with no conversation on its server uses the same rule for the earlier turns. It needs the decision of section 7 for the steps inside one turn.
