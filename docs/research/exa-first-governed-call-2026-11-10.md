# Exa First Governed Call Record, 2026-11-10

This report records the first live governed Tool calls of the Tool Gateway for [S5-03](https://github.com/FrankQDWang/StoryOS/issues/399) (criterion 11). The real Agent Plan model asked for the calls. StoryOS dispatched them to the remote Exa MCP server through the complete path: Tool Exposure, ToolCall derivation, effect intersection, the Context Assembly gates, the recheck at the claim, the dispatch claim, the exchange, result validation, the Tool Effect Outcome, the Run Event, and the panel line.

## Authorization and plan

- Plan: [#399 comment 6757997410](https://github.com/FrankQDWang/StoryOS/issues/399#issuecomment-6757997410), at most 4 Exa exchanges: request 1 `web_search`, request 2 `web_fetch`, requests 3 and 4 as reserves for an environment failure.
- The author authorized the plan on 2026-11-10 ("授权运行"). The coordinator session relayed the authorization.
- The model requests used the Agent Plan route under the standing authorization of #362. Each Run made one Agent Plan Create.

## Method

- Build: `cf3ec5b4` of [PR #1416](https://github.com/FrankQDWang/StoryOS/pull/1416), `make release-package` output.
- Deployment: the author's Mac, local HTTP profile. A disposable PostgreSQL from `scripts/dev-postgres.sh up --interactive` (the controlled fixture, Project A). The Server ran on `127.0.0.1:8743` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. One `storyos-worker --once` ran for each Run, with `STORYOS_LOG=debug`.
- The Worker resolved `settings:STORYOS_AGENT_PLAN_API_KEY` and `settings:STORYOS_EXA_API_KEY` from the env file of the main checkout, by the lookup order of [Run the Local Real Model Route](../operations/local-real-model-route.md). The executor did not copy the env file into the worktree, and printed no key.
- The author view enabled web research for Project A (`setProjectToolEnablement`, HTTP 200) and sent each author message through the assistant panel.

## Request log

Each Exa exchange is one MCP session: `initialize`, `notifications/initialized`, `tools/list` (the drift check against the pinned Registration), and one `tools/call`. The byte counts are the request and response bytes of the diagnostic lines.

| # | Time (UTC) | Run | Tool | Exchanges | `tools/call` bytes | Log |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 04:40:19 to 04:40:27 | `01a1c3c1-892b-7513-a679-41e84eb7fea7` | `web_search_exa` | 2 (sessions `658bdf79`, `c71ed6d0`) | 323 / 27,964; 315 / 28,859 | [comment 6759172687](https://github.com/FrankQDWang/StoryOS/issues/399#issuecomment-6759172687) |
| 2 | 04:43:03 to 04:43:10 | `01a1c3c4-1eb5-7553-a64f-c47571930672` | `web_fetch_exa` | 1 (session `968b52e6`) | 146 / 3,352 | [comment 6759203908](https://github.com/FrankQDWang/StoryOS/issues/399#issuecomment-6759203908) |

Total: 3 Exa exchanges of the authorized 4. No reserve was used.

**Deviation from the plan.** The plan expected one `web_search` call for request 1. The model put two `web_search` calls in one Decision, with two different queries. The Tool Gateway dispatched them in item order, each with its own Destination Attempt and Outbound Disclosure Event. The coordinator accepted the deviation on 2026-11-10 and set a hard stop at 4 exchanges for request 2. A guard in the request 2 run stopped the Worker at the start of a fourth exchange; it did not act.

## Results

| Run | Call | Arguments | State | Tool Effect Outcome | Result bytes | Result digest |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | query `1890 Liverpool tide tables historical tidal predictions public archive`, result count 10 | `settled` | read `confirmed`, disclosure `confirmed` | 27,375 | `sha256:a23cc00dbb9cb9f159b44443dbd54b40a7cfb067750dc6a4f1350ad8b7eaa08b` |
| 1 | 1 | query `Liverpool tide table 1890 Admiralty tide tables archive HathiTrust`, result count 10 | `settled` | read `confirmed`, disclosure `confirmed` | 28,199 | `sha256:106dcddc1444abe9b00ccea72d676f6268b9c8dd3552544a9cf7602715350702` |
| 2 | 0 | URLs `https://en.wikipedia.org/wiki/Tide_table` | `settled` | read `confirmed`, disclosure `confirmed` | 3,127 | `sha256:be7b7875f50df913c5d26a909673dd1756e4e27c1014ad11d31b1fdec20db0d5` |

For each call:

- The drift check found no drift. No Registration revision was quarantined.
- One Operation Requirement and one Context Assembly Manifest (role `tool_call`, sufficiency `complete`, grant scope `research_tool_v1`) committed before the dispatch claim. The attempt record refers to the manifest.
- The attempt record holds the Admission Decision with the exact authority evidence, the Wire Payload Projection, the Destination Attempt (`terminal_result`), and the Outbound Disclosure Event. The Event stays `outcome_unknown`; a separate confirmation records `confirmed`.
- The ToolCall holds the result text, its digest, and its byte count. The Run has one `tool_call_settled` Run Event for each call, at positions 0 and 1 of Run 1 and position 0 of Run 2.
- Each Run is `completed`. The panel showed `已联网搜索：<query>` for each search and `已阅读：https://en.wikipedia.org/wiki/Tide_table` for the page read, with no identifier.

The record keeps no result text. The digests identify the stored texts in the disposable database, which the executor removed after the run.

## Key check

- The Worker log of each Run has 0 matches of either key.
- A data-only dump of the `storyos` schema after the runs has 0 matches of either key.
- The Wire Payload Projection of each attempt holds the JSON-RPC body with no key.

## Findings for the next tickets

1. **The model can ask for more than one call in one Decision.** A live plan must count Exa exchanges, not Runs. S5-05 must give the model each result of the Decision in one continuation input.
2. **A search result is large.** With a result count of 10, each search result was about 27 KiB of text. Two searches gave about 55 KiB. S5-05 must project these results inside the input bounds of the continuation request.
3. **The model wrote English queries for a Chinese author message.** The search language is the model's choice; no StoryOS rule constrains it.
4. **The Run completes after its ToolCalls.** Until S5-05, the model does not get the results, and the panel shows only the research lines.
