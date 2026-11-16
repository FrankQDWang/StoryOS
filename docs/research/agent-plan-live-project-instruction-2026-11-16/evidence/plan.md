## Live run plan (S7-A1b)

Written before the first request.

- Purpose: show on the real route that the request of a Run with a bound Project Instruction carries the section `Project instruction`, that the answer follows its rule, and that a later author request that asks for the opposite wins for that answer (Instruction Precedence).
- Product code: PR #1603, head `dc30a926cbde99630d2de9743e53706f939706d7`, release package from that clean head. Head Model Registration of the deployment (`fa22`, mapping `v8`), Agent Plan route through the admitted path (ADR 0049, ADR 0050). The key reaches the Worker only through the typed settings.
- Project Instruction (one rule that a reader can see and that conflicts with no Host instruction): `每次回答的最后一行单独写「——你的写作助手」。`
- Run 1 (new Conversation): an author request for a short comment on the Chapter text that says nothing about the rule. Expected: the section `Project instruction` in the user message of RunStep 0, and an answer whose last line is `——你的写作助手`.
- Run 2 (same Conversation): an author request that asks, for this answer only, to not write that last line. Expected: the answer has no `——你的写作助手` line.
- Requests: each Run has the decision request of RunStep 0 (`finish`) and the closing request. Planned: 4 model requests, at most 6 (one more send of a step with a clearer message, and each automatic request, are in this count). No Exa exchange.
- Expected reported input: about 3,000 tokens for each first request and about 3,500 for each closing request, about 14,000 in total.
- Request guard: the `live_guard` triggers of the record of 2026-11-14 with a cap of 6 model requests and 0 Exa exchanges.
- Raw evidence: a tag `evidence/1574-live-project-instruction-2026-11-16`. Record: `docs/research/agent-plan-live-project-instruction-2026-11-16.md`.
