---
status: accepted
---

# Replace the Typed Reply Form With Host Tools and End Each Run With an Explicit Finish Call

The author decided this on 2026-11-12 in conversation, before the Skill tickets of Stage 6 ([#364](https://github.com/FrankQDWang/StoryOS/issues/364)). This ADR changes how the model gives its result to StoryOS. It changes no rule about what a Proposal or a Research Artifact can hold, and it changes nothing that the author sees in a Run that succeeds.

## Context

At `main` `8aa2bcb3`, the model gives its result in two different ways:

- **A typed reply form.** The Host instructions tell the model to reply with one JSON object in the message text: an answer, a clarification, a prose change, or a research synthesis ([ADR 0062](0062-make-the-exact-selection-references-of-a-message-the-working-target.md), [ADR 0071](0071-let-the-agent-grow-a-block-proposal-over-adjacent-chapter-blocks.md), [ADR 0087](0087-record-a-research-synthesis-as-one-immutable-research-artifact.md)). The adapter parses that text. A text that does not parse becomes an answer, and the Web client finds the malformed reply by its text. A prose change or a synthesis that fails its validation gives the RunStep no Agent Decision, and the Run completes with no result. The model never reads the reason.
- **A function call.** Since Stage 5, the model asks for web research with a native function call. StoryOS validates the call, executes it, and gives the result to the next RunStep ([ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md), [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md)).

Other facts:

- Stage 6 adds more Tools that StoryOS itself executes. Each new Tool and each new test would be built on the two ways.
- A response that holds a function call with no output cannot be the prior continuation binding of the next Run. ADR 0085 section 2 gives the reason: each native function call needs its output. The next Run then continues from an older response, and the model does not read that turn.
- One Run has no limit on its RunSteps (ADR 0085, Consequences).
- The recorded probe of 2026-11-03 did not qualify `tool_choice`, parallel function calls, more than one function, or a failed function output ([probe record](../research/agent-plan-function-calling-probe-2026-11-03.md)). The probe of 2026-11-12 qualified `tool_choice: "required"`, three function declarations, two function calls in one response, and a function output that is empty or reports a refusal. It did not qualify `tool_choice: "none"` ([probe record](../research/agent-plan-tool-choice-probe-2026-11-12.md)).

## Decision

### 1. The model output of a RunStep is function calls or the final answer

The typed reply form goes. The Agent Decision of a RunStep is one of two things: a request for ToolCalls, or the final answer text of the Run. An ordinary answer and a question to the author are final answer texts. No Tool asks the author a question, because the panel shows a question as text, the same as an answer.

### 2. Three Host Tools

A Host Tool is a Tool that StoryOS executes in its own process. It has no external destination, so its ToolCall has no Destination Attempt and no Outbound Disclosure Event. Its ToolCall record, its validation, and its result stay. StoryOS adds three Host Tools:

- **`propose_edit`.** Its arguments are the block changes of one prose change. It runs the validation of the prose change reply (ADR 0061, ADR 0062, ADR 0071) and opens, revises, or grows the Proposal as that reply did. One call is one change set. Its result says that the Proposal waits for the decision of the author. One response accepts one `propose_edit` call, and the Host refuses a second call of the same response.
- **`submit_research`.** Its arguments are the summary and the cited claims of one research synthesis. It runs the validation of ADR 0087 and records the Research Artifact. One Agent Decision has at most one Research Artifact, as ADR 0087 section 4 says, so the Host refuses a second call of the same response. One Run records at most one Research Artifact: after a call that recorded an Artifact, the Host refuses each later `submit_research` call of that Run.
- **`finish`.** It has no arguments, and its result is empty. Section 4 gives its rule.

A Host Tool gives the Agent no new authority. A Proposal still changes the manuscript only when the author accepts it.

### 3. Each result returns to the model

No Tool stops a Run. Each ToolCall gets a result, and the result enters the next RunStep through the re-entry of ADR 0085. Thus each response with a function call has its outputs in the Provider conversation, and the next Run can continue from it.

A refusal is a result. A call that fails its validation becomes a refused ToolCall, and its result gives the reason in one sentence. The other calls of the same response continue. This replaces the rule that one invalid member rejects the whole batch, and the Run no longer ends with the refusal `tool_request_invalid`. The model can correct the call in the next RunStep.

### 4. A Run ends only after a finish call

The author decided this rule. The model ends its work with a call to `finish`. After the result of that call, the model writes the final answer, and the Run completes. A text with no earlier `finish` call does not end the Run.

- A `finish` call can be in one response with other calls. It counts only when no other call of that response was refused. Otherwise its result says that the Run continues.
- The probe of 2026-11-12 qualified `tool_choice: "required"` and did not qualify `tool_choice: "none"` ([probe record](../research/agent-plan-tool-choice-probe-2026-11-12.md)). The author approved the rule of the next three bullets on 2026-11-12.
- Each request before a `finish` call that counted has `tool_choice: "required"`. Thus a text with no `finish` call cannot end a Run.
- The request after a `finish` call that counted is the closing request, and it has `tool_choice: "none"`. The route does not enforce this value.
- If the closing response holds no function call, its text is the final answer, and the Run completes. If the closing response holds a function call, the `finish` call no longer counts, each call of that response gets its result as section 3 says, and the Run continues.

### 5. Limits

The author approved these numbers on 2026-11-12.

| Item | Limit |
| --- | --- |
| `propose_edit` calls of one Run that the validation can refuse | 3. After the third refusal, the Host refuses each later call, and the result tells the model to end the Run and to tell the author. |
| RunSteps of one Run | 24. The request of the last RunStep has `tool_choice: "none"`. The text of its response is the final answer. The Host executes no function call of that response. A response with no text completes the Run with no result. |

### 6. The author sees no change in a Run that succeeds

- The public `decision` of an AgentRun stays. The Host derives it from the records of the Run: a Proposal gives `prose_change` with its locations, a Research Artifact gives `research_synthesis`, and a Run with neither gives the final answer text.
- The panel shows a Run that made a Proposal as it does today: the Host sentence and the change locations, and a click opens the Chapter. The final answer text of such a Run is not shown.
- A Run in which the model makes an invalid call no longer ends with a fixed sentence. The model corrects the call, or it tells the author in the final answer.

### 7. Order

This change lands before the Skill tickets of Stage 6. Its first ticket is a recorded probe of the model route for `tool_choice` with the values that section 4 needs, more than one function, parallel function calls, and a failed function output. The Skill Tools of Stage 6 are then Host Tools of the same kind.

## Considered options

- **Keep the typed reply form until Stage 6 is released.** Rejected. Stage 6 adds Tools and tests on the two ways, and the change then moves them a second time.
- **A Tool that stops the Run, for example a `propose_edit` call that ends the turn.** Rejected. Its call has no output, so the next Run cannot continue from that response, and the model does not read its own Proposal.
- **A response with no function call ends the Run.** The coordinator recommended this for the first version, because it needs no extra request. The author rejected it: a sentence that the model writes between two calls must not end the Run by accident. The author accepted the cost of one more model request in each Run.
- **A finish call that carries the final answer as its argument.** Rejected. It is again a call with no output, and the answer would stream as function arguments.
- **A Tool that asks the author a question.** Rejected. The panel shows the question as text, and a Tool adds a request with no gain.
- **A fallback for a route that does not enforce `tool_choice`: the Host continues the Run one time with one Host sentence that tells the rule, and a second text with no `finish` call is the final answer.** Rejected after the probe of 2026-11-12. The route enforces `tool_choice: "required"`, and that value gives the rule that the author asked for.

## Consequences

- Each Run has two or more model requests. A Run that only answers a question had one.
- The parser of the reply form, the answer that an unparsed text becomes, the malformed reply sentence of the panel, and the refusal `tool_request_invalid` go.
- These sentences are superseded when the tickets land: the bullet "One Decision" of ADR 0062, the sentences of ADR 0071 that one reply is one Agent Decision and that the reply contract keeps its shape, the reply form of ADR 0087 section 1 and the repair sentence of its section 3, the last sentence of ADR 0085 section 5, and the batch rule of the glossary entry Model Tool Request. The validation rules of these ADRs stay, and they apply to one call.
- Steering, compaction, and the recovery of an unknown Attempt work today only at the first RunStep position or have a bounded rule for a continuation ([ADR 0088](0088-recover-an-unknown-continuation-attempt-by-retrieval-and-one-bounded-successor.md)). Each Run now has more than one RunStep, so the tickets must make these paths correct at each position.
- The fake model adapter emits function calls, and each test that it drives goes through the ToolCall path.
- The request mapping of the model route changes, so the Model Registration gets a new mapping revision ([ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md)).
- Tool Registration gets a kind for a Host Tool. The Skill Tools of Stage 6 use it.
- The External MCP Surface of [ADR 0082](0082-keep-room-for-an-external-mcp-surface.md) is not affected.
