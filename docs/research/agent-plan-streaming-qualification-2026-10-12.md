# Agent Plan streaming qualification, 2026-10-12

Evidence record of [S4-06b: Stream the Agent Plan Response as Typed Model Stream Events](https://github.com/FrankQDWang/StoryOS/issues/1107). It records the live requests that qualified the streaming mapping of the Volcengine Agent Plan Responses route and measured the event rate for the commit bound (ADR 0054). The profile revision `storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v6` cites it. It is not a claim about model quality, Provider attention, retention, training, or cache savings.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072), 2026-10-11.
- Step 1 (qualification and measurement) planned 3 live requests and sent 3. Step 2 (qualified route, selected bound) planned 2 and sent 2. No retry and no anomaly.
- Each request ran in one `storyos-worker --once` process with the debug log. Each Worker log has one `create` dispatch for each Attempt, and no authorization header or key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh`, Server with `STORYOS_WORKER=0` |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v1`: 8192 output tokens, 180 s |
| Content | Synthetic Project with one Chapter, 渡口, two paragraphs, made in the author view for this run |
| Conversation | One Project Conversation, `01a12fc3-2dd0-7fb1-8240-36ecac83f3a5`, for all five requests |

| Step | Build | Registration | Decision | Commit rule |
| --- | --- | --- | --- | --- |
| 1 | Release package from commit `bb86379a`, tree `2c69b4a9` | `018f0000-0000-7001-8000-00000000fa06`, pending profile v5, mapping `responses.create.v3` | `qualification_authorized` | Each event (bound 1), for the measurement |
| 2 | Release package from commit `5128fa8c`, tree `28340869` | `018f0000-0000-7001-8000-00000000fa07`, qualified profile v6 | `qualified` | 64 events or 500 ms (ADR 0054) |

## Requests

Each response reported `usage.input_tokens_details.cached_tokens = 0`. Commits are the closed `append_model_stream_events` spans of the Worker debug log. A commit time is the span duration, which includes the database round trips.

| # | Time (UTC) | Purpose | Input mapping | Model Attempt | Wire digest | Outcome | Usage (input / output) | Commits | Mean commit | Exchange |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 10:58:10 | First turn, a short Advisory | `none` | `01a12fc3-4f5e-7d92-8437-4c2f4ba1dea6` | `sha256:92201c65…` | Terminal Advisory, reference `resp_02179180269191546c14d470d092c1c2bed457a462426b23894c8` | 426 / 113 | 77 | 4.9 ms | 5.6 s |
| 2 | 10:58:55 | A prose change on two blocks | `incremental` | `01a12fc3-fadc-7be1-bbff-1a20054a7bb7` | `sha256:215c57ea…` | Terminal Prose Change with two locations | 676 / 262 | 216 | 4.9 ms | 8.5 s |
| 3 | 10:59:38 | A longer Advisory | `incremental` | `01a12fc4-a312-7ce2-bc91-46db88687160` | `sha256:48721cf0…` | Terminal Advisory | 940 / 751 | 413 | 3.9 ms | 20.3 s |
| 4 | 11:20:25 | A longer Advisory after the Registration change from `fa06` to `fa07` | `new_transport` | `01a12fd7-ac4f-79c1-b8ec-4ad562611330` | `sha256:157f4b13…` | Terminal Advisory | 308 / 2656 | 121 | 15.6 ms | 61.3 s |
| 5 | 11:21:50 | A short Advisory | `incremental` | `01a12fd8-f75d-7033-9a3f-b44b9c51df34` | `sha256:83a5122a…` | Terminal Advisory | 688 / 95 | 10 | 13.5 ms | 5.4 s |

Total reported usage: 3,038 input and 3,877 output tokens.

## Measurement

- With bound 1, the event count was 0.55 to 0.82 events for each output token. The commits used 4 to 8 percent of the exchange time.
- With 64 events or 500 ms, request 4 made 121 commits in 61 s, approximately two each second. The events arrived at fewer than 64 in 500 ms, so the time condition started most commits. Request 5 made 10 commits for 95 output tokens, and request 1 made 77 commits for 113 output tokens with bound 1.
- A commit with the selected rule took longer (13.5 to 15.6 ms) than a commit of one event, because each commit writes the accumulated item text.

## What this evidence qualifies

- Streaming on the exact route: typed `response.created`, `response.output_item.added`, `response.output_text.delta`, `response.output_item.done`, and `response.completed` events in order, with native item identifiers.
- The combination of transport, native text, continuation, and streaming.
- The terminal response event maps to the same observation as a response that is not streamed.

## What it does not qualify

- A dropped stream, a timeout, or a stop during a live stream. Deterministic tests with a local scripted destination cover these cases, and they are OutcomeUnknown. Recovery stays with S4-04.
- Retrieval, abort, explicit cache, and compaction. Their profile entries keep their earlier evidence.
- A cache hit or a saving, model quality, Provider attention, retention, and training (S4-REQ-007).
