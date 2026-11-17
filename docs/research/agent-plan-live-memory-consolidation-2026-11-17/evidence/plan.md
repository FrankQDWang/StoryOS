## Plan of the live run

- **Head:** the head of `claude/s7-d4c-conflict-live` after the merge of the tip that the coordinator names before the run. The record gives the SHA. When S7-D5 (PR #1667) is in that head, a commit of a Chapter also makes a capture row, and the first completed Run of the author releases no `held` row before S7-D5b. The run commits no Chapter. When S7-D5 is not in the head, the record says that the run started on a head without S7-D5.
- **Route:** `doubao-seed-2-1-pro-260915`, Registration `fa22`, mapping `v8`, output bound 32,768 tokens. The packaged Server (`STORYOS_WORKER=0`) and one packaged Worker, each with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` (ADR 0049, ADR 0050). The key reaches a request only through the typed Worker and Server settings.
- **Before the run:** the release package on the clean head, after the `make check-full` of PR #1667 ends. A rehearsal of the same steps on `host_fake`. The browser storage of the origin is clear. The request guard of the record of 2026-11-14 (`live_guard` triggers, counted at the claim): model 26, Exa 0, embedding 0.

### Steps

1. A new Project, one Chapter typed in the editor. Conversation A, Run 1: a writing habit.
2. Conversation A, Run 2: a decision for the story that the manuscript does not have.
3. SQL statement 1: `UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'conversation_capture' AND conversation_id = '<A>'`. The Worker runs the capture.
4. SQL statement 2, only when the row is not due after the capture: `UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'consolidation' AND project_id = '<P>'`. The Worker runs the pass.
5. A new Conversation B, Run 3: a question whose answer is the decision of step 2.

The content goes through the panel and the editor. The run writes no other row directly.

### Requests and expected reported input

The plan count comes from the measured requests of S7-D3f ([record](https://github.com/FrankQDWang/StoryOS/blob/stage7-memory/docs/research/agent-plan-live-memory-capture-2026-11-17.md), M1 to M6): each Run of the author used 2 requests, and the capture used 2.

| Request | Run | Expected input (tokens) |
| --- | --- | --- |
| R1, R2 | Run 1, steps 0 and 1 | 3,400 and 5,200 |
| R3, R4 | Run 2, steps 0 and 1 | 4,300 and 6,300 |
| R5, R6 | capture, steps 0 and 1 | 3,400 and 3,700 |
| R7 | pass, one RunStep | 2,500 |
| R8 to R10 | Run 3, steps 0 to 2 (the Memory Index lists the new entries; one `read_memory_entry` call is possible) | 3,700, 5,300, 6,500 |
| more | one more step of a Run or of the capture | at most 7,000 each |

- **Planned:** about 12 requests, at most 26 (about 3 for each Run of the author, at most 8 for the capture, and 1 for the pass).
- **Expected total:** about 50,000 reported input tokens, at most 400,000.
- **Stop for:** a refusal of the route, a key or a home path in a payload or a log, a count above a maximum, a reported input of one request above 60,000 tokens, or another anomaly. When the route refuses the form of S7-D4c1, this ticket corrects the form with its wire test, and the record gives the refusal and the correction.

### Evidence

- Record: `docs/research/agent-plan-live-memory-consolidation-2026-11-17.md`, in the form of `agent-plan-live-skill-load-2026-11-15.md`.
- Tag: `evidence/1587-live-memory-consolidation-2026-11-17`, with the request payloads, the ToolCalls, the plan, the two SQL statements, the guard log, and the Server and Worker logs. No database dump and no Project Export Archive.
- Before the push and the tag push: a scan of each file for a key, an e-mail address, and a home path.
