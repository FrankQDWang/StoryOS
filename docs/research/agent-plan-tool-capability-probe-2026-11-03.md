# Agent Plan Tool capability probes of doubao-seed-2-1-pro-260915, 2026-11-03

Evidence record of [S5-10: Probe the Tool Capabilities of the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/696). It records the live requests that examined four Tool entries of the Model Capability Profile on the Volcengine Agent Plan Responses route: function calling, hosted web search, remote MCP, and the `max_tool_calls` bound. The profile revision `storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v4` cites it. It is not a claim about model quality, search quality, Provider attention, retention, training, or billing.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). Personal, non-commercial use (#392).
- The author decisions of 2026-11-03 through the coordinator of Stage 5: the probe design of the ADR 0049 amendment, the keyless public MCP server `https://mcp.deepwiki.com/mcp`, the Ark object form `"allowed_tools": {"tool_names": [...]}`, and a native rejection records `unsupported` with no retry.
- The plan was posted before the first request: [5 requests, cap 10](https://github.com/FrankQDWang/StoryOS/issues/696#issuecomment-6605197856). A probe runs one more time only after a timeout or a dropped stream.
- Sent: P1 to P5. 5 requests in all. No retry, no repeat, and no anomaly.
- Each probe ran in one `storyos-worker --once` process with the debug log. No Worker log has an authorization header or a key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2-1-pro-260915` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh` with the controlled fixture, Server with `STORYOS_WORKER=0`. The author messages went through the browser composer. |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v2`: 32,768 output tokens, 600 s |
| Build | Measurement commit `eec03369`, tree `0a47611a`, on the branch of #696. It has Registration `fa16` as the head. |
| Registration | `018f0000-0000-7001-8000-00000000fa16`, profile v3: the qualified profile v2 with the four Tool entries pending. Decision `qualification_authorized`. |
| Grant scope | `project_writing_assistance_v2` |
| Conversation | One Project Conversation, `01a1a1a1-5873-7f20-b34b-c97b80d56b42`, for P1 to P5 |

Each probe is a Create request of an AgentRun whose author message is the exact constant text of one probe (ADR 0049 amendment). The request is not streamed and stores the response (`"stream": false`, `"store": true`). It has one Tool and a constant probe instruction. It sends no Project content. Its response makes no Agent Decision.

| Probe | Tool in the request | Input |
| --- | --- | --- |
| P1 `function_call` | `function` `storyos_probe_echo`, one string parameter `text` | Constant author message |
| P2 `function_call_output` | The same function | `function_call_output` with the `call_id` of P1 and the constant output `{"text":"storyos-probe"}`, and `previous_response_id` of P1 |
| P3 `hosted_web_search` | `web_search` with `sources: ["doubao"]` | Constant author message |
| P4 `remote_mcp` | `mcp` with `server_label` `deepwiki`, the DeepWiki URL, `allowed_tools: {"tool_names": ["read_wiki_structure"]}`, and `require_approval: "never"`. Header `ark-beta-mcp: true`. | Constant author message |
| P5 `max_tool_calls_bound` | `web_search` with `sources: ["doubao"]`, and `max_tool_calls: 1` | Constant author message that needs two searches |

## Requests

| # | Time (UTC) | Model Attempt | Wire digest | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- |
| P1 | 13:38:02 | `01a1a1a1-904d-7ad1-ba98-18a1e2a10394` | `sha256:27e0e801…` | HTTP 200, completed. One `function_call` item. | 534 / 65 |
| P2 | 13:38:19 | `01a1a1a1-d198-7183-bbc8-b8562c8edbe2` | `sha256:7d07b0d8…` | HTTP 200, completed. A reasoning item and a final message that uses the function output. | 618 / 45 |
| P3 | 13:38:33 | `01a1a1a2-08c7-7872-b0e1-c8e72db52bda` | `sha256:5c0ae762…` | HTTP 200, completed. One `web_search_call` item and three `url_citation` annotations. | 4,015 / 351 |
| P4 | 13:39:00 | `01a1a1a2-71bd-7690-963e-e356829d185d` | `sha256:0a4317e8…` | HTTP 200, completed. A reasoning item and a message. No MCP item. | 85 / 648 |
| P5 | 13:39:58 | `01a1a1a3-578d-7a31-942a-626f92df1e1e` | `sha256:f82b6c03…` | HTTP 200, completed. Two `web_search_call` items. | 10,605 / 313 |

Known usage of P1 to P5: 15,857 input and 1,422 output tokens.

Each Attempt has its own Outbound Disclosure Event, and each records `runtime_qualification = qualification_authorized`, the Registration `fa16`, the `tool_qualification` of its probe, and no Agent Decision. Each AgentRun completed.

Response references: P1 `resp_02179371308309520c19fad7de95dc34d140d8afb6e084065311f`, P2 `resp_0217937130999342f58812a10f9c6b8f25a472cef1ad3e59fd090`, P3 `resp_021793713113598ea9999fb310f5017a5d432d00d65dc9a710f3f`, P4 `resp_021793713140456cfacb7e54fe5570d0ae3d0783d99a770aad705`, P5 `resp_0217937131999933a5ef8b5d1eba84c17b54858e5dd40fddaf904`.

## Native evidence for each entry

### Function calling: qualified

- P1 returned one item of type `function_call` with the status `completed`, the name `storyos_probe_echo`, the call ID `call_4d922uogbsqzuxus4c0suxzt`, and the arguments `{"text": "storyos-probe"}`. The arguments agree with the parameter schema.
- The Host read the call ID from the recorded Tool facts of P1. P2 sent the `function_call_output` with the same call ID and `previous_response_id` of P1. The route accepted the output, and the final message states the text `storyos-probe` from the output.

### Hosted web search: qualified

- P3 returned one item of type `web_search_call` with the status `completed` and the action `{"query":"Rust 1.0 release date","type":"search"}`.
- The final message has three `url_citation` annotations: `https://blog.rust-lang.org/2015/05/15/Rust-1.0/`, `https://blog.rust-lang.org/`, and `https://blog.rust-lang.org/2015/02/13/Final-1.0-timeline.html`. The answer is 15 May 2015.
- `usage.tool_usage` reports `web_search: 1`. The input usage of 4,015 tokens includes the search results.
- Thus the hosted search component was active for the account. The route puts the URL on the annotation itself (the flat form).

### Remote MCP: unsupported

- The wire body of P4 has the `mcp` tool, and the request has the header `ark-beta-mcp: true`. An adapter test shows that only the remote MCP probe sends this header.
- The route returned HTTP 200 and `completed`, and no rejection. The response has no item of type `mcp_list_tools`, `mcp_call`, or `mcp_approval_request`, and no `usage.tool_usage` count.
- The input usage is 85 tokens. A Tool definition that the route gives to the model would add tokens, as the function of P1 (534 tokens) and the search of P3 show. Thus the route did not give the `mcp` tool to the model.
- The final message states a `read_wiki_structure` call and a first topic, but no native item records a call. This text is the model's own invention.
- Result: the route ignores the `mcp` tool. The entry records `unsupported`. This is not a "component not activated" rejection, so a change of the account setting is not a known fix.
- Limit: the Ark example of the general route uses `"stream": true`. P4 was not streamed. This record does not examine a streamed MCP request.

### The `max_tool_calls` bound: unsupported

- The wire body of P5 has `"max_tool_calls": 1`.
- The route returned two items of type `web_search_call`, each with the status `completed`, in two separate rounds: the queries `Rust 1.0 release date` and `Python 3.0 release date`. The output order is reasoning, search, reasoning, search, reasoning, message.
- `usage.tool_usage` reports `web_search: 2`, and the final message has six `url_citation` annotations.
- Result: the route accepts the field and does not enforce it. The entry records `unsupported`.

## Hosted research decision

GO for a hosted research Tool on this route: hosted web search is qualified. Remote MCP is not, so a hosted research design uses `web_search` only. Because the route does not enforce `max_tool_calls`, the Host must bound the number of hosted Tool rounds and the token cost itself.

## Registrations

- `018f0000-0000-7001-8000-00000000fa16` (profile v3) has the four Tool entries pending. It was the head for this run.
- `018f0000-0000-7001-8000-00000000fa17` (profile v4) has the verdicts of this record, and it is the new head. It has no pending Tool entry, so the authorization link authorizes no more probes. An ordinary AgentRun sends no Tool.

## What this evidence qualifies

- Function calling with a typed parameter schema, the correlation of one call and its output by the call ID, and the continuation with `previous_response_id`, on the exact route and the dated version, in non-streamed requests.
- Hosted web search with the Doubao source, its `web_search_call` items, its flat `url_citation` annotations, and its `usage.tool_usage` count, in non-streamed requests.

## What it does not qualify

- Streamed Tool events, parallel function calls, more than one function, a function call with a failed output, and `tool_choice`.
- A streamed remote MCP request, MCP approval, and any other MCP server.
- Search quality, search freshness, the other search sources, and the cost of a hosted search.
- Exa or any other external research destination (ADR 0083).
- Provider attention, retention, and training (S4-REQ-007).
