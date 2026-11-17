## Live run plan, second run (S7-C3c group 2)

Written before the first request of the second run. The coordinator decided on 2026-11-17 that the second run uses a sample that a Run does not read whole (choice B). The first run stays in the record as a finding: for 3 Chapters of about 110 characters, the model read the Chapters in both Runs.

- Product code: the same as the first run. `stage7-memory` at `9252769e540dd9779991e109b11624d17af2cdf0` (tree `455c1222`), branch `claude/s7-c3c-live-hybrid-search`. The release package is the same build, web manifest `sha256:77fd85b17f826c1d5c94b3690a84b5b98202c9295d5b2605925c2d4eb9deffd8`. The only commit on the branch after that head changes four comment sentences, and the package does not include it. No product text changes before this run.
- Registrations: the same. Model Registration `018f0000-0000-7001-8000-00000000fa22` (mapping v8), Embedding Registration `018f0000-0000-7001-8000-0000000eb002`. Agent Plan route through the admitted path (ADR 0049, ADR 0050). The key reaches the Worker only through the typed settings.
- Topology: the same as the first run. One `storyos-server` with `STORYOS_WORKER=0`, and one continuous `storyos-worker`, each with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` and `STORYOS_EMBEDDING_DESTINATION=volcengine_agent_plan`, and a process environment that has only the named variables, `PATH`, and `HOME`. A new `scripts/dev-postgres.sh up --interactive` database (the database of the first run is removed).
- Sample: a new Project with the Volume 第一卷 and 20 Chapters, 58,371 characters in total. A script with a sentence bank and the fixed seed 1600 makes the filler; one target paragraph is written by hand. The text is not a work of the author and not a copy of a published text. Each Chapter has 5 to 7 paragraphs of 381 to 650 characters, so two paragraphs are always above the passage limit of 800, and each paragraph is one passage.
- Passages: `cut_passages` (`storyos.passage-cut.v1`) gives **123 passages** (counted with the same rule before the run).
- The passage to find: Chapter 12 旧债, paragraph 4 of 6 (381 characters). 秦婆婆 burns bundles of reed with pine resin on the stone pier through a night of a white haze, and the seven boats find the harbour by that red. No Chapter title names the event. The target has no two-character word of either question, and the filler has none of 灯, 光, 亮, 照, 火, 雾, 夜, 红, 回来, or 方向, so a keyword search for a word of the question or for a light word finds no Block of the target.
- Steps:
  1. Start the Server only (`STORYOS_WORKER=0`).
  2. In the panel of the packaged Web client, create the Project, the Volume, and the 20 Chapters, and type the text (the editor sends `createChapter` and `applyAuthorEdit`). No mark exists yet, because the Project has no binding.
  3. In the panel, admit Run R0 ("准备检索。") in a new Conversation of Chapter 1 春网, and cancel it before any Worker runs. R0 makes the binding and the `Live` marks of the 20 Chapters, and it makes no model request.
  4. Start the continuous Worker. Poll the count of current passage vectors every 10 seconds through a read connection, at most 20 minutes. No author message goes to the model before the index is complete. When the index is not complete after 20 minutes, the Worker stops and the coordinator decides.
  5. In a new Conversation of Chapter 1 春网, send the first wording: "书里有没有谁在大家看不清方向时，为还没回来的人照亮归途？在哪一章？"
  6. Only when Run 1 has no `hybrid_search_manuscript` call: one time, in a new Conversation of 春网, send the second wording: "请按意思在全书中找一找：有没有一段写某个人在视线模糊的时刻，替尚未返航的人指明方向？告诉我它在哪一章。" No third message.
  7. Expected: `search_manuscript` (no Block of the target), then `hybrid_search_manuscript` (one question request; the result has the passage of 旧债), maybe `read_chapter` 12, then `finish`, and a final answer that names 旧债.
  8. The Worker stops well before 30 minutes after the first completed Run, so no Conversation capture starts. Then the Server stops, the guard triggers and the schema `live_guard` are removed, and the database is removed.
- Requests (inside the numbers that the coordinator confirmed on 2026-11-17):
  - Embedding: 123 passage requests and 1 question request for each hybrid call. Planned **125** (123 + 2), at most **129** (123 + 6).
  - Model: planned **6**, at most **10** for the two author messages. R0 makes none.
  - Exa: 0.
- Expected reported input of each model request:
  - The first request is about 6,000 tokens: the Host instructions and twelve Tools (about 3,600 in the first run), the Chapter list with 20 Chapters, and the Working text of 春网 (2,862 characters).
  - After `search_manuscript`: about 6,200. After the hybrid search: about 9,000 (up to 8 passages of about 480 characters each). After `read_chapter` 12: about 11,000. Later requests add about 200 each.
  - Total about 55,000 for 6 requests, at most about 110,000 for 10. If the model reads many Chapters in one step, one request can be about 45,000; that is inside the limit of 120,000 for one request.
- Usage of the embedding requests: 123 requests of about 330 input tokens each (about 41,000 in total), and fewer than 100 for each question.
- Request guard: the `live_guard` triggers of the first run, with the caps model 10, Exa 0, and embedding 129. Each trigger counts at the insert of the attempt in the claim transaction, before the I/O, and above the cap it refuses the claim, so nothing goes out.
- Stop and tell the coordinator: no hybrid call after two messages (with what the model called and the reported input of each request), a refusal of the route, a key or a home path in a payload or a log, a count above a maximum, a reported input of one model request above 120,000 tokens, or an index that is not complete after 20 minutes.
- Record: the title, the length, and the SHA-256 of each Chapter (paragraphs joined with a line feed, UTF-8), and the complete text of 旧债. The tag `evidence/1600-live-hybrid-search-2026-11-17` holds the two runs, the complete sample, and the script. No database dump and no Project Export Archive. Before each push and the tag push, each file is scanned for a key, an e-mail address, and a home path.
