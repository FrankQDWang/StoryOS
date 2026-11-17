## Live run plan (S7-C3c group 2)

Written before the first request.

- Purpose: show on the real route the complete path of ADR 0095. The Worker makes the vectors of a sample manuscript with no action of the author. Then the model finds, with `hybrid_search_manuscript`, a passage that a question describes with no word of the passage, and the final answer names the Chapter of the passage. The run is also the database proof of the line that gives the embedding configuration to the store of the Server (S7-C2b2).
- Product code: `stage7-memory` at `9252769e540dd9779991e109b11624d17af2cdf0` (tree `455c1222`), with S7-C2b2b (#1647), on the branch `claude/s7-c3c-live-hybrid-search` with no change yet. Release package: `make generate-persistence-catalog` on that clean head, web manifest `sha256:77fd85b17f826c1d5c94b3690a84b5b98202c9295d5b2605925c2d4eb9deffd8`.
- Model: head Model Registration `018f0000-0000-7001-8000-00000000fa22` (`doubao-seed-2-1-pro-260915`, mapping `storyos.volcengine-agent-plan.responses.create.v8`). Embedding Registration `018f0000-0000-7001-8000-0000000eb002` (`doubao-embedding-vision-251215`, mapping `storyos.volcengine-agent-plan.embeddings.multimodal.v1`). Agent Plan route through the admitted path (ADR 0049, ADR 0050). The key reaches the Worker only through the typed settings. No step reads the env file.
- Topology: the ADR 0022 production surface. `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_EMBEDDING_DESTINATION=volcengine_agent_plan`. One continuous `storyos-worker` with the same three settings. Both come from the release package, with a process environment that has only the named variables, `PATH`, and `HOME`.
- Project: a new Project with the Volume 第一卷 and three Chapters. The Chapters and their text come from HTTP commands (`createChapter`, then `applyAuthorEdit`), not from SQL. Each Chapter has two paragraphs and one scene separator block between them (`＊　＊　＊`, no letter and no digit). The record holds the text:
  - 渡口: "雨下了一整夜。林舟站在渡口，等姐姐林雪把渡船从对岸撑回来。河面上的雾很浓，他只能听见竹篙一下一下点进水里的声音。" / "天亮的时候，渡船靠了岸。林雪的袖口湿透了，她把一包还带着热气的米糕塞给林舟，说对岸的集市今天开得早。"
  - 集市: "集市在镇子东头，卖鱼的、卖布的、补锅的挤在一条窄街上。林舟第一次一个人来，攥着姐姐给的三枚铜钱，不知道先买什么。" / "补锅的老周认出了他，问林雪的手伤好些没有。林舟说好多了，老周就从摊子底下摸出一卷新麻绳，让他捎回去。"
  - 潮夜: "入秋以后，河口的潮水一夜比一夜高。镇上的人把小船拖上岸，用石头压住缆绳。" / "阿婆每到起雾的晚上，都会提着那盏旧马灯爬上河口的石台，把灯挂在最高的木桩上。晚归的渔船看见那点昏黄的光，就知道离家只剩半里水路。"
- Passages: `cut_passages` (`storyos.passage-cut.v1`, limit 800 characters) gives **6 passages**: the separator ends each first paragraph, and each paragraph has 36 to 64 characters.
- The passage to find: the second paragraph of 潮夜. The author question is "书里有没有谁在大家看不清方向时，为还没回来的人照亮归途？在哪一章？". It has no two-character word of that paragraph, so `search_manuscript` with a term of the question finds no passage of 潮夜.
- Index work case: the first binding, with no quiet time. In the tip with S7-C2b2b, a mark needs an embedding use binding of the Project, and `createAgentRun` makes that binding. A new binding marks each live Chapter with `MarkedChapters::Live` and a wait of 0. A change after the binding (`MarkedChapters::Changed`) waits `PASSAGE_INDEX_QUIET_TIME` (10 minutes). The run uses the first case, so the Chapters exist before the binding, and nobody moves a stored time with SQL.
- Steps:
  1. Start the Server only (`STORYOS_WORKER=0`). No Worker runs yet.
  2. Through HTTP from the panel of the packaged Web client (the editor sends `createChapter` and `applyAuthorEdit`), create the Project, the three Chapters, and their text. No mark exists yet, because the Project has no binding.
  3. Through the panel, admit Run R0 in a new Conversation of 渡口 with the message "准备检索。", and cancel R0 with its cancel control before any Worker runs. The admission makes the binding and the `Live` marks of the three Chapters. R0 makes no model request.
  4. Start the continuous Worker. It settles the cancel of R0, then claims the index work of the three Chapters at once, with no action of the author. The wait is about one minute for 6 embedding requests: poll the count of current passage vectors every 10 seconds, at most 5 minutes. A longer wait is an anomaly: stop and report it.
  5. In the panel of the packaged Web client, send the author message in a new Conversation of 渡口.
  6. Expected: the model calls `search_manuscript` (no passage of 潮夜), then `hybrid_search_manuscript` (one Embedding Attempt of the question; the result has the passage of 潮夜), and maybe `read_chapter` of 潮夜. Then `finish`, and the final answer names 潮夜.
  7. The panel shows the line `按意思在全书中搜索了「<question>」` in the current turn. The record has a screenshot of the turn.
  8. Stop the Worker and the Server. Remove the guard triggers. The Conversation capture of the completed Run is due 30 minutes after its completion (`CONVERSATION_QUIET_TIME`). The Worker stops before that time, so the capture makes no request.
- Rehearsal: before the live run, the same steps run one time with `host_fake` for the model and the embedding destination. The rehearsal shows that R0 makes no Model Attempt, that the `Live` marks have no wait, and that the Worker makes 6 vectors. It sends no live request.
- Second wording: if the first author message gives no `hybrid_search_manuscript` call, the author message goes one more time, in a new Conversation of 渡口, with these words: "请按意思在全书中找一找：有没有一段写某个人在视线模糊的时刻，替尚未返航的人指明方向？告诉我它在哪一章。" It has no two-character word of the target passage either. The two messages stay inside 8 model requests. A third message is not sent, and the result is a finding.
- Requests:
  - Embedding: 6 passage requests and 1 question request for each hybrid call. Planned 7, at most 9.
  - Model: planned 5 (search, hybrid, read or finish, finish, final answer), at most 8. R0 makes none.
  - Two hybrid search calls in one Run make 8 embedding requests, inside the maximum.
  - If the model answers without the hybrid search, the author message goes one more time with clearer words, inside the counts. Each automatic request counts. No Exa exchange.
- Expected reported input of each model request:
  - The first request is about 3,600 tokens: Host instructions, twelve Tools, the Chapter list with three Chapters, and the Chapter 渡口.
  - The request after `search_manuscript` is about 3,800.
  - The request after the hybrid search is about 4,600: up to 6 passages of about 60 characters each, with their lines.
  - Later requests add about 200 each.
  - Total about 21,000 tokens for 5 requests, at most about 38,000 for 8.
- Usage of the embedding requests: about 7 requests of fewer than 100 input tokens each. The record gives the reported usage.
- Request guard: the `live_guard` triggers of the record of 2026-11-14, with a cap of 8 model requests and 0 Exa exchanges. They are a guard of the run and no product data.
  - One more trigger of the same form is BEFORE INSERT on `storyos.embedding_attempts`, with a cap of 9. The store writes the Embedding Attempt in the dispatch claim transaction before the I/O, so the trigger counts at the claim, and above the cap no request goes out.
  - The model trigger counts at the insert of the Model Attempt, also before the I/O.
  - After the run, the triggers and the schema `live_guard` are removed, and the record says so. Above a cap, the trigger refuses the claim transaction, so nothing goes out.
- Values to record: each initial value of S7-C2a1, S7-C2b1, S7-C2b2, and S7-C3a in the run (for example `PASSAGE_CHARACTER_LIMIT` 800, `PASSAGE_RESULT_LIMIT` 8, `PASSAGE_LIST_LIMIT` 40, `PASSAGE_FUSION_CONSTANT` 60), with the measure that speaks for it. A proposed change goes to the coordinator as a finding.
- Comment sentences (group 3 of #1600): the second pull request makes four comment sentences shorter than 26 words. They are in `agent_run_hybrid_search.rs` (near line 107), `agent_run_hybrid_search_tests.rs` (near line 705), `manuscript_use.rs` (near line 34), and `manuscript-use-list.tsx` (near line 18).
- Rules: the key reaches the request only through the typed Worker and Server settings. It is in no command line, log, or record. No direct SQL write of product data: the only SQL writes are the guard. Before each push and each tag push, each file is scanned for a key, an e-mail address, and a home path.
- Raw evidence: a tag `evidence/1600-live-hybrid-search-<date>` with the request payloads, the ToolCalls, the plan, and the logs. No database dump and no Project Export Archive. Record: `docs/research/agent-plan-live-hybrid-search-<date>.md`, in its own pull request.
- Conditions of the coordinator (2026-11-17):
  - The record says that R0 was admitted and cancelled before a Worker ran, that it made no Model Attempt, and why it is there: it is the first Run of the author, which makes the binding. The record shows the binding row, the three marks with their Registration, and no Model Attempt of R0.
  - The rehearsal with `host_fake` comes first. If R0 does not leave the binding after the cancel, the run stops, and the coordinator decides. No change to the 10-minute case without the coordinator.
  - The poll of the index work reads through a read connection only. If the three Chapters do not have their six rows after 5 minutes, the Worker stops, and the result is a finding for chain 3.
  - The Worker stops well before 30 minutes after the last Run, so that no Conversation capture starts on the real route.
