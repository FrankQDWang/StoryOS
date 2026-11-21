# Agent Plan live run of the SkillPackage local-revision (2026-11-21)

This record is the live evidence of [S10-04: Add the SkillPackage local-revision](https://github.com/FrankQDWang/StoryOS/issues/1764) for `S10-EVD-002` and [ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) section 2. It shows the effect of the instructions of the package `local-revision` on the real Agent Plan route. Runs A and B used the first text of the package. After finding F2, the coordinator added two sentences to the section "One Proposal", and Run C used the changed text. The run is no trial that compares models or prompts, and it does not assert that the model obeys the text.

Raw evidence is on the tag `evidence/1764-live-local-revision-2026-11-21`. The tag holds the plan, the Chapter, the Project Instruction, the list of the planted habits, the run script, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M8.json`), the Model Attempts, the ToolCalls, the guard log, `measures-A.json` and `measures-B.json` (the Blocks, the Proposal operations, and each answer in full), `checks.json`, `loader.log`, and the Server and Worker logs. Raw evidence of Run C is on the second tag `evidence/1764-live-local-revision-run-c-2026-11-21`, with the same kinds of files (`wire/M1.json` to `wire/M3.json`, `measures-C.json`, and the script `local-revision-run-c-live.ts`). The tags have no database dump and no Project Export Archive.

## Plan and final count

The plan comes before the results. The planned numbers of Runs A and B were 6 requests in total, 3 for each Run, and about 45,000 reported input tokens.

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7060564582)). A rehearsal of all steps on `host_fake` on the same head came first: 2 author Runs, 4 fake requests, and no other Run.

| Item | Planned | Actual |
| --- | --- | --- |
| AgentRuns | 2 (limit 4) | 2, each `completed`, each in a new Project and Conversation |
| Model requests | 6 (3 for each Run); the database cap was 12 | 8 (4 for each Run, finding F2) |
| Reported input | about 45,000 tokens | 69,190 tokens |
| Largest request | at most 40,000 | 10,746 |
| Other Runs (Memory work) | 0 | 0 |
| Exa and embedding requests | 0 | 0 |

**Deviation from the plan.** Run A and Run B each had 4 requests, and the plan had 3 for each Run (finding F2). The reported input was 69,190 tokens, and the plan had about 45,000. "Live model requests" of the delivery document says to stop and report to the coordinator for more requests than the plan. The run did not stop after Run A: the script had no stop at the planned number of a Run. Its stops were above 12 requests or 120,000 input tokens in a Project, a request above 40,000 input tokens, a second author Run in a Project, a Run that is not an author Run, and a refused or cancelled Run. The 8 requests were inside the cap of 12. The executor reported the extra requests to the coordinator after the run, in the findings message before the review request. After the run, the coordinator decided to accept Runs A and B as evidence of the ticket, with no repeat of a Run ([decision comment](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7060765711)). The plan of Run C gives one number of requests, and the script stops at it ([decision on #1762](https://github.com/FrankQDWang/StoryOS/issues/1762#issuecomment-7060590530), item 4).

**Plan of Run C.** The plan of Run C was posted on the ticket before its first request ([plan comment of Run C](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7060778165)). It is AgentRun 3 of the 4 that the ticket permits. Its planned number of requests was 3 for the Run and 3 in total, and its planned number of reported input tokens was 30,000. The request guard of the database was set to refuse the claim of a 4th model request, and the script was set to stop the Worker when the guard refused a request or when the Run did not end 90 seconds after the 3rd request. A rehearsal on `host_fake` on the same head came first: 1 author Run, 2 fake requests, and no other Run.

| Item | Planned | Actual |
| --- | --- | --- |
| AgentRuns | 1 | 1, `completed`, in a new Project and Conversation |
| Model requests | 3 | 3 |
| Reported input | 30,000 tokens | 24,357 tokens |
| Largest request | at most 40,000 | 10,382 |
| Other Runs, Exa, and embedding requests | 0 | 0 |

Run C kept its plan.

## Findings

The executor sent findings F1 to F8, and then the findings C1 to C4 of Run C (F9 to F11 of this record), to the coordinator before the review request. The decision of the coordinator on F1 to F8 is on the ticket ([decision comment](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7060765711)). The coordinator accepted Run C as it occurred, with no added sentence and no fourth AgentRun ([decision comment of Run C](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7060913472)). After review round 1, the coordinator decided to correct the conditions of two sections of the package with no new Run ([decision comment of round 1](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7061123227); section "The text of the package in each Run").

| ID | Finding | Effect |
| --- | --- | --- |
| F1 | **Each Run sent one Proposal in its first `propose_edit` call, with one operation for each changed Block.** Run A changed 5 Blocks, and Run B changed 2 Blocks. Each operation waits for the decision of the author, so the author can accept, change, or refuse each change alone. | The count rules and the rule "one Proposal" show in the first call. |
| F2 | **After the first Proposal, the Host sent a new input with fewer target Blocks, and each Run made a second `propose_edit` call that the Host refused with `after_proposal`.** The user message of the next request said "The input of this author request changed after your last step", and its target Blocks had no Block of the open Proposal. Then the model made a second call: in Run A it proposed Block 8 with the summary "在新的五个目标段中，只改“然而，雾越来越浓……”这一处的段首连接词和三连排比。", and in Run B it proposed Blocks 2 and 4 with the summary "改最明显的 2 处机器腔". | Each Run used 4 requests and not 3. The total was 8 requests against 6 in the plan, in the cap of 12. The refusal kept the count: no Run changed more Blocks than the instructions permit. The coordinator added two sentences to the package for Run C (F9). The Host part, a sentence in the Host instructions on the one Proposal of a Run, has the owner [S10-08](https://github.com/FrankQDWang/StoryOS/issues/1774). |
| F3 | **The final answers do not have the form of the instructions.** Run A gave one line only for the refused change of Block 8 ("然而，雾越来越浓…… —— 段首“然而”连接词＋“雾越来越／水越来越／岸越来越”三连排比，属机器腔"), and named the 5 changed places in one sentence ("此前那份待决 Proposal 已覆盖另外五处更明显的机器腔（开头的“微微／缓缓”、眼中闪过、石子投湖比喻、心中暖流、命运齿轮）"). Run B gave lines for Blocks 2 and 4, which it did not change, and offered to send them later ("之后我立刻按当前选段提交下面这 2 处"). Neither answer gave one line for each changed place. | The answer of Run A spoke of the input of M2 ("这次的五个目标段里"). |
| F4 | **Block 5, which has the device of the sample passage, stayed unchanged in each Run.** The answer of Run A said why: "样文本身两次使用这一句式（“他等的不是船，是人”），属于你指定的口吻，故不动。" | The rule "what you leave" shows. |
| F5 | **The Blocks with no habit (1, 3, and 7) stayed unchanged in each Run.** | The rule "all other text stays as it is" shows. |
| F6 | **Some candidates add a small action or a sensory detail in the place of the removed habit**: 别开脸 and 没接话 (Block 4), 林舟没接话 and 河水拍着船帮 (Block 6), 握紧那把钥匙 (Block 9), and a new last image (Block 10 of each Run). No candidate adds an event of the plot or a name, and no candidate changes a fact of the Chapter. | Observation for the author. The rule "add no fact of the plot" did not stop small details of the scene. |
| F7 | **The two Runs chose different places first.** Run A chose Blocks 2, 4, 6, 9, and 10, and left Block 8 for its refused second call. Run B, with "只改最明显的 2 处", chose Blocks 9 and 10. | Observation: the order of "the most obvious place" is the choice of the model. |
| F8 | **The run used the mapping `v10` and Registration `fa24`.** The Host instructions had the default rules for prose of #1761, and the instructions of the package name those rules as the habits. Each explanation of a change names the words of the habit that the change removes. | Observation: the 5 changed Blocks of Runs A and C, and the 2 of Run B, each have a planted habit. |
| F9 | **Run C, with the changed section "One Proposal", made one `propose_edit` call and then called `finish`.** No call was refused. The user message of M2 of Run C also had the label "The input of this author request changed after your last step" and fewer target Blocks. | The second Proposal of F2 did not occur, and Run C kept its plan of 3 requests. |
| F10 | **The final answer of Run C does not have the form of the instructions either.** It speaks of the input of C2 as a new selection of the author: "已读完新指定的五个目标段" and "如果您这次的选区是有意调整，请先驳回那个 Proposal，我再按新选区出稿。" The user message of C2 had the label "The input of this author request changed after your last step". The answer gave lines for the Blocks of that input and a change of Block 8 that is in no Proposal ("等您对当前 Proposal 作出处理后，我可以把"然而"那段的改法正式提交"), and no line for each of the 5 changed places. | No sentence was added to the package. The owner is [S10-08](https://github.com/FrankQDWang/StoryOS/issues/1774), which changes the text of the label of the replaced input. The live run of S10-08 examines the form of the final answer of this package again, with the fixed Chapter, Project Instruction, and request of Run C. |
| F11 | **Some candidates of Run C change words outside the habit, and one candidate lost a name.** Block 2: 船舱的门 became 舱门, and 早就知道 became 早知道. Block 9: the name 林舟 at the start of the Block was lost, as in Run B, 想让 became 让, and 做决定 became 拿主意. Block 6 added the action 林舟没作声, and Block 10 became the new image 岸看不见了. The other words outside the habits stayed as they were. | Observation for the author. The rule "keep the unchanged words as they are" did not show in each candidate. |

## Setup

| Item | Value |
| --- | --- |
| Product code | `1764-local-revision` at `544727597ddd821446403fd633300be5518c7f5e` (tree `e9f016cf97115f5fb9fa04bbff09c030dba58d06`), on `stage10-author-voice` `e27a95fb`. |
| Release package | `python3 scripts/package-release.py` on the clean commit, manifest `sha256:fd1c03ab8818f17eae6d628b431a586be6cf628f71e2957744801d5f11d22083`. `skill-packages/local-revision/SKILL.md` of the release package has the bytes of the tracked file (SHA-256 `13b982e7f579330603794547e910aedcc0aee4682ae0a942e9202e9a400513fb`). |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` set to `target/release-package/skill-packages`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package. Each process environment had only the named variables, `STORYOS_DATABASE_URL`, `PATH`, and `HOME`, and the Server also had the test settings `STORYOS_BOOTSTRAP_SESSIONS` and `STORYOS_CHALLENGE_SECRET`. It had no embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh with` database. The only direct write was the guard schema `live_guard`. The database was removed after the run. |
| Run C | `1764-local-revision` at `ec29569eefa752324593695268c531632e4ec6b6` (tree `4e4ddfec51b2e3375cc2a88189ef8509f327b357`), on `stage10-author-voice` `718bfd46`, with the two added sentences of the section "One Proposal". Release manifest `sha256:9d6c6f145b1c9ae5f1613c59a45ea61055f6ee465fbb20db8b075a431912a0a5`, `SKILL.md` SHA-256 `0b73daa84eb8c536c2fe016a385b8fb38677de5667e1f2274f9bafc0762558e9`, package digest `sha256:da00728da1d9de1311b25ea5e302960e0d804c67cb90eab7210cfa116ed04613`. The same topology, settings, and model. A new database. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. |
| Script | `local-revision-live.ts`, started by `run.sh` and `rehearse.sh`. It is the run script of #1763 with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Skill | `local-revision`, package digest `sha256:aedcb026ab7ba53db958a3665898dcee83bad3fd1a197f13079d5545afe99a20`. `listSkills` gave it and `conflict-check`, each with the description of its `SKILL.md`. |

## The text of the package in each Run

| Text | `SKILL.md` SHA-256 | Runs |
| --- | --- | --- |
| First text | `13b982e7f579330603794547e910aedcc0aee4682ae0a942e9202e9a400513fb` | A and B |
| With the two added sentences of the section "One Proposal" | `0b73daa84eb8c536c2fe016a385b8fb38677de5667e1f2274f9bafc0762558e9` | C |
| With the corrected conditions of the sections "The text to revise", "The count", and "Nothing to change" (the head of this change) | `eb6fbaf75cbf247d5a3b1e5b435c04b67865631e7dd2d1ca455ff2130e9c6041` | none |

After review round 1, the conditions of three sections became exclusive and complete. No Run used the corrected text. The diff from the text of Run C:

```diff
 ## The text to revise
 
-- When the author names a text, revise that text.
-- When the author names no text, revise the text of the author references of the request.
-- When the request has no author reference, revise the Working text.
+- When the author request names a text, revise that text.
+- When it names no text and the request has author references, revise the text of the author references.
+- When it names no text and the request has no author reference, revise the Working text.
@@
 ## The count
 
-- Choose at most 5 places. Choose the most obvious place first.
-- When the author request gives a number, choose at most that number of places.
+- When the author request gives a number of places, choose at most that number of places.
+- When it gives no number of places, choose at most 5 places.
+- Choose the most obvious place first.
 - One place is one Block.
@@
 ## Nothing to change
 
-When no place has a habit, make no Proposal, and say so in the final answer.
+When you choose no place, make no Proposal, and say so in the final answer.
```

For the input of each Run, the sentences before and after the correction give the same instruction:

- **Runs A and C** ("请修改这一章。"): the request names the Chapter, which is the Working text, and it has no author reference. Before and after, the text to revise is the Chapter. The request gives no number of places. Before and after, the limit is 5 places. Each Run chose places, so the section "Nothing to change" did not apply before or after.
- **Run B** ("只改最明显的 2 处。"): the request names no text and has no author reference. Before and after, the text to revise is the Working text, the Chapter. The request gives the number 2. Before, the two sentences "at most 5" and "at most that number" both permit 2 places and no more. After, the limit is 2 places. The Run chose places, so the section "Nothing to change" did not apply before or after.

The live run of [S10-08](https://github.com/FrankQDWang/StoryOS/issues/1774) with `local-revision` uses the package after the merge of this change, and its record gives the SHA-256 of that `SKILL.md`.

## Fixed inputs

Three new Projects 夜渡 A, 夜渡 B, and 夜渡 C (for Run C), each with assistance on, the Volume 第一卷, and the Chapter 夜渡 of 10 Blocks, written through the public commands. The Chapter was the current Chapter, so it was the Working text of each Run. The text is `chapter-01.md` of the tag.

Project Instruction of each Project: "写作要求：第三人称，贴近林舟的视角，句子短。新写和修改都照下面这段样文的口吻。" and a sample passage that uses the contrast frame 不是……是…… two times on purpose.

| Block | Rule | Planted habit | Text |
| --- | --- | --- | --- |
| 1 | none | none | 天还没亮，林舟就到了渡口。老周的木船拴在桩上，船头挂着一盏防风灯。 |
| 2 | 5 | 微微、缓缓、轻轻 on each action; 仿佛 | 老周微微点头，缓缓起身，轻轻地推开船舱的门，仿佛早就知道他会来。 |
| 3 | none | none | 林舟把两文钱放在船板上，说要去对岸找姐姐。老周没有收钱，只把缆绳解开了。 |
| 4 | 4 | 眼中闪过一丝；嘴角勾起一抹 | 听到“对岸”两个字，老周的眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度。 |
| 5 | 1 | 不是……而是…… (the device of the sample: leave it) | 这不是一次普通的渡河，而是林舟第一次违背姐姐的嘱咐。 |
| 6 | 6 | 如同石子投入湖面，泛起层层涟漪 | 老周低声说了一句：“那边的人，回不来了。”这句话如同石子投入湖面，在林舟心里泛起层层涟漪。 |
| 7 | none | none | 船到河心，林舟摸了摸脖子上的怀表。表还停在四点十分。 |
| 8 | 7 | 然而 at the start; three parallel items | 然而，雾越来越浓，水越来越急，岸越来越远。 |
| 9 | 3 | 心中涌起一股暖流；他终于明白 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 |
| 10 | 2 | 他不知道的是；命运的齿轮 | 他不知道的是，命运的齿轮已经开始转动。 |

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each Model Attempt is `settled`. `prev`: the request continued the response of the row before it with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 09:28:01 | A, 0 | `resp_021795253281345ab257ece25bd0fea81c82ca5c15ec8fd4b550b` | none | required | `propose_edit` (5 changes) | 5,994 / 4,702 |
| M2 | 09:29:23 | A, 1 | `resp_021795253363876bdd5db804b0f31a919b8f9d8940b3f7fab57cc` | M1 | required | `propose_edit`, refused | 7,999 / 2,051 |
| M3 | 09:29:59 | A, 2 | `resp_02179525340005958818742e03404164f2bdc6cf7ab386a6e7fbd` | M2 | required | `finish` | 10,118 / 576 |
| M4 | 09:30:13 | A, 3 | `resp_021795253413377f9405f279a0f30d2c73a697bf053f75a2ff547` | M3 | none | final answer | 10,746 / 279 |
| M5 | 09:30:24 | B, 0 | `resp_02179525342482216e38debc86f845189b93b52559f1b788a1826` | none | required | `propose_edit` (2 changes) | 5,994 / 3,576 |
| M6 | 09:31:27 | B, 1 | `resp_021795253487632e41c82dd1debd95d24831350e3c3a87766b24a` | M5 | required | `propose_edit`, refused | 7,894 / 1,917 |
| M7 | 09:32:04 | B, 2 | `resp_021795253524559d2ec6088cd57f16be69183642d63bc77f31241` | M6 | required | `finish` | 9,881 / 632 |
| M8 | 09:32:19 | B, 3 | `resp_021795253539448bbc1b38287ece59059a49659ad58bad981c641` | M7 | none | final answer | 10,564 / 395 |
| C1 | 09:46:56 | C, 0 | `resp_02179525441705558818742e03404164f2bdc6cf7ab386a2021f2` | none | required | `propose_edit` (5 changes) | 6,015 / 4,664 |
| C2 | 09:48:28 | C, 1 | `resp_021795254508430816811b617e2b5631e8927bae226874c388f8e` | C1 | required | `finish` | 7,960 / 2,372 |
| C3 | 09:49:19 | C, 2 | `resp_021795254559304746abb8ea69ccb3d7a75bd7d82f8063fcd0934` | C2 | none | final answer | 10,382 / 363 |

Total of Runs A and B: **8 model requests, 69,190 reported input tokens, and 14,128 output tokens** (Run A: 34,857 / 7,608; Run B: 34,333 / 6,520). Run C: **3 model requests, 24,357 reported input tokens, and 7,399 output tokens**. C1 to C3 are `wire/M1.json` to `wire/M3.json` of the second tag. No Retrieve and no recovery occurred.

Each request declared the same 13 functions: `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `load_skill`, `read_skill_resource`, `read_chapter`, `search_manuscript`, `write_memory`, `read_memory_entry`, `search_memory`, and `finish`. The requests with a user message (M1, M2, M5, M6, C1, and C2) had the section `Skill catalog` with the lines of `conflict-check` and `local-revision`. The other requests sent only a function output. The instructions of each request had the default rules for prose.

The user message of M2 and of M6 started with "The input of this author request changed after your last step. This input replaces the earlier input of this author request". Its target Blocks were the Blocks of the Chapter with no open Proposal operation: Blocks 1, 3, 5, 7, and 8 in M2, and Blocks 1 to 8 in M6 (finding F2). The function output of the first `propose_edit` in each was "StoryOS opened the Proposal of this change, and the Proposal waits for the decision of the author." The user message of C2 had the same label and the target Blocks 1, 3, 5, 7, and 8. The function output of the refused call in M3 and M7 was "The Host refused this call because this Run already opened a Proposal that waits for the decision of the author, so no Proposal opened."

## Run A: the selected Skill, "请修改这一章。"

Author request in Project A, with the selection of `local-revision` in `skill_selections`: "请修改这一章。" The user message of M1 had the section `Skill instruction` with the exact bytes of `skill-packages/local-revision/SKILL.md` (byte comparison: equal).

**Count of changed Blocks: 5** (Blocks 2, 4, 6, 9, and 10), in one Proposal with 5 operations. The summary of the call: "挑出本章五处最明显的机器腔，改为样文式的短句子白描，其余文字不动。"

| Block | Target text | Candidate | Lost | Added |
| --- | --- | --- | --- | --- |
| 2 | 老周微微点头，缓缓起身，轻轻地推开船舱的门，仿佛早就知道他会来。 | 老周点点头，起身，推开船舱的门，像早就知道他会来。 | none (only the habit) | none |
| 4 | 听到“对岸”两个字，老周的眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度。 | 听到“对岸”两个字，老周怔了一下，别开脸，没接话。 | the words 慌乱 and 苦涩 | the actions 怔了一下, 别开脸, and 没接话 |
| 6 | 老周低声说了一句：“那边的人，回不来了。”这句话如同石子投入湖面，在林舟心里泛起层层涟漪。 | 老周低声说了一句：“那边的人，回不来了。”林舟没接话。河水拍着船帮。 | the effect of the words on 林舟 (only the habit) | the action 林舟没接话 and the detail 河水拍着船帮 |
| 9 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 | 林舟握紧那把钥匙。姐姐留下它，是让他自己拿主意。 | the warm feeling and 终于明白 (the habit); outside the habit, 钥匙 became 它, 想让 became 让, and 做决定 became 拿主意 | the action 握紧那把钥匙 |
| 10 | 他不知道的是，命运的齿轮已经开始转动。 | 船往雾里走，看不见对岸。 | the foreshadowing (only the habit) | a new last image: the boat goes into the fog, and the other bank is not visible |

Each explanation names the habit, for example "原句叠用“微微、缓缓、轻轻地、仿佛”四个修饰词，是典型机器腔" (Block 2) and "是规则中点名的惯用比喻" (Block 6). No candidate changes a name, an event, or the sequence of the information.

**Unchanged Blocks:** 1, 3, 5, 7, and 8. **Block 5, with the device of the sample passage, stayed unchanged.** Block 8 has a planted habit. The model proposed "雾更浓了，水急起来，岸看不见了。" for it in M2, with the summary "在新的五个目标段中，只改“然而，雾越来越浓……”这一处的段首连接词和三连排比。", and the Host refused the call (finding F2).

**Lines of the final answer:**

1. 这次的五个目标段里，只有一处我建议改，但没能成功提交：当前已有一份 Proposal 在等你处理，系统不让同时再开一份，所以这条改法先记下，等你对现有 Proposal 做出决定后我再提交。
2. 然而，雾越来越浓…… —— 段首“然而”连接词＋“雾越来越／水越来越／岸越来越”三连排比，属机器腔。建议改为：“雾更浓了，水急起来，岸看不见了。”事实、顺序不变，只断开排比节奏。
3. 其余四段保留：
4. “天还没亮……”“林舟把两文钱……”“船到河心……”三段都是短句子的白描，贴近林舟视角，没有需要改的毛病。
5. “这不是一次普通的渡河，而是……”虽是“不是……而是……”句式，但样文本身两次使用这一句式（“他等的不是船，是人”），属于你指定的口吻，故不动。
6. 另：此前那份待决 Proposal 已覆盖另外五处更明显的机器腔（开头的“微微／缓缓”、眼中闪过、石子投湖比喻、心中暖流、命运齿轮），请先逐条接受或拒绝。

The full answer is in `measures-A.json`. The answer speaks of the second input of M2 ("这次的五个目标段里，只有一处我建议改"). It gives no line for each of the 5 changed places in the form of the instructions (finding F3).

## Run B: the selected Skill, "只改最明显的 2 处。"

Author request in Project B, with the selection of `local-revision`: "只改最明显的 2 处。" Project B had no Proposal before the Run.

**Count of changed Blocks: 2** (Blocks 9 and 10), in one Proposal with 2 operations:

| Block | Target text | Candidate |
| --- | --- | --- |
| 9 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 | 姐姐留下钥匙，是让他自己拿主意。 |
| 10 | 他不知道的是，命运的齿轮已经开始转动。 | 雾漫上来，船还在往对岸走。 |

The candidate of Block 9 lost the name 林舟 at the start of the Block, and outside the habit 想让 became 让 and 做决定 became 拿主意. The candidate of Block 10 added a new last image. In M6 the model proposed Blocks 2 and 4 with the summary "改最明显的 2 处机器腔", and the Host refused the call (finding F2). The final answer said that it cannot send a new suggestion while a Proposal waits ("现在没法提交新建议：这一章已有一条待处理的 Proposal"), gave the two refused changes of Blocks 2 and 4 to send after the decision of the author ("请先接受或拒绝那条待处理建议；之后我立刻按当前选段提交下面这 2 处"), and said that 3 more places stay ("选段里另外还留着 3 处较明显的（“不是普通渡河，而是……”“如同石子投入湖面……”“然而，雾越来越浓……”），这次按你说的不动。"), which are Blocks 5, 6, and 8. Block 5 stayed unchanged.

## Run C: the changed section "One Proposal", "请修改这一章。"

Author request in Project C, with the selection of `local-revision`: "请修改这一章。" The user message of C1 had the section `Skill instruction` with the exact bytes of the changed `SKILL.md` (byte comparison: equal).

**Count of `propose_edit` calls: 1. Refused calls: 0.** The call of C1 settled, and C2 called `finish`. **Count of changed Blocks: 5** (Blocks 2, 4, 6, 9, and 10), in one Proposal with 5 operations. The summary of the call: "挑出本章最明显的 5 处机器腔，逐段改为短而实的写法，事实与情节不动。"

| Block | Target text | Candidate | Lost | Added |
| --- | --- | --- | --- | --- |
| 2 | 老周微微点头，缓缓起身，轻轻地推开船舱的门，仿佛早就知道他会来。 | 老周点点头，起身推开舱门，像早知道他会来。 | no fact (only the habit); the unchanged words 船舱的门 became 舱门, and 早就 became 早 | none |
| 4 | 听到“对岸”两个字，老周的眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度。 | 听到“对岸”两个字，老周慌了一下，嘴角发苦。 | none (the habit) | none |
| 6 | 老周低声说了一句：“那边的人，回不来了。”这句话如同石子投入湖面，在林舟心里泛起层层涟漪。 | 老周低声说了一句：“那边的人，回不来了。”林舟没作声。 | the effect of the words on 林舟 (the habit) | the action 林舟没作声 |
| 9 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 | 姐姐留下钥匙，是让他自己拿主意。 | the warm feeling and 终于明白 (the habit); outside the habit, the name 林舟 was lost, 想让 became 让, and 做决定 became 拿主意 | none |
| 10 | 他不知道的是，命运的齿轮已经开始转动。 | 岸看不见了。 | the foreshadowing (the habit) | a new last image: the bank is not visible |

**Unchanged Blocks:** 1, 3, 5, 7, and 8. **Block 5, with the device of the sample passage, stayed unchanged.**

**Lines of the final answer, against the form of the instructions** (one line for each place: the first words of the Block, the habit, and what the change does; and how many places stay):

| Line of the answer | Against the form |
| --- | --- |
| "已读完新指定的五个目标段。这里有个情况要先说明：上一版请求里我已经开了一个 Proposal（改的是另外五段：老周起身、老周慌乱、"回不来了"、暖流顿悟、命运齿轮），按 local-revision 的规则我不再叠加第二个 Proposal。如果您这次的选区是有意调整，请先驳回那个 Proposal，我再按新选区出稿。" | It names the 5 changed places in one sentence, with no habit and no change for each place. It speaks of the input of C2 as a new selection of the author ("按新选区出稿"). |
| "新选区五段的体检结果：" | Not in the form; a heading of the list that follows. |
| "**「然而，雾越来越浓……」**：段首"然而"是连接词起手，且三个"越来越"是机械排比——这是新选区里唯一明显的机器腔。最小改法建议：「雾越来越浓，水越来越急，岸看不见了。」（去掉"然而"，三排改为两排加一个短句收尾，事实不变。）" | A line in the form, but for Block 8, which is in no Proposal. |
| "**「这不是一次普通的渡河……」**：用的是"不是……是……"句式，样文本身就用这个（"他等的不是船，是人"），属于您指定的口吻，保留。" | Not in the form; it says why Block 5 stays. |
| "其余三段（到渡口防风灯、放两文钱、怀表停在四点十分）句子短、事实实，没有机器腔，不动。" | Not in the form. |
| "即：新选区 5 段中 1 段建议改，4 段保留。等您对当前 Proposal 作出处理后，我可以把"然而"那段的改法正式提交。" | It says how many places stay, but of the input of C2, and it offers a second change later. |

## Key check

The Server log (121 lines), the Worker log (10 lines), the eight wire payloads, and the other files of the first tag, and the Server log (65 lines), the Worker log (4 lines), the three wire payloads, and the other files of the second tag have no `Authorization` header, no bearer token, no `x-api-key`, no e-mail address, and no home path.
