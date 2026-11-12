# Agent Plan `tool_choice` probe of doubao-seed-2-1-pro-260915, 2026-11-12

Evidence record of [Qualify `tool_choice` and Several Function Calls on the Model Route](https://github.com/FrankQDWang/StoryOS/issues/1452), the first ticket of [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451). It records the live requests that give the facts for the finish rule of [ADR 0091](../adr/0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 4. It is not a claim about model quality, Provider attention, retention, training, or billing.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). Personal, non-commercial use (#392).
- The plan was posted before the first request: [7 requests, cap 12](https://github.com/FrankQDWang/StoryOS/issues/1452#issuecomment-6823219493). R1 to R6 were sent. The loop stopped after R6, because R6 had no function call.
- R2 showed that `tool_choice: "none"` does not prevent a function call. An amendment added 3 requests before they were sent, and kept the cap: [amendment 1](https://github.com/FrankQDWang/StoryOS/issues/1452#issuecomment-6823256868). It tests a request with no `tools` field.
- Sent: R1 to R9, from 12:10:19 to 12:12:30 UTC. 9 requests in all. No retry, no repeat, and no anomaly: each status was HTTP 200, and the time and the usage were in the expected range.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2-1-pro-260915`, the provider model of Registration `fa17` |
| Path | The ignored tests `live_tool_choice_probe_of_the_agent_plan_route` (R1 to R6) and `live_no_tools_probe_of_the_agent_plan_route` (R7 to R9) in `crates/storyos-adapter-volcengine-responses/src/probe_tests.rs`. Each body goes through the `exchange` of the adapter: one POST, no retry, no redirect, and the response mapping of the product. |
| Credential | The typed settings (`storyos_settings::load`, ADR 0050) with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. No agent read the value, and no log has it. |
| Request bounds | `agent_plan_request_v3`: 32,768 output tokens, 600 s |
| Build | `main` `68304b12` with the probe tests of this record |

Each request is not streamed and stores the response (`"stream": false`, `"store": true`). It sends a constant probe instruction that tells the finish rule, and no Project content. When it has tools, it declares three strict functions (`"strict": true`, `"additionalProperties": false`):

- `storyos_probe_echo` with one string `text`. Its output is always empty text.
- `storyos_probe_add` with the integers `a` and `b`. Its output is the sum, or the refusal `Refused: the argument b must be 2 or less. Correct the call.` when `b` is above 2.
- `finish` with no parameters. Its output is empty text.

A continuation sends `previous_response_id` and one `function_call_output` for each call of the prior response. In R4 to R6, the test follows the finish rule with enforcement in the request: `required` until a response has a `finish` call and no refused call, then `none`.

## Requests

| # | Time (UTC) | Request | Response | Body digest | Observed | Usage (input / output) |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | 12:10:19 | fresh, `required`, "Reply with the word hello. Do not call a tool." | `resp_021794485420197d2ec6088cd57f16be69183642d63bc77636161` | `sha256:77f54911…` | completed: a message `hello` and one `finish` call | 652 / 167 |
| R2 | 12:10:25 | fresh, `none`, "Call storyos_probe_echo with the text beta." | `resp_021794485425626c097066c207cab501b126d22ce2732948e956a` | `sha256:7782556b…` | completed: one `storyos_probe_echo` call `{"text": "beta"}` and no message | 652 / 54 |
| R3 | 12:10:28 | fresh, `required`, two calls asked | `resp_0217944854287424a18dd3860b887d86b9a7f28a91109c3eddb3c` | `sha256:0f473a77…` | completed: two calls, `storyos_probe_echo` `{"text": "alpha"}` and `storyos_probe_add` `{"a": 2, "b": 3}`, and no message | 678 / 134 |
| R4 | 12:10:33 | R3 continued, `required`, an empty echo output and a refused add output | `resp_0217944854335356525c18cf4d9efd868f038535426f8165ec4a1` | `sha256:3424906a…` | completed: a message that tells the refusal, and one corrected call `storyos_probe_add` `{"a": 2, "b": 2}` | 844 / 151 |
| R5 | 12:10:37 | R4 continued, `required`, the add output `4` | `resp_0217944854375154e0d847e114bd906533eadfa024bfe6db55f43` | `sha256:257628ee…` | completed: one `finish` call and no message | 1,006 / 85 |
| R6 | 12:10:40 | R5 continued, `none`, an empty `finish` output | `resp_02179448544042823c5b6bc0dca724893eb7d2b1f969c7cae502c` | `sha256:850963d2…` | completed: a final message and no function call | 1,102 / 82 |
| R7 | 12:12:26 | fresh, no `tools`, the author message of R2 | `resp_021794485546691ab257ece25bd0fea81c82ca5c15ec8fdef35ad` | `sha256:78a43e54…` | completed: a message whose text is the raw call markup `<seed:tool_call><function name="storyos_probe_echo">…</seed:tool_call>`, and no function call item | 85 / 57 |
| R8 | 12:12:28 | the body of R3 again | `resp_02179448554916420c19fad7de95dc34d140d8afb6e0840fa1fe9` | `sha256:0f473a77…` | completed: the same two calls as R3, and no message | 678 / 80 |
| R9 | 12:12:30 | R8 continued, no `tools`, the outputs of R4 | `resp_021794485551125d2ec6088cd57f16be69183642d63bc7767d814` | `sha256:dd630c30…` | completed: one `finish` call item and no message | 223 / 193 |

Each response also had one reasoning item first. Each function call item had the status `completed` and arguments that agree with its schema. R8 has the same body digest as R3, so the second test sent the same body as the first test.

Usage of R1 to R9: 5,920 input and 1,003 output tokens.

## Result

- **`tool_choice: "required"` is enforced.** Each of the 5 responses to a `required` request held at least one function call: R1, R3, R4, R5, and R8. R1 had an author message that asked for no call. Such a response can also hold a message: R1 and R4.
- **`tool_choice: "none"` is not enforced.** R2 asked for a call with `none`, and the response held a function call and no text. R6 held text and no call, but there the model had no call to make.
- **A request with no `tools` field does not prevent a call either.** In the continuation R9, the route returned a native `function_call` item for `finish`, although the request declared no function. In the fresh R7, the model wrote the raw call markup as message text.
- **Three declarations and two calls.** The route accepts three strict declarations, also `finish` with an empty `properties` object. R3 and R8 each held two function calls in one response.
- **Function outputs.** The route accepts an output with empty text (the echo outputs of R4 and R9, the `finish` output of R6) and an output that reports a refusal (R4, R9). After the refusal in R4, the model made the corrected call. In R9, with no tools, it called `finish` and did not correct the call.
- **An empty output can get invented content.** The reasoning of R5 and the final message of R6 say that the echo returned `alpha`, but its output was empty text.

## Effect on the finish rule

- The requests before a `finish` call can enforce the rule with `tool_choice: "required"`. A text with no `finish` call then cannot end a Run, because each response has a call.
- No request form that this probe tested makes the closing RunStep permit no function call. A response of the closing RunStep, or of RunStep 24, can hold a function call. #1451 says that such a response holds no call. That statement needs a rule from the author before the ticket that builds the closing RunStep.

## What this evidence qualifies

- On the exact route and the dated version, in non-streamed requests: `tool_choice: "required"`, three strict function declarations, two function calls in one response, a function output with empty text, a function output that reports a refusal, and the continuation of each with `previous_response_id`.

## What it does not qualify

- `tool_choice: "none"` and a request with no `tools` as a means to permit no function call. The evidence shows that neither prevents a call.
- The object form of `tool_choice`, streamed function call events, and `parallel_tool_calls`.
- A rate of each behavior. Each case had one or two samples.
- Provider attention, retention, and training (S4-REQ-007).
