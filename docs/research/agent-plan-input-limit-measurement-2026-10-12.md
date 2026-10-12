# Agent Plan input and output size measurement, 2026-10-12

Evidence record of [S4-07: Measure the Model Input Limit and Raise the Context Item Limit](https://github.com/FrankQDWang/StoryOS/issues/1104). It records the live requests that measured the input size and the output size that the qualified Agent Plan route accepts. It is the evidence that the ADR of S4-07 cites. It is not a claim about model quality, Provider attention, retention, or training.

## Authorization and plan

- Live requests: the standing author authorization on [#362](https://github.com/FrankQDWang/StoryOS/issues/362). Personal, non-commercial use (#392).
- The coordinator capped the input ladder at about 128k tokens. The decision needs a per-item limit far below the context window, because one request carries several items, the Host instructions, and an output reserve. The ladder goes above 128k only if 128k is refused or the author asks.
- Planned requests: 8 at most, written before the first request, with the expected totals: about 254k input tokens and about 36k output tokens (hard upper bound 7 x 8192 + 65536 = 122,880 output tokens). Sent: 8. Each request ran in one process. No request was sent again.
- Stop rule: stop at the first refusal or at an anomaly. No refusal and no anomaly occurred.

## Exact route and request shape

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3/responses` |
| Model | `doubao-seed-2.1-pro` (the qualified route of S4-02c) |
| Credential | Operator quota, `STORYOS_AGENT_PLAN_API_KEY` through the `storyos-settings` layer (ADR 0050). A throwaway probe outside the repository loaded the key with `storyos_settings::load` and `EnvFile::RealRoute`. No agent read, printed, or searched `.env`. |
| Request shape | The product wire shape of `wire.rs` at `main` `1fc55a1b`: the exact Host instructions (879 scalars), one user message with `Author request:` and `Working text:` sections, `stream: false`, `store: true` |
| Output bound | `max_output_tokens` 8192 for the input requests, 65536 for the rewrite probe |
| Client timeout | 300 s for the input requests, 900 s for the rewrite probe. No request reached its timeout. |
| Content | Synthetic. Generated Chinese prose from the seeded generator of `prototypes/chinese-long-form-baseline/generate.py` (seed 20261005). The public-domain excerpt of that prototype. Generated CJK Extension B scalars and emoji scalars. |

## Requests

Scalars are the Unicode scalars of the user message text, which the current Token Counting Profile counts. The Host instructions are not in this count. Tokens are the values that the Provider reported in `usage`.

| # | Purpose | Time (UTC) | User text scalars | Body bytes | Input tokens (cached) | Output tokens (reasoning) | Latency | Outcome |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| C1 | Calibration, generated prose | 08:28:35 | 20,048 | 60,817 | 14,066 (0) | 191 (160) | 6.5 s | completed |
| C2 | Calibration, public-domain classical prose | 08:28:47 | 2,449 | 8,298 | 2,490 (0) | 3,767 (3,748) | 83.8 s | completed |
| C3 | Calibration, CJK Extension B scalars | 08:30:11 | 1,044 | 5,170 | 4,236 (0) | 105 (92) | 5.7 s | completed |
| C4 | Calibration, emoji scalars | 08:30:17 | 1,044 | 5,170 | 2,401 (0) | 81 (68) | 3.7 s | completed |
| L1 | Input ladder, about 32k tokens | 08:30:34 | 46,048 | 138,369 | 31,964 (13,112) | 199 (170) | 8.6 s | completed |
| L2 | Input ladder, about 64k tokens | 08:30:52 | 92,648 | 277,318 | 64,144 (31,544) | 181 (152) | 7.1 s | completed |
| L3 | Input ladder, about 128k tokens | 08:31:05 | 185,548 | 554,332 | 128,186 (63,288) | 188 (159) | 10.7 s | completed |
| W1 | Rewrite one 12,000-scalar Chapter in full | 08:31:28 | 12,123 | 37,140 | 8,576 (0) | 29,577 (20,421) | 451.5 s | completed, complete JSON reply with 12,486 rewritten scalars |

Totals: 256,063 input tokens and 34,289 output tokens.

Response references and request digests:

| # | Response reference | Request body digest |
| --- | --- | --- |
| C1 | `resp_021791793715402833b5625726f7c6da3e0b25f98b670c6a5e9de` | `sha256:1fa396af18e0ddf37fabe1d0576239a5acaee360d3c3ca1569662d7a478bf461` |
| C2 | `resp_02179179372777388db556329240132465f0386a4fcfe59aff1b0` | `sha256:042959babd54b291d078d836a4375a64d0d157d3f2d71b7e921b7adfea83c843` |
| C3 | `resp_0217917938115961dd291674643982da436c46312b5f1c91beda6` | `sha256:13a36fc22107b9832177a9a8daae6e835b3286a7f2d07770c3bc96143340d1d9` |
| C4 | `resp_021791793817316b71a54db2718758900f1b5b7cc45ea65c6066d` | `sha256:29dddf79c4be48640c65f99e15c897308d19799cd6f6f0f0b4edeb1a7c03ece6` |
| L1 | `resp_021791793834949d3a5eaea6011fe6a16a81478091c72937f1b59` | `sha256:4eae0fddc9d2653558567eadfc048720a91068811e0353bea7020b08bb197bfd` |
| L2 | `resp_0217917938530498c140f267f4351837af8462310b5bfeea545d9` | `sha256:8b214ec44a56ce7416ba7629c59f6d0efa267132f05700e9aa9e6c9f434ad868` |
| L3 | `resp_0217917938652036c7b67136913b8291adfcb1079846bf41ba5ce` | `sha256:05f4b4beb2380b5332f0da28bf512b58860d84840e92459ba9a45f57ee07bf95` |
| W1 | `resp_021791793888831c65202e590747a44f6b901a7b0655cff25b715` | `sha256:fb18d7aff68a3fb983363bc7b9ab9644a658fa90b2b457b9049a5a4feac3e7ce` |

The ladder requests share a prefix, so the implicit cache of the route served part of L1, L2, and L3. The cache does not change the reported input token count.

## Results

### Accepted input

- The route accepted 128,186 input tokens (185,548 scalars of Chinese prose, 554 KB of request body) in 10.7 s. No request was refused.
- The documented values of the model are a 256K context window that input and output share, and a 256K maximum output ([Ark model list](https://www.volcengine.com/docs/82379/1799865), and the model page of Seed 2.1). This record did not send a request above 128k tokens. The documented 262,144-token window is the upper bound that StoryOS cites. A brief of the ticket named a 1024k context. No first-party page that this record found confirms 1024k for this model.
- Input latency is small. A 128k-token input completed in 10.7 s, so the input size does not set the request timeout.

### Provider tokens for each scalar

A linear fit of C1, L1, L2, and L3 gives 0.6896 input tokens for each scalar of generated prose, and a fixed overhead of about 241 tokens for the Host instructions and the message frame.

| Content | Provider tokens for each scalar |
| --- | ---: |
| Generated modern Chinese prose | 0.69 |
| Public-domain classical Chinese prose | 0.92 |
| Emoji | 2.13 |
| CJK Extension B | 3.97 |

The Unicode scalar count is thus a safe upper bound of Provider tokens for ordinary Chinese prose, but not for emoji or rare CJK scalars. A text of only rare scalars can need about 4 Provider tokens for each counted token.

### Output of a long Chapter rewrite

- W1 asked for a full rewrite of a 12,000-scalar Chapter in one reply. The route returned a complete reply with 12,486 rewritten scalars.
- It used 29,577 output tokens: 20,421 reasoning tokens and 9,156 reply tokens. The reply used 0.72 tokens for each reply scalar.
- It took 451.5 s, which is about 65.5 output tokens for each second.
- The current bound `agent_plan_request_v1` (8192 output tokens, 180 s) cannot complete this request. The token bound stops the reply at less than one third, and the timeout stops the wait first.
- The reasoning share changes much between requests. C2 used 3,748 reasoning tokens for a one-sentence answer.

## What this evidence supports

- The qualified route accepts at least 128,186 input tokens in one Create request with the product request shape.
- The route completes a non-streamed request that takes 451.5 s and returns 29,577 output tokens.
- The mapping from the scalar count to Provider tokens for the four content classes above.

## What it does not support

- Acceptance above 128,186 input tokens. The documented 262,144-token window stays a documented value.
- The exact refusal shape of the route for an input above its window.
- Streaming, continuation, cache behavior, and timeout behavior of the route (S4-06 and S4-04).
- Model quality of the rewrite.
