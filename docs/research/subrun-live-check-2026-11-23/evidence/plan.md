## Plan of the live check (item 4)

This is the plan of the one live run of item 4. I post it before the first request.

Inputs:

- Product code and release package: branch `claude/1836-subrun-reads` at `446ba984fc45075bdfb9e73b468f45e6c9a5f6b1` (tree `76f93b7effeb52fe31b6b5e6ed4bf5a6e81c9f0b`), `python3 scripts/package-release.py` on the clean tree. Registration `018f0000-0000-7001-8000-00000000fa30`, mapping `storyos.volcengine-agent-plan.responses.create.v16`, Host Tool Registration `018f0000-0000-7001-8000-0000000e0107`.
- Topology: the topology of the live check of S9-01 (#435). `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The run reads no env file and no key. No browser is used.
- Sample: the 10 Chapters of `fixtures/memory-calibration/` (`chapter-01.md` to `chapter-10.md`), in one new Project with one Volume. The script writes each Chapter through `applyAuthorEdit`. Chapter 1 is the Current Chapter of each Run.
- Script: `subrun-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). The script writes no product row directly. The request guard of the schema `live_guard` counts model requests and AgentRuns with triggers and changes no product row.
- Release package manifest `sha256:8776b01a71d71fa453b3dae66783a24d31960647582b3acc57417fcda771771a`.
- Two rehearsals of all steps on `host_fake` with the same package and a separate database passed. The first: 3 Runs, 6 requests, each `completed`, and no Subrun. In the second, Run C sends the Subrun script of the fake model (`SCRIPT:start_subrun`, only when `STORYOS_REHEARSAL_SUBRUN=1`): 3 Runs and 1 Subrun, 10 requests, each `completed`. The run read gave the Subrun `working`, then `completed`, and `getSubrun` gave the title, the brief, one `chapter_read` entry, 3 RunSteps, and the answer. The live run does not set that variable.

Steps:

1. Write the 10 Chapters in order (at most 8 commands in one minute).
2. Run A, in a new Conversation: "沈知遥在第一章到第十章里每次提到自己的家人时是怎么说的？请把出处列出来，并指出前后不一致的地方。"
3. Run B, in a new Conversation: "第一章的第一句话是什么？"
4. Run C, in a new Conversation: "请交给一个子任务：从第一章到第十章逐章通读，每一步只读一章，然后列出每一章里出现的地名。"
5. Stop the Worker. Then read `getSubrun` for each Subrun, and write the measures and the raw evidence.

While each Run works, the script reads `getAgentRun` each 3 seconds and keeps each new value of `subruns`. For each Subrun with the state `working`, it also reads `getSubrun` and keeps each new record. The quiet time of a Conversation capture is 30 minutes and of a Chapter capture 1 hour, and the script stops the Worker after Run C. Thus the plan has no request of the Memory work.

| Item | Expected | Stop at |
| --- | --- | --- |
| AgentRuns of the author | 3 | 3 (script and guard) |
| AgentRuns of the Memory work | 0 | 1 (script and guard) |
| Subruns of one Run | Run A 0 or 1, Run B 0, Run C 1 | more than 4 (script), more than 12 in all (guard) |
| RunSteps of one Subrun | Run C 10 to 12 | more than 12 (script), more than 14 requests of one Subrun (guard) |
| Model requests of the parent Run A | 3 to 15 | 30 (script and guard) |
| Model requests of the parent Run B | 1 or 2 | 4 (script) |
| Model requests of the parent Run C | 2 to 4 | 8 (script) |
| Model requests in all, with the Memory work (0) | 20 to 50 | 80 (script and guard) |
| Reported input in all | 300,000 to 1,500,000 tokens | 4,000,000 |
| Reported input of one request | 5,000 to 80,000 tokens | 150,000 |
| Exa and embedding requests | 0 | 1 (script and guard) |
| Time of Run A and of Run C | — | 40 minutes each |
| Time of Run B | — | 10 minutes |

The run starts one time. A stop ends the run, and I report it. When a Run does not give the expected effect (Run B starts a Subrun, or the Subrun of Run C gets no sentence near the limit or gives no final answer by its last RunStep), the record says so, I send the findings to the coordinator, and I change no text and start no second run.

The raw evidence goes on the local tag `evidence/436-subrun-live-<date of the run>`, and the record goes to `docs/research/subrun-live-check-<date of the run>.md`.
