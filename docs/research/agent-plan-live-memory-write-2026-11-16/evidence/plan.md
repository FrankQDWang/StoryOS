## Live run plan (S7-D1a2)

Written before the first request.

- Purpose: show on the real route that the model calls `write_memory` when the author asks for a lasting change of Memory, that the route accepts the section `Memory index` and the paragraph of the Tool, and that the model does not call the Tool for an ordinary writing request.
- Product code: branch `claude/s7-d1a2-write-memory`, head `8c601672`, release package from that clean head. Head Model Registration of the deployment (`fa22`, mapping `v8`), Agent Plan route through the admitted path (ADR 0049, ADR 0050). The key reaches the Worker only through the typed settings. No step reads the env file.
- Project: a new Project with one Chapter and one paragraph that the author types in the editor. No Memory Entry at the start.
- Run 1 (new Conversation): the author asks the assistant to remember one fact of the story. Expected: RunStep 0 has one `write_memory` call with `create`, RunStep 1 calls `finish`, and the closing request gives the answer.
- Run 2 (new Conversation): the request has the section `Memory index` with the entry of Run 1, and the route accepts it. The author asks for a change of that fact. Expected: one `write_memory` call with `change` and the number of the entry, then `finish`, then the answer.
- Run 3 (new Conversation): an ordinary writing request with no Memory request. Expected: no `write_memory` call; `finish`, then the answer.
- Requests: 8 expected (3 + 3 + 2), planned 9 (one more send of a step with a clearer message), at most 12. Each automatic request counts. No Exa exchange.
- Expected reported input of each request: about 2,500 tokens for the first request of a Run (Host instructions, the Tools, the Chapter list, the Chapter), plus about 100 tokens for the section `Memory index` in Run 2 and Run 3, plus the earlier items of the Run for each later request. Run 1: about 2,500, 2,700, and 2,900. Run 2: about 2,600, 2,800, and 3,000. Run 3: about 2,600 and 2,800. Total about 22,000 tokens. The limit of the ticket is 100,000 reported input tokens.
- Request guard: the `live_guard` triggers of the record of 2026-11-14 with a cap of 12 model requests and 0 Exa exchanges. Above the cap, the trigger refuses the claim transaction, so nothing goes out.
- Raw evidence: a tag `evidence/1576-live-memory-write-2026-11-16`. Record: `docs/research/agent-plan-live-memory-write-2026-11-16.md`.
