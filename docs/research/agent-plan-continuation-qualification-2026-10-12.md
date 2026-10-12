# Agent Plan continuation qualification, 2026-10-12

Evidence record of [S4-06a: Continue the Agent Plan Conversation With the Prior Response and Record Cache Evidence](https://github.com/FrankQDWang/StoryOS/issues/1106). It records the live requests that qualified the continuation mapping of the Volcengine Agent Plan Responses route (ADR 0053). The profile revision `storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v4` cites it. It is not a claim about model quality, Provider attention, retention, training, or cache savings.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072), 2026-10-11.
- Step 1 (qualification) planned 3 live requests and sent 3. Step 2 (qualified route) planned 4 and sent 4. No retry and no anomaly.
- Each request ran in one `storyos-worker --once` process. Each Worker log has one `create` dispatch for each Attempt, and no authorization header or key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh`, Server with `STORYOS_WORKER=0` |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v1`: 8192 output tokens, 180 s |
| Content | Synthetic Project with one Chapter, 雨夜, two paragraphs, made in the author view for this run |
| Conversation | One Project Conversation, `01a12f5f-9a03-7ae0-96e0-ec003902f179`, for all seven requests |

| Step | Build | Registration | Decision |
| --- | --- | --- | --- |
| 1 | Release package from commit `e5f64c11`, tree `cbc58049` | `018f0000-0000-7001-8000-00000000fa04`, pending profile v3, mapping `responses.create.v2` | `qualification_authorized`, link to the standing authorization |
| 2 | Release package from commit `c94dc71c`, tree `e887f612` | `018f0000-0000-7001-8000-00000000fa05`, qualified profile v4, mapping `responses.create.v2` | `qualified`, no link |

## Requests

Each terminal response reported `usage.input_tokens_details.cached_tokens = 0`. The Attempts record it as reported cache use, with the setting `implicit: destination_default, explicit: not_requested`.

| # | Time (UTC) | Purpose | Input mapping | Model Attempt | Wire digest | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 09:09:24 | First turn, an Advisory question | `none` | `01a12f5f-ba58-7222-a6cb-4d7d6e4ec5c8` | `sha256:837f4e78…` | Terminal Advisory, reference `resp_021791796165377d58e3edad71566bed3cb03770f22004fe04d3a` | 454 / 162 |
| 2 | 09:11:15 | Second turn, after an author edit of paragraph two | `incremental` | `01a12f61-6a6c-7c51-9a6b-fda104ecc124` | `sha256:3f257d87…` | Terminal Advisory. The body held `previous_response_id` of request 1, the instructions, and the edited paragraph. | 783 / 138 |
| 3 | 09:12:15 | Third turn with a fabricated reference (see below) | `incremental` | `01a12f62-55b4-7f11-991f-1bf6c5d1e0c0` | `sha256:55b9efa0…` | Rejected: `http_400:InvalidParameter.PreviousResponseNotFound` | unknown |
| 4 | 09:22:17 | First turn after the Registration change from `fa04` to `fa05` | `new_transport` | `01a12f6b-83d5-7463-9600-9e0babed0ec9` | `sha256:a3068484…` | Terminal Advisory. No `previous_response_id`. | 467 / 205 |
| 5 | 09:23:01 | Next turn | `incremental` | `01a12f6c-2de4-78b2-b8b9-e8ce7bc80f90` | `sha256:3c9a152d…` | Terminal Advisory. The body held `previous_response_id` of request 4. | 749 / 219 |
| 6 | 09:23:45 | Next turn with a fabricated reference | `incremental` | `01a12f6c-da6d-75e1-ad10-5916768a341f` | `sha256:8ca70cf8…` | Rejected and mapped to `continuation_unusable:http_400:InvalidParameter.PreviousResponseNotFound`. The Attempt role is `rejected_create`. | unknown |
| 7 | 09:23:45 | The delivered recovery in the same AgentRun as request 6 | `full` | `01a12f6c-daef-76b0-a854-6d818714e4a8` | `sha256:54e10a1a…` | Terminal Advisory. The reference recovery is `rebuilt`, and the AgentRun completed. | 469 / 141 |

Total reported usage: 2,922 input and 865 output tokens.

The input token count of request 2 (783) is more than that of request 1 (454), and the count of request 5 (749) is more than that of request 4 (467). This agrees with the general Ark statement that `previous_response_id` brings the earlier input and output into the new context. It is not proof of what the model used.

## Fault injection on the disposable database

Requests 3 and 6 needed a prior reference that the destination does not have. Before each request, an admin statement on the disposable qualification database replaced the stored response reference of the prior binding with a fabricated value of the same shape. The Host then sent that value through the normal Model Gateway sequence, with its own admission, Attempt, and Outbound Disclosure Event. No other record changed. This was not a product path, and the database was deleted after the run.

## What this evidence qualifies

- Continuation on the exact route: the destination accepted `previous_response_id` of a stored response of the same Conversation, with the instructions sent again.
- The combination of transport, native text, and continuation.
- The native rejection of a missing prior reference, `InvalidParameter.PreviousResponseNotFound`, and its mapping to `continuation_unusable` with a rebuild in the same AgentRun.
- A changed Registration made a new transport continuation in the same Conversation.

## What it does not qualify

- Expiry: the rejection code does not tell an expired reference from another missing reference. Confirmed expiry recovery stays with S4-04.
- A cache hit or a saving. Each response reported zero cached input tokens. StoryOS infers no cache behavior.
- Streaming, explicit cache, retrieval, abort, and compaction. Their profile entries keep their earlier evidence.
- Model quality, Provider attention, retention, and training (S4-REQ-007).

## Observations for later owners

- After request 3, the panel showed the author request with no result sentence. The AgentRun completed with a destination rejection and no Agent Decision. ADR 0040 asks for one plain-language sentence about the outcome. This behavior is from before this ticket, and it applies to each ordinary destination rejection.
