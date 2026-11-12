# Agent Plan `tool_choice` probe of doubao-seed-2-1-pro-260915, 2026-11-12

Evidence record of [Qualify `tool_choice` and Several Function Calls on the Model Route](https://github.com/FrankQDWang/StoryOS/issues/1452), the first ticket of [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451). It records the live requests that give the facts for the finish rule of [ADR 0091](../adr/0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md) section 4. It is not a claim about model quality, Provider attention, retention, training, or billing.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). Personal, non-commercial use (#392).
- The plan was posted before the first request: [7 requests, cap 12](https://github.com/FrankQDWang/StoryOS/issues/1452#issuecomment-6823219493). R1 to R6 were sent. The loop stopped after R6, because R6 had no function call.
- R2 showed that `tool_choice: "none"` does not prevent a function call. An amendment added 3 requests before they were sent, and kept the cap: [amendment 1](https://github.com/FrankQDWang/StoryOS/issues/1452#issuecomment-6823256868). It tests a request with no `tools` field.
- Sent: R1 to R9, from 12:10:19 to 12:12:30 UTC. 9 requests in all. No retry, no repeat, and no anomaly: each status was HTTP 200, and the time and the usage were in the expected range.
- The requests did not go through the Model Gateway, and a test process loaded the key. The author approved this as a one-time exception to ADR 0049 and ADR 0050 on 2026-11-12 ([ADR 0049 amendment](../adr/0049-admit-an-author-authorized-runtime-qualification.md#amendment-one-exception-for-the-tool_choice-probe-2026-11-12)). The tests that sent them are not in the repository. This record keeps the request bodies.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2-1-pro-260915`, the provider model of Registration `fa17` |
| Path | Two local tests of the model adapter at `main` `68304b12`, not merged. Each body went directly to the `exchange` of the adapter: one POST, no retry, no redirect, and the response mapping of the product. No AgentRun, Model Attempt, or Outbound Disclosure Event records the requests. |
| Credential | The settings loader (`storyos_settings::load`) in the test process, with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. No agent read the value, and no file, log, or output has it. |
| Request bounds | `agent_plan_request_v3`: 32,768 output tokens, 600 s |

Each request is not streamed and stores the response (`"stream": false`, `"store": true`). It sends a constant probe instruction that tells the finish rule, and no Project content. When it has tools, it declares three strict functions (`"strict": true`, `"additionalProperties": false`):

- `storyos_probe_echo` with one string `text`. Its output is always empty text.
- `storyos_probe_add` with the integers `a` and `b`. Its output is the sum, or the refusal `Refused: the argument b must be 2 or less. Correct the call.` when `b` is above 2.
- `finish` with no parameters. Its output is empty text.

A continuation sends `previous_response_id` and one `function_call_output` for each call of the prior response. In R4 to R6, the test follows the finish rule of ADR 0091 section 4: `required` until a response has a `finish` call and no refused call, then `none`.

## Request bodies

Each body is the canonical JSON of these members: sorted keys, no whitespace. The SHA-256 of each body is the digest in the table of requests.

The members of each request:

```json
{
  "model": "doubao-seed-2-1-pro-260915",
  "instructions": "You answer a capability probe of StoryOS. Use the supplied tools as the request asks. When your work is complete, call finish. After the result of finish, write one short final answer.",
  "max_output_tokens": 32768,
  "stream": false,
  "store": true
}
```

A request with tools also has `tool_choice` and these `tools`:

```json
[
  {
    "type": "function",
    "name": "storyos_probe_echo",
    "description": "Returns the text that it receives.",
    "strict": true,
    "parameters": {
      "type": "object",
      "properties": {
        "text": {
          "type": "string",
          "description": "The text to return."
        }
      },
      "required": [
        "text"
      ],
      "additionalProperties": false
    }
  },
  {
    "type": "function",
    "name": "storyos_probe_add",
    "description": "Returns the sum of two integers.",
    "strict": true,
    "parameters": {
      "type": "object",
      "properties": {
        "a": {
          "type": "integer"
        },
        "b": {
          "type": "integer"
        }
      },
      "required": [
        "a",
        "b"
      ],
      "additionalProperties": false
    }
  },
  {
    "type": "function",
    "name": "finish",
    "description": "Tells StoryOS that your work is complete.",
    "strict": true,
    "parameters": {
      "type": "object",
      "properties": {},
      "required": [],
      "additionalProperties": false
    }
  }
]
```

The other members:

| # | `tool_choice` | `previous_response_id` | `input` |
| --- | --- | --- | --- |
| R1 | `required` | none | one `user` message with one `input_text`: `Reply with the word hello. Do not call a tool.` |
| R2 | `none` | none | one `user` message: `Call storyos_probe_echo with the text beta.` |
| R3, R8 | `required` | none | one `user` message: `Call storyos_probe_echo with the text alpha, and call storyos_probe_add with a = 2 and b = 3. Make the two calls in this response.` |
| R4 | `required` | R3 | `function_call_output` `call_rdwo9jj80hbeksuqp99b0oy0` with `""`, then `call_d7j57rkhhgvgm001wg4xjxsv` with the refusal text |
| R5 | `required` | R4 | `function_call_output` `call_ac5qbxwq3x03cfa8n62wueay` with `"4"` |
| R6 | `none` | R5 | `function_call_output` `call_gz48dcp7rsfsr4esz8sxq1pm` with `""` |
| R7 | no `tools` | none | the `user` message of R2 |
| R9 | no `tools` | R8 | `function_call_output` `call_snbrbzzp5zw5yodzdw8he7xm` with `""`, then `call_le1r53z7xp0cw69kba3e75hr` with the refusal text |

A `user` message is `{"role": "user", "content": [{"type": "input_text", "text": <text>}]}`. A function output is `{"type": "function_call_output", "call_id": <ID>, "output": <text>}`. The refusal text is `Refused: the argument b must be 2 or less. Correct the call.`

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

The author approved this rule on 2026-11-12, and it amends ADR 0091 sections 4 and 5:

- Each request before a `finish` call that counted has `tool_choice: "required"`. Thus a text with no `finish` call cannot end a Run.
- The closing request after a `finish` call that counted has `tool_choice: "none"`, which the route does not enforce. A closing response with no function call gives the final answer. A closing response with a function call makes the `finish` call stop counting: each call gets its result, and the Run continues.
- The request of RunStep 24 has `tool_choice: "none"`. The Host executes no function call of its response, and the text is the final answer.
- The fallback with one Host sentence goes.

## What this evidence qualifies

- On the exact route and the dated version, in non-streamed requests: `tool_choice: "required"`, three strict function declarations, two function calls in one response, a function output with empty text, a function output that reports a refusal, and the continuation of each with `previous_response_id`.

## What it does not qualify

- `tool_choice: "none"` and a request with no `tools` as a means to permit no function call. The evidence shows that neither prevents a call.
- The object form of `tool_choice`, streamed function call events, and `parallel_tool_calls`.
- A rate of each behavior. Each case had one or two samples.
- Provider attention, retention, and training (S4-REQ-007).
