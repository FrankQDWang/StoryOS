# Agent Plan runtime qualification, 2026-10-11

Evidence record of [S4-02b: Admit an Authorized Runtime Qualification of the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/1096). It records the first admitted live requests to the Volcengine Agent Plan Responses route. It is the evidence that the S4-02c profile revision cites. It is not a claim about model quality, Provider attention, retention, or training.

## Authorization and bounds

- Author authorization of live requests: [#393 comment](https://github.com/FrankQDWang/StoryOS/issues/393#issuecomment-6098554693), 2026-10-10. Personal, non-commercial use (#392).
- Admission: the compatibility Decision of the Project recorded `runtime_qualification = qualification_authorized` with that link (ADR 0049). The Model Capability Profile stayed pending.
- Request bounds of the binding: `agent_plan_request_v1`, at most 8192 output tokens and a 180 second request timeout (author decision, 2026-10-10).
- Planned live requests: 2, written before the run. Sent: 2. Each request ran in one `storyos-worker --once` process.

## Exact route

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro` (Model Registration `018f0000-0000-7001-8000-00000000fa02`, profile `storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v1`) |
| Account boundary | The Volcengine account of the deployment operator that holds the Agent Plan subscription |
| Credential | Operator quota, Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`, value from the git-ignored `.env` of the main checkout (ADR 0050). No agent read the value. |
| Mapping | `storyos.volcengine-agent-plan.responses.create.v1`, execution profile `storyos.volcengine-agent-plan.execution.v1` |
| Request shape | Native text, `stream: false`, `store: true`, Host instructions with the JSON reply contract, no prior response reference |
| Build | Release package from commit `893511422aad1776d1924829652cbd4b222b11f2`, tree `78092fdc` |
| Deployment | Author's Mac, local HTTP profile, Server with `STORYOS_WORKER=0`, separate `storyos-worker` |
| Content | Synthetic two-paragraph chapter, created for this run |

## Requests

| # | Purpose | Time (UTC) | Model Attempt | Wire digest | Response reference | Outcome | Reported usage (input / output tokens) | Latency |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Advisory request about one paragraph | 2026-10-11T14:05:28Z | `01a12b48-69ee-7c71-b2cf-33297814e8b4` | `sha256:9a5dd156bffbcb964003bd66040e9315fa32578963f74d9f20d06d954af06f88` | `resp_02179172752875438521d8aeb1173234c174f01c4d2f3c18d455c` | Terminal response. A reasoning summary item and one complete message. The message satisfied the reply contract and mapped to an Advisory. The AgentRun completed. | 463 / 1394 | About 43 s |
| 2 | Prose change request on two target blocks | 2026-10-11T14:07:59Z | `01a12b4a-b7b7-7dd3-84ef-f6154ef55d69` | `sha256:38a77fe5facbd42a1359d90f63f4c9893574a0750fc279060c1c4fb88a5f00da` | `resp_021791727679312a5cf16cfa3c4ebff37e7c43914719d1addc410` | Terminal response. The message satisfied the reply contract with one change for each declared block. Core validation passed, and Proposal `01a12b4a-e5c7-7bb1-8d17-810b505e6798` opened with two locations for explicit Accept or Reject. The AgentRun completed. | 477 / 431 | About 12 s |

Each AgentRun has one Model Attempt. Each Worker process logged one `create` dispatch, so no retry or extra request occurred. The Attempts record `network_io: true` and `runtime_qualification: qualification_authorized`. The Diagnostic Projection of the Worker and the Server holds no authorization header and no key.

An earlier attempt on 2026-10-10 sent nothing: the removed Keychain resolver waited 24,707 seconds for a macOS prompt, and the request refused before the dispatch claim with `destination_credential_unavailable`.

## What this evidence qualifies

- Transport: the exact route accepted two requests and returned complete Responses objects.
- Native text: the exact model returned complete assistant messages that the Host mapped to typed candidates.
- Their combination for one Create request, with the bounds above.

## What it does not qualify

- Streaming, stored continuation, implicit or explicit cache, retrieval, abort, structured output with `text.format`, and native compaction. Their profile entries keep their earlier evidence.
- Create idempotency, recovery after an unknown outcome, and expiry behavior (S4-04).
- Model quality, Provider attention, retention, and training (S4-REQ-007).

## Observations for later owners

- The `getAgentRun` query of a real Attempt still showed labels of the fake destination: `destination_io` was `host_fake`, the continuation admission named `storyos.host-fake.mapping.v1`, and the Provider report evidence was `host_fake_no_provider_usage`. The response reference was not in the evidence list.
- When the Worker is a separate process and a request waits in the queue, the panel can stop at its waiting state until the page reloads.
- The reasoning summary of the model is in English, while the reply follows the manuscript language.
