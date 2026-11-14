# Agent Plan live compaction run (2026-11-14)

This record is the live evidence of [Qualify the Mapping v6 on the Real Route and Reach the Threshold in One Live Run](https://github.com/FrankQDWang/StoryOS/issues/1513) for the specification [#1507](https://github.com/FrankQDWang/StoryOS/issues/1507) and [ADR 0093](../adr/0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md). It records two Runs in one Conversation on the real Agent Plan route and the real Exa MCP server. Run 1 qualifies the read of a stored result. Run 2 reaches the threshold at RunStep 2, compacts automatically, sends stubs and the section of the earlier author messages, reads a stub result again, and answers.

Raw evidence: the small files are in [`agent-plan-live-compaction-2026-11-14/evidence/`](agent-plan-live-compaction-2026-11-14/evidence/). The wire payload of each request, the Server and Worker logs, the manifest items, and a data dump of the disposable database are on the tag `evidence/1513-live-compaction-2026-11-14`.

## Setup

| Item | Value |
| --- | --- |
| Product code | `compaction-stubs` at `676c7f95960168ead16558c5d961aca77af37861` (tree `ee69b0a8`). This branch changes no product file. |
| Release package | `python3 scripts/package-release.py` on the clean tip above, manifest `sha256:fba7f398fc56b1099d1b58cbff1def2ea84998b8fbc7cda0ca006a0e5b54a6c7`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8747`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package, each with a process environment that has only the named variables, `PATH`, and `HOME`. |
| Settings | ADR 0050: the Worker resolved both keys from the env file of the main checkout, which it found from its working directory in the worktree. This run never read, copied, or printed that file or a key. |
| Key check | The Server log (401 lines), the Worker log (193 lines), each wire payload, and a data dump of the `storyos` and `live_guard` schemas have no `Authorization` header, no bearer token, and no `x-api-key`. The matches of the check are the column `qualification_authorization`, the Credential References `settings:STORYOS_AGENT_PLAN_API_KEY` and `settings:STORYOS_EXA_API_KEY`, the title "The Torchbearer", and the path `ask-a-librarian`. |
| Database | New `scripts/dev-postgres.sh up --interactive` database of this worktree with the controlled fixture. The only direct writes were the guard schema `live_guard`. The Chapter text went through the editor. The database was removed after the run. |
| Request guard | Triggers of the guard schema ([`request-guard.sql`](agent-plan-live-compaction-2026-11-14/evidence/request-guard.sql)): the insert of a Model Attempt counts one model request, and the first `destination_attempt_id` of a ToolCall counts one Exa exchange. Above the cap the trigger refuses the claim transaction, so nothing goes out, and the Worker tries the claim again. |
| Model | `doubao-seed-2-1-pro-260915`, Model Registration `018f0000-0000-7001-8000-00000000fa20`, mapping `storyos.volcengine-agent-plan.responses.create.v6`, bounds `agent_plan_request_v4` (32,768 output tokens), window 1,048,576 tokens, threshold (1,048,576 - 32,768) x 80 percent = 812,646 tokens. Grant scope `project_writing_assistance_v3`. |
| Author surface | The packaged Web client in the built-in browser of the Claude desktop app, bootstrap session `session-a`. `localStorage`, `sessionStorage`, and IndexedDB of the origin were clear before the first step. New Project `01a1da35-a3e6-72f3-9072-9b209f03b6fd` ("压缩实测"), Volume 第一卷, Chapter 灯塔 `01a1da40-1e99-79a3-817d-c5b284354e53` with one paragraph typed in the editor, Conversation `01a1da40-a172-7273-9694-2e56d91ab6ea`. |

A Run needs a Chapter: the first send before the Chapter existed made no request and no Run.

## Plan

Written at 13:18:42 UTC, before the first request.

- Purpose: qualify each request form of the mapping `v6` with ordinary Runs on `fa20`, and reach the threshold with the automatic compaction at RunStep 1 or later in one Run of the same Conversation. No product change and no measurement build.
- A design fact (verified on the tip, `compaction_due` and `sent_input_tokens` in `crates/storyos-core/src/compact_active_context.rs`): the expected input of a later RunStep is the reported input and output of the prior response plus the counted tokens (Unicode scalars) of the new Tool outputs. One Tool result has at most 32,000 counted tokens. Each request with a prior reference is charged for the full context. Thus the cheapest plan puts many Tool results into one response, and the compaction starts before the reported usage gets near 812,646.
- Run 1, qualification and calibration: 3 parallel `web_search` calls, then `read_tool_result` of the first call together with `finish`, then the closing request. The author message also fixes three story facts for the summary topics. Plan: 3 model requests, 3 Exa exchanges, about 70,000 input tokens.
- Run 2, threshold: RunStep 0 with 5 parallel searches (group A, these become stubs), RunStep 1 with N parallel searches (group B, N from the calibration), RunStep 2 with the automatic compaction (summary request and later request), the model reads the first A result again with `finish`, then the closing request. Plan: 5 model requests, 5 + N Exa exchanges, about 950,000 input tokens.
- Total plan: 8 model requests, about 1,020,000 input tokens. Automatic requests (a Retrieve, a compaction request, a correction after a refusal) count.
- Hard cap: 11 model requests and 55 Exa exchanges, with a manual gate before the RunStep 2 request of Run 2. The gate opens only when the expected input is above the threshold and the later request stays below the window after the output bound (1,015,808 tokens).
- No `propose_edit` and no steering input in this Conversation, because the rebuild path after an opened Proposal (finding F2 of the [tool loop live run](agent-tool-loop-live-run-2026-11-14.md)) leaves the prior response.
- The plan went to the coordinator before the first request, because the coordinator expected the reported input to reach 812,646 tokens.

Plan change at 13:33:07 UTC, after Run 1 and before the first request of Run 2. Calibration of Run 1: one English search result of 10 entries has about 28,400 counted tokens and about 7,160 reported tokens (3.97 counted tokens for each reported token). With a prior usage of about 77,000 tokens after RunStep 1, the threshold needs at least 26 group B results, and the window limit of the later request permits at most 34. N = 31 gives an expected input of about 950,000. New caps: 9 model requests (3 used, Run 2 at most 6) and 39 Exa exchanges (3 used, Run 2 exactly 36). The gate cap was 5 model requests.

Host limits, verified on the tip before the gate opened (coordinator decision of 2026-11-14):

| Limit | Value | Source |
| --- | --- | --- |
| Function calls in one response | No Host limit. The route accepted 31 calls in M5. | No such constant in `storyos-core`; `step_tool_choice` in `finish_rule.rs` counts only `finish` and refused calls. |
| RunSteps of one Run | 24 | `MAX_RUN_STEPS`, `finish_rule.rs` |
| Results of one `web_search` | 10 | `RESULT_COUNT_MAX`, `tool_spec.rs` |
| URLs of one `web_fetch` | 5 | `URLS_MAX`, `tool_spec.rs` |
| Counted tokens of one Context item (one Tool result) | 32,000 | `CONTEXT_ITEM_TOKEN_LIMIT`, `assemble_context.rs` |
| Stored text of one Tool result | 262,144 bytes | `TOOL_RESULT_TEXT_MAX_BYTES`, `tool_spec.rs` |
| Refused `propose_edit` calls of one Run | 3 | `MAX_REFUSED_PROPOSE_EDIT_CALLS`, `propose_edit.rs` |
| The later request check | The counted tokens of the selected items, the reference texts, and the declared targets of the later request must stay below the window after the output bound, 1,015,808. | `later_request_too_long`, `compact_active_context.rs` |

At the gate, the 31 group B results had 840,972 characters. With the citable sources tag (about 1,600 counted tokens for each result in Run 1), the estimate was about 890,000 counted tokens, an expected input of about 961,000, and a later request of at most about 937,000 counted tokens with a summary at its limit of 32,000. The records later gave 888,771, 959,628, and 913,509.

Final count: **8 model requests and 39 Exa exchanges, equal to the plan.** No Retrieve, no recovery, and no refusal occurred. Each Model Attempt is `settled`. The gate refused 64 claim attempts from 13:36:09 to 13:43:35 UTC, and none of them sent a request.

## Live request log

Each row is one claim that the guard counted. The time is the claim time (UTC). The request ID of a model request is the response reference of the Provider. Usage is the input / output tokens that the Provider reported, and "cached" is `cached_tokens`. `prev` is the request whose response the request continued with `previous_response_id`. The 39 Exa exchanges are in [`guard-log.jsonl`](agent-plan-live-compaction-2026-11-14/evidence/guard-log.jsonl) and [`tool-calls.json`](agent-plan-live-compaction-2026-11-14/evidence/tool-calls.json).

| # | Time | Run, RunStep | Request ID | Mapping, `prev` | `tool_choice` | Result | Usage (cached) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 13:30:31 | 1, 0 | `resp_02179466303144836a85968bdbb8a949a5bf709eb7d71bc56eb96` | none, none | required | 3 `web_search` calls in one response | 2,135 / 389 (0) |
| E1–E3 | 13:30:39 | 1, 0 | 3 ToolCalls | — | — | each settled, 13:30:39 to 13:30:45, 26,823 to 27,067 bytes | — |
| M2 | 13:30:49 | 1, 1 | `resp_021794663049395039442d358435d172954296e77d251e5e61ad1` | incremental, M1 | required | `read_tool_result` of the first call and `finish` in one response | 23,997 / 168 (1,336) |
| M3 | 13:30:55 | 1, 2 | `resp_021794663055930c37b19e6f2deafccdd0d14a1cba0df7b4f84f8` | incremental, M2 | none | final answer | 31,621 / 2,032 (0) |
| M4 | 13:33:34 | 2, 0 | `resp_021794663214693d2ec6088cd57f16be69183642d63bc775c9204` | incremental, M3 | required | 5 `web_search` calls (group A) | 32,373 / 427 (0) |
| E4–E8 | 13:33:43 | 2, 0 | 5 ToolCalls | — | — | each settled, 13:33:43 to 13:33:55, 26,732 to 27,372 characters | — |
| M5 | 13:34:00 | 2, 1 | `resp_021794663240213fe278ef4a49fd99537752324bd80717ae5acdd` | incremental, M4 | required | 31 `web_search` calls (group B) | 69,312 / 1,545 (1,848) |
| E9–E39 | 13:34:25 | 2, 1 | 31 ToolCalls | — | — | each settled, 13:34:25 to 13:36:04, 23,419 to 28,300 characters | — |
| M6 | 13:44:05 | 2, 2 | `resp_0217946638453562a2bed297ad64eae2b89e2702342ef979aab71` | summary request, M5 | (no tools) | summary, 11,929 counted tokens | 299,550 / 6,473 (0) |
| M7 | 13:46:51 | 2, 2 | `resp_021794664011248b97e702619832bacb4ebf6c3ac1ede678f5ea9` | new transport (later request), none | required | `read_tool_result` of A1 and `finish` in one response | 242,134 / 497 (1,848) |
| M8 | 13:47:17 | 2, 3 | `resp_02179466403782141f89ff67558dec75454d97963e9341b90b736` | incremental, M7 | none | final answer | 249,781 / 1,325 (1,848) |

Usage total: **950,903 input tokens and 12,856 output tokens in 8 model requests.** Run 1: 57,753 / 2,589. Run 2: 893,150 / 10,267.

## Qualification of the mapping v6

Each request went through the admitted path: an AgentRun on `fa20`, the Model Gateway of the Worker, and a Model Attempt with its disclosure records. The route accepted each request form of `v6`.

| Request form of `v6` | Requests |
| --- | --- |
| The declaration of `read_tool_result` in `tools` (six tools: `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `finish`) | M1, M2, M3, M4, M5, M7, M8 |
| The Host instructions with the paragraph on `read_tool_result` and the paragraph on stubs | M1, M2, M3, M4, M5, M7, M8 |
| A `read_tool_result` call, and its output in the exact form of the first read (`<tool_result source="<read call>" state="ok">…`) | M2 and M3 (Run 1), M7 and M8 (Run 2) |
| A stub output (`form="stub"`) | M7: 5 stubs |
| The section of the earlier author messages | M7 |
| The summary request with the three new topics of the summary instruction | M6 |
| The later request of a compaction at RunStep 1 or later, and the incremental request after it | M7 (RunStep 2) and M8 (RunStep 3) |

## Compaction record

`getAgentRun` of Run 2 (`01a1da43-6d75-7f12-a047-7be6c683a0ae`) has one item in `compactions`, and `active_compaction` is the same record ([`compaction-record.json`](agent-plan-live-compaction-2026-11-14/evidence/compaction-record.json)):

- `compaction_id` `01a1da4f-9436-7031-a9bf-fc72d953edf2`, Decision position 2, `install_state` `installed` with the Model Attempt of M7, `producer` `agent_plan_summary`, `mapping_kind` `host_managed`, `mapping_revision` `…responses.create.v6`, loss fact `semantic_preservation_unknown`.
- `stub_tool_call_ids`: the five group A ToolCalls `01a1da43-9137-7681-a8dd-351b28bdafcf`, `…9138-76c2-…`, `…9139-78a1-…`, `…913a-77a2-…`, `…913b-7931-…`.
- `kept_author_message_ids`: `01a1da40-a172-7273-9694-2e68070c5ca9` (the message of Run 1, whose identity is the Run).
- `preserved_item_ids`: the reasoning item and the 31 function calls of M5.

The decision manifest of the active RunStep 3 in the same read selects 5 `tool_result` items with the projection mode `tool_result_stub` and 33 with `exact_required` (31 group B results, the read result, and the `finish` result).

The Host started the compaction at RunStep 2 with these numbers:

| Value | Tokens |
| --- | --- |
| Reported usage of M5 (input + output) | 70,857 |
| Counted tokens of the 31 new outputs (the `compaction` manifest of position 2) | 888,771 |
| Expected input of RunStep 2 | **959,628** (threshold 812,646) |
| Counted tokens of the later request (summary 11,929 + 2 author items 3,246 + 5 stubs 9,535 + 31 exact results 888,771 + working target 28) | 913,509 (window after the output bound 1,015,808) |
| Margin of the later request to the bound of `later_request_too_long` | 102,299 (10.1 percent of 1,015,808) |

The check `later_request_too_long` compares counted tokens with the window after the output bound. The same counted measure that started the compaction early can thus also end a Run with a false `conversation_too_long` ([#1520](https://github.com/FrankQDWang/StoryOS/issues/1520)). This later request was 102,299 counted tokens below that bound. Its reported input was 242,134 tokens.

## The later request as the route got it

M7 sent no `previous_response_id`. Its input is one user message, then the 36 function calls of Run 2, then 36 function outputs: 5 stubs for the group A calls and 31 exact outputs for the group B calls ([`later-request-user-message.json`](agent-plan-live-compaction-2026-11-14/evidence/later-request-user-message.json), [`later-request-stubs.txt`](agent-plan-live-compaction-2026-11-14/evidence/later-request-stubs.txt)). The function calls and outputs of Run 1 are not in it.

The user message has these parts in order: `Summary of the earlier conversation:` (offset 0), `Earlier author messages, in the exact words of the author, from before the summary, oldest first:` (offset 11,968) with `[message 1]` and the exact message of Run 1, `Author request:` with the exact message of Run 2 (offset 12,497), `Working text:`, and `Target blocks:`.

The stub of A1, as the route got it (1,884 characters):

```text
<tool_result source="call_8w17emeafdzwv6r4igmywxzn" state="ok" form="stub">The Host replaced this result with a stub after a compaction of the conversation. Tool: web_search. Arguments: {&quot;query&quot;:&quot;steam fog whistle and fog trumpet specifications at 19th century lighthouses&quot;,&quot;result_count&quot;:10}. To read the full result again, call read_tool_result with the call_id call_8w17emeafdzwv6r4igmywxzn. The tool_sources tag gives only the first 7 of the 10 sources of the result. Read the full result again before you cite one of the other 3 sources.</tool_result><tool_sources source="call_8w17emeafdzwv6r4igmywxzn">
<source id="01a1da43-9cc3-7780-b12d-07eac8122842" title="Fog-Signals - trumpet, air, sound, powerful and boiler" locator="http://gluedideas.com/content-collection/chambers-6/Fog-Signals.html"/>
… 6 more source lines …
</tool_sources>
```

The stubs of A2, A3, and A4 kept 6 of 10 source lines, and the stub of A5 kept 7 of 9.

## The read of a stored result

The response of M7 called `read_tool_result` with `{"call_id": "call_8w17emeafdzwv6r4igmywxzn"}` (A1) and `finish`. The read settled with 28,241 bytes. M8 sent the read result as the output of the read call `call_0et2ya8pum0c0whle2ff8cot` in its exact form, `<tool_result source="call_8w17emeafdzwv6r4igmywxzn" state="ok">Title: Fog-Signals …` with all 10 source lines, and the empty output of `finish`. M8 had `tool_choice: "none"`, and its response was the final answer.

In Run 1, M2 read the first search of M1 in the same way, and M3 sent the result (28,635 counted tokens).

## The summary

M6 continued M5 with `previous_response_id`. Its input was the 31 exact outputs of group B and the user text "Summarize the conversation so far.", with no tools. The instructions had the paragraph of ADR 0093 section 6: "Also keep these topics when the conversation shows them: each Proposal that the author accepted or rejected; the story facts that the conversation fixed, …; and each research result with its sources."

The summary ([`summary-text.json`](agent-plan-live-compaction-2026-11-14/evidence/summary-text.json), 11,929 counted tokens, 6,473 output tokens) kept the three topics:

- Story facts: 林三 is the apprentice of 周伯, the island is 鹰嘴岛, the fog bell strikes three times per minute, and the current paragraph with its block identity.
- Research results with their sources: ten topic sections, each with its URLs.
- Proposals: "No text revision has yet been accepted or rejected." The Conversation had no Proposal.

It also named the pending step: "reread A1's full result and call `finish`". The summary is in English with the Chinese names, although the instruction says "Write in the language of the manuscript" (finding F3).

## Measurement

Reported usage of each request of Run 2, before and after the compaction:

| Request | Content | Reported input | Change |
| --- | --- | --- | --- |
| M4, RunStep 0 | Run 1, the new author message | 32,373 | — |
| M5, RunStep 1 | + 5 group A results | 69,312 | +36,939 |
| M6, summary request, RunStep 2 | + 31 group B results; the same context that a RunStep 2 decision request with no compaction would have | 299,550 | +230,238 |
| M7, later request, RunStep 2 | summary, section, current message, 5 stubs, 31 exact results | 242,134 | **−57,416 (−19.2 percent) against M6** |
| M8, RunStep 3 | + read result and `finish` output | 249,781 | +7,647 |

- Counted tokens against reported tokens: the 31 group B results have 888,771 counted tokens and about 228,700 reported tokens (3.9 to 1). The 5 group A results have 143,543 counted tokens and about 36,500 reported tokens. Run 1 gave 3.97 to 1. An English search result of 10 entries is about 28,400 counted tokens and 7,200 reported tokens.
- The compaction removed Run 1 (about 32,000 reported tokens) and the 5 group A results (about 36,500 reported tokens). It added the summary (6,473 output tokens of M6), the 5 stubs (9,535 counted tokens), and the section (417 counted tokens). The 31 group B results stay exact by design (ADR 0093 section 5), and they are about 94 percent of the reported input of the later request (97 percent of its counted tokens).
- The part that could become stubs: the 5 group A results went from 143,543 counted tokens to 5 stubs of 9,535 counted tokens. This is a reduction of 134,008 counted tokens (93.4 percent).
- The reduction of 19.2 percent is a property of this Run, not of an ordinary Run. This Run put 31 results into the response that the summary continued, and these waiting outputs stay exact.
- The summary request M6 took 2 min 46 s from its claim to the claim of M7. This is below the request timeout of 600 s.
- `cached_tokens` stayed at most 1,848 for each request.

**What this Run does not show.** This Run reached the threshold by the Host rule: the expected input was 959,628. The largest reported input of a request was 299,550. Thus this Run does not show the behaviour of the model, the summary, or the route at a reported context near 812,646.

## Estimate of a second Run to a reported input of about 800,000

The coordinator asked for one estimate, with its arithmetic, of a second Run that brings the reported input of the summary request to about 800,000 tokens. This Run did not send it. The author decides.

- Rule: a RunStep compacts when P + C >= 812,646, where P is the reported usage of the prior response and C is the counted tokens of the new outputs. A result adds about C / r reported tokens, where r is the counted to reported ratio. For the summary request to have about 800,000 reported tokens, each earlier RunStep must stay below the threshold, so each RunStep can add at most (812,646 - P) / r reported tokens. The gap to the threshold thus falls by the factor (1 - 1/r) at each RunStep.
- English results (r = 3.9): the factor is 0.744. From P = 35,000 (gap 777,646) to a gap of about 25,000 needs ln(25,000 / 777,646) / ln(0.744) = 11.6, thus 12 RunSteps. The input of these requests is the sum of 812,646 - 777,646 x 0.744^j for j = 0 to 11: 12 x 812,646 - 777,646 x (1 - 0.744^12) / (1 - 0.744) = 9,751,752 - 2,955,000 = about 6,800,000 tokens. With the summary request and the later requests, about 7,700,000 input tokens and about 105 Exa exchanges (about 752,000 reported tokens at 7,200 for each result).
- Chinese results (r = about 1.45, the ratio of generated Chinese prose; the ratio of real Chinese web pages is not measured): the factor is 0.31. One Chinese result of about 28,000 counted tokens is about 19,300 reported tokens.
  - Each RunStep j sends a request with the input P_j and asks for n_j results. The check at RunStep j+1 is P_j + 28,000 x n_j < 812,646.
  - RunStep 0: P = 35,000, 27 results (check 35,000 + 756,000 = 791,000). RunStep 1: P = 556,100, 9 results (check 808,100). RunStep 2: P = 729,800, 2 results (check 785,800). RunStep 3: P = 768,400, 1 result (check 796,400). RunStep 4: P = 787,700, 1 result. RunStep 5: the check is 787,700 + 28,000 = 815,700, so the compaction starts, and the summary request has about 807,000 reported input tokens.
  - Requests and their input: RunSteps 0 to 4 35,000 + 556,100 + 729,800 + 768,400 + 787,700, the summary request about 807,000, the later request about 60,000 (summary, stubs, one exact result), and the closing request after the read about 80,000. Total about 3,800,000 input tokens in 8 model requests and 40 Exa exchanges.
  - Risk: a calibration Run must measure r for real Chinese search results first. A lower r makes the first RunStep cross the threshold early, and a gate before each RunStep is necessary.

## Author view

After Run 2, the panel showed the two author messages, the two answers, and a source list of 357 lines, each "<title>（获取于 …）". The database has 30 Source Snapshots of Run 1 and 357 of Run 2. After Run 1, the panel showed the 30 lines of Run 1. After Run 2, the page had only the 357 lines of Run 2, and the 30 lines of Run 1 were not on the page. This record does not examine the cause. A text search of the page found no "Conversation Summary", no "Summary of the earlier conversation", no "Earlier author messages", no "stub", no "compaction", no "摘要", and no "存根". The one match of "Summary" and "summary" is in two source titles ("Summary of lights, buoys, and beacons in Chinese waters", "A summary of researches in sound"), and the one match of "压缩" is the Project title. The editor showed the Chapter with its one paragraph. No summary and no compaction fact are visible to the author.

Final answer of Run 2 (short): it confirmed the 36 searches and the read of A1, gave the fog signal numbers of A1 (steam pressure 50–100 psi, siren 72 psi, Henry's relative power 9 : 7 : 4, a range of 20–30 miles in good conditions, Tyndall's advice of 2–3 miles), gave the facts that the 鹰嘴岛 scene can use, and cited 7 URLs.

Languages of this Conversation:

- The two author messages are in Chinese. Only the search queries in them are in English, because this Run needed English results for the calibration.
- The Chapter text is in Chinese: one paragraph of 28 characters, "雾从海面升起来的时候，林三爬上钟楼，抓住绳子，开始敲钟。"
- The Tool results are in English.
- The summary (M6) is in English, with the Chinese names (finding F3).
- The final answer of Run 1 (M3, before the compaction) and the final answer of Run 2 (M8, after the compaction) are in Chinese, as the author messages asked. The English summary did not change the language of the final answer.

## Observations

- Citations after the compaction: of the 7 cited URLs, 2 are source lines that the A1 stub kept (and the model read A1 again), and 5 are in exact group B results. The model cited no source that a stub left out without a read.
- The summary kept the three new topics (above).
- The quality of the answer after the compaction is good. The numbers agree with the read A1 result and the group B results, and the answer keeps the story facts of Run 1 (three strikes per minute is the rule of the author) from the summary and the section.
- In both Runs the model put `read_tool_result` and `finish` in one response, as the author message asked. Thus a read after a compaction cost one request and no more.

## Open item of #1451

The open item "no live request covered the request form after a compaction at RunStep 1 or later" is closed. The compaction occurred at RunStep 2 of Run 2. The route accepted the later request M7 (a new transport with `tool_choice: "required"`, the 36 function calls, 5 stub outputs, 31 exact outputs, and the section) and the incremental request M8 of RunStep 3 after it.

## Findings

The IDs are the IDs of the findings message to the coordinator.

| ID | Finding | Effect | Owner |
| --- | --- | --- | --- |
| F1 | The expected input counts new Tool outputs in Unicode scalars (ADR 0052), and the reported usage counts Provider tokens. For English Exa results, one reported token is 3.9 counted tokens. The compaction started at an expected input of 959,628 when the reported context was 299,550 tokens (29 percent of the window). For Chinese text the ratio is about 1.45 ([pinned version record](agent-plan-pinned-version-qualification-2026-10-14.md)). The check `later_request_too_long` uses the same counted measure. The later request was 913,509 counted tokens, 102,299 below the bound of 1,015,808. | In a Run with many English Tool results, the threshold of 812,646 starts a compaction at about 300,000 reported tokens. A reported input near 812,646 is not reachable through Tool results without a much larger cost. The same measure can give a false `conversation_too_long`. This is the measurement for the threshold decision of the author. | [#1520](https://github.com/FrankQDWang/StoryOS/issues/1520). The author decides the threshold. |
| F2 | The later request keeps the outputs of the response that the summary continued in their exact form (ADR 0093 section 5). Here they were 31 results and about 94 percent of the reported input of the later request. The part that could become stubs went from 143,543 to 9,535 counted tokens (93.4 percent less). | The compaction reduced the reported input by 19.2 percent only. This is a property of this Run, which put 31 results into the last response, and not the reduction of an ordinary Run. | No issue. Property of this Run. |
| F3 | The summary is in English, although the instruction says "Write in the language of the manuscript" and the manuscript is Chinese. The author messages and the Chapter text are in Chinese, and the Tool results and the instruction are in English. | The summary text costs more tokens than a summary in Chinese would. The final answer after the compaction (M8) stayed in Chinese, as the author message asked. No loss of content was found. | No issue. |
| F4 | The stubs kept 7 of 10 source lines (A1), 6 of 10 (A2 to A4), and 7 of 9 (A5). The final answer cited 7 sources: 2 from the kept lines of A1, which the model also read again, and 5 from exact group B results. | The model cited no source that a stub left out without a read. | Observation. |
| F5 | The public `usage` of `getAgentRun` and of each compaction record shows only `{"kind": "reported"}`, with no token numbers. The numbers are only on the Model Attempt rows. | An operator cannot read the measured sizes through the public read. | Observation. |
| F6 | The panel lists each Source Snapshot of a research Run: 30 lines for Run 1 and 357 lines for Run 2. After Run 2, the page had only the 357 lines of Run 2. | A long source list for the author after many searches. | Observation. |
| F7 | The summary request M6 took 2 min 46 s, with 299,550 input and 6,473 output tokens. | Below the request timeout of 600 s. | Observation. |
