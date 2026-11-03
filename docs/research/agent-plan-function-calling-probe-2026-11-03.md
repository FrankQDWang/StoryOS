# Agent Plan function calling probe of doubao-seed-2-1-pro-260915, 2026-11-03

Evidence record of [S5-10: Probe the Tool Capabilities of the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/696). It records the live requests that qualified function calling on the Volcengine Agent Plan Responses route. The profile revision `storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v4` cites it. It is not a claim about model quality, Provider attention, retention, training, or billing.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). Personal, non-commercial use (#392).
- The ticket body of the morning of 2026-11-03 had four probes: function calling, hosted web search, remote MCP, and the `max_tool_calls` bound. The plan was posted before the first request: [5 requests, cap 10](https://github.com/FrankQDWang/StoryOS/issues/696#issuecomment-6605197856).
- Sent: P1 to P5, from 13:38 to 13:40 UTC. 5 requests in all. No retry, no repeat, and no anomaly.
- After the run, the author changed the scope: StoryOS performs no Provider-hosted operation ([ADR 0084](../adr/0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md)), and S5-10 qualifies function calling only. P1 and P2 are the evidence of this record. P3 to P5 were already sent. They are recorded below as facts, and they qualify nothing.
- Each probe ran in one `storyos-worker --once` process with the debug log. No Worker log has an authorization header or a key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2-1-pro-260915` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh` with the controlled fixture, Server with `STORYOS_WORKER=0`. The author messages went through the browser composer. |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v2`: 32,768 output tokens, 600 s |
| Build | Measurement commit `eec03369`, tree `0a47611a`, on the branch of #696. It had Registration `fa16` as the head, and its profile v3 had the four Tool entries of the first plan pending. The merged `fa16` has only the function calling entry pending. The probe path of P1 and P2 did not change. |
| Decision | `qualification_authorized`, grant scope `project_writing_assistance_v2` |
| Conversation | One Project Conversation, `01a1a1a1-5873-7f20-b34b-c97b80d56b42`, for P1 to P5 |

Each probe is a Create request of an AgentRun whose author message is the exact constant text of the probe (ADR 0049 amendment). The request is not streamed and stores the response (`"stream": false`, `"store": true`). It has a constant probe instruction and sends no Project content. Its response makes no Agent Decision.

- P1 sends the function tool `storyos_probe_echo` with one required string parameter `text` (`"additionalProperties": false`), and the constant author message that asks for one call.
- P2 sends the same function tool, `previous_response_id` of P1, and one `function_call_output` item with the call ID of P1 and the constant output `{"text":"storyos-probe"}`. The Host reads the call ID from the recorded Tool items of P1.

## Requests

| # | Time (UTC) | Model Attempt | Wire digest | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- |
| P1 | 13:38:02 | `01a1a1a1-904d-7ad1-ba98-18a1e2a10394` | `sha256:27e0e801…` | HTTP 200, completed. One `function_call` item. | 534 / 65 |
| P2 | 13:38:19 | `01a1a1a1-d198-7183-bbc8-b8562c8edbe2` | `sha256:7d07b0d8…` | HTTP 200, completed. A reasoning item and a final message that uses the function output. | 618 / 45 |

Response references: P1 `resp_02179371308309520c19fad7de95dc34d140d8afb6e084065311f`, P2 `resp_0217937130999342f58812a10f9c6b8f25a472cef1ad3e59fd090`.

Each Attempt has its own Outbound Disclosure Event, and each records `runtime_qualification = qualification_authorized`, the Registration `fa16`, the `tool_qualification` of its probe, and no Agent Decision. Each AgentRun completed.

Usage of P1 and P2: 1,152 input and 110 output tokens.

## Result: function calling is qualified

- P1 returned one item of type `function_call` with the status `completed`, the name `storyos_probe_echo`, the call ID `call_4d922uogbsqzuxus4c0suxzt`, and the complete arguments `{"text": "storyos-probe"}`. The arguments agree with the parameter schema.
- P2 continued P1 with the matching `function_call_output`. The route accepted it, and the response completed with a final text message that states the text `storyos-probe` from the output.

## Requests sent before the scope change

These requests were part of the first plan. They are not evidence of a profile entry, because StoryOS sends no Provider-side tool (ADR 0084).

| # | Time (UTC) | Model Attempt | Request | Observed | Usage (input / output) |
| --- | --- | --- | --- | --- | --- |
| P3 | 13:38:33 | `01a1a1a2-08c7-7872-b0e1-c8e72db52bda` | `web_search` with `sources: ["doubao"]` | One `web_search_call` item, three `url_citation` annotations, `usage.tool_usage.web_search = 1` | 4,015 / 351 |
| P4 | 13:39:00 | `01a1a1a2-71bd-7690-963e-e356829d185d` | `mcp` tool for `https://mcp.deepwiki.com/mcp` with the header `ark-beta-mcp: true` | HTTP 200, completed, but no MCP item and no tool usage. 85 input tokens: the route did not give the tool to the model. | 85 / 648 |
| P5 | 13:39:58 | `01a1a1a3-578d-7a31-942a-626f92df1e1e` | `web_search` with `max_tool_calls: 1` | Two `web_search_call` items, `usage.tool_usage.web_search = 2`: the route did not enforce the bound | 10,605 / 313 |

Usage of all five requests: 15,857 input and 1,422 output tokens.

## What this evidence qualifies

- Function calling with a typed parameter schema, the correlation of one call and its output by the call ID, and the continuation with `previous_response_id`, on the exact route and the dated version, in non-streamed requests.

## What it does not qualify

- Streamed function call events, parallel function calls, more than one function, a failed function output, and `tool_choice`.
- The Exa tool path of Stage 5 (ADR 0083, ADR 0084).
- Provider attention, retention, and training (S4-REQ-007).
