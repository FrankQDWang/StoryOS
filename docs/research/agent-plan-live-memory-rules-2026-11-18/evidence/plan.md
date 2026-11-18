## Plan of the live run (criterion 8)

The first request waits for the confirmation of the coordinator.

- **Head:** `1e40e7f4b4a978a85458dc828dc768a3310096d4` on `claude/1700-memory-rules-v9`, tree `bf9516dede18969bb3c55b8490567762e7ab0c15`.
- **Package:** `python3 scripts/package-release.py` on that clean commit, manifest `sha256:84ca6879875c119b6d35d0b8673c27364a2c29471f3d4b0e012c50918f45baa0`.
- **Route:** `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa23`, mapping `storyos.volcengine-agent-plan.responses.create.v9`. Packaged Server with `STORYOS_WORKER=0` and one packaged Worker (ADR 0022, ADR 0049, ADR 0050). The key reaches a request only through the typed settings. The run sets no embedding destination and no Exa key.
- **Database:** a new `scripts/dev-postgres.sh` database. The direct writes are the guard schema `live_guard` and the one statement C1.
- **Sample:** `fixtures/memory-calibration/chapter-01.md`, sent through the public commands of the editor, as the runs of S7-D7 and S7-E1b did.
- **Rehearsal:** the same script on `host_fake` passed on this head: 5 fake requests, statement C1 changed 1 row, no failed work.

### Steps

1. **Part A, Conversation A.** The message of the author: "请帮我记住一个伏笔：茶馆门板上那张写着“东主有事”的白纸，其实是赵德明替沈怀山写的，这件事后面才揭开。先别改正文。" The record gives this message and the text of the entry.
2. **Part B, Conversation B.** The message of the author: "第一章里，顾青禾是怎么跟知遥说爷爷走时的情形的？请引用正文里的原句回答，不要修改正文。另外我定下一件事：沈明川最后不会卖掉茶馆，他会在第十章的镇上会议上当众撕掉那份意向书。"
3. **Statement C1** of the test administrator connection. It must change 1 row:
   `UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '<project>'::uuid AND work_kind = 'conversation_capture' AND source_id = '<Conversation B>'::uuid`
4. The Worker runs the capture of Conversation B. A capture that stores observations makes the pass due, so the Worker then runs one pass with no statement. The record gives each observation of the capture.
5. The script stops the Worker. The capture of Conversation A and the capture of the Chapter do not start: their rows are due 30 minutes and 1 hour later.

### Requests and expected reported input

| Kind of request | Planned count | At most | Expected reported input of one request (tokens) |
| --- | --- | --- | --- |
| Run of the author, Part A | 3 | 5 | 11,000 (range 9,000 to 14,000) |
| Run of the author, Part B | 3 | 6 | 12,000 (range 9,000 to 16,000) |
| Capture | 3 | 8 | 5,000 (range 3,000 to 8,000) |
| Pass | 1 | 1 | 3,000 (range 2,500 to 5,000) |
| **Total** | **10** | **20** | **about 87,000 tokens in all** |

The ranges come from the record of S7-E1b, with one Chapter in place of three. A request with a prior reference reports the full conversation as its input, and the ranges include this.

### Limits and stops

- **Limits of the ticket:** at most 20 model requests and 200,000 reported input tokens.
- **Request guard:** the `live_guard` triggers of the record of S7-E1b, counted at the claim: model 20, Exa 0, embedding 0.
- **The script stops the run** in each poll when: the reported input of all requests is above 180,000 tokens, one request of a Run of the author is above 40,000 tokens, one background request is above 20,000 tokens, a capture or a pass fails, a Run of the author is refused or cancelled, or statement C1 changes another row count.
- **The run also stops** for a refusal of the route, or for a key, an e-mail address, or a home path in a payload or a log.

### Evidence

- **Record:** `docs/research/agent-plan-live-memory-rules-2026-11-18.md`, in the form of the record of S7-D3f.
- **Tag:** `evidence/1700-live-memory-rules-2026-11-18`, with the plan, the request payloads, the Model Attempts, the ToolCalls, the request guard and its log, statement C1, the run script, and the Server and Worker logs. It has no database dump and no Project Export Archive.
- **The run is an observation.** When the model still adds a fact of the manuscript, the record says so as a finding, and the ticket does not fail.

## New head of the live run

The coordinator confirmed the plan of [this comment](https://github.com/FrankQDWang/StoryOS/issues/1700#issuecomment-6971895008) after the change of criterion 4. The change is not a part of the request text, so the counts, the ranges, and the stops of the plan stay. The run uses this head:

- **Head:** `5aa7cb9c90ad3f2b34dcf7f4b503de89e80888e3`
- **Tree:** `1f133c79eb84e1d2f454055df3b2cece46c35079`
- **Package manifest:** `sha256:d73239c535124c863585098234d56644c13c06881120345a23d68b6457a53b7e`
- **Rehearsal on `host_fake` on this head:** passed: 5 fake requests, statement C1 changed 1 row, no failed work.

## Head of the second live run

The author decided one more text and one more live run (the change notes of criterion 2 and criterion 8). The plan of [the first run](https://github.com/FrankQDWang/StoryOS/issues/1700#issuecomment-6971895008) applies: the same Part A, Part B, statement C1, stops, and limits (at most 20 model requests and 200,000 reported input tokens for this run). The second run uses this head:

- **Head:** `527ced925400b7ce92e107ce46581d08bde57497`
- **Tree:** `328bcd3dbbc9b7d75e1b382e1f7121e1bf66cb98`
- **Package manifest:** `sha256:72d5b9f071022c73612391258ba8d5f8162a56301c9acdff978ad8a49397f427`
- **Rehearsal on `host_fake` on this head:** passed: 5 fake requests, statement C1 changed 1 row, no failed work.
- **Raw evidence:** tag `evidence/1700-live-memory-rules-run2-2026-11-18`.
