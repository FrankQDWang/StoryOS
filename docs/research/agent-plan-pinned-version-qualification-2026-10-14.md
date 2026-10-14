# Agent Plan qualification of the dated version doubao-seed-2-1-pro-260915, 2026-10-14

Evidence record of [Pin the Served Doubao Model Version and Record Its Context Window](https://github.com/FrankQDWang/StoryOS/issues/1143). It records the live requests that qualified the capabilities of the dated model version `doubao-seed-2-1-pro-260915` on the Volcengine Agent Plan Responses route (ADR 0059). The profile revision `storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v2` cites it. It is not a claim about model quality, summary quality, Provider attention, retention, training, or billing.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). Personal, non-commercial use (#392).
- The author decision of 2026-10-14 on #1143: pin the dated identifier, confirm at Claim that the route accepts it, and re-qualify the qualified capabilities with a small planned number of live requests.
- The plan was written before Q1: 6 product-path requests (Q1 to Q6) and one large-input probe (Q7). Caps: 9 requests in all. Q1 to Q6 at most 40,000 input and 40,000 output tokens together. Q7 at most 900,000 input tokens.
- Sent: P0 at Claim, Q1 to Q7. 8 requests in all. No retry, no repeat, and no anomaly.
- Each product-path step ran in one `storyos-worker --once` process with the debug log. No Worker log has an authorization header or a key.

## Which capabilities need new evidence

The earlier qualifications (S4-02, S4-06, S4-04) sent the undated identifier `doubao-seed-2.1-pro`, and their records do not state the served version. Thus each qualified capability of profile v12 gets new evidence on the dated identifier:

| Capability | Evidence in this record |
| --- | --- |
| Transport, native text, streaming | Q1: a streamed Create with a terminal Advisory |
| Continuation | Q2: a Create with `previous_response_id`, which answers from the earlier turn |
| Host-managed compaction | Q3 and Q4: a summary request that continues the chain, and a later request with the summary and a new transport |
| Retrieval | Q5 and Q6: a Create whose Worker stops after the reference commit, and a Retrieve of the stored response by a new Worker |
| Abort | No request. The route has no cancel operation (ADR 0057), and the entry stays unknown. |

The automatic trigger is a Host rule (ADR 0056). Its model fact is the window. Q7 examines an input above the new threshold.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2-1-pro-260915` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh` with the controlled fixture (Project A, Chapter A), Server with `STORYOS_WORKER=0` |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v2`: 32,768 output tokens, 600 s |
| Build | Measurement commit `b46cc883`, tree `ab47e5c8`, tag `evidence/1143-pinned-version-measurement`. It is not for merge. It has the pending Registration as the head, and the author message prefix of the Host fake also compacts on the real route. |
| Registration | `018f0000-0000-7001-8000-00000000fa14`, pending profile v1 of the dated version, mapping `responses.create.v4`, decision `qualification_authorized` |
| Grant scope | `project_writing_assistance_v2` |
| Conversation | One Project Conversation, `01a13890-8464-7a62-9ad3-f506d71bdf6b`, for Q1 to Q6 |

## Requests

| # | Time (UTC) | Purpose | Input mapping | Model Attempt | Wire digest | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P0 | 03:49:54 | Claim check of the exact identifier, throwaway probe | | | `sha256:2cd7f904…` | HTTP 200 after 1.1 s. `response.created` reports the model `doubao-seed-2-1-pro-260915`. The probe closed the stream after 3 events. | not reported |
| Q1 | 03:59:24 | First turn: keep a passphrase, and assess the Chapter | `none` | `01a13890-9e34-7460-8e0b-f258774089f9` | `sha256:c4c5678d…` | Terminal Advisory | 314 / 517 |
| Q2 | 04:00:20 | Second turn: give the passphrase | `incremental` | `01a13891-7a51-7571-bed6-c22ee3df3933` | `sha256:1db7ad62…` | Terminal Advisory with the passphrase of Q1 | 457 / 55 |
| Q3 | 04:00:44 | Summary request of the compaction Run, with `previous_response_id` set to the reference of Q2 | (summary) | `01a13891-d70b-7402-b9ce-49d5d61e29b0` | `sha256:ab1da9c6…` | Terminal summary that keeps the passphrase | 361 / 6,189 |
| Q4 | 04:02:39 | Later request of the same Run: the summary first, then the author request and the Working Target | `new_transport` | `01a13893-98f7-7672-bad7-89cad417fc09` | `sha256:9ff4d12e…` | Terminal Advisory with the passphrase | 640 / 1,604 |
| Q5 | 04:03:51 | Create that continues Q4. After the reference commit, the Worker was frozen with `SIGSTOP` at 04:03:53.2 and killed with `SIGKILL` at 04:04:53.3. | `incremental` | `01a13894-b419-75c1-ac33-a0030381ef24` | `sha256:6e7e8415…` | Reference committed, outcome unknown | 732 / 64 (from Q6) |
| Q6 | 04:04:59 | Retrieve by a new Worker after the lease expired | | `01a13895-bd9c-7392-a72f-33193a9fa09e` | `sha256:683809c2…` | Terminal and complete. The Host settled the Advisory on the Q5 Attempt, and the Run completed. | (of Q5) |
| Q7 | 04:05:22 | Create above the new threshold, throwaway probe | | | `sha256:43655f8b…` | HTTP 200 after 105.9 s, `text/event-stream`. `response.created` reports the model `doubao-seed-2-1-pro-260915`. The probe closed the stream after 3 events. | not reported |

Each product-path response reported `usage.input_tokens_details.cached_tokens = 0`. Each product-path Attempt has its own Outbound Disclosure Event, and each records `runtime_qualification = qualification_authorized` and the Registration `fa14`.

Known usage of Q1 to Q5: 2,504 input and 8,429 output tokens.

Response references: P0 `resp_02179194979476592ebd1237b33dc9f829320565798fa7e8b1194`, Q1 `resp_0217919503645229e13d3b3ea5f02ecc58377d19143e38dc3c89f`, Q2 `resp_0217919504209180249e54cf23e108bd1702536cbe0373fd10376`, Q3 `resp_021791950444651fef52c4b25b30e28d151ef041e4abfe5cc6162`, Q4 `resp_021791950559788dbb26dcaf774bd0633eca8434ac673a3fcbdbb`, Q5 `resp_021791950632272fcb32ededa8f2abcad5bfef4fa311b1dbfd877`, Q7 `resp_02179195072254058f1497fbd9e03fa5d60a1e5b6b4655b46f661`.

The compaction record `01a13893-98ce-77d0-8a75-ff3027f83fab` has the producer `agent_plan_summary`, the mapping `responses.create.v4`, reported usage, and the loss fact `semantic_preservation_unknown`. Its install state is `installed`, with the Model Attempt of Q4. Q4 sent no `previous_response_id`, and its first input section is "Summary of the earlier conversation".

The author view showed the four answers and no summary, compaction, or recovery fact.

## Large-input probe Q7

- Method: the throwaway probe of the over-long input record (#1110) with the product wire shape. It loaded the key through `storyos_settings::load` and `EnvFile::RealRoute`. No agent read, printed, or searched `.env`. A probe outside the Model Gateway qualifies no capability (ADR 0049). It is evidence of the window fact only.
- Content: 1,200,044 user text scalars of generated Chinese prose from the seeded generator of `prototypes/chinese-long-form-baseline/generate.py` (seed 20261005). Body 3,586,488 bytes.
- Expected provider tokens: approximately 827,800. This uses 0.6896 tokens for each scalar of generated prose and a fixed overhead of 241 tokens (S4-07). The route reported no usage, because the probe closed the stream.
- The new threshold is 812,646 tokens, and the window after the output bound is 1,015,808 tokens. Thus Q7 is above the threshold and below the window after the output bound.

## Results

- The route accepts the exact dated identifier, and it reports the same version in each response (P0, Q7).
- Transport, native text, streaming, continuation, Host-managed compaction, and retrieval work on the dated version through the Model Gateway.
- The route accepted an input of approximately 827,800 provider tokens. With the over-long input record, accepted inputs of approximately 283,000 and 827,800 tokens and a rejected input of approximately 1,104,000 tokens agree with the documented window of 1024K tokens.
- The prefill of Q7 took 105.9 s before the first event. This is shorter than the request timeout of 600 s.

## What this evidence qualifies

- Transport, native text, streaming, continuation, Host-managed compaction, and retrieval of a stored response on the exact route and the dated version, and their combination.

## What it does not qualify

- The exact window. It stays the documented value of the Ark model list, because no request examined the full window.
- An automatic compaction that starts at the new threshold, a summary of a long Conversation, or the time of a summary request near the threshold.
- The semantic preservation or the quality of a summary.
- Native compaction, abort, explicit cache, a cache saving, Provider attention, retention, and training (S4-REQ-007).
