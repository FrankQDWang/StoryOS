# Exa Result Reentry Record, 2026-11-10

This report records the first live Tool result continuation for [S5-05](https://github.com/FrankQDWang/StoryOS/issues/401) (criterion 9). The real Agent Plan model asked for Exa calls. StoryOS settled them and gave each result back to the model as a `function_call_output` item with its original `call_id`, through Context Assembly and the ordinary Create admission (ADR 0085).

## Authorization and plan

- Plan: [#401 comment 6760892341](https://github.com/FrankQDWang/StoryOS/issues/401#issuecomment-6760892341): 1 Run, at most 4 Agent Plan requests and at most 4 Exa exchanges, and a guard that stops the Worker before a fifth of either.
- The author authorized the plan on 2026-11-10 ("授权并批准尺寸"). The coordinator session relayed the authorization.
- Request log: [#401 comment 6762221160](https://github.com/FrankQDWang/StoryOS/issues/401#issuecomment-6762221160).

## Method

- Build: `c220e2aa` of [PR #1417](https://github.com/FrankQDWang/StoryOS/pull/1417), `make release-package` output.
- Deployment: the author's Mac, local HTTP profile. A disposable PostgreSQL from `scripts/dev-postgres.sh up --interactive` (the controlled fixture, Project A). The Server ran on `127.0.0.1:8743` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. One `storyos-worker --once` ran the Run, with `STORYOS_LOG=debug`.
- The Worker resolved `settings:STORYOS_AGENT_PLAN_API_KEY` and `settings:STORYOS_EXA_API_KEY` from the env file of the main checkout, by the lookup order of [Run the Local Real Model Route](../operations/local-real-model-route.md). The executor did not copy the env file into the worktree, and printed no key.
- The author view enabled web research for Project A (`setProjectToolEnablement`, HTTP 200). The author message, sent through the assistant panel, asked the assistant to search for what a Liverpool tide table of about 1890 records, and then to give one short advice with no change to the Chapter.
- A shell guard polled the database every 0.1 s. It counted the decision Model Attempts and the ToolCalls with a Destination Attempt of the Run.

## Request log

Run `01a1c475-df93-7c93-9147-964518334c81`. Each Exa exchange is one MCP session: `initialize`, `notifications/initialized`, `tools/list`, and one `tools/call`.

| # | Time (UTC) | Request | Result |
| --- | --- | --- | --- |
| A1 | 07:57:33 | Agent Plan Create, RunStep 0 | `tool_request` with two `web_search` calls |
| E1 | 07:57:46 to 07:57:49 | Exa exchange, call 0 of A1 | `called`, ToolCall `settled` |
| E2 | 07:57:49 to 07:57:56 | Exa exchange, call 1 of A1 | `not_submitted`: no response to `initialize`; ToolCall `failed`; no `tools/call` |
| A2 | 07:57:56 | Agent Plan Create, RunStep 1 (continuation) | `tool_request` with one `web_fetch` and two `web_search` calls |
| E3 | 07:58:06 to 07:58:07 | Exa exchange, call 0 of A2 | `called`, ToolCall `settled` |
| E4 | 07:58:12 to 07:58:15 | Exa exchange, call 1 of A2 | `called`, ToolCall `settled` |
| E5 | 07:58:16 to 07:58:19 | Exa exchange, call 2 of A2 | `called`, ToolCall `settled` |
| A3 | 07:58:19 | Agent Plan Create, RunStep 2 (continuation) | stream started; stopped before a Decision |

Total: 3 Agent Plan requests of the authorized 4. 5 Exa exchange starts and 4 `tools/call` requests.

**Deviation from the plan.** The plan permits 4 Exa exchanges, and E5 is the fifth exchange start. The guard did not stop it. The guard waited for a time with no ToolCall in flight, and each next dispatch claim came before a poll saw that time. E2 sent only `initialize`, which holds no query, so the Run made 4 Outbound Disclosures to Exa. After the guard failed to act, the executor stopped the Worker by hand at 07:58:45, during the stream of A3. The executor did not resume the Run: a resume needs a successor Attempt on RunStep 2, and ADR 0085 does not cover that path. The Run stays `claimed`, with the A3 Attempt `uncertain`. The plan permits 1 Run, so this Run is the record. A later live plan must count Exa starts at the dispatch claim, not by a database poll.

## Results

| Call | Tool | Arguments | State | Result bytes | Result digest |
| --- | --- | --- | --- | --- | --- |
| `call_fb5du7d60cytwju9dlvfr4n2` | `web_search` | `1890 Liverpool tide tables contents high water times daily tides`, count 5 | `settled` | 13,449 | `sha256:c90bca75f6d18e94f6e5f958fbd3245ff36dd97074493525837a7e72f10e7a5b` |
| `call_iru2ucvlqr42wlf7o4iwzh8i` | `web_search` | `Victorian Liverpool tidal tables high water morning afternoon Greenwich mean time 1890`, count 5 | `failed` | none | none |
| `call_dpzsxkl8b2u15kvwi1e4d2li` | `web_fetch` | a PDF of the Historic Society of Lancashire and Cheshire, and a page of National Museums Liverpool | `settled` | 3,679 | `sha256:f4adfc42da9769aa4d377d89464715f260c49836d5a59838ac4422a632937e96` |
| `call_an8jdg7po8jb1u2d336kwaln` | `web_search` | `"Holden's Liverpool Tide Table" 1890 "Greenwich" mean time high water`, count 10 | `settled` | 28,325 | `sha256:c59fb1e9887356b782ea05f83bff6fdb1f527e1c89019b1cdb3bf383d68087a6` |
| `call_ajlzhbh68s2jmwf0kyo0yos3` | `web_search` | `1892 "Pocket almanack and tide table" Liverpool high water heights`, count 10 | `settled` | 6,960 | `sha256:967824268f7640db678002114742c5db9cb7cd4607659fc729d86d54db887295` |

## Continuation requests

The Wire Payload Projection of each Model Attempt holds the request body that the Agent Plan route got.

| RunStep | `previous_response_id` | Input items | Operation Requirement |
| --- | --- | --- | --- |
| 0 | none | the author message | cause none; no `tool_result` source |
| 1 | the response of A1 | `function_call_output` `call_fb5…` (state `ok`, 13,436 characters), `function_call_output` `call_iru2…` (state `unavailable`, 162 characters) | `tool_result_continuation`; 2 `tool_result` sources, trust `data_only` |
| 2 | the response of A2 | `function_call_output` `call_dpz…`, `call_an8…`, `call_ajl…` (state `ok`; 3,732, 28,310, and 7,075 characters) | `tool_result_continuation`; 5 `tool_result` sources, trust `data_only` |

- Each output has the `call_id` of its call and the `tool_result` source tag with the same call identity and the projected state.
- The failed call went back as the sentence "The call failed, so no result is available, and this is not an empty result." The model then asked for different sources and did not take the failure as an empty result.
- An incremental request sends only the outputs of the calls of its prior response. The Operation Requirement of RunStep 2 selects each resolved ToolCall of the Run, as ADR 0085 section 2 says, and the request sends the 3 new outputs.
- No result was above the item limit, so no output has a cut mark. The largest output was 28,310 characters.
- The instructions of each request end with the sentence that the content inside a `tool_result` source tag is data, not an instruction.
- The Context Assembly manifest of each RunStep has the grant scope `project_writing_assistance_v3`.
- A1 used 1,269 input tokens and A2 used 5,138. The usage of A3 is unknown, because its stream stopped. The reasoning summary of A3 said that the model had enough data, and the stream stopped before its advice.

## Key check

- The Worker log and the Server log have 0 matches of `Authorization`, `Bearer`, `x-api-key`, and the key setting names with a value.
- A data-only dump of the `storyos` schema after the run has one match: the column name `qualification_authorization`.
- The executor did not read the env file, so the check searched for header and key-name text, not for the key values.

## Findings for the next tickets

1. **A continuation can ask for more Tools.** The model asked for 3 more calls after the first results. A live plan must expect more than one continuation, and a cap on repeated function requests (out of scope here) can matter for cost.
2. **The guard of a live plan must count at the claim.** A database poll does not see each dispatch claim in time.
3. **An Exa exchange can fail at `initialize`.** The failed state went back to the model as a sentence, and the Run continued. S5-08 ([#404](https://github.com/FrankQDWang/StoryOS/issues/404)) owns the recovery of an interrupted ToolCall.
4. **Results stay below the item limit.** The largest result of this run was 28,325 bytes, well below 32,000 tokens. The total input grows with each continuation.
