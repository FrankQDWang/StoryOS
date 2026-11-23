## Plan of the second live check (F1)

The author answered F1 of the first live check on 2026-11-23 ("加一句并重跑"). This is the plan of the one more run of Run A. I post it before the first request.

Inputs:

- Product code and release package: branch `claude/435-plan-list-8dcb4b` at `6a7a49108de5047bba7ab2e6f187408312396554` (tree `3e26cda939104de7a8fe6233ba5d225cbc6c1194`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:fdabd815d67c4480def6e7b2f0e90f86f2026d1022e66616e59dedb968aa0cc9`). `UPDATE_PLAN_INSTRUCTION` has the new sentence "When you start the next step, call update_plan again in the same response as the first Tool call of that step." directly after "... and a done step is completed." Registration `fa27`, mapping `v13`, and Host Tool Registration `…0e0106` do not change.
- Topology and sample: as in the first run. A new `scripts/dev-postgres.sh with` database, a new Project with the 10 Chapters of `fixtures/memory-calibration/`. No embedding destination, no Exa key, no Skill directory, no browser. The run reads no env file and no key.
- Script: `plan-live-2.ts`, the script of the first run with only Run A. The request guard of the schema `live_guard` changes no product row.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 1 Run, 2 requests, `completed`, and the Run read with no plan.

Steps:

1. Write the 10 Chapters in order (at most 8 commands in one minute).
2. Run A, in a new Conversation, with Chapter 3 as the Current Chapter: "请帮我检查第一章到第五章：找出每一处沈知遥的年龄、或者两件事之间相隔的时间前后对不上的地方，把这些地方一一列出来；然后把第三章的第一段改得读起来更顺一些。"
3. Stop the Worker, then write the measures and the raw evidence.

No Run B, no Run C, and no Memory work.

| Item | Expected | Stop at |
| --- | --- | --- |
| AgentRuns | 1 of the author | 1 (script and guard) |
| Model requests of Run A, and in all | 5 to 15 | 30 |
| Reported input in all | 150,000 to 600,000 tokens | 2,000,000 |
| Reported input of one request | 10,000 to 80,000 tokens | 150,000 |
| Exa and embedding requests | 0 | 1 |
| Time of Run A | — | 40 minutes |

Result rule: when Run A calls `update_plan` between its steps, F1 is closed. When it does not, the record and the pull request body give it as a known limit, and no text changes again. There is no third run. The raw evidence goes on the local tag `evidence/435-plan-list-live-2-2026-11-23`, and the record gets a section for this run.
