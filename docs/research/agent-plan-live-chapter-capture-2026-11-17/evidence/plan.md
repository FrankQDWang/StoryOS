## Live plan (written before the first request)

- Head: `s7-d5b-existing-manuscript` at `da1c01f5` (tree `74a877ae`), on `stage7-memory` `265bab51`. The head has S7-D4c (PR #1670) and fix #1671: a capture that stores observations makes the consolidation row due, and the pass is a model request. The plan counts it. Release package of that clean commit, manifest `sha256:5ffc5f5491c5b656d62daf58c99a0a51c09bdd42953a6a8bc8ac6dd62dad65b7`.
- Model Registration: head `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, model `doubao-seed-2-1-pro-260915`, the request bounds of the pinned Model Use Binding.
- Topology (ADR 0049, ADR 0050): packaged `storyos-server` with `STORYOS_WORKER=0` and one continuous packaged `storyos-worker`, each with `env -i` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. No `STORYOS_EMBEDDING_DESTINATION`, no `STORYOS_EXA_API_KEY` in the process environment, and the Project gets no research binding. The key reaches the Worker only through its typed settings.
- Database: a new `scripts/dev-postgres.sh up --interactive` database. Direct writes: the guard schema `live_guard`, and the one SQL statement below. Each other row comes from the panel, the editor, and the Worker.
- Request guard: the `live_guard` triggers of the record of 2026-11-17 (S7-D3f), counted at the claim: model 12, Exa 0, embedding 0. The guard is removed with the database after the run.
- Rehearsal: the same steps on `host_fake` first.
- Author surface: the packaged Web client in the built-in browser, with `localStorage` and `sessionStorage` clear. New Project, Volume 第一卷, Chapter 第一回 with one paragraph typed in the editor: the first 790 characters of `prototypes/chinese-long-form-baseline/public-domain.txt` with its line breaks removed, from "此開卷第一回也" to "利物濟人之德．". The Chapter is written before the first Run, as an existing manuscript.
- One Run of the author in a new Conversation, so that the Project has its grant: "你先读一下这一回，告诉我里面出现了哪些人物，不要修改正文。"
- The one SQL statement, after the Run completes: `UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'chapter_capture' AND chapter_id = '<the Chapter>'`. The Worker then runs the Chapter capture. When the capture stores observations, the consolidation row is due and the Worker runs one pass. The Worker stops after the pass settles. The schedule row of the Conversation is due 30 minutes after the Run, so the Worker stops before it.

| Step | Requests | Expected reported input of each request |
| --- | --- | --- |
| Run of the author | 2 to 3 (a `read_chapter` call or `finish`, then the closing) | 3,400, 4,600, 5,000 |
| Chapter capture | 2 to 8 (one optional read, then `record_memory_observations`) | 3,500, 4,500, then at most 6,000 each |
| Consolidation pass | 1 | 2,500 |
| Total | 7 planned, at most 12 | about 26,000, at most about 70,000 (limit 150,000) |

- The input of the first capture request is the capture instructions, the line `{"chapter_number":1,"chapter_title":"第一回"}`, and the 790 characters as one Block (`exact_required`, far below the Context item limit).
- One repeat of the author message with clearer words is in the count. Stop conditions: a refusal of the route, a key or a home path in a payload or a log, a count above a maximum, or a reported input of one request above 60,000 tokens.
- Evidence: the record `docs/research/agent-plan-live-chapter-capture-<date>.md` in the form of the record of S7-D3f, and the tag `evidence/1664-live-chapter-capture-<date>` with the request payloads, the ToolCalls, the plan, the guard log, the SQL statement, and the Server and Worker logs. No database dump and no Project Export Archive.
