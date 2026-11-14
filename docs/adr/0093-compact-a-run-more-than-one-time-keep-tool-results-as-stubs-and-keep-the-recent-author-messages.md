---
status: accepted
---

# Compact a Run More Than One Time, Keep Tool Results as Stubs, and Keep the Recent Author Messages

The author approved the specification [#1507](https://github.com/FrankQDWang/StoryOS/issues/1507) on 2026-11-14. This ADR records its decisions. Each ticket of the specification adds its section. This version has the sections of [#1509](https://github.com/FrankQDWang/StoryOS/issues/1509): the Host Tool `read_tool_result`, the mapping `v6`, and the new Registrations. It amends [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 2 with a fourth Host Tool.

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

## Considered options

- **The read result is the raw stored text, and the next request projects it again with the identity of the read call.** Rejected. The model would see a second source tag for the same result, and the `tool_sources` tag would have no Snapshot, so `submit_research` could not cite the sources.
- **Execute the research Tool again.** Rejected for the reason of ADR 0085: a second exchange is a second external read with a second disclosure.

## Consequences

- An AgentRun that pinned `fa19` refuses with Registration drift after the deployment offers `fa20`, as for each earlier head (ADR 0053).
- No migration and no public schema change are necessary. The row of the Host Tool Registration `e0102` appears at its first ToolCall.
