---
status: accepted
---

# Recover an Unknown Continuation Attempt by Retrieval and One Bounded Successor

[#1429](https://github.com/FrankQDWang/StoryOS/issues/1429) makes this decision. The author chose option B on 2026-11-11 ([decision comment](https://github.com/FrankQDWang/StoryOS/issues/1429#issuecomment-6797083734)). It applies the recovery of [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md) section 5 and [ADR 0057](0057-retrieve-the-stored-response-of-an-unknown-agent-plan-create.md) to the continuation RunStep of [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md). [#1435](https://github.com/FrankQDWang/StoryOS/issues/1435) delivers the retrieval and this ADR. [#1436](https://github.com/FrankQDWang/StoryOS/issues/1436) delivers the successor, and it writes no second ADR.

## Context

At `main` `93674c51`, the retrieval and the one automatic successor exist only at Decision position 0. A Model Attempt with an unknown outcome on a continuation RunStep (Decision position 1 or later, after the ToolCalls of its Decision settled) makes the Run wait with `{"kind":"outcome_unknown","reason":"unknown_result"}` ([ADR 0086](0086-fence-an-interrupted-tool-call-and-classify-it-from-retained-records.md) section 5). The author then starts a new Conversation, and the research turn is lost: the Exa exchanges and the synthesis stream. In the live record, that stream ran for 5 min 50 s.

The route facts of ADR 0057 apply. Retrieval works only for a response that finished and that the destination stored. A stream that broke before its terminal event returns `404` later.

## Decision

### 1. Retrieval first

When a continuation Attempt has an unknown outcome, the Host records its retrieval subject as at position 0. The subject uses the response reference that the stream committed. When the Attempt retained a reference and the pinned Registration qualifies retrieval, the Host retrieves the original result of the continuation response.

- The Retrieve has its own Destination Attempt, Wire Payload Projection, and Outbound Disclosure Event (ADR 0057). Its retrieval Attempt (`attempt_role` `retrieval`), its Context Assembly manifest (`manifest_role` `retrieval`), and its Operation Requirement (`requirement_role` `retrieval`) have the Decision position of the interrupted Attempt. The manifest and the requirement are copies of the active Context Assembly of that position.
- The admission checks of ADR 0057 do not change. Assistance must be available, and the binding, the Registration, and the qualification must be current. A changed binding or destination keeps the subject unknown with `destination_mismatch`, and no Retrieve leaves the Host.
- The Host admits one Retrieve for one Attempt. A restart does not add a second one: a claimed Retrieve sends nothing again (ADR 0057).
- A complete retrieved response settles the continuation Attempt as the direct continuation does. Core gives its disposition (`response_disposition`). A `research_synthesis` or another Agent Decision goes through `validate_agent_decision`, and a synthesis records its Research Artifact ([ADR 0087](0087-record-a-research-synthesis-as-one-immutable-research-artifact.md)). A further function request goes through the batch validation of ADR 0084, and a refusal keeps its capability. The response usage belongs to the original Attempt as evidence. The Retrieve has no usage of its own, so nothing is charged twice.
- A `404`, another rejection, and an incomplete result keep the unknown outcome and the worst-case reservation of the original Attempt.

### 2. Then one bounded successor

When the outcome stays unknown after the retrieval, the Host permits at most one automatic successor Attempt for the interrupted continuation Invocation. The conditions of ADR 0033 section 5 apply:

- the same route and the same request;
- the current authorization;
- a budget that covers both Attempts;
- a persisted fence on the predecessor before the successor starts;
- a Recovery Decision and fresh disclosure evidence;
- no unresolved Tool effect. At position 1 or later, the ToolCalls of the Decision settled before the continuation started, so this condition holds by construction.

A Run Cancellation prohibits the successor. The successor sends the same stored `function_call_output` items, from the stored ToolCall results (ADR 0085), after the original `function_call` items, with the same `previous_response_id`. One physical submission is not charged or disclosed twice. The predecessor keeps its unknown usage and its worst-case reservation until evidence settles them.

### 3. Paused with a reason otherwise

When the conditions fail or the allowance is spent, the Run stops with an inspectable reason: it waits with `outcome_unknown` or pauses with the successor reason. The panel shows the stopped-request sentence of ADR 0057, and the author starts a new Conversation, as before.

`没有收到这次的结果，正文没有改动。请开始新对话重试。`

### 4. Delivery

#1435 delivers section 1. Until #1436 delivers section 2, the Run waits with `outcome_unknown` when the retrieval gives no result, and no successor marker is recorded at position 1 or later. This ADR amends the position-0 rule of ADR 0086 section 5.

## Considered options

- **A. Retrieval only.** Rejected. A stream that broke before its terminal event returns `404`, so retrieval alone does not recover it.
- **C. A resume in the same Conversation.** Rejected. It loses the synthesis of the turn, and it changes the chain rules of ADR 0033 section 6.
- **D. The status quo with a clearer sentence.** Rejected. Each lost turn repeats the Exa exchanges and the long synthesis stream.

## Consequences

- An unknown continuation Attempt whose response finished and was stored settles without a new Conversation.
- `getAgentRun` shows the retrieval of the latest Decision position that has one.
- #1436 adds the successor marker for a continuation Attempt where the retrieval subject is recorded, and the successor follows the retrieval as at position 0.
