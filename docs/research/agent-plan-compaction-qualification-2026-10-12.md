# Agent Plan compaction qualification, 2026-10-12

Evidence record of [S4-06c2b2: Qualify Host-Managed Compaction on the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/1123). It records the live requests that qualified the Host-managed compaction mapping of the Volcengine Agent Plan Responses route (ADR 0055). The profile revision `storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v8` cites it. It is not a claim about summary quality, semantic preservation, model quality, Provider attention, retention, training, or cache savings.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072), 2026-10-11.
- The plan was 3 live requests: a first turn, the summary request, and the later request. 3 were sent. No retry and no anomaly.
- One earlier compaction Run refused before dispatch, with no network request. The measurement build did not accept the mapping `responses.create.v4` in the adapter. Commit `6a4cc1ef` fixed it, and a new Run in the same Conversation sent the two compaction requests.
- Each Run settled in one `storyos-worker --once` process with the debug log. No Worker log has an authorization header or a key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh`, Server with `STORYOS_WORKER=0` |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v2`: 32,768 output tokens, 600 s |
| Build | Measurement commit `6a4cc1ef`, tree `4472a0c1`, tag `evidence/s4-06c2b2-compaction-measurement`. It is not for merge. |
| Registration | `018f0000-0000-7001-8000-00000000fa08`, pending profile v7, mapping `responses.create.v4`, decision `qualification_authorized` |
| Grant scope | `project_writing_assistance_v2` |
| Content | Synthetic Project with one Chapter, 渡口, two paragraphs, made in the author view for this run |
| Conversation | One Project Conversation, `01a130f9-ed83-70c1-90d5-f3c90b3110ae`, for all three requests |

## Requests

Each response reported `usage.input_tokens_details.cached_tokens = 0`.

| # | Time (UTC) | Purpose | Input mapping | Model Attempt | Wire digest | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 16:37:35 | First turn, a short Advisory | `none` | `01a130fa-0b77-7820-8329-1c627047d79d` | `sha256:a1dbf252…` | Terminal Advisory, reference `resp_021791823056457383bb4c6a1b19ad3ebc8164afdd107cd6b1af8` | 450 / 777 |
| 2 | 16:41:16 | Summary request of the compaction Run, with `previous_response_id` set to the reference of request 1 | (summary) | `01a130fd-6b9a-7220-a747-71e1433c6883` | `sha256:464c0391…` | Terminal summary of 336 characters, reference `resp_021791823277290aef91fec3a81c53339a5720099214cdede3acd` | 441 / 427 |
| 3 | 16:41:33 | Later request of the same Run: the summary first, then the author request and the Working Target | `new_transport` | `01a130fd-abe9-7a42-8765-f8ee8472e556` | `sha256:132d9013…` | Terminal Advisory that uses the earlier evaluation, reference `resp_02179182329375072ef85a6275c3d23ad0e45d46a442cf7166488` | 725 / 552 |

Total reported usage: 1,616 input and 1,756 output tokens.

The compaction record `01a130fd-abc7-7f90-964d-cda206c99903` has the producer `agent_plan_summary`, the mapping `responses.create.v4`, reported usage, and the loss fact `semantic_preservation_unknown`. Its install state is `installed`, with the Model Attempt of request 3. Request 3 sent no `previous_response_id`. Its first input section is "Summary of the earlier conversation", and its Context Assembly Manifest has the role `later_request` with the first item `conversation_summary` as `derived_summary`.

The author view showed the two answers and no summary or compaction fact.

## What this evidence qualifies

- A summary request on the exact route that continues the stored chain with `previous_response_id` and one Host instruction, streamed, with a complete summary message.
- A later request that sends that summary as Context with a new transport, and gets an answer that uses the earlier turn.
- The combination of transport, native text, continuation, streaming, and Host-managed compaction.

## What it does not qualify

- The semantic preservation or the quality of a summary.
- A summary of a long Conversation near the window limit. [S4-06c3](https://github.com/FrankQDWang/StoryOS/issues/1116) owns the threshold and a near-limit request.
- A dropped summary stream, a refused install, and a summary refusal. Deterministic tests cover them.
- Native compaction, retrieval, abort, explicit cache, a cache hit or a saving, Provider attention, retention, and training (S4-REQ-007).
