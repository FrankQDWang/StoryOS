# Plan of the live check of the earlier turns (#1826)

## Purpose

One run on the real Agent Plan route shows if a decision request with a full input and no prior response sends the earlier turns of the Project Conversation from the Host records (ADR 0102), and if a request that continues a prior response does not send them. It is one check of the new request form, not a trial that compares texts. It does not repeat.

## Inputs

- Product code and release package: branch `claude/1826-earlier-turns-39b962` at `9d3e82ea6b128e4e0a527a50c3dee0afc3bd6424` (tree `4442314fa3bf2f15b639d24958fe80db83f993db`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:7df2bfce7892971066ca094699bb6fb4cbf48ae61679fc71ff4cd70f74173b3c`). Registration `018f0000-0000-7001-8000-00000000fa26`, mapping `storyos.volcengine-agent-plan.responses.create.v12`.
- Topology: the topology of #1792 and #1818. `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The Worker resolves the key by its typed settings (ADR 0050). The run never reads an env file or a key.
- Sample: Chapter 1 of `fixtures/memory-calibration/`, in a new Project with one Volume. The script writes the Chapter through `applyAuthorEdit`.
- Script: `earlier-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). The only SQL write is the guard schema. The script makes no other SQL write: no due time of the Memory work, no Chapter, and no Memory Entry.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 3 Runs, 6 requests, each `completed`. Run 2 and Run 3 recorded the earlier turns of the Runs before them.

## Steps

All three Runs are in one Project Conversation, on Chapter 1.

1. Run 1, in a new Conversation: "第一章里，沈知遥为什么回到青溪镇？用一两句话回答。" Its first request has no prior response and no earlier turn: the Conversation has no earlier Run.
2. Run 2, in the same Conversation: "Submit the complete current request. 请一字不差地重复你上一个回答的第一句话，只输出那一句。" The prefix gives the mapping `Full`. Its first request must have no `previous_response_id`, and it must have the section of the earlier turns with the exact author message and the exact final answer of Run 1.
3. Run 3, in the same Conversation: "她回到青溪站那天的天气怎么样？用一句话回答。" Its first request must have `previous_response_id` and no section.
4. Stop the Worker. Write the measures and the raw evidence.

The Memory work does not start: the quiet time of a Conversation capture is 30 minutes, and of a Chapter capture 1 hour. The script stops the Worker after Run 3, before these times. No character entry exists, so no Intent Pass starts.

## Numbers

| Item | Expected | Stop at (the request guard and the script) |
| --- | --- | --- |
| AgentRuns | 3 of the author, no Memory work | 3 for the script, 5 for the guard |
| Model requests of one Run | 2 or 3 (a `finish` call and the closing request, and one Tool call when the model reads the Chapter) | 6 |
| Model requests in all, with the automatic requests of the Memory work (0) | about 7 | 20 |
| Reported input in all | about 80,000 tokens | — |
| Reported input of one request | about 10,000 to 25,000 | 60,000 |
| Exa and embedding requests | 0 | 1 (the guard refuses the first one) |

The script also stops at a Run that is `refused` or `cancelled` and at a Run that is not a Run of the author. Each planned Run counts one time; no Run repeats. Time: about 10 minutes.

## What the record states

- For each of the three Runs, from the wire payloads: if its first request has `previous_response_id`, and if it has the section of the earlier turns.
- The section of Run 2 with the exact author message and the exact final answer of Run 1.
- The answer of Run 2, and if it repeats the first sentence of the answer of Run 1.
- The raw evidence is on a local tag `evidence/1826-earlier-turns-live-2026-11-23`. The coordinator examines it and pushes it.
