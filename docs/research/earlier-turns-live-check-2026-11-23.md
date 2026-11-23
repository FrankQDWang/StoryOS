# Live check of the earlier turns of a Project Conversation (2026-11-23)

This record is the live check of [Send the Earlier Turns of a Project Conversation From the Host Records When a Request Has No Prior Response](https://github.com/FrankQDWang/StoryOS/issues/1826) and [ADR 0102](../adr/0102-send-the-earlier-turns-of-a-project-conversation-from-the-host-records-when-a-request-has-no-prior-response.md). One run on the real Agent Plan route shows, from the wire payloads, which request of a Conversation sends the section of the earlier turns. It is one check of the new request form, not a trial that compares texts.

Raw evidence is on the local tag `evidence/1826-earlier-turns-live-2026-11-23` (commit `63dc42994cfc87514486c64d2fc0f39694772e0e`). The tag holds the plan, the run script `earlier-live.ts`, `run.sh`, `rehearse.sh`, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M6.json`), the Model Attempts, the ToolCalls, the Run rows, the earlier turns of each Operation Requirement, the guard log, `loader.log`, `measures.json`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. Before the tag, each of its 20 files (282,158 bytes) was examined for a key, an e-mail address, and a home path, and none was found. The coordinator examines the tag again and pushes it.

## Result

- **Run 2 sends the earlier turn.** Its first request (M3) has no `previous_response_id`, and it has the section of the earlier turns directly before `Author request`, with the exact author message and the exact final answer of Run 1.
- **Run 3 continues the prior response.** Its first request (M5) has `previous_response_id` and no section.
- **Run 2 repeats the sentence.** The answer of Run 2 is the first sentence of the answer of Run 1, character for character. The answer of Run 1 is one sentence.
- Each request is in the plan. No stop occurred, and no Run repeated.

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1826#issuecomment-7090561266)).

Inputs:

- Product code and release package: branch `claude/1826-earlier-turns-39b962` at `9d3e82ea6b128e4e0a527a50c3dee0afc3bd6424` (tree `4442314fa3bf2f15b639d24958fe80db83f993db`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:7df2bfce7892971066ca094699bb6fb4cbf48ae61679fc71ff4cd70f74173b3c`). Registration `018f0000-0000-7001-8000-00000000fa26`, mapping `storyos.volcengine-agent-plan.responses.create.v12`.
- Topology: the topology of #1792 and #1818. `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package of this worktree and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The Worker resolved the key by its typed settings (ADR 0050), which find the settings file of the main checkout from a worktree. The run never read an env file or a key.
- Sample: Chapter 1 of `fixtures/memory-calibration/` (`第一章 归`), in a new Project with one Volume. The script wrote the Chapter through `applyAuthorEdit`.
- Script: `earlier-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). The only SQL write was the guard schema.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 3 Runs, 6 requests, each `completed`.

Steps, all in one Project Conversation on Chapter 1:

1. Run 1, in a new Conversation: "第一章里，沈知遥为什么回到青溪镇？用一两句话回答。"
2. Run 2: "Submit the complete current request. 请一字不差地重复你上一个回答的第一句话，只输出那一句。" The prefix gives the mapping `Full`.
3. Run 3: "她回到青溪站那天的天气怎么样？用一句话回答。"
4. Stop the Worker, then write the measures and the raw evidence.

The quiet time of a Conversation capture is 30 minutes and of a Chapter capture 1 hour, and the script stopped the Worker after Run 3. Thus no Memory work started, and no request of the Memory work was planned.

| Item | Expected | Stop at | Actual |
| --- | --- | --- | --- |
| AgentRuns | 3 of the author | 3 (script), 5 (guard) | 3 |
| Model requests of one Run | 2 or 3 | 6 | 2, 2, 2 |
| Model requests in all, with the Memory work (0) | about 7 | 20 | 6 |
| Reported input in all | about 80,000 | — | 87,460 |
| Reported input of one request | 10,000 to 25,000 | 60,000 | 11,849 to 19,860 |
| Exa and embedding requests | 0 | 1 | 0 |

## Runs

Time is UTC on 2026-11-23, from 03:13:16 (the Project) to 03:13:51 (the measures). Each Run is `completed`, pinned the Registration `fa26`, and has the mapping `v12` on each Model Attempt. Input and output are the tokens that the Provider reported.

| Request | Run | RunStep | Started | `previous_response_id` | Section of the earlier turns | Input | Output | Decision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 1 | 0 | 03:13:16 | no | no | 11,849 | 120 | `finish` call |
| M2 | 1 | 1 | — | yes | no | 12,037 | 32 | answer |
| M3 | 2 | 0 | 03:13:28 | no | yes | 11,951 | 218 | `finish` call |
| M4 | 2 | 1 | — | no | yes | 12,040 | 147 | answer |
| M5 | 3 | 0 | 03:13:41 | yes | no | 19,723 | 68 | `finish` call |
| M6 | 3 | 1 | — | yes | no | 19,860 | 26 | answer |

The first request of each Run, as the criterion asks:

- **Run 1 (M1):** no `previous_response_id` and no section. The Conversation has no earlier Run.
- **Run 2 (M3):** no `previous_response_id`, and the section. The mapping `Full` sends the full input at each RunStep, so M4 also has no `previous_response_id` and has the same section (ADR 0102 section 3: "at each RunStep position").
- **Run 3 (M5):** `previous_response_id` and no section.

The section of M3, from the user message of `wire/M3.json`. The line `Author request:` follows it directly:

```text
Earlier turns of this conversation, oldest first. Each turn has the exact words of the author and your final answer. The Tool calls and the Tool results of those turns are not here:
[turn 1]
Author:
第一章里，沈知遥为什么回到青溪镇？用一两句话回答。

Assistant:
因为爷爷去世了，父亲打电话通知她，她从上海赶回青溪镇参加葬礼，并处理爷爷留下的后事。
```

The author message is the exact message of Run 1, and the answer is the exact final answer of Run 1 (`model_attempts.payload #>> '{decision,text}'` of M2). The Operation Requirement of each RunStep of Run 2 records two sources: the Run 1 author message and the Model Attempt of M2. Each RunStep of Run 3 records four: the two of Run 1 and the two of Run 2 (`earlier-turns.json`). The section has no line `Older turns are left out.`, because both turns fit the limit.

## Answers

| Run | Answer |
| --- | --- |
| 1 | 因为爷爷去世了，父亲打电话通知她，她从上海赶回青溪镇参加葬礼，并处理爷爷留下的后事。 |
| 2 | 因为爷爷去世了，父亲打电话通知她，她从上海赶回青溪镇参加葬礼，并处理爷爷留下的后事。 |
| 3 | 她到青溪站那天正下着大雨，雨水顺着屋檐挂成一道帘子。 |

The answer of Run 2 repeats the first sentence of the answer of Run 1 exactly. Run 2 had no prior response, so the model got that sentence only from the section of the earlier turns. The answer of Run 3 agrees with Chapter 1.

## Queries

`earlier-live.ts` on the tag has each query in its functions `checkLimits`, `firstRequests`, and `rawEvidence`. Each one is read-only:

- Runs: `agent_runs` with `model_registration_revision`, the `execution_profile.mapping_revision` of its Model Attempts, and the count and the reported usage of its Model Attempts.
- The first request of each Run: the `wire.serialized_payload` of its first Model Attempt, the key `previous_response_id`, and the text of the user message from the heading of the section to `Author request:`.
- The earlier turns: `payload #> '{operation_requirement,earlier_turns}'` of each decision `context_assembly_manifests` row.
- Answers: `model_attempts.payload #>> '{decision,text}'` of the last decision of each Run.
