## Plan of the live check (item 10)

This is the plan of the one live run of item 10. I post it before the first request.

Inputs:

- Product code and release package: branch `claude/435-plan-list-8dcb4b` at `8cdd59dfef304c9d47d4d290ca11bf4114f295aa` (tree `8fc2257a2259e1a8427239f1199af97c729c8ff2`), `python3 scripts/package-release.py` on the clean tree. Registration `018f0000-0000-7001-8000-00000000fa27`, mapping `storyos.volcengine-agent-plan.responses.create.v13`, Host Tool Registration `018f0000-0000-7001-8000-0000000e0106`.
- Topology: the topology of #1792, #1818, and #1826. `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The run reads no env file and no key. No browser is used.
- Sample: the 10 Chapters of `fixtures/memory-calibration/` (`chapter-01.md` to `chapter-10.md`), in one new Project with one Volume. The script writes each Chapter through `applyAuthorEdit`.
- Script: `plan-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). The script writes no product row directly. The request guard of the schema `live_guard` counts model requests and AgentRuns with triggers and changes no product row.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 3 Runs, 6 requests, each `completed`, and each Run read with no plan.

Steps:

1. Write the 10 Chapters in order (at most 8 commands in one minute).
2. Run A, in a new Conversation, with Chapter 3 as the Current Chapter: "请帮我检查第一章到第五章：找出每一处沈知遥的年龄、或者两件事之间相隔的时间前后对不上的地方，把这些地方一一列出来；然后把第三章的第一段改得读起来更顺一些。"
3. Run B, in a new Conversation, with Chapter 1 as the Current Chapter: "第一章的第一句话是什么？"
4. Run C, in a new Conversation, with Chapter 2 as the Current Chapter: "请用三句话概括第二章。"
5. Stop the Worker, then write the measures and the raw evidence.

While each Run works, the script reads `getAgentRun` each 3 seconds and keeps each new `plan`. The quiet time of a Conversation capture is 30 minutes and of a Chapter capture 1 hour, and the script stops the Worker after Run C. Thus the plan has no request of the Memory work.

| Item | Expected | Stop at |
| --- | --- | --- |
| AgentRuns | 3 of the author, 0 of the Memory work | 3 (script and guard), 1 of another kind (script) |
| Model requests of Run A | 5 to 15 | 30 |
| Model requests of Run B | 1 or 2 | 4 |
| Model requests of Run C | 1 to 3 | 6 |
| Model requests in all, with the Memory work (0) | 7 to 20 | 40 |
| Reported input in all | 200,000 to 800,000 tokens | 2,000,000 |
| Reported input of one request | 10,000 to 80,000 tokens | 150,000 |
| Exa and embedding requests | 0 | 1 |
| Time of Run A | — | 40 minutes |
| Time of Run B and of Run C | — | 10 minutes each |

The run starts one time. A stop ends the run, and I report it. When Run A records no plan, or Run B records a plan, the record says so, I report it to the coordinator, and I change no text.

The raw evidence goes on the local tag `evidence/435-plan-list-live-2026-11-23`, and the record goes to `docs/research/plan-list-live-check-2026-11-23.md`.
