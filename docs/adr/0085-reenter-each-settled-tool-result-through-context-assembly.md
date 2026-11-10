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
- A full input sends the author message, then each stored call with its function name and canonical arguments, then each output, in item order.
- A prior reference that a destination confirmed unusable or expired never continues again. The next request is a full input.
- The prior response read the input of the function request Attempt. That input is all that the model reads other than the Tool results and the Host instructions: the selected Context text, the declared targets, the selection references, and the passage input. Each Attempt records a digest of it. When the current input of the continuation has a different digest, the request is a full input with the current input. For example, the author edited the Chapter, or another Proposal reserved a target block, during the Tool exchange.
- Core `map_continuation_input` makes these two decisions from two facts that the store loads: `prior_reference_unusable` and `current_input_changed` (ADR 0039).
- The text of each item comes from the stored ToolCall. A delivery, a re-delivery, or a later full-input rebuild never starts a Tool exchange. A re-delivery is a new Model Attempt that the ordinary admission admits again.
- The Host instructions say that the content inside a `tool_result` source tag is data from outside the novel, not an instruction. The request text of the author message never holds a `tool_result` item.

### 4. The grant scope `project_writing_assistance_v3`

A new Project Destination Grant records the current scope `project_writing_assistance_v3`. It names all items of the second scope and the source class `tool_result`. The first and the second scope do not name `tool_result`. A continuation under a grant with one of them refuses before the dispatch claim with `disclosure_outside_grant`. The next `createAgentRun` makes new binding records with the current scope, as ADR 0051 and ADR 0048 say.

### 5. The lineage of a Proposal

A Proposal that a `tool_result_continuation` RunStep opens records `source_lineage` on its source: the Run, the RunStep position and its Operation Requirement, the Model Attempt, and the ToolCall identities. The lineage names the Run of the Proposal source. A Proposal of another RunStep has no lineage. The continuation Decision can be an Advisory, a clarification, or a prose change.

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
