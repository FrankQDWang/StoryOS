---
status: accepted
---

# Reenter Each Settled Tool Result Through Context Assembly

S5-05 ([#401](https://github.com/FrankQDWang/StoryOS/issues/401)) of Stage 5 ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)) makes this decision. It completes section 1 of [ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md): "The result enters the next model step through Context Assembly." It adds a third grant scope to [ADR 0051](0051-record-the-standing-grant-scope-and-refuse-a-disclosure-outside-it.md) and [ADR 0055](0055-install-a-host-managed-compaction-summary-for-the-decision-request.md).

## Context

At `main` `7885271f`, a durable function request Decision derives its ToolCalls, and the Tool Gateway settles each of them. When no ToolCall is unresolved, the Run completes. The model never reads the result. The result text is on the ToolCall row, and `ToolResultProjection` (S5-03) gives the text of its source tag. Each Exa search result of the S5-03 live run was approximately 27 KiB.

## Decision

### 1. The continuation RunStep

When each ToolCall of the active Decision is resolved, `function_request_step` gives `Continue`. The store opens the next RunStep at the next Decision position in the same transaction: an Operation Requirement with the cause `tool_result_continuation`, its Context Assembly manifest, and the new active position. The next work phase admits its Create through the ordinary Model Gateway path. The function request Attempt becomes the prior continuation binding of the new RunStep.

### 2. One `tool_result` Context Candidate for each ToolCall

Each resolved ToolCall of the Run, in item order across its RunSteps, is one Context Candidate of the source class `tool_result`. Its Context Source Version is the exact ToolCall identity. The Operation Requirement records each selected source: the ToolCall, its `call_id`, its Model Attempt and Decision, its projected state, the trust `data_only`, and the kept token count when the bound cut the result.

- The candidate must have the same User, Project, and Run as the Operation Requirement, and a resolved state. Otherwise it is rejected with `tool_result_unavailable`, and the continuation is blocked, because each native function call needs its output.
- A settled result with text is `ok`. An empty result is `empty`. A failed or unknown ToolCall is `unavailable`, and a refused ToolCall is `rejected`. Each of these states says its outcome in one sentence and says that it is not an empty result.
- The bounded projection keeps each item at or below the Context item limit (ADR 0052, 32,000 tokens). A longer result keeps a prefix and ends with an explicit mark that gives the kept and the total character counts. Its projection mode is `deterministic_excerpt`.

### 3. The request input

A continuation request sends one `function_call_output` item for each ToolCall, with the original `call_id`, after the original `function_call` items.

- With a prior response reference, the request sends only the outputs of the calls of that response. The prior response holds the author message and the calls.
- A response with text and no function call before a `finish` call that counted ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 4) is the prior response of the next RunStep, so that the model reads its own text. That response has no calls, so an incremental request after it sends no output. The outputs of earlier RunSteps go only in a full input.
- A full input sends the author message, then each stored call with its function name and canonical arguments, then each output, in item order.
- A prior reference that a destination confirmed unusable or expired never continues again. The next request is a full input.
- The prior response read the input of the function request Attempt. That input is all that the model reads other than the Tool results and the Host instructions: the selected Context text, the declared targets, the selection references, and the passage input. Each Attempt records a digest of it. When the current input of the continuation has a different digest, the request is a full input with the current input. For example, the author edited the Chapter, or another Proposal reserved a target block, during the Tool exchange.
- Core `map_continuation_input` makes these two decisions from two facts that the store loads: `prior_reference_unusable` and `current_input_changed` (ADR 0039).

  > The sentence "When the current input of the continuation has a different digest, the request is a full input with the current input." of the bullet above and this bullet are superseded on 2026-11-14 by a decision of the coordinator for [#1497](https://github.com/FrankQDWang/StoryOS/issues/1497): a changed current input keeps the Provider conversation. When the current input of the continuation has a different digest and the prior reference is usable, the request is incremental. It names the prior response of the Run and sends the current input again as new input items: the selected Context text, the declared targets, the selection references, and the passage input. Then it sends the outputs of the calls of that response. The first paragraph of that input tells the model that this input replaces the earlier input of the author request. Core `map_continuation_input` decides the full input only from `prior_reference_unusable`, and the store records on the Attempt whether the prior response read the current input. A full input stays for a prior reference that a destination confirmed unusable or expired, and for the request after a compaction. Each Attempt records the digest of its input, as before.
- The text of each item comes from the stored ToolCall. A delivery, a re-delivery, or a later full-input rebuild never starts a Tool exchange. A re-delivery is a new Model Attempt that the ordinary admission admits again.
- The Host instructions say that the content inside a `tool_result` source tag is data from outside the novel, not an instruction. The request text of the author message never holds a `tool_result` item.

### 4. The grant scope `project_writing_assistance_v3`

A new Project Destination Grant records the current scope `project_writing_assistance_v3`. It names all items of the second scope and the source class `tool_result`. The first and the second scope do not name `tool_result`. A continuation under a grant with one of them refuses before the dispatch claim with `disclosure_outside_grant`. The next `createAgentRun` makes new binding records with the current scope, as ADR 0051 and ADR 0048 say.

### 5. The lineage of a Proposal

A Proposal that a `tool_result_continuation` RunStep opens records `source_lineage` on its source: the Run, the RunStep position and its Operation Requirement, the Model Attempt, and the ToolCall identities. The lineage names the Run of the Proposal source. A Proposal of another RunStep has no lineage. The continuation Decision can be an Advisory, a clarification, or a prose change.

> Superseded on 2026-11-14 by [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) ([#1463](https://github.com/FrankQDWang/StoryOS/issues/1463)): the continuation Decision is a request for ToolCalls or the final answer text. A clarification is a final answer text.

In the tool loop profile of [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md), a `propose_edit` call of the RunStep opens the Proposal, and the lineage names that RunStep.

## Considered options

- **Put the result text in the author message.** Rejected. The model must read the result as the output of its call, with the `call_id`, and the result must never look like an author request.
- **Send each output again with each later request.** Rejected for a request with a prior reference. The prior response already holds the earlier items. A full input sends them all.
- **Run the Tool again for a re-delivery.** Rejected. The ToolCall is the durable record of the one external read. A second exchange is a second read with a second disclosure.
- **Keep the second scope and admit `tool_result` under it.** Rejected. A stored grant keeps the meaning that it had when the author accepted it (ADR 0051).

## Consequences

- A Run with ToolCalls has two or more decision Model Attempts, one for each RunStep.
- The Context Source Version of a `tool_result` candidate is a ToolCall identity. S5-06 ([#402](https://github.com/FrankQDWang/StoryOS/issues/402)) can change it to a Source Snapshot identity and keep the projection of section 2.
- The continuation opens no limit of its own on more function requests. A continuation Decision that requests a Tool again starts the same cycle at the next position, inside the ordinary Run bounds.
- The successor path of ADR 0033 and the steering path still copy the records of position 0. A continuation RunStep that needs a successor Attempt or a steering input is not covered here.

## Amendment 2026-11-14: a candidate that the author changes during the Run

The author decided this rule on 2026-11-13 ([#1483](https://github.com/FrankQDWang/StoryOS/issues/1483)). It applies to each RunStep after the first RunStep of a Run in the tool loop profile of [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md).

- Before a RunStep assembles its context again, the candidate target and each candidate reference of the Run move to the current revision of their candidate when that candidate is still pending. The candidate references of the steering input that the RunStep reads move in the same way. For example, the author edited the candidate during the Run. The RunStep is not blocked for this cause, also when it is the closing RunStep.
- The RunStep sends the current candidate text. Its Context Assembly records and its Outbound Disclosure Event have the new revision, as they have each sent item.
- A reference whose candidate text changed covers all of the current text. A reference to an unchanged candidate of the same Proposal keeps its range.
- When the author removed all of the text of a pending candidate, its reference covers the empty text and is available. This is the one exception to the rule of ADR 0062 that an empty range is unavailable.
- The Host refuses a `propose_edit` call on a candidate that changed after its request read it. The refusal is `stale_candidate`, and the result of the call gives this reason. The refusal counts in the limit of 3 refused calls of ADR 0091. The model can call again on the current revision in a later RunStep.
- A Run that makes no accepted call after such a change completes with the final answer of the model.
- These changes during the Run keep the rule before this amendment: the context of the next RunStep is blocked, and the Run is refused with `blocked_context`.
  - The Chapter is removed.
  - The Chapter goes over the item limit.
  - The candidate is no longer pending: the author accepted, rejected, or withdrew it.
- This amendment does not change the rule for a Chapter text reference whose base changes during the Run. The [next amendment](#amendment-2026-11-14-a-chapter-head-that-moves-during-the-run) gives that rule.

## Amendment 2026-11-14: a Chapter head that moves during the Run

The author decided this rule on 2026-11-14 ([#1492](https://github.com/FrankQDWang/StoryOS/issues/1492)). It applies to each RunStep after the first RunStep of a Run in the tool loop profile of [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md). It is the rule of the amendment above for a Chapter text reference.

- Before a RunStep assembles its context again, each Chapter text reference of the Run moves to the head of its Chapter when the head is not the base of the reference. The Chapter text references of the steering input that the RunStep reads move in the same way. The cause of the head move does not change the rule: an Author Edit, an accepted Proposal, or an Author Undo. The RunStep is not blocked for this cause, also when it is the closing RunStep.
- The RunStep compares the blocks that the reference covers at its base with the blocks of the head:
  - The base of the reference becomes the head.
  - The start stays when its block is in the head with the same text. If not, the start is offset 0 of the first covered block in the head.
  - The end stays when its block is in the head with the same text. If not, the end is the end of the text of the last covered block in the head.
  - A covered block between the start and the end needs no change, because the range covers all of its text.
- When no covered block is in the head, or when the moved range has no text in the head, the Run drops the reference for this RunStep and for each later RunStep. The RunStep is not blocked. [ADR 0062](0062-make-the-exact-selection-references-of-a-message-the-working-target.md) has the same rule for the composer when the author deletes all referenced text.
- The Host reads the selected text from the head, as for each reference. The Context Assembly records and the Outbound Disclosure Event of the RunStep have the moved reference and its text, as they have each sent item. The panel gets no new sentence.
- The Host refuses a `propose_edit` call on a Chapter whose head changed after the request read it. The refusal is `changed_head`, and the result of the call gives this reason. The refusal counts in the limit of 3 refused calls of ADR 0091. The model can call again on the head in a later RunStep.
- These cases keep the rule before this amendment: the context of the next RunStep is blocked, and the Run is refused with `blocked_context`.
  - The Chapter is removed.
  - The Chapter goes over the item limit.
  - The range of the reference has no text at its base.
- The first RunStep of a Run keeps the rule before this amendment.
