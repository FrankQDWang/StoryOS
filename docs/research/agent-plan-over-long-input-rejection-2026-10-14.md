# Agent Plan rejection of an input above the model window, 2026-10-14

Evidence record of [Show a Result Sentence When the Real Destination Refuses a Request](https://github.com/FrankQDWang/StoryOS/issues/1110). It records the live requests that observed the native rejection of an input above the model window on the qualified Agent Plan route. ADR 0056 cites it for the mapping of that rejection. It is not a claim about model quality, Provider attention, retention, or training.

## Authorization and plan

- Live requests: the [standing author authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). Personal, non-commercial use (#392).
- Plan of request 1, written before it: one request of about 283,000 provider tokens, above the 262,144-token window that ADR 0056 records. Stop on any outcome. Acceptance is an anomaly.
- Request 1 was accepted. The executor stopped and reported to the author. The author then approved one GET of the stored response, and after it one request above 1,048,576 tokens.
- Sent: 2 Create requests and 1 GET. No retry and no repeat.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro`. The route reported the version `doubao-seed-2-1-pro-260915`. |
| Credential | Operator quota, `STORYOS_AGENT_PLAN_API_KEY` through the `storyos-settings` layer (ADR 0050). A throwaway probe outside the repository loaded the key with `storyos_settings::load` and `EnvFile::RealRoute`. No agent read, printed, or searched `.env`. |
| Request shape | The product wire shape of `wire.rs` at `main` `37b78050`: the exact Host instructions, one user message with `Author request:` and `Working text:` sections, `stream: true`, `store: true`, `max_output_tokens` 32768 (`agent_plan_request_v2`) |
| Client | `reqwest` with the options of the adapter: HTTP/1 only, no retry, no redirect, no proxy, 600 s timeout |
| Content | Synthetic. Generated Chinese prose from the seeded generator of `prototypes/chinese-long-form-baseline/generate.py` (seed 20261005), the first scalars of the corpus |

## Requests

| # | Time (UTC) | Purpose | User text scalars | Body bytes | Expected provider tokens | Outcome |
| --- | --- | --- | ---: | ---: | ---: | --- |
| R1 | 2026-10-14 02:09:43 | Create above the recorded 262,144-token window | 410,054 | 1,226,191 | about 283,000 | HTTP 200 after 24.1 s, `text/event-stream`, `response.created` and reasoning deltas. Accepted. The probe closed the stream after the first events. |
| G1 | 2026-10-14 02:22 | GET of the R1 response | | | | HTTP 404 `ResourceNotFound`. A stream that closes before the end is not stored, as S4-04 found. |
| R2 | 2026-10-14 02:24:15 | Create above 1,048,576 tokens | 1,600,054 | 4,781,590 | about 1,104,000 | HTTP 400 after 7.5 s, before any stream |

Expected provider tokens use 0.69 tokens for each scalar of generated prose (S4-07). The route reported no usage for R1 or R2.

Request body digests: R1 `sha256:a9850a5ca1d3a44fad87f1cf98989fa448f6a0b39532503845c08bf57abffd28`, R2 `sha256:69fd8732c5b3e41f69eaa07ad1c0e88ba2f025f3816d4955572e181836571e99`. Response reference of R1: `resp_021791943784026ec6553b2bbb2bcb1bdf9627ab55f9f64a02640`. Request ID of R2: `021791944655769a92b4afba7ed5745e150097919eafbc6bc7b19`.

## Native rejection of R2

```text
HTTP/1.1 400
content-type: application/json; charset=utf-8
content-length: 204

{"error":{"code":"InvalidParameter","message":"Total tokens of image and text exceed max message tokens. Request id: 021791944655769a92b4afba7ed5745e150097919eafbc6bc7b19","param":"","type":"BadRequest"}}
```

## Results

- The route accepted an input of about 283,000 provider tokens. The [Ark model list](https://www.volcengine.com/docs/82379/1799865), read on 2026-10-14, gives `doubao-seed-2-1-pro-260915` a 1024k context window, a 1024k maximum input, and a 256k maximum answer. The earlier version `doubao-seed-2-1-pro-260628` has a 256k window. The 262,144-token window that ADR 0056 and the Model Capability Profile record is thus the window of the earlier version.
- The route rejects an input above the window before the stream, with HTTP 400 and the native code `InvalidParameter`. Other parameter errors have the same code. Only the message start `Total tokens of image and text exceed max message tokens` identifies the condition.
- The rejection came in 7.5 s and the route reported no usage.

## What this evidence supports

- The mapping of this native rejection to the `input_too_long` condition in the adapter, and to the `conversation_too_long` refusal of the AgentRun.
- That the current version of the model accepts an input above 262,144 tokens.

## What it does not support

- The exact window of the current version. No request was sent between 283,000 and 1,104,000 tokens.
- The rejection of an input that is below the window, but whose input and output bound together are above it.
- The rate limit behavior. The model list gives 1,000,000 tokens per minute. R2 got the window rejection, not a rate limit.
- A change of the compaction threshold. ADR 0056 owns it.
