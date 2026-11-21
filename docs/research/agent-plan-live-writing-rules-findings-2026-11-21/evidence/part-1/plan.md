## Plan of the live run (S10-08)

The purpose: one recorded run of the changed request text of the mapping `v10` on the real route: the block of the writing rules with the changes F1, F2, and F3, the paragraph on the one `propose_edit` call, and the label of the replaced input (`S10-EVD-001`, `S10-EVD-002`). The run is an observation. It compares no models or prompts, it makes no claim about a cause, and it does not assert that the model obeys a rule.

- **Head of the product code:** `6e8b97a48f76747cc8086c0bff3108a728e5d32e` on `1774-writing-rules-findings` (PR #1777), which merges the integration tip `050d710239003047a6a9f585156c0f4fbcfbff4e` (the merge of #1764). Tree `e5f0cca2f5c569a8b15788265e753443f0c6302a`.
- **Package:** `python3 scripts/package-release.py` on the clean head, manifest `sha256:4b668f4b10cad52eb2b42228cfa1fe5ee7c902fb3da014e32be5814f24d496f9`. The packaged `skill-packages/local-revision/SKILL.md` and the file of the head have the SHA-256 `eb6fbaf75cbf247d5a3b1e5b435c04b67865631e7dd2d1ca455ff2130e9c6041`.
- **Route:** `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. The packaged Server with `STORYOS_WORKER=0` and `STORYOS_SKILLS_DIR` set to the SkillPackages of the package, and one continuous packaged Worker, both with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` (ADR 0022, ADR 0049, ADR 0050). Only the Worker reads the key, through its typed settings. No embedding destination and no Exa key.
- **Database:** a new `scripts/dev-postgres.sh` database. The only direct write is the guard schema `live_guard`. The database goes after the run.
- **Script:** `writing-rules-live.ts`, started by `run.sh` and `rehearse.sh`: the script of the tag `evidence/1762-live-writing-rules-2026-11-21` with a fourth Project and the stops of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL.
- **Rehearsal:** the same script on `host_fake` on this head passed: 4 Runs `completed`, 8 fake requests, no refusal, and the Server listed `local-revision`.

### Fixed inputs

The texts are on the evidence tag (`inputs/`).

| Project | Chapter | Project Instruction |
| --- | --- | --- |
| 渡口 A | 第一章 渡口 of the first run: 12 paragraphs | none |
| 渡口 B | the same Chapter | the sample passage of the first run |
| 归来 C | 第三章 归来 of the first run: one paragraph with habits of the rules 1, 2, 3, 4, 5, 6, 7, and 10 | none |
| 夜渡 D | 夜渡 of Run C of S10-04: 10 Blocks (tag `evidence/1764-live-local-revision-run-c-2026-11-21`, `chapter-01.md`) | the Project Instruction of Run C (`instruction.md`) |

### The four author requests

Each request is one AgentRun in a new Project Conversation. The plan has 4 AgentRuns, and the ticket permits at most 6.

| # | Project | Author request | What the request shows |
| --- | --- | --- | --- |
| R1 | A | 请接着这一章的结尾，写下一场。 | New prose with no Project Instruction: each stock physical reaction (F1), the `propose_edit` calls, and the label after the Proposal. |
| R2 | B | 请接着这一章的结尾，写下一场。 | The same request with a sample: each event, object, and setting of the sample in the scene (F3), the sentence length, and the stock physical reactions. |
| R3 | C | 请改写这一段，保留其中的情节。 | A revision of a paragraph with habits: each habit that changed to the same device in another form (F2), and the stock physical reactions. |
| R4 | D | 请修改这一章。, with the selected Skill `local-revision` | The changed Blocks, the lines of the final answer against the form of the package, each change outside the words of a habit, and each sentence of the answer on a new selection, a new request, or a change that is not in the Proposal. |

### Planned numbers

The planned number of a Run is the number of the stop rule. Before each Run, the script sets the cap of the request guard to the planned number of that Run, so the guard refuses the claim of one more request. The script stops the run when a Run does not end 120 seconds after its planned number of requests, or when a planned number of tokens is above its number.

| Run | Planned number of model requests | Planned number of reported input tokens |
| --- | --- | --- |
| R1 | 3 | 30,000 |
| R2 | 3 | 30,000 |
| R3 | 3 | 20,000 |
| R4 | 3 | 30,000 |
| **Total** | **12** | **110,000** |

For information only: the usual steps of a Run are `propose_edit`, `finish`, and the final answer. In the first run, the three requests of R2 had 25,117 reported input tokens, and the three requests of Run C of S10-04 had 24,357.

### Other stops

The script stops the run at a background request (a capture, a pass, or a summary), an embedding request, an Exa exchange, or a refused or cancelled Run of the author. The run also stops for a refusal of the route, or for a key, an e-mail address, or a home path in a payload or a log. The Memory work of the Host is due 30 minutes after a Run, and the script stops the Worker after R4.

### Evidence

- **Record:** `docs/research/agent-plan-live-writing-rules-findings-2026-11-21.md`, linked at `S10-EVD-001` and `S10-EVD-002` of the delivery continuation section 7.
- **Tag:** `evidence/1774-live-writing-rules-findings-2026-11-21`: this plan, the inputs, the wire payload of each request, the Model Attempts, the ToolCalls, the measures, the request guard and its log, the SHA-256 of the `SKILL.md`, the run script, and the Server and Worker logs. It has no database dump and no Project Export Archive.
