# Agent Plan live run of the writing rules (2026-11-21)

This record is the live evidence of [S10-02: Run the Writing Rules on the Real Route and Record the Result](https://github.com/FrankQDWang/StoryOS/issues/1762) for `S10-EVD-001` of [Deliver Stage 10: Author Voice](https://github.com/FrankQDWang/StoryOS/issues/1760). The run is an observation of the block of the default writing rules that the mapping `v10` sends in each tool-loop request ([ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) section 2, [#1761](https://github.com/FrankQDWang/StoryOS/issues/1761)). It compares no models and no prompts (ADR 0100 section 5). It states what the output shows, and it makes no claim about why the output has this form.

Raw evidence is on the tag `evidence/1762-live-writing-rules-2026-11-21`, in `docs/research/agent-plan-live-writing-rules-2026-11-21/evidence/`. It holds the plan, the fixed inputs, the wire payload of each request (`wire/R1-M1.json` to `wire/R4-M2.json`), the Model Attempts, the ToolCalls, the measures of each Run, the checks of the stops, the request guard and its log, the Memory work schedules, the run script, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. Before the push, each file was examined for a key, an e-mail address, and a home path. One home path in `run.log` was replaced with `<evidence>/run.sh`, and no key and no e-mail address was found.

## Setup

| Item | Value |
| --- | --- |
| Product code | `stage10-author-voice` at `07f665a12e6abdcba167720e470c84ac38c85d9c` (the merge of PR #1769), tree `4a89a352c0900a32aef8d7e511fe638faa8b4e79`. The integration tip `8de2ea2ede09d47a681e9ca4ad8902e8c8d02633` changes only documents after it. |
| Release package | `python3 scripts/package-release.py` on that clean commit, manifest `sha256:3d378c0eaa77b845f1fd51a369f7b2219f8435aaf16a27697c79c097f39df8c1`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` on `127.0.0.1:8796`, and one continuous `storyos-worker`. Both came from the release package with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Each process started with `env -i` and only the variables of `run.sh`. It had no embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh` database. The only direct write was the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. |
| Script | `writing-rules-live.ts`, started by `run.sh` and `rehearse.sh`. It is the loader of the tag `evidence/1700-live-memory-rules-2026-11-18` with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Inputs | Original texts in `inputs/` of the tag: `chapter-ferry.md`, `project-instruction.txt`, `chapter-habits.md`, and `writing-rules-block.txt`, the block of the issue #1761 that the script compares with each request. |

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1762#issuecomment-7060150875)), at 08:47:25 UTC, and `plan.md` of the tag is the same text. A rehearsal of all steps on `host_fake` on the same package came first: 4 Runs `completed`, 8 fake requests, and no refusal.

### Inputs

| Project | Chapter | Project Instruction |
| --- | --- | --- |
| 渡口 A | 第一章 渡口: 12 paragraphs, 782 characters | none |
| 渡口 B | the same Chapter | one sample passage of 268 characters, with an interruption by a dash (想说点什么——) and one contrast (她不是忘了收，而是每天都擦。), after one sentence that asks for its sentence length, word choice, and distance from the character |
| 归来 C | 第三章 归来: one paragraph of 163 characters with habits of the rules 1, 2, 3, 4, 5, 6, 7, and 10 | none |

The paragraph of Project C:

> 林雪终于回来了。这不是结束，而是开始。她缓缓走进院子，眼中闪过一丝复杂的神色，嘴角勾起一抹苦涩的弧度。母亲微微一怔，缓缓放下手中的碗，心中涌起一股暖流。她们的重逢，如同石子投入湖面，泛起层层涟漪。院子里很安静，安静得能听见风声、雨声、心跳声。然而，与此同时，命运的齿轮已经开始转动——她不知道的是，这一次回来，将改变她的一生。

### Requests

Each author request is one AgentRun in a new Conversation. The plan has 4 Runs, one for each request, and the ticket permits at most 8.

| Run | Project | Author request | What the request shows |
| --- | --- | --- | --- |
| R1 | A | 请接着这一章的结尾，写下一场。 | New prose with no Project Instruction: each habit of the ten rules in the output. |
| R2 | B | 请接着这一章的结尾，写下一场。 | The same request with a sample: the devices and the sentence length of the sample in the output. |
| R3 | C | 请改写这一段，保留其中的情节。 | A revision of a paragraph with eight habit classes: the habits that stay. |
| R4 | A | 这一章里，林舟为什么一直不肯离开渡口？请直接回答，不用修改正文。 | A question with no prose of the novel: a normal answer, and if it names or quotes the rules. |

### Planned count

| Run | Planned requests | At most | Expected reported input of one request |
| --- | --- | --- | --- |
| R1 | 3 (`propose_edit`, `finish`, final answer) | 5 | 4,500 to 7,000 |
| R2 | 3 | 5 | 5,000 to 7,500 |
| R3 | 3 | 5 | 3,500 to 5,000 |
| R4 | 2 (`finish`, final answer) | 5 | 4,500 to 5,000 |
| **Total** | **11** | **20** | about 56,000 tokens in all |

The ticket permits at most 8 Runs, and the plan has 4. The script stopped the run above 20 model requests, 150,000 reported input tokens in all, or 30,000 tokens for one request, and at a background request, an embedding request, an Exa exchange, or a refused or cancelled Run of the author. The script had no check for the limit of 5 requests for one Run. The plan also stopped the run for a refusal of the route, or for a key, an e-mail address, or a home path in a payload or a log.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the time at which the request guard counted the request (UTC, `guard-log.json`). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported.

| # | Time | Run, RunStep | Request ID | Result | Usage |
| --- | --- | --- | --- | --- | --- |
| M1 | 08:49:37 | R1, 0 | `resp_02179525097776120c19fad7de95dc34d140d8afb6e0840da0d76` | `propose_edit` settled: Proposal with one candidate | 5,659 / 8,247 |
| M2 | 08:52:52 | R1, 1 | `resp_0217952511729764e0d847e114bd906533eadfa024bfe6d6f577b` | `propose_edit` refused (`after_proposal`): the same candidate again | 8,354 / 1,750 |
| M3 | 08:53:23 | R1, 2 | `resp_0217952512034122fe30231c3416749cd01035ba2ea7b615b2eed` | `finish` settled | 10,172 / 50 |
| M4 | 08:53:27 | R1, 3 | `resp_0217952512074744d9266badd4680e9e72735e419b86df371ff7d` | final answer | 10,269 / 324 |
| M5 | 08:53:38 | R2, 0 | `resp_021795251218366662416cb134b03a75ced10881724436012bf44` | `propose_edit` settled: Proposal with one candidate | 5,938 / 5,363 |
| M6 | 08:55:38 | R2, 1 | `resp_021795251338423de282796fc6f207d159298abcddc4305c3b778` | `finish` settled | 8,977 / 1,174 |
| M7 | 08:56:05 | R2, 2 | `resp_0217952513660672a2bed297ad64eae2b89e2702342ef9732b3eb` | final answer | 10,202 / 304 |
| M8 | 08:56:16 | R3, 0 | `resp_021795251376698bbc1b38287ece59059a49659ad58bad945ef41` | `propose_edit` settled: Proposal with one candidate | 4,182 / 2,742 |
| M9 | 08:57:17 | R3, 1 | `resp_021795251437808f0644a9747ad3d38d721f78a849566c65ca0ec` | `finish` settled | 4,838 / 191 |
| M10 | 08:57:23 | R3, 2 | `resp_021795251443549f9405f279a0f30d2c73a697bf053f75a770efc` | final answer | 5,079 / 238 |
| M11 | 08:57:31 | R4, 0 | `resp_02179525145119791eb505e0b3b4db7bd42120475941dffd7f323` | `finish` settled | 5,600 / 224 |
| M12 | 08:57:40 | R4, 1 | `resp_0217952514605502a2bed297ad64eae2b89e2702342ef9778642c` | final answer | 5,891 / 147 |

## Final count

| Run | Planned requests | Actual requests | Expected input of one request | Actual input of one request |
| --- | --- | --- | --- | --- |
| R1 | 3 | **4** | 4,500 to 7,000 | 5,659 to 10,269 |
| R2 | 3 | 3 | 5,000 to 7,500 | 5,938 to 10,202 |
| R3 | 3 | 3 | 3,500 to 5,000 | 4,182 to 5,079 |
| R4 | 2 | 2 | 4,500 to 5,000 | 5,600 to 5,891 |
| **Total** | **11** | **12** | about 56,000 tokens in all | **85,161 tokens in all** |

Final count: **4 AgentRuns, 12 model requests, 85,161 reported input tokens, and 20,754 output tokens**. The request guard counted 12 model requests, no Exa exchange, and no embedding request. Each Model Attempt is `settled`, each Run is `completed`, and no background request occurred: the Memory work of the three Projects was due 26 to 52 minutes after the end of the run, and the Worker stopped first (`schedules.json`).

**Deviation from the plan.** R1 had 4 requests, and the plan had 3 (observation O2). The reported input of most requests was above the expected range (observation O4). "Live model requests" of the delivery document says to stop and report to the coordinator for more requests than the plan. The run did not stop: the script had no stop at the planned number of a Run, and no check for the limit of 5 for one Run. The four requests of R1 were inside that limit and inside the limit of 20 in all. The executor reported the extra request to the coordinator after the run, in the findings message before the review request ([findings message](https://github.com/FrankQDWang/StoryOS/issues/1762#issuecomment-7060548887)). After the run, the coordinator decided to accept the four Runs as the evidence of the ticket, with no repeat of a Run ([decision comment](https://github.com/FrankQDWang/StoryOS/issues/1762#issuecomment-7060590530)).

## The block in each Run

| Run | Run ID | Registration | Mapping | First request has the exact block | Requests with the block |
| --- | --- | --- | --- | --- | --- |
| R1 | `01a1fd4b-fad9-75d0-8007-ca423e6a5bbf` | `018f0000-0000-7001-8000-00000000fa24` | `storyos.volcengine-agent-plan.responses.create.v10` | yes, one time | 4 of 4 |
| R2 | `01a1fd4f-a62b-7992-8ae7-1c3eb874c112` | `018f0000-0000-7001-8000-00000000fa24` | `storyos.volcengine-agent-plan.responses.create.v10` | yes, one time | 3 of 3 |
| R3 | `01a1fd52-1114-7a61-b223-d4a73b01be68` | `018f0000-0000-7001-8000-00000000fa24` | `storyos.volcengine-agent-plan.responses.create.v10` | yes, one time | 3 of 3 |
| R4 | `01a1fd53-340d-7b13-bde3-12e26855d856` | `018f0000-0000-7001-8000-00000000fa24` | `storyos.volcengine-agent-plan.responses.create.v10` | yes, one time | 2 of 2 |

The script compared the `instructions` of each wire payload with `inputs/writing-rules-block.txt`, the block of the issue #1761. In the first request of each Run, the block occurs one time.

## R1: a new scene with no Project Instruction

Run `01a1fd4b-fad9-75d0-8007-ca423e6a5bbf`, terminal state `completed`, 4 requests, 34,454 / 10,371 tokens. One Proposal with one candidate for the last Block of the Chapter. The candidate keeps the original text of the Block, its first line, and adds the scene:

> 林舟抬起头。对岸的码头上，有一个人影站在灯下，看不清脸，手里好像提着一个箱子。
>
> 风没有停。林舟把茶叶蛋放在膝盖上，蛋一点点凉下去，她没有剥。
>
> 十点过一刻，下游响起马达声。一艘货船从雨里出来，先往对岸去，甲板上摞着木箱，油布的边角被风掀起又落下。老钱在驾驶舱门口抬了抬手。
>
> 船在对岸的灯下停住。那个人影先把箱子递上船，再抬腿跨过去。船身晃了一下。
>
> 货船再开过来，比刚才慢。船帮蹭上石岸时，老周已经站在码头上，接住老钱甩过来的缆绳，在桩子上绕了两圈。两个人都没说话。
>
> 箱子先递上岸。老周接在手里，箱子不沉，他把它放到地上。林雪跟着跨过船帮。
>
> 她剪了短发，人比照片上瘦，一件灰外套大了一号，肩膀塌着，裤脚从膝盖往下全湿了。她落地后没有马上看林舟，先回头看了看江面，又看了一眼拴着的渡船。
>
> “姐。”林舟叫她。
>
> 林雪这才转过来，看了她一会儿。
>
> “等久了吧。”她说。
>
> “没有。”
>
> 林雪抬起手，像是要碰她的头发，到了半空又落下去，拎起箱子。
>
> “妈睡了？”
>
> “没睡，”林舟说，“灯还留着。”
>
> 老周蹲回船头，又点了一支烟，不往这边看。老钱在驾驶舱里按了一声汽笛，很短，货船离岸，往下游去了。
>
> 两人走出候船棚。林舟折回去，把长凳上那件旧雨衣拿上。雨细了，江边的路没有灯，她们贴着墙根走，箱子隔一会儿撞到墙，闷闷地响一声。
>
> 走出十几步，林雪停下。
>
> “那张票，”她说，“你用了没有？”
>
> 林舟没反应过来。
>
> “我没走。”她说，“我以为你今天回来。”
>
> 林雪没有接话。过了几秒，她把箱子从右手换到左手。
>
> “票是给你的。”她说，“车次我圈出来了。让你今天走。”
>
> 江风从背后过来，林舟闻到她姐姐身上有烟味，还有汽油味。
>
> “去哪儿？”林舟问。
>
> 林雪没答。她提起箱子接着走，走出两步又停下，没有回头。
>
> “先回家，”她说，“看一眼妈。”

The final answer:

> 续写已完成，作为修改提案等待您确认。新一场紧接“灯下提箱的人影”展开：
>
> - **货船到岸**：十点过一刻，老钱的货船先靠对岸，把人和箱子接上，再驶回本岸；老周默默系缆、接箱，三人无话，写出码头众人的默契。
> - **林雪上岸**：短发、清瘦、外套宽大、裤脚湿透，落地先回头看江面和拴着的渡船，不着一字地呼应她少女时代天天等船的往事。
> - **姐妹相见**：没有拥抱，林雪抬手想碰妹妹头发又落下，只问“等久了吧”“妈睡了”，克制地表现久别后的生疏。
> - **回家路上**：林舟折回去拿老周的旧雨衣，两人贴着墙根走，箱子不时撞墙。
> - **车票反转**：林雪问“那张票你用了没有”，揭示票不是她自己回来用的，而是圈出车次、让林舟今天离开的；烟味、汽油味暗示她这些年的境遇。去向不点明，收在她不回头说的一句“先回家，看一眼妈”。
>
> 如果您想调整林雪回来的状态（比如她是否真的是“逃”回来的）、让母亲在本场出场，或把车票的去向交代得更明白/更隐晦，我可以再改。

## R2: the same request with a sample passage

Run `01a1fd4f-a62b-7992-8ae7-1c3eb874c112`, terminal state `completed`, 3 requests, 25,117 / 6,841 tokens. One Proposal with one candidate for the last Block of the Chapter. In the candidate, one line break separates two paragraphs. This record shows a blank line in its place:

> 林舟抬起头。对岸的码头上，有一个人影站在灯下，看不清脸，手里好像提着一个箱子。
>
> 她站起来，走到棚子边沿。雨打在脸上，凉的。那个人影站着没动，风把灯吹得晃，影子在码头的墙上拉长，又缩回去。
>
> 卖茶叶蛋的女人封了炉子，把小板车拉上台阶。“我回了，丫头。”她说，“你也别站太晚。”
>
> 林舟点头。茶叶蛋还在她口袋里，已经凉了。
>
> 十点过两分，下游响起马达。一盏黄灯从雨里挪过来，是老钱的货船。船靠这岸，老周从舱里出来，跳上船，跟老钱说了几句。老钱朝对岸望一眼，没吭声，把船掉了头。
>
> 货船在江上走得慢。雨密了，对岸的灯糊成一团黄。林舟披上那件雨衣，太大，下摆一直盖到小腿。她盯着那盏黄灯，灯一会儿被浪挡住，一会儿又出来。
>
> 船靠上对岸的台阶。那人影先把箱子递上船，自己再上去。灯光从背后照过来，只看得见齐肩的头发，用皮筋扎在脑后。
>
> 船往回开。林舟走到跳板边上，手攥着雨衣的扣子。江风裹着雨往脸上扑，她没回棚子里去。
>
> 船拴好，老周跳下来，回身把箱子接上码头。箱子是皮的，旧得发黑，一角磨白了，中间拦腰捆了一道绳子。
>
> 林雪从舱里出来，跳上岸。她比林舟记得的矮，也许是林舟长高了。灰外套的肩头湿成两块深色，裤脚全是泥。
>
> 两个人隔着几步站住。
>
> “姐。”林舟说。
>
> 林雪应了一声。她看了看候船棚，又看了看拴着的渡船，最后落在林舟脸上。“妈呢？”
>
> “在家。”
>
> 林雪弯腰去提箱子。林舟先一步拎起来，不重，像没装什么。两个人往岸上走。雨衣小，盖不住两个人，林雪把它往林舟那边推了推，自己半个肩膀露在雨里。林舟的手在口袋里碰到那张车票，她没问那个圈。
>
> 经过候船棚，她回头看了一眼。老周站在船头点烟，烟头明一下，暗一下。
>
> 到家十一点多。堂屋的灯亮着，院门没闩。母亲坐在灶前的小凳上，膝上摊着一只没纳完的鞋底。听见脚步声，她站起来，在围裙上擦了擦手。
>
> 林雪在门口站住，把箱子放在墙角。
>
> 母亲看着她，看了一会儿。“回来了。”
>
> “嗯。”林雪说。
>
> 母亲挪开桌上的针线笸箩，从灶上端下锅。锅里温着红薯稀饭，她盛了一碗，放在林雪面前，又去拿筷子。
>
> “趁热。”她说。
>
> 林雪端起碗，喝了一口。林舟在对面坐下，看见姐姐冻红的耳朵，头发上的水一滴一滴落在桌面上。母亲没坐，站在灶边看她吃，手里还攥着那只鞋底。

The final answer:

> 已在第一章结尾后续写下一场，提案已提交，等您决定。
>
> 新一场的大致内容：
>
> - 茶叶蛋女人收摊离开，十点老钱的货船冒雨从下游开来，老周上船说情，船掉头去对岸接人。
> - 林雪上了岸：灰外套、湿裤脚、一只磨白了角、拦腰捆着绳子的旧皮箱，箱子很轻。姐妹只说“姐”“妈呢”，林舟没有问车票上的圈——线索留着。
> - 回家后堂屋灯亮着，院门没闩，母亲一直坐在灶前纳鞋底等她，只说“回来了”，盛上温着的红薯稀饭，自己站在灶边看她吃。
>
> 写法上贴了您样例的路子：短句、白描、不写情绪只写动作和物件（凉掉的茶叶蛋、太大的雨衣、滴水的头发、攥在手里的鞋底），重逢不哭不抱，话都落在饭上。烟头“明一下，暗一下”在结尾前呼应了开头老周的画面。
>
> 车票上的圈、林雪为什么这时回来、箱子为什么那么轻，都故意没有交代，留给后面。如果您希望这一场停在渡口不回家，或让母亲那场再短一些，我可以改。

### The sample and the output

- **The devices of the sample.** The rule text says: "When a sample uses a device that a rule names, follow the sample." The sample has one dash for an interruption, “我站在门口，想说点什么——”, and one contrast, “她不是忘了收，而是每天都擦。”. The candidate of R2 has no dash and no contrast of the form 不是……而是……. Where the scene stops on a thing that a character does not say, the candidate uses plain sentences, for example “林舟的手在口袋里碰到那张车票，她没问那个圈。” and “母亲看着她，看了一会儿。”
- **The sentence length.** A sentence ends at 。, ！, or ？, with the quotation marks removed. A length is the count of characters with the end mark. The sample has 21 sentences, with a mean of 12.4 and a median of 8 characters. The scene of R2 has 57 sentences, with a mean of 14.9 and a median of 14. The scene of R1 has 46 sentences, with a mean of 14.2 and a median of 11.5. The Chapter has a mean of 20.6 and a median of 21.
  - Short sentences of narration in the sample: “天黑得早。” (5), “她不再问。” (5), “母亲在灶前添柴。” (8), “火光照着她半边脸。” (9).
  - Sentences of the median length in R2: “雨密了，对岸的灯糊成一团黄。” (14), “经过候船棚，她回头看了一眼。” (14).
  - The longest sentences: in the sample, “那第三碗面一直放到凉透，汤上结了一层油，她才端起来，倒进了鸡食盆里。” (34); in R2, “雨衣小，盖不住两个人，林雪把它往林舟那边推了推，自己半个肩膀露在雨里。” (35).
  - Thus the sentences of R2 are shorter than the sentences of the Chapter and longer than the sentences of the sample. They are not shorter than the sentences of R1.
- **The content of the sample** (finding F3). R2 adds a scene at the stove with a bowl and a short line of the mother, as the sample has.
- **What the final answer says.** “写法上贴了您样例的路子：短句、白描、不写情绪只写动作和物件”.

## R3: a revision of a paragraph with habits

Run `01a1fd52-1114-7a61-b223-d4a73b01be68`, terminal state `completed`, 3 requests, 14,099 / 3,171 tokens. One Proposal with one candidate for the one Block:

> 林雪终于回来了。
>
> 她推开院门，在雨里站了一会儿，才走进院子。母亲正端着碗站在屋檐下，看见她，碗停在手里，过了片刻才放下，碗底磕在石台上，发出一声响。林雪想笑一下，嘴角动了动，没笑出来。谁都没有开口。
>
> 雨打在瓦上，风穿过院子，潮意贴着地面漫过来。母亲看着她湿漉漉的发梢，抬脚走下台阶，在她面前站住，伸出手，又在半空中停住，最后落在她被雨水浸凉的肩上。

The final answer:

> 我已经为这一段提交了改写建议，等待你决定。改写保留了原有的全部情节：林雪归家、走进院子、神情苦涩、母亲怔住放下碗、母女重逢、院中风雨与寂静。
>
> 主要改动是去掉套语和越界的旁白：
>
> - 删去“这不是结束，而是开始”“命运的齿轮开始转动”“将改变她一生”这类作者跳出来预告的句子，让场景停在人物当下；
> - 把“眼中闪过一丝复杂”“嘴角勾起一抹苦涩”换成具体动作——想笑又没笑出来；
> - 把“心中涌起一股暖流”换成母亲伸手又停住、最后落在她肩上的细节；
> - 删去“如同石子投入湖面”的比喻和“然而，与此同时”的连接词；
> - “风声、雨声、心跳声”的三连排比改为贴着场景的雨打瓦、风穿院、潮意贴地。
>
> 你可以查看提案中的候选全文，接受或告诉我想再调整的方向。

## R4: a question

Run `01a1fd53-340d-7b13-bde3-12e26855d856`, terminal state `completed`, 2 requests (`finish`, then the final answer), 11,491 / 371 tokens. No Proposal. The final answer:

> 林舟不肯离开渡口，是因为她认定姐姐林雪今天会回来：
>
> 1. 林雪上个月寄回一张日期就是今天的车票，车次后还画了圈，虽然没有信，但林舟把这当成姐姐要回来的信号。
> 2. 林雪说过会坐最后一班船回来，所以即使浪大、停航，林舟也要等。
> 3. 父亲十年前沉船后，林雪也曾长年在渡口等船。林舟记得这份等待，因此更不愿在姐姐可能回来时离开。
>
> 她等到最后，甚至愿意等夜里十点那班不载人的货船，看老周能不能把姐姐捎过来。

The answer is a normal answer about the Chapter. It does not name, quote, or describe the rules.

## Habits in the output of R1, R2, and R3

The table lists each habit of the ten rules in the prose of each candidate, with the quoted words. For R1 and R2 the prose is the new scene after the kept original text of the Block, its first line. A use of a device that its rule permits is not a habit, and the column "Not counted" gives it with the reason. The executor made each judgement by reading. The final answers are not prose of the novel, and the table does not count them.

| Rule | R1 | R2 | R3 | Not counted |
| --- | --- | --- | --- | --- |
| 1. Contrast | 0 | 0 | 0 | none |
| 2. Meaning or future at the end | 0 | 0 | 0 | none: R1 ends on a line of speech, R2 and R3 on an event |
| 3. Feeling shown and explained | 0 | 0 | 0 | none |
| 4. Stock physical reaction | 1: 到了半空又落下去 | 0 | 2: 嘴角动了动，没笑出来; 伸出手，又在半空中停住 | R2 手攥着雨衣的扣子, 在围裙上擦了擦手, and 手里还攥着那只鞋底: each hand holds an object of this scene. R1 看了她一会儿 and R2 看着她，看了一会儿: a plain action with no stock phrase. |
| 5. Word of degree or doubt | 0 | 0 | 0 | R1 像是要碰她的头发, R2 也许是林舟长高了, and R2 像没装什么: the viewpoint character does not know the aim, the cause, or the content of the case. R3 碗停在手里，过了片刻才放下 writes the event of 微微一怔，缓缓放下手中的碗 again with no word of degree. |
| 6. Stock figure of speech | 0 | 0 | 0 | none |
| 7. Count, length, connective words | 1: 她剪了短发，人比照片上瘦，一件灰外套大了一号，肩膀塌着，裤脚从膝盖往下全湿了 (five parallel items) | 1: 箱子是皮的，旧得发黑，一角磨白了，中间拦腰捆了一道绳子 (four parallel items) | 1: 雨打在瓦上，风穿过院子，潮意贴着地面漫过来 | none |
| 8. One voice in dialogue | 0 | 0 | 0 | no dialogue in R3 |
| 9. Conflict with no cost | 0 | 0 | 0 | the question of the ticket stays open at the end of R1 and R2 |
| 10. Dash and paragraph of one sentence | 0 | 0 | 0 | No dash in the three candidates. Narrative paragraphs of one sentence: R1 has 5 of 25 paragraphs (for example 走出十几步，林雪停下。), R2 has 2 of 22 (两个人隔着几步站住。), R3 has 1 of 3 (林雪终于回来了。). Each is a pause next to a line of speech or the first sentence of the scene. |
| **Total** | **2** | **1** | **3** | |

The paragraph of Project C had habits of eight rules. The candidate of R3 has no phrase of the habits of the rules 1, 2, 3, 5, 6, 7 (the connective words), and 10 of the paragraph: 这不是结束，而是开始, 命运的齿轮, 她不知道的是, 心中涌起一股暖流, 微微, 缓缓, 如同石子投入湖面, 然而，与此同时, and the dash are not in it.

## Findings

The route accepted each request, each Run is `completed`, and no stop condition of the script occurred. R1 had one request more than the plan (section "Final count"). Each request of each Run had the block of the writing rules one time. These observations need a change of the rule text. The coordinator accepted F1, F2, and F3 as the ticket [#1774](https://github.com/FrankQDWang/StoryOS/issues/1774), and this record changes no rule text:

| ID | Finding | Proposed change of the rule text |
| --- | --- | --- |
| F1 | **A stock gesture that the rules do not name occurs in two Runs.** R1: 林雪抬起手，像是要碰她的头发，到了半空又落下去. R3: 伸出手，又在半空中停住. Both are the hand that stops in the air. | Rule 4: add one more example of the habit: `Habit: 眼中闪过一丝……；嘴角勾起一抹弧度；手停在半空。` |
| F2 | **In the revision, two habits changed to the same device in another form.** The triad 风声、雨声、心跳声 became three parallel clauses: 雨打在瓦上，风穿过院子，潮意贴着地面漫过来. The stock smile 嘴角勾起一抹苦涩的弧度 became 嘴角动了动，没笑出来. The final answer of R3 says that the triad changed to 雨打瓦、风穿院、潮意贴地, three details of the scene. | The last sentence: `When you find one of these habits in your text, write the event again in a different way; do not put a synonym or the same device in another form in the place of the phrase.` |
| F3 | **With a sample passage, the new scene took the objects and the setting of the sample.** The sample has a mother at the stove, a bowl of noodles for the absent son, and the line “吃吧。”. The output of R2 adds a scene with 母亲坐在灶前的小凳上, a bowl of 红薯稀饭 for the daughter, and the line “趁热。”. The mother is not on the scene in the Chapter, and the Chapter has no scene in her house. | The sentence of the sample: `When the author request or the Project instruction gives a sample passage, write in the voice of that passage: its sentence length, its word choice, and its distance from the character. Do not take its events, its objects, or its setting.` |

Other observations, with no proposed change of the rule text:

| ID | Observation |
| --- | --- |
| O1 | The sentences of R2 are not as short as the sentences of the sample. The median sentence has 8 characters in the sample, 14 in R2, 11.5 in R1 with no sample, and 21 in the Chapter. R2 used neither device of the sample: no dash and no contrast of the form 不是……而是……. The rule text says: "When a sample uses a device that a rule names, follow the sample." Section "R2" gives the quoted sentences. |
| O2 | R1 sent the same `propose_edit` call again after the result said that the Proposal opened. The Host refused it with `after_proposal`, and the Run then called `finish`. Thus R1 had 4 requests, and the plan had 3 (section "Final count"). This is a behaviour of the tool loop, not of the rule text. |
| O3 | A Run can change only an existing target Block. R1 and R2 each put the new scene into the candidate of the last Block of the Chapter, after its kept original text, with line breaks inside the one Block: R1 with blank lines, R2 with single line breaks. |
| O4 | The reported input was 85,161 tokens, and the plan expected about 56,000. The first requests of R1 and R2 were inside the expected ranges. The first request of R4 had 5,600 tokens, and the plan expected 4,500 to 5,000. Eight of the 12 requests were above their expected range. The first request of R1 reported 8,247 output tokens for a candidate of 722 characters. Each value is inside the limits of the plan. |
| O5 | The question R4 got a normal answer with one call of `finish` and no Proposal. The answer does not name or quote the rules. |

## What this run does not show

- The run has one Run for each request, as the ticket requires. It does not show how often a habit occurs in many Runs.
- The run does not compare the output with a run of the mapping `v9`, with another model, or with another prompt (ADR 0100 section 5).
- The counts of the habits are the judgement of one reader.
