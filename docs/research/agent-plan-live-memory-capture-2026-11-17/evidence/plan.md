## Live plan (written before the first request)

- Head: `claude/s7-d3f-capture-live` at `dd315129` (tree `7092d423`), on `stage7-memory` `749a0bc9`. Release package of that clean commit, manifest `sha256:632c4683d5d9e0778846093ba19523c0de45f8d708943f5978cb17535a746a41`.
- Model Registration: head `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, model `doubao-seed-2-1-pro-260915`, the request bounds of the pinned Model Use Binding.
- Topology (ADR 0049, ADR 0050): packaged `storyos-server` with `STORYOS_WORKER=0` and one continuous packaged `storyos-worker`, each with `env -i` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. No `STORYOS_EMBEDDING_DESTINATION`, no `STORYOS_EXA_API_KEY` in the process environment, and the Project gets no research binding. The key reaches the Worker only through its typed settings. The Worker log filter `info,storyos_application::passage_index_work=warn` keeps the idle index polls out of the log.
- Database: a new `scripts/dev-postgres.sh up --interactive` database. Direct writes: the guard schema `live_guard`, and the one SQL statement below. Each other row comes from the panel, the editor, and the Worker.
- Request guard: the `live_guard` triggers of the record of 2026-11-14, counted at the claim: model 14, Exa 0, embedding 0. The guard is removed with the database after the run.
- Rehearsal: done on `host_fake` with the same steps (2 Runs, the SQL statement, a capture with `no_observation`, 5 fake requests, no error line).
- Author surface: the packaged Web client in the built-in browser, with `localStorage` and `sessionStorage` clear. New Project, Volume 第一卷, Chapter 渡口 with one paragraph typed in the editor. One Project Conversation:
  - Run 1 (a writing habit): "我写人物对白有个固定习惯：对白里从不用感叹号，人物的情绪全靠动作和停顿来表现。你先看看这一段的对白读起来怎么样，不要修改正文。"
  - Run 2 (an intent that the manuscript does not have): "说一下我对后面的打算：林雪其实早就知道父亲落水的真相，但她会一直瞒着林舟，直到最后一卷才说出来。正文里现在还没写到这些。你觉得这个伏笔先在哪里露一点苗头比较好？不要修改正文。"
- The one SQL statement, after the two Runs complete: `UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'conversation_capture' AND conversation_id = '<the Conversation>'`. The Worker then runs the capture. No schedule starts a consolidation pass on this tip, so the capture is the last request. The Worker stops after the capture settles.

| Step | Requests | Expected reported input of each request |
| --- | --- | --- |
| Run 1 | 3 (decision, `finish`, closing) | 3,400, 3,500, 3,700 |
| Run 2 (continues the Conversation) | 3 | 4,300, 4,500, 4,700 |
| Capture | 2 to 4 (a read, then `record_memory_observations`) | 2,600, 2,900, 3,200, 3,500 |
| Total | 10 planned, at most 14 | about 36,000, at most about 55,000 (limit 200,000) |

- One repeat of an author message with clearer words is in the count. Stop conditions: a refusal of the route, a key or a home path in a payload or a log, a count above a maximum, or a reported input of one request above 60,000 tokens.
