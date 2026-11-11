---
status: accepted
---

# Fence an Interrupted ToolCall and Classify It from Retained Records

S5-08 ([#404](https://github.com/FrankQDWang/StoryOS/issues/404)) of Stage 5 ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)) makes this decision. It applies AP-14 of the threat model and the Run Lease rules of [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md) to the StoryOS ToolCall of [ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md).

## Context

At `main` `800cd3ee`, `cancelAgentRun` settles each `dispatched` ToolCall `unknown` in the cancel transaction, and the next claim after a crash settles a `dispatched` ToolCall `unknown`. Three facts were not recorded. The settlement did not record which fence settled the ToolCall. A `pending` ToolCall of a cancelled Run stayed `pending`. The Worker continued the exchange after the fence, and the store discarded a result that arrived after the fence. The panel showed an unknown call as a failed call.

The MCP transport has no remote abort. The Exa server has no operation that tells whether a call ran. Thus the Host cannot stop a call that left, and it cannot reconcile an unknown call later.

## Decision

### 1. The fence

- The cancel transaction writes one ToolCall Cancellation fence on each `dispatched` ToolCall of the Run. The fence is `tool_calls.attempt.fence`. It records the cause `run_cancelled` and the Run fence token of the cancel. It also records `local_stop = drop_exchange` and `remote_abort = unsupported`. The same transaction settles the ToolCall `unknown` with its Tool Effect Outcome and its `tool_call_settled` Run Event. The cancel also increments the Run fence token.
- The cancel refuses each `pending` ToolCall of the Run with `run_cancelled`. It has no Destination Attempt and no Disclosure Event.
- The next claim after a lost lease writes the same fence with the cause `worker_restart` on a `dispatched` ToolCall, and settles it `unknown`. It never sends the ToolCall again.
- The Worker stops only after it reads a committed fence. The Tool Gateway renews the Run Lease before the exchange and during it. When a renewal finds that the lease is lost, the Gateway drops the exchange, or does not start it. The Host never claims that the server stopped.

### 2. The four classes

The class of a resolved ToolCall comes only from its row: its state, the settlement of its Destination Attempt, and the settlement of a late observation (`storyos_core::tool_call_class`).

| Class | Records |
|---|---|
| `not_attempted` | A refusal before the dispatch claim, a cancel before the claim, or an exchange that proved that the request did not leave the process. |
| `confirmed` | A terminal answer (a result or a native error) settled the ToolCall before any fence. |
| `partial` | The fence settled the ToolCall `unknown`, and then a terminal answer arrived. |
| `unknown` | The dispatch claim committed, and no terminal answer arrived. |

The row lock orders the settlement and the fence. A terminal answer that commits first settles the ToolCall, and the fence then finds no `dispatched` ToolCall. A fence that commits first makes each later answer a late observation.

### 3. A late result is evidence only

A fence can end the lease of a Worker while its exchange continues. The store keeps the observation of that Worker only on a ToolCall that a fence settled. The observation must have the same Destination Attempt, and the store keeps it one time. It writes `attempt.late_observation`. This record has the settlement and the Tool Effect Outcome. It also has a disclosure confirmation that refers to the Outbound Disclosure Event, and the validated result text with its digest and length. It sets the Tool Effect Outcome of the row to the effects that the observation confirmed, and it appends one `tool_call_late_result` Run Event with the class.

The state stays `unknown` and `result_text` stays empty. Thus the late result never enters a continuation input, never creates a Source Snapshot, and never supplies a Decision. The Outbound Disclosure Event keeps its state `outcome_unknown`. A late observation on a ToolCall that no fence settled yet is not kept: the next claim settles that ToolCall.

### 4. The reservation

The dispatch claim records a worst-case reservation of one request on the attempt. A `not_attempted` or `confirmed` class releases it. An `unknown` or `partial` ToolCall keeps it, also after a late result.

### 5. No automatic successor and no retry

An unknown or partial ToolCall has no automatic retry, because nothing can reconcile it. The one automatic successor of [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md) exists only for the first Decision position, before any ToolCall exists. A Model Attempt with an unknown outcome on a later position makes the Run wait with `outcome_unknown`. Recovery is the existing path of S4-04: the author sends the next message, a new Run starts, and a new ToolCall with fresh admission and its own dispatch claim runs when the model asks again. The old ToolCall stays as history. [ADR 0088](0088-recover-an-unknown-continuation-attempt-by-retrieval-and-one-bounded-successor.md) amends this rule for the Model Attempt: an unknown continuation Attempt gets the retrieval first and then one bounded successor. An unknown or partial ToolCall still has no retry: an unknown ToolCall of the Run is an unresolved effect, and it pauses that successor.

### 6. Visible state

`getAgentRun` shows an unknown or partial ToolCall with the outcome `unknown`. `listResearchSources` lists it in `unknown_calls`, with `late_result`. `late_result` comes from the class: it is true only for a `partial` ToolCall. A late answer with no terminal result keeps the class `unknown` and `late_result` false. Each entry has the function request Decision of the call. The panel shows one line for each unknown call. A call of the Decision that the panel shows is in the list of the calls of that Decision. Each other unknown call comes from `unknown_calls`, and its line follows the research sources of the Run. An example is a call of an earlier Decision that a continuation replaced:

- `可能已联网搜索，但没有收到结果：<query>`
- `可能已阅读，但没有收到结果：<URLs>`

### 7. Contract Fault Points

The Tool Gateway reports `ToolContextCommitted` after the store committed the Context Assembly of a ToolCall, and `ToolDispatchClaimed` after its dispatch claim committed.

## Considered options

- **Settle the ToolCall only from the Worker observation.** Rejected. A Worker that crashed never reports, and the cancel must not wait for it.
- **Discard a late result.** Rejected. The result proves the disclosure and the read, and AP-14 keeps such evidence for reconciliation.
- **Make a late result the result of the ToolCall.** Rejected. The Run was cancelled or continued without it; a later rebuild would then show a result that the model never read.
- **Keep the late text out of the record and keep only its digest.** Rejected. The retention contract keeps destination and effect evidence until inspection ends, and a digest alone cannot be inspected.

## Consequences

- An uncertain Model Attempt on a continuation RunStep gets the retrieval and one bounded successor ([ADR 0088](0088-recover-an-unknown-continuation-attempt-by-retrieval-and-one-bounded-successor.md)). When the recovery stops without a result, the author starts a new conversation (ADR 0057).
- S5-09 (#405) can read the class of a cancelled ToolCall from `listResearchSources` and from the Run Events.
