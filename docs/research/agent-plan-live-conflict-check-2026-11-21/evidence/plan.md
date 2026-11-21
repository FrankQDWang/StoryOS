## Plan of the live run of `conflict-check`

- **Purpose:** show the effect of the instructions of the package `conflict-check` on the real Agent Plan route (`S10-EVD-002`). The run is no trial that compares models or prompts, and it does not assert that the model obeys the text.
- **Product:** `1763-distribution-skillpackages` at `037239c31e2104d069fbcb21fb5057fdff6bca92` (tree `c93b448f000ab71eb39d6e197348b15798bb393c`). The release package of `python3 scripts/package-release.py` on that clean commit: the packaged Server with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` set to `target/release-package/skill-packages`, and one continuous packaged Worker with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Each process environment has only the named variables, `PATH`, and `HOME`. No embedding destination and no Exa key. The run never reads an env file or a key.
- **Fixed inputs:** a new Project 渡口 with the Volume 第一卷 and two Chapters, written through the public commands of the Web client: Chapter 1 渡口 (5 paragraphs) and Chapter 2 来信 (5 paragraphs, the current Chapter). Chapter 2 has three planted conflicts with Chapter 1 and one planted conflict with a Memory Entry that the author asked the assistant to remember:
  - P1 kinship: Chapter 2 "妹妹林雪", Chapter 1 "姐姐林雪比他大六岁".
  - P2 date: Chapter 2 "五年前的夏天，父亲出事以后", Chapter 1 "三年前的冬天，父亲林德在河上翻了船".
  - P3 object state: Chapter 2 "玻璃光洁如新，一道裂纹也没有", Chapter 1 "怀表的玻璃裂了一道斜纹".
  - P4 Memory: Chapter 2 "跳进河里，一口气游到了对岸", Memory Entry of Run 1 "林舟怕水，从小不会游泳".
- **AgentRuns:** 3, each in a new Conversation (the limit of the ticket is 5).
  - Run 1, no selected Skill: "请帮我记住一件事：林舟怕水，从小不会游泳，这是后面一个情节的关键。先别改正文。" Expected: one `write_memory` call. The run stops when Run 1 writes no Memory Entry.
  - Run 2, with the selected Skill `conflict-check`: "请检查这一章。"
  - Run 3, no selected Skill: "请帮我看看这一章和前面的章节、和你记住的事情有没有前后矛盾的地方。"
- **Planned requests:** 12 (Run 1: 3; Run 2: 4; Run 3: 5, with a possible `load_skill` call). Automatic requests count. The Memory work of the Host is due only 30 minutes after a Run (Conversation capture) or 1 hour after a Chapter change, so the run ends and stops the Worker before it.
- **Expected reported input:** about 4,000 tokens for each request of Run 1, 5,000 to 8,000 for each request of Runs 2 and 3. Total about 75,000 tokens.
- **Hard caps:** a database trigger of the disposable database refuses the claim of the 25th model request, of each Exa exchange, and of each embedding request, so nothing goes out. The run script stops the Worker at more than 24 requests, more than 200,000 reported input tokens, a request above 40,000 input tokens, a fourth author Run, a Run that is not an author Run, or a refused or cancelled Run.
- **Rehearsal:** the same script on `host_fake` first, on the same head.
- **Record:** `docs/research/` with a section Findings, and the raw evidence on the tag `evidence/1763-live-conflict-check-2026-11-21`.
