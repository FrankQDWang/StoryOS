## Plan of the live run (S10-02)

The purpose: one recorded run that shows the default writing rules of the mapping `v10` on the real route (`S10-EVD-001`). The run is an observation. It compares no models or prompts, and it does not assert that the model obeys a rule.

- **Head of the product code:** `07f665a12e6abdcba167720e470c84ac38c85d9c` on `stage10-author-voice` (the merge of PR #1769), tree `4a89a352c0900a32aef8d7e511fe638faa8b4e79`. The integration tip `8de2ea2ede09d47a681e9ca4ad8902e8c8d02633` changes only documents after it.
- **Package:** `python3 scripts/package-release.py` on the clean commit `07f665a1`, manifest `sha256:3d378c0eaa77b845f1fd51a369f7b2219f8435aaf16a27697c79c097f39df8c1`.
- **Route:** `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. The packaged Server with `STORYOS_WORKER=0` and one continuous packaged Worker, both with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` (ADR 0022, ADR 0049, ADR 0050). Only the Worker reads the key, through its typed settings. The run sets no embedding destination and no Exa key.
- **Database:** a new `scripts/dev-postgres.sh` database. The only direct write is the guard schema `live_guard`. The database goes after the run.
- **Script:** `writing-rules-live.ts`, started by `run.sh` and `rehearse.sh`, from the loader of the tag `evidence/1700-live-memory-rules-2026-11-18`. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL.
- **Rehearsal:** the same script on `host_fake` on this head passed: 4 Runs `completed`, 8 fake requests, no refusal.

### Fixed inputs

The texts are original and are on the evidence tag (`inputs/`).

| Project | Chapter | Project Instruction |
| --- | --- | --- |
| 渡口 A | 第一章 渡口: 12 paragraphs, 782 characters | none |
| 渡口 B | the same Chapter | one sample passage of 268 characters with an interruption by a dash (想说点什么——) and one contrast 她不是忘了收，而是每天都擦。, and one sentence that asks for its sentence length, word choice, and distance |
| 归来 C | 第三章 归来: one paragraph of 163 characters with habits of rules 1, 2, 3, 4, 5, 6, 7, and 10 | none |

### The four author requests

Each request is one AgentRun in a new Conversation.

| # | Project | Author request | What the request shows |
| --- | --- | --- | --- |
| R1 | A | 请接着这一章的结尾，写下一场。 | New prose with no Project Instruction: each habit of the ten rules in the output. |
| R2 | B | 请接着这一章的结尾，写下一场。 | The same request with a sample: the devices and the sentence length of the sample in the output. |
| R3 | C | 请改写这一段，保留其中的情节。 | A revision of a paragraph with eight habit classes: the habits that stay. |
| R4 | A | 这一章里，林舟为什么一直不肯离开渡口？请直接回答，不用修改正文。 | A question with no prose of the novel: a normal answer, and if it names or quotes the rules. |

### Requests and expected reported input

| Run | Planned model requests | At most | Expected reported input of one request (tokens) |
| --- | --- | --- | --- |
| R1 | 3 (`propose_edit`, `finish`, final answer) | 5 | 4,500 to 7,000 |
| R2 | 3 | 5 | 5,000 to 7,500 |
| R3 | 3 | 5 | 3,500 to 5,000 |
| R4 | 2 (`finish`, final answer) | 5 | 4,500 to 5,000 |
| **Total** | **11** | **20** | **about 56,000 tokens in all** |

A request with a prior reference reports the full conversation as its input, and the ranges include this. The ticket names no token limit.

### Limits and stops

- **Runs:** 4 planned; the ticket permits at most 8. A Run counts one time, and no Run repeats.
- **Request guard:** the `live_guard` triggers of the tag `evidence/1700-live-memory-rules-2026-11-18`, counted at the claim: model 20, Exa 0, embedding 0.
- **The script stops the run** in each poll when: there are more than 20 model requests, the reported input of all requests is above 150,000 tokens, one request is above 30,000 tokens, a background request (a capture, a pass, or a summary) occurs, an embedding request or an Exa exchange occurs, or a Run of the author is refused or cancelled.
- **The run also stops** for a refusal of the route, or for a key, an e-mail address, or a home path in a payload or a log.

### Evidence

- **Record:** `docs/research/agent-plan-live-writing-rules-2026-11-21.md`, linked at `S10-EVD-001` of the delivery continuation section 7.
- **Tag:** `evidence/1762-live-writing-rules-2026-11-21`: this plan, the inputs, the wire payload of each request, the Model Attempts, the ToolCalls, the measures, the request guard and its log, the run script, and the Server and Worker logs. It has no database dump and no Project Export Archive.
