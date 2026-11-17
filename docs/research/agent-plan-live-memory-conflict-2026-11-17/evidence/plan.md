## Plan of the live run

**Head.**
- Product code: `stage7-memory` `a9fe479cc5323d5ddd3cea8328c2c34bff71bf1d` (tree `ece5d0e5`), with S7-D6, S7-D5, S7-D4c, and fix #1671.
- Release package: `python3 scripts/package-release.py` on that clean commit, manifest `sha256:c8e3b67ea6436c2f2cabe0fc340523f57ed733365a7c8779a565ea556071727a`.
- The branch `claude/s7-d6b-live-conflict` at `9aabf4e4` adds only the store test, so the product code of the run is the same.

**Topology, settings, and guard.** The same as the [record of S7-D4c](https://github.com/FrankQDWang/StoryOS/blob/stage7-memory/docs/research/agent-plan-live-memory-consolidation-2026-11-17.md):
- Processes: the packaged `storyos-server` with `STORYOS_WORKER=0`, and one continuous packaged `storyos-worker`. Both use `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`.
- Process environment: each process has only the named variables, `PATH`, and `HOME`. There is no embedding destination and no Exa key.
- Key: the key reaches the request only through the typed settings (ADR 0050). No agent reads, copies, or prints an env file or a key.
- Database: a new `scripts/dev-postgres.sh up --interactive` database. The author surface is the packaged Web client in the built-in browser, with clear browser storage.
- Request guard: the `live_guard` triggers, counted at the claim: model 28, Exa 0, embedding 0.
- Rehearsal: the same steps on `host_fake` first, on the same package.

**Steps.** The content goes through the panel and the editor. The Project has Volume 第一卷 and Chapter 渡口, which is empty until step 2.

| Step | Work | Requests planned (at most) | Reported input of each request |
| --- | --- | --- | --- |
| 1 | Conversation A: "请记住：林舟不会游泳，他从小怕水。" Expected: one `write_memory` create of a `character` entry with the origin `author_request`. | 3 (4) | 3,500–7,000 |
| 2 | The author types one paragraph in Chapter 渡口 in which 林舟 swims across the river. SQL statement 1 makes the Chapter row due. The Worker runs the Chapter capture. Expected: a conflict item for the entry, with or without a fact. | 2–3 (8) | 3,500–8,000 |
| 3 | The pass. The settlement of the capture makes the pass row due at once (a conflict counts as a waiting observation). SQL statement 2 runs only when the row is not due. | 1 (1) | 2,000–5,000 |
| 4 | Conversation B (new): "帮我写下一段：林舟在渡口看见一个孩子落水。" Expected: the Index line has `"conflict_flag": true`. The model reads the entry, tells the conflict, asks which is correct, and calls no `write_memory`. | 3 (6) | 5,000–10,000 |
| 5 | Conversation B: "以第一章为准，他会游泳。请更新这条记忆。" Expected: one `write_memory` change. The flag is removed. | 3 (6) | 6,000–12,000 |

- **Totals:**
  - About 12–13 requests are expected. The plan has 19 (the ticket value), and the guard stops at 28.
  - About 70,000 reported input tokens are expected, at most 450,000.
  - A reported input of one request above 60,000 tokens stops the run.
- **The two SQL statements are the only direct writes of product data:**
  1. `UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'chapter_capture' AND chapter_id = '<chapter>' AND NOT held`. It must change exactly one row. A `held` row, zero rows, or an error stops the run as a finding.
  2. `UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'consolidation' AND project_id = '<project>'`. It runs only when that row is not due.
  
  The guard schema `live_guard` is the only other write. Each other check is a read.
- **A Conversation capture:** the row of Conversation A is due 30 minutes after the Run of step 1, and the row of Conversation B is due 30 minutes after step 5. The run prevents it by time:
  - Step 5 ends within 25 minutes of the completion of step 1. The Worker stops at once after step 5 settles.
  - If step 4 has not started 20 minutes after step 1, the Worker stops, and I report to the coordinator before a new start.
  - The record shows that no Conversation capture started.
- **Probable shape:** the capture gives only a conflict item. Then the pass request has the entries item and no section `Memory observations` (fix #1671). The record shows that request and the answer of the model as a finding. The run does not force this shape.
- **Stops:** each anomaly stops the run and goes to the coordinator, for example a refused request, a `held` row, a background change of the entry text, or a guard trip.
- **Evidence:**
  - Tag `evidence/1669-live-memory-conflict-<UTC date>`. It holds the wire payloads, the Model Attempts, the ToolCalls, this plan, the guard and its log, the SQL statements, and the Server and Worker logs.
  - It holds no database dump and no Project Export Archive.
  - Before each push, each file is scanned for a key, an e-mail address, and a home path.
  - Record: `docs/research/agent-plan-live-memory-conflict-<UTC date>.md`, in the form of the record of S7-D3f.
