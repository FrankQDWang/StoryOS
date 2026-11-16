## Live run plan (S7-B2b)

Written before the first request.

- Purpose: show on the real route that the model uses the Chapter list, `search_manuscript`, and `read_chapter` to answer a question about another Chapter, and that the route accepts the strict schema of `read_chapter` with its two nullable integers.
- Product code: branch `claude/s7-b2b-manuscript-uses` at `714e44a6`, release package from that clean head. Head Model Registration of the deployment (`fa22`, mapping `v8`), Agent Plan route through the admitted path (ADR 0049, ADR 0050). The key reaches the Worker only through the typed settings.
- Setup through the author surface (the packaged Web client): one new Project with one Volume and three Chapters, each with one paragraph typed in the editor. Chapter 1 `听潮` names the object `听潮铃`, and no other Chapter names it. Chapter 2 `雾港`. Chapter 3 `归航` is the last Chapter and the Current Chapter.
- Run 1 (new Conversation): the author asks who gave `听潮铃` to the main character and what it does. Expected calls: `search_manuscript` with `听潮铃`, then `read_chapter` for Chapter 1 or for the Blocks of the match, then `finish`. Expected answer: the fact of Chapter 1.
- Run 2 (same Conversation): the author names Chapter 2 by its title `雾港` and asks what its start says. Expected calls: `read_chapter` with the number 2 from the Chapter list, then `finish`.
- Requests: Run 1 has RunStep 0 (search), RunStep 1 (read), RunStep 2 (`finish`), and the closing request. Run 2 has RunStep 0 (read), RunStep 1 (`finish`), and the closing request. Planned: 7 model requests, at most 11 (one more send of a step with a clearer message, and each automatic request, are in this count). No Exa exchange.
- Expected reported input: M1 about 2,800, M2 about 3,100, M3 about 3,400, M4 about 3,600; M5 about 4,000, M6 about 4,300, M7 about 4,500. Total about 26,000, below the limit of 100,000.
- Request guard: the `live_guard` triggers of the record of 2026-11-14 with a cap of 11 model requests and 0 Exa exchanges.
- Raw evidence: a tag `evidence/1573-live-manuscript-tools-2026-11-17`. Record: `docs/research/agent-plan-live-manuscript-tools-2026-11-17.md`.
