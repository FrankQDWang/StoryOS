# Agent Plan automatic compaction qualification, 2026-10-13

Evidence record of [S4-06c3: Compact the Conversation at a Threshold From the Model Window](https://github.com/FrankQDWang/StoryOS/issues/1116). It records the live requests that qualified the automatic compaction trigger of the Volcengine Agent Plan Responses route (ADR 0056). The context window stays a documented value. The profile revision `storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v10` cites it. It is not a claim about summary quality, semantic preservation, model quality, Provider attention, retention, training, or cache savings.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). The coordinator approved the near-threshold plan on 2026-10-13: 6 requests, at most 7, each input at most about 230,000 tokens, and total output at most about 140,000 tokens.
- The plan was written before the first request and corrected twice after a response, in the same order of magnitude and with the same caps. The Chapter tokenized to more provider tokens than planned, and the reasoning of a response does not carry forward in the chain.
- Sent: 6 requests. No retry, no repeat, and no anomaly.
- Each Run settled in one `storyos-worker --once` process with the debug log. No Worker log has an authorization header or a key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh`, Server with `STORYOS_WORKER=0` |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v2`: 32,768 output tokens, 600 s |
| Threshold | (262,144 - 32,768) x 80 percent = 183,500 tokens |
| Build | Measurement commit `dd000a34`, tree `a695cfce`, tag `evidence/s4-06c3-threshold-measurement`. It is not for merge. |
| Registration | `018f0000-0000-7001-8000-00000000fa10`, profile v9 (documented window, pending trigger), mapping `responses.create.v4`, decision `qualification_authorized` |
| Content | Synthetic Project with one Chapter of 29,085 scalars, typed in the author view. The text repeats sentences of the vocabulary of the seeded generator of `prototypes/chinese-long-form-baseline`. |
| Conversation | One Project Conversation, `01a1331c-bfae-7c71-9033-881293464da2`, for all six requests |

An earlier attempt wrote a Chapter directly into the disposable database. The editor then showed that the save needed attention, and the composer sent nothing. No request was sent from those Projects.

## Requests

The expected input of the trigger is the reported input and output tokens of the last chain response, plus the counted tokens of the new turn (29,085 Chapter scalars and the author message).

| # | Time (UTC) | Purpose | Input mapping | Model Attempt | Wire digest | Outcome | Usage (input / output, cached) | Expected input of the next turn |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| G1 | 02:34:52 | First turn, a long Advisory | `none` | `01a1331c-e3f5-7973-80cb-fa279fc51ceb` | `sha256:396e38b5…` | Terminal Advisory | 39,894 / 18,880, cached 0 | about 87,900 |
| G2 | 02:43:24 | Second turn, a long Advisory | `incremental` | `01a13324-adb2-7151-bcd3-b870e8506475` | `sha256:64d89ab5…` | Terminal Advisory | 87,962 / 13,178, cached 39,736 | about 130,300 |
| G3 | 02:50:46 | Third turn, a long Advisory | `incremental` | `01a1332b-6dad-7473-81a8-ef137ad711c7` | `sha256:1e27ada7…` | Terminal Advisory | 137,362 / 9,146, cached 87,864 | about 175,700, below the threshold |
| G4 | 02:55:36 | Fourth turn, a long Advisory | `incremental` | `01a1332f-ddce-73a2-a2bf-c6e9eb5bfe52` | `sha256:0ab78d52…` | Terminal Advisory | 185,373 / 7,262, cached 137,016 | about 221,700, above the threshold |
| S | 02:59:36 | Summary request of the compaction Run, with `previous_response_id` set to the reference of G4 | (summary) | `01a13333-8543-76a1-8b1f-9819ee4c4323` | `sha256:f396ba5a…` | Terminal summary of 3,539 characters | 190,516 / 3,275, cached 0 | |
| L | 03:00:58 | Later request of the same Run: the summary first, then the author request and the Working Target | `new_transport` | `01a13334-c51b-7b22-acd8-9080b15bb887` | `sha256:a999977b…` | Terminal Advisory that names problem 1 of the ten problems that G4 listed | 42,289 / 485, cached 0 | |

Total reported usage: 683,396 input and 52,226 output tokens.

Response references: G1 `resp_021791858894108b79d8748f7bc7d5b56cf6863add8bd06e8422b`, G2 `resp_0217918594044862bbef52b8030dbcfb1b140243411abf26bdcfe`, G3 `resp_021791859846843bfe35b6525b6a24c7ddbea7b420bd558106076`, G4 `resp_021791860137729b71a54db2718758900f1b5b7cc45ea65e199a5`, S `resp_021791860377157144adcb5cb1cc044cee1f1b0d9da12ecea7add`, L `resp_021791860459039fbcc013cde10a689f8f390ac862f8b0424fa1c`.

The compaction record `01a13334-c4e3-7a51-a5b5-40ca62e0a6ed` has the producer `agent_plan_summary`, reported usage, and the install state `installed` with the Model Attempt of L. L sent no `previous_response_id`, and its first input section is "Summary of the earlier conversation".

The author view showed the answers and no summary or compaction fact.

## Results

- The route accepted 185,373 input tokens (G4) and 190,516 input tokens (S). This is above the 128,186 tokens that S4-07 measured, and below the documented window of 262,144 tokens.
- The trigger did not start at G4, where the expected input was below the threshold. It started at the next turn, where the expected input was above the threshold.
- The Chapter of 29,085 scalars was about 29,000 to 40,000 provider tokens. The trigger counts the new turn in scalars, so the provider can count more tokens than the trigger. The margin of 20 percent under the window after the output bound covers this difference in this run.
- The summary request had no cached input, because its instruction differs from the decision requests.

## What this evidence qualifies

- The automatic trigger at 80 percent of the documented window after the output bound, with the reported usage of the chain, on the exact route.
- A summary request at about 190,000 input tokens, and a later request with that summary and a new transport.
- The combination of transport, native text, continuation, streaming, and Host-managed compaction with the automatic trigger.

## What it does not qualify

- The full documented window of 262,144 tokens, an input above it, and the native rejection for it (#1110).
- The semantic preservation or the quality of a summary.
- Native compaction, retrieval, abort, explicit cache, a cache saving, Provider attention, retention, and training (S4-REQ-007).
