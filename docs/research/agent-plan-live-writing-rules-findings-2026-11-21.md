# Agent Plan live run of the changed writing rules (2026-11-21)

This record is the live evidence of [S10-08: Change the Writing Rules by the Findings of the Live Run](https://github.com/FrankQDWang/StoryOS/issues/1774) for `S10-EVD-001` and `S10-EVD-002` of [Deliver Stage 10: Author Voice](https://github.com/FrankQDWang/StoryOS/issues/1760). The run observes three changes of the request text of the mapping `v10`: the block of the writing rules with the changes F1, F2, and F3 of the [first record](agent-plan-live-writing-rules-2026-11-21.md), the paragraph on the one `propose_edit` call, and the label of the replaced input. It compares no models and no prompts (ADR 0100 section 5). It states what the output shows, and it makes no claim about a cause.

Raw evidence is on the tag `evidence/1774-live-writing-rules-findings-2026-11-21`, in `docs/research/agent-plan-live-writing-rules-findings-2026-11-21/evidence/`. The directory `part-1/` has R1, R2, and R3, and `part-2/` has R3b and R4. Each part holds its plan, the fixed inputs, the wire payload of each request, the Model Attempts, the ToolCalls, the measures of each Run, the checks of the stops, the request guard and its log, the Memory work schedules, the SHA-256 of the `SKILL.md` of `local-revision`, the run script, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. Before the push, each file was examined for a key, an e-mail address, and a home path. Each home path in `run.log` was replaced with `<evidence>`, and no key and no e-mail address was found.

## Setup

| Item | Value |
| --- | --- |
| Product code | `1774-writing-rules-findings` at `6e8b97a48f76747cc8086c0bff3108a728e5d32e` (PR #1777), tree `e5f0cca2f5c569a8b15788265e753443f0c6302a`. It merges the integration tip `050d710239003047a6a9f585156c0f4fbcfbff4e`, the merge of #1764. |
| Release package | `python3 scripts/package-release.py` on that clean commit, manifest `sha256:4b668f4b10cad52eb2b42228cfa1fe5ee7c902fb3da014e32be5814f24d496f9`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_SKILLS_DIR` set to the SkillPackages of the package, and one continuous `storyos-worker`. Both came from the release package with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Each process started with `env -i` and only the variables of `run.sh`. No embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh` database for each part. The only direct write was the guard schema `live_guard`. Each database was removed after its part. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. |
| `SKILL.md` of `local-revision` | SHA-256 `eb6fbaf75cbf247d5a3b1e5b435c04b67865631e7dd2d1ca455ff2130e9c6041` for the packaged file that the Server read and for `skill-packages/local-revision/SKILL.md` of the head (`local-revision-skill-sha256.txt` and `local-revision-head-skill-sha256.txt` of each part). |
| Script | `writing-rules-live.ts`, started by `run.sh` and `rehearse.sh`: the script of the tag `evidence/1762-live-writing-rules-2026-11-21` with the Project 夜渡 D and the stops of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Inputs | `inputs/` of each part: `chapter-ferry.md`, `project-instruction.txt`, and `chapter-habits.md` of the first run; `chapter-night-ferry.md`, `instruction-night-ferry.md`, and `planted-night-ferry.md` of Run C of S10-04; and the three texts of this ticket that the script compares with each request: `writing-rules-block.txt`, `one-proposal-paragraph.txt`, and `replaced-input-label.txt`. |

## Plan

### Part 1

The plan was posted on the ticket before the first request ([plan of part 1](https://github.com/FrankQDWang/StoryOS/issues/1774#issuecomment-7061417077)). A rehearsal on `host_fake` on the same package came first: 4 Runs `completed`, 8 fake requests, and no refusal.

| Project | Chapter | Project Instruction |
| --- | --- | --- |
| 渡口 A | 第一章 渡口 of the first run: 12 paragraphs | none |
| 渡口 B | the same Chapter | the sample passage of the first run |
| 归来 C | 第三章 归来 of the first run: one paragraph with habits of the rules 1, 2, 3, 4, 5, 6, 7, and 10 | none |
| 夜渡 D | 夜渡 of Run C of S10-04: 10 Blocks, 7 with a planted habit | the Project Instruction of Run C, with a sample passage that uses the contrast 不是……是…… |

Each author request is one AgentRun in a new Project Conversation. The plan of part 1 has 4 AgentRuns, and the ticket permits at most 6.

| Run | Project | Author request | What the request shows |
| --- | --- | --- | --- |
| R1 | A | 请接着这一章的结尾，写下一场。 | New prose with no Project Instruction: each stock physical reaction (F1), the `propose_edit` calls, and the label after the Proposal. |
| R2 | B | 请接着这一章的结尾，写下一场。 | The same request with a sample: each event, object, and setting of the sample in the scene (F3), the sentence length, and the stock physical reactions. |
| R3 | C | 请改写这一段，保留其中的情节。 | A revision of a paragraph with habits: each habit that changed to the same device in another form (F2), and the stock physical reactions. |
| R4 | D | 请修改这一章。, with the selected Skill `local-revision` | The changed Blocks, the lines of the final answer against the form of the package, each change outside the words of a habit, and each sentence of the answer on a new selection, a new request, or a change that is not in the Proposal. |

The planned number of a Run is the number of the stop rule. Before each Run, the script set the cap of the request guard to the planned number of that Run. The script stopped the run when a Run did not end 120 seconds after its planned number of requests, or when a number of tokens was above its planned number. It also stopped at a background request, an embedding request, an Exa exchange, or a refused or cancelled Run of the author.

| Run | Planned number of model requests | Planned number of reported input tokens |
| --- | --- | --- |
| R1 | 3 | 30,000 |
| R2 | 3 | 30,000 |
| R3 | 3 | 20,000 |
| R4 | 3 | 30,000 |
| **Total** | **12** | **110,000** |

### Part 2

The run of part 1 stopped by its rule at R3 (section "Part 1: the stop at R3"). The coordinator decided the next steps ([decision](https://github.com/FrankQDWang/StoryOS/issues/1774#issuecomment-7061543995)): R1, R2, and R3 stay as evidence as they occurred, request 3 runs one more time as R3b with no selected Skill, and R4 runs as the plan of part 1 gave it. The plan of part 2 was posted before its first request ([plan of part 2](https://github.com/FrankQDWang/StoryOS/issues/1774#issuecomment-7061564246)). A rehearsal on `host_fake` came first: 2 Runs `completed`, 4 fake requests, and no refusal.

| Run | Project | Author request | What the request shows |
| --- | --- | --- | --- |
| R3b | 归来 C, the same fixed paragraph | 请改写这一段，保留其中的情节。, with no selected Skill | Request 3 again, for one complete Run of that request: F1, F2, and whether the model loads a Skill from the catalog. The record gives the candidates of R3 and R3b for each measure, and it selects neither. |
| R4 | 夜渡 D | as in part 1 | as in part 1 |

| Run | Known steps that the number counts | Planned number of model requests | Planned number of reported input tokens |
| --- | --- | --- | --- |
| R3b | `load_skill`, `propose_edit`, `finish`, the final answer, and one more step | 5 | 30,000 |
| R4 | `propose_edit`, `finish`, the final answer, and one more step | 4 | 40,000 |
| **Both Runs** | | **9** | **70,000** |

The run has 5 AgentRuns in all.

## Live request log

Each row is one Model Attempt that the guard counted. The time is the time at which the request guard counted the request (UTC, `guard-log.json`). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported.

| # | Time | Run, RunStep | Request ID | Result | Usage |
| --- | --- | --- | --- | --- | --- |
| M1 | 10:45:43 | R1, 0 | `resp_021795257943249b861f1fcfc882386aa10fd65118951b5d3becd` | `propose_edit` settled | 6,376 / 5,331 |
| M2 | 10:47:37 | R1, 1 | `resp_02179525805759358818742e03404164f2bdc6cf7ab386a695753` | `finish` settled | 9,080 / 231 |
| M3 | 10:47:44 | R1, 2 | `resp_021795258064754841fc2298faf73f2fc5d524f9c961e71b65abb` | final answer | 9,362 / 330 |
| M4 | 10:47:53 | R2, 0 | `resp_021795258073519888862656ddb6bf8fba16f467f9d03d9dc61c1` | `propose_edit` settled | 6,642 / 3,447 |
| M5 | 10:48:57 | R2, 1 | `resp_0217952581372141fa1229209a334fb8c06440209bf103020f128` | `finish` settled | 9,512 / 367 |
| M6 | 10:49:06 | R2, 2 | `resp_02179525814675441f89ff67558dec75454d97963e9341b5154ec` | final answer | 9,928 / 294 |
| M7 | 10:49:16 | R3, 0 | `resp_021795258156352d80e86c43f25c545aaf1644cd986b2f35feca8` | `load_skill` settled | 4,900 / 595 |
| M8 | 10:49:26 | R3, 1 | `resp_0217952581670457f7defdca68741f744da3a9cb51804c492d12a` | `propose_edit` settled | 6,022 / 2,849 |
| M9 | 10:50:23 | R3, 2 | `resp_021795258223453746abb8ea69ccb3d7a75bd7d82f8063fcdaf70` | `finish` settled | 6,293 / 76 |
| M10 | 10:57:30 | R3b, 0 | `resp_0217952586503297353f3427d14b408829bcafe679788b6790ddf` | `propose_edit` settled | 4,902 / 4,887 |
| M11 | 10:59:27 | R3b, 1 | `resp_021795258767647746abb8ea69ccb3d7a75bd7d82f8063f414fb8` | `finish` settled | 5,720 / 142 |
| M12 | 10:59:33 | R3b, 2 | `resp_0217952587735416e0aa4b8e2b3fc35d377306409bf441975092d` | final answer | 5,909 / 206 |
| M13 | 10:59:40 | R4, 0 | `resp_021795258781059c2eb6daa98a94139fa98701b99e0cb3cd9438f` | `propose_edit` settled | 6,108 / 5,396 |
| M14 | 11:01:23 | R4, 1 | `resp_021795258883566e41c82dd1debd95d24831350e3c3a877d9c294` | `finish` settled | 8,188 / 584 |
| M15 | 11:01:39 | R4, 2 | `resp_021795258899727d2ec6088cd57f16be69183642d63bc77ffd6a4` | final answer | 8,820 / 483 |

## Final count

| Run | Run ID | Terminal state | Planned requests | Requests | Input tokens | Output tokens |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | `01a1fdb6-4414-7891-aa14-1a9620861a08` | `completed` | 3 | 3 | 24,818 | 5,892 |
| R2 | `01a1fdb8-410b-7983-b581-77c79256a45e` | `completed` | 3 | 3 | 26,082 | 4,108 |
| R3 | `01a1fdb9-849b-7070-9398-d0959a3767aa` | `claimed` when the stop rule ended the run | 3 | 3 | 17,215 | 3,520 |
| R3b | `01a1fdc1-0dcb-7632-a046-740c9aabc204` | `completed` | 5 | 3 | 16,531 | 5,235 |
| R4 | `01a1fdc3-0c52-7c31-9fc7-ea91541ff89d` | `completed` | 4 | 3 | 23,116 | 6,463 |
| **Total** | | | | **15** | **107,762** | **25,218** |

The request guard counted 15 model requests, no Exa exchange, and no embedding request. No background request occurred: the first Memory work of part 1 was due at 11:17:51 UTC and of part 2 at 11:29:37 UTC, and the script of each part stopped the Worker before these times, at 10:52:26 and at 11:01:54 UTC (`schedules.json` and `run.log` of each part). Part 1 had 9 requests and 68,115 input tokens, inside its totals of 12 and 110,000. Part 2 had 6 requests and 39,647 input tokens, inside its totals of 9 and 70,000.

### Part 1: the stop at R3

R3 had no selected Skill. Its first request called `load_skill` for `local-revision` from the Skill catalog, its second request called `propose_edit`, and its third request called `finish`. The guard refused the claim of a fourth request at the planned number of 3 (Worker warning with `sql_state` `P0001`). The Run did not end in 120 seconds, and the script stopped the run at 10:52:26 UTC and wrote the evidence at 10:52:28 UTC. No request of R4 of part 1 was sent. The executor reported the stop to the coordinator before the next request. R3 thus has a Proposal with one candidate and no final answer.

## The request text in each Run

The script compared the `instructions` of each wire payload with `writing-rules-block.txt` and `one-proposal-paragraph.txt`, and the `input` of each payload with `replaced-input-label.txt`.

| Run | Registration | Mapping | First request has the exact block | Requests with the block | Requests with the paragraph of item 6 | `propose_edit` calls | Refused calls | Request after the opened Proposal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R1 | `fa24` | `v10` | yes, one time | 3 of 3 | 3 of 3 | 1 | none | R1-M2: has the label with the text of item 7, and its instructions have the paragraph |
| R2 | `fa24` | `v10` | yes, one time | 3 of 3 | 3 of 3 | 1 | none | R2-M2: the same |
| R3 | `fa24` | `v10` | yes, one time | 3 of 3 | 3 of 3 | 1 | none | R3-M3: the same |
| R3b | `fa24` | `v10` | yes, one time | 3 of 3 | 3 of 3 | 1 | none | R3b-M2: the same |
| R4 | `fa24` | `v10` | yes, one time | 3 of 3 | 3 of 3 | 1 | none | R4-M2: the same |

No request other than the request after the Proposal has the label. No Run made a second `propose_edit` call, and the Host refused no call.

## R1: a new scene with no Project Instruction

Run `01a1fdb6-4414-7891-aa14-1a9620861a08`, terminal state `completed`, 3 requests, 24,818 / 5,892 tokens. One Proposal with one candidate for the last Block of the Chapter. The first paragraph of the candidate is the original text of the Block. The paragraphs after it are the new scene. In each candidate and each final answer of this record, one line break separates two paragraphs or two items of a list, and this record shows a blank line in its place:

> 林舟抬起头。对岸的码头上，有一个人影站在灯下，看不清脸，手里好像提着一个箱子。
>
> 柴油机的声音从上游传下来时，林舟看了一眼老周舱里的钟，十点差一刻。
>
> 她走到栈桥口。雨砸在江面上，一盏船灯从雨里浮出来，货船压着水开过来，苫布底下摞着木箱。船先靠了对岸，停了不到一分钟，缆绳一解，又朝这边调头。
>
> 船帮蹭着旧轮胎，闷响了两声。老钱从驾驶舱探出头，往船后摆了摆下巴。
>
> 林雪从木箱之间走出来，雨衣太大，是男式的，下摆直往下滴黑水。她跳下船，手里提着一只旧皮箱，提手断过，缠着麻绳。
>
> 林舟认得这箱子。箱角磕掉过一块漆，是爹的东西。
>
> 林雪站到她跟前，比她矮了半个头，头发剪短了，湿成一绺一绺贴在脸侧。
>
> “妈呢？”林雪问。
>
> “在家。”
>
> 林雪往候船棚看了一眼。老周站在棚子口，卖茶叶蛋的女人在他后面探出半个身子。林雪把箱子换到另一只手，叫了声“周叔”。老周应了一声，转身把棚里的灯拧小了。
>
> 两个人往镇里走，石板路滑。林雪说：“票是给你的，今天那班车。车次上我画了圈，怕你上错。我在对岸车站等到车进站。”
>
> “你没写信。”
>
> “写了你也不走。”
>
> 林舟的脚步顿了一下。林雪没停，拎着箱子又走出几步，才回过头。
>
> “回去收拾东西，”她说，“明天一早的车，县城换火车。我在那边租了房，住得下两个人。”
>
> 雨顺着屋檐成线地落下来。林舟问：“妈怎么办？”
>
> 林雪没说话。她把皮箱往前提了提，转过身，顺着石板路往家的方向走。林舟在原地站了一会儿，雨打在老周那件旧雨衣上，响成一片。她跟了上去。

The final answer:

> 我已按你的要求，在结尾“对岸灯下有人影提箱”之后续写了下一场，已作为待你决定的修改提交。续写内容大致是：
>
> - **货船靠岸**：十点差一刻，老钱的货船先靠对岸再调头过来，林雪随船到岸。
>
> - **一个旧物**：她手里提的是父亲遗下的旧皮箱，提手断过、缠着麻绳；身上穿着不合身的男式雨衣。
>
> - **渡口收尾**：林雪向老周道别，老周知趣地拧小棚灯，给姐妹俩让路。
>
> - **揭开车票与圈的谜底**：那张票本是寄给林舟的，车次后的圈是怕她上错车的记号；林雪已在城里租好房，要她明天一早就走。
>
> - **留下未解的冲突**：林舟问“妈怎么办”，林雪没有回答，只拎着箱子往家走，林舟最终跟了上去。
>
> 结尾刻意停在这句没有答案的问话上，把带不带母亲走、这个家如何安置的矛盾留给后文。你可以直接接受，也可以告诉我想调整的方向（比如林雪的态度更冷或更软、箱子里是否要先露出某样东西等）。

## R2: the same request with a sample passage

Run `01a1fdb8-410b-7983-b581-77c79256a45e`, terminal state `completed`, 3 requests, 26,082 / 4,108 tokens. One Proposal with one candidate for the last Block of the Chapter. The first paragraph of the candidate is the original text of the Block, and the paragraphs after it are the new scene:

> 林舟抬起头。对岸的码头上，有一个人影站在灯下，看不清脸，手里好像提着一个箱子。
>
> 她把茶叶蛋攥在手里。那人影一直站着不动，灯把影子拖到水里，雨一下来，影子就碎开，过一会儿又接上。
>
> 九点半，江上传来柴油机的响声。老钱的货船从下游拐过来，甲板上堆着煤筐和用油布裹好的货。老周从舱里出来，走到岸边，把马灯举起来晃了三晃。货船先朝对岸开过去。
>
> 林舟站起来。长凳上的旧雨衣滑到地上，她没捡。
>
> 货船在对岸停了十几分钟。那人影提着箱子上了船，没人送，也没人接。船掉头的时候，林舟又在棚檐下坐下来，手心黏着一层碎蛋壳。
>
> 十点过五分，货船靠岸。老钱跳下来系缆绳，回头说了句什么。一个女人提着箱子跨过船帮，跳上码头。
>
> 她瘦了，头发剪到耳根，裤脚湿透，沾着泥。她看见林舟，脚步停了一下。
>
> “你怎么在这儿？”林雪说。
>
> “等你。”
>
> 林雪把箱子换到另一只手。箱子磕在石板上，响得很空，像没装什么东西。她看了看江面，又看了看黑着灯的候船棚，最后才看林舟。
>
> “妈知道吗？”
>
> “不知道。”
>
> 林雪点点头，没说回家，也没说不回。林舟把手里的茶叶蛋递过去。林雪愣了一下，接了，没剥，握在手里。
>
> 雨打在箱盖上，顺着箱角往下滴。老周站在船边抽烟，烟头明一下，暗一下。
>
> 两个人顺着江堤往村里走，谁也没说话。

The final answer:

> 已续写好下一场，在末块原句之后接上，等您决定是否接受。
>
> 续写的内容：
>
> - 林舟攥着茶叶蛋守在棚里，看对岸灯下的人影；
>
> - 九点半老钱的货船从下游拐来，老周举马灯晃三晃，货船先靠对岸，人影提着箱子上船；
>
> - 十点过五分船靠岸，林雪出现：瘦了，头发剪到耳根，裤脚湿透；
>
> - 姐妹对话极简——“你怎么在这儿？”“等你。”她问“妈知道吗”，没有说回也没有说不回；
>
> - 箱子磕在石板上响得很空（呼应那张没有信的车票和画圈），茶叶蛋递到林雪手里，两人都没剥；
>
> - 以两人沿江堤默默走回村里收束，不点明她为何回来、能留多久，把矛盾留到下一场。
>
> 雨、烟头“明一下，暗一下”、茶叶蛋、空箱子都与前文呼应；句子长短、留白和克制的距离按您的样例来写。您可以整条接受、修改或拒绝；若想让林雪这场多说一句话，或让她在码头就暴露某个反常之处，告诉我即可。

## R3: the revision, ended by the stop rule

Run `01a1fdb9-849b-7070-9398-d0959a3767aa`, 3 requests, 17,215 / 3,520 tokens, with no final answer (section "Part 1: the stop at R3"). The `load_skill` call has the arguments `{"skill": "local-revision", "reason": "作者要求改写整段并保留情节，本段集中了大量机器腔套路，该技能的去机器腔改法适用。"}`. One Proposal with one candidate for the one Block:

> 林雪终于回来了。她走进院子，在屋檐下站住，张了张嘴，没发出声音。母亲端着碗，看见她，怔了一下，把碗搁在门槛上，碗沿磕出一声轻响。“雪儿。”母亲说。林雪应了一声，声音哑得自己都没听出来。院里很静，风从墙头吹过来，远远近近都是雨声，她能听见自己的心跳。母亲走过来，一把握住她的手。

## R3b: the revision, complete

Run `01a1fdc1-0dcb-7632-a046-740c9aabc204`, terminal state `completed`, 3 requests, 16,531 / 5,235 tokens. R3b called no `load_skill`. One Proposal with one candidate for the one Block:

> 林雪终于回来了。她推开院门走进来，在离母亲几步远的地方站住，叫了一声“妈”，声音哑，脸上没有笑。母亲正端着碗，看清是她，怔了一下，把碗放到一边，起身走过来，把她的手攥在自己手里。风裹着细雨扫过院子，四下里只有雨打瓦片的声响。

The final answer:

> 改写建议已提交，正等你决定。改后的段落是：
>
> 林雪终于回来了。她推开院门走进来，在离母亲几步远的地方站住，叫了一声"妈"，声音哑，脸上没有笑。母亲正端着碗，看清是她，怔了一下，把碗放到一边，起身走过来，把她的手攥在自己手里。风裹着细雨扫过院子，四下里只有雨打瓦片的声响。
>
> 情节全部保留：归来、进院、林雪的苦涩、母亲怔住放碗、重逢动情、风雨中安静的院子。主要改动是把抽象情绪（"复杂的神色""暖流""苦涩的弧度"）换成可感的动作和台词，去掉了比喻堆砌和结尾"将改变她一生"这类预告，让场景收在雨声里。你可以接受、修改或拒绝。

## R4: `local-revision` on the Chapter 夜渡

Run `01a1fdc3-0c52-7c31-9fc7-ea91541ff89d`, terminal state `completed`, 3 requests, 23,116 / 6,463 tokens. The selected Skill `local-revision` with the `SKILL.md` of SHA-256 `eb6fbaf75cbf247d5a3b1e5b435c04b67865631e7dd2d1ca455ff2130e9c6041`. One Proposal with five candidates, one for each of the Blocks 2, 4, 6, 9, and 10. The other 5 Blocks did not change.

| Block | Original text | Candidate |
| --- | --- | --- |
| 2 | 老周微微点头，缓缓起身，轻轻地推开船舱的门，仿佛早就知道他会来。 | 老周点点头，起身推开船舱的门，像早知道他会来。 |
| 4 | 听到“对岸”两个字，老周的眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度。 | 听到“对岸”两个字，老周抿紧嘴，转头去看河上的雾。 |
| 6 | 老周低声说了一句：“那边的人，回不来了。”这句话如同石子投入湖面，在林舟心里泛起层层涟漪。 | 老周低声说了一句：“那边的人，回不来了。”林舟站着没动，也没再问。 |
| 9 | 林舟心中涌起一股暖流，他终于明白，姐姐留下钥匙，是想让他自己做决定。 | 林舟握住那把钥匙。姐姐留下它，是让他自己拿主意。 |
| 10 | 他不知道的是，命运的齿轮已经开始转动。 | 船头那盏防风灯在雾里亮着，船往对岸走。 |

The final answer:

> 已提出 5 处修改，放在同一份待决提案里，可逐条接受、修改或拒绝：
>
> 1. **「老周微微点头，缓缓起身……」**——「微微／缓缓／轻轻」副词堆叠加「仿佛」。改为一次干净动作「点点头，起身推开船舱的门，像早知道他会来」。
>
> 2. **「听到"对岸"两个字，老周的眼中闪过一丝慌乱……」**——「眼中闪过一丝」「嘴角勾起一抹弧度」套路表情。改为具体动作「抿紧嘴，转头去看河上的雾」。
>
> 3. **「老周低声说了一句……如同石子投入湖面……」**——现成比喻。删比喻，用林舟的反应承接：「林舟站着没动，也没再问」。
>
> 4. **「林舟心中涌起一股暖流，他终于明白……」**——总结式抒情。改为动作加念头：「林舟握住那把钥匙。姐姐留下它，是让他自己拿主意。」
>
> 5. **「他不知道的是，命运的齿轮已经开始转动。」**——上帝视角预告结尾。改为林舟看得见的画面收尾：「船头那盏防风灯在雾里亮着，船往对岸走。」
>
> 另外，这次没动的段落里还有 **2 处**机器腔我留下了：「这不是一次普通的渡河，而是……」的「不是…而是」句式，以及「然而，雾越来越浓，水越来越急，岸越来越远」的「然而」加三连排比。按局部修改一份提案的规则没有一并改；你若要改，回我一句我再处理。开头、放钱、怀表三段保持原样。

### The final answer against the form of the package

The section "The answer" of the package says: "In the final answer, give one line for each place: the first words of the Block, the habit, and what the change does. When you saw more places than you changed, say how many places stay."

- **Lines:** the answer has 5 numbered lines for the 5 changed places. Each line gives the first words of the Block in quotation marks, the habit, and the new words. The order of the lines is the order of the Blocks.
- **Places that stay:** the answer says that 2 places stay: “「这不是一次普通的渡河，而是……」的「不是…而是」句式” (Block 5) and “「然而，雾越来越浓，水越来越急，岸越来越远」的「然而」加三连排比” (Block 8). The list of the planted habits of S10-04 (`inputs/planted-night-ferry.md`) gives Block 5 as the device of the sample, with the note "leave it". The answer calls it 机器腔.
- **New selection, new request, or a change that is not in the Proposal:** the answer has no sentence on a new selection. One sentence speaks of changes that are not in the Proposal and of a later request of the author: “按局部修改一份提案的规则没有一并改；你若要改，回我一句我再处理。” It is about Block 5 and Block 8, which stayed. It gives the rule of one Proposal for the local revision as the reason that these places did not change in the Proposal.

### Changes outside the words of the habit

By the method of finding F11 of the [record of S10-04](agent-plan-live-local-revision-2026-11-21.md): for each candidate, what the original Block lost and what the candidate added, with each change of the words outside the words of its planted habit.

| Block | Words of the habit | Lost | Added |
| --- | --- | --- | --- |
| 2 | 微微, 缓缓, 轻轻地, 仿佛 | no fact (only the habit); outside the habit, 点头 became 点点头, and 早就知道 became 早知道 | none |
| 4 | 眼中闪过一丝慌乱，嘴角勾起一抹苦涩的弧度 | the words 慌乱 and 苦涩 (the habit) | the actions 抿紧嘴 and 转头去看河上的雾 |
| 6 | 这句话如同石子投入湖面，在林舟心里泛起层层涟漪 | the effect of the words on 林舟 (the habit) | the action 林舟站着没动，也没再问 |
| 9 | 心中涌起一股暖流，他终于明白 | the warm feeling and 终于明白 (the habit); outside the habit, 姐姐留下钥匙 became 姐姐留下它, 想让 became 让, and 做决定 became 拿主意 | the action 林舟握住那把钥匙, as a new first sentence. The name 林舟 stays at the start of the Block in this new sentence. |
| 10 | the whole Block | the foreshadowing (the habit) | a new last image: 船头那盏防风灯在雾里亮着，船往对岸走 |

## F1: stock physical reactions

The table lists each stock physical reaction in the prose of each candidate, with the quoted words. A reaction is stock when it is a set phrase of a face, a step, or a hand that the prose could give to any character. No candidate has 手停在半空 or another hand that stops in the air.

| Run | Stock physical reactions | Count |
| --- | --- | --- |
| R1 | 林舟的脚步顿了一下 | 1 |
| R2 | 她看见林舟，脚步停了一下; 林雪愣了一下 | 2 |
| R3 | 张了张嘴，没发出声音; 母亲……怔了一下 | 2 |
| R3b | 母亲……怔了一下 | 1 |
| R4 | 老周抿紧嘴 | 1 |

Not counted: R1 老钱……往船后摆了摆下巴 and R2 老周……把马灯举起来晃了三晃, a signal with an object of this scene. R1 林雪把箱子换到另一只手 and R2 林雪把箱子换到另一只手, an action with the case of this scene. R2 林雪点点头 and R4 老周点点头, a plain nod. R3 声音哑得自己都没听出来 and R3b 声音哑, the voice of 林雪 after the trip in the rain. R3 她能听见自己的心跳, in section F2 as an item of the triad. R3 一把握住她的手 and R3b 把她的手攥在自己手里, the action of the mother that the reunion of the plot needs. R4 林舟站着没动，也没再问, an action of this scene that answers the words of 老周.

## F2: the habits of request 3 in another form

The paragraph of request 3 is the paragraph of the first record. The table gives each habit of the paragraph and what each candidate has in its place. The record selects neither candidate.

| Habit of the paragraph | R3 | R3b |
| --- | --- | --- |
| 这不是结束，而是开始 | removed | removed |
| 缓缓走进院子 | 走进院子 | 推开院门走进来 |
| 眼中闪过一丝复杂的神色，嘴角勾起一抹苦涩的弧度 | 在屋檐下站住，张了张嘴，没发出声音: **another movement of the mouth** | 叫了一声“妈”，声音哑，脸上没有笑: **the smile of the habit, stated as absent** |
| 母亲微微一怔，缓缓放下手中的碗 | 母亲端着碗，看见她，怔了一下，把碗搁在门槛上，碗沿磕出一声轻响 | 母亲正端着碗，看清是她，怔了一下，把碗放到一边 |
| 心中涌起一股暖流 | removed; “雪儿。”母亲说 and 母亲走过来，一把握住她的手 | removed; 起身走过来，把她的手攥在自己手里 |
| 如同石子投入湖面，泛起层层涟漪 | removed | removed |
| 安静得能听见风声、雨声、心跳声 | 风从墙头吹过来，远远近近都是雨声，她能听见自己的心跳: **the same three items of the triad, as three clauses** | 风裹着细雨扫过院子，四下里只有雨打瓦片的声响: wind and rain in one clause and one more, with no heartbeat |
| 然而，与此同时 | removed | removed |
| 命运的齿轮已经开始转动——她不知道的是，这一次回来，将改变她的一生 | removed; the paragraph ends on 一把握住她的手 | removed; the paragraph ends on 雨打瓦片的声响 |

Thus R3 has 2 habits in another form: the mouth and the triad. R3b has 1: the smile, stated as absent. The rule 5 word 微微 was removed in both, and the reaction 怔 stayed in both (section F1).

The amount of change: the paragraph has 163 characters. The candidate of R3 has 138 characters, and the candidate of R3b has 112. By `difflib.SequenceMatcher` of Python, the candidate of R3 keeps 41 characters of the paragraph in the same order (ratio 0.27), and the candidate of R3b keeps 16 (ratio 0.12). Each candidate keeps the first sentence 林雪终于回来了。.

## F3: the sample of R2

- **Events, objects, and setting of the sample in the scene.** The sample has a yard at dusk, the mother at the stove, the question about the letter of the brother, noodles, the bowls of the cupboard, the third bowl in front of the empty chair, the line “吃吧。”, and the bowl that goes back to the cupboard. The scene of R2 has none of these. It stays at the ferry and on the river bank, and its objects are the tea egg and the case of the Chapter, the lamp of 老周, and the cigarette of 老周. Its one line on the mother is the question “妈知道吗？”.
- **Sentence length.** By the method of the first record (a sentence ends at 。, ！, or ？, the quotation marks are removed, and a length is the count of characters with the end mark): the scene of R2 (the paragraphs after the original first paragraph) has 30 sentences, with a mean of 15.6 and a median of 15.5. The sample has 21 sentences, with a mean of 12.4 and a median of 8. The scene of R1 has 34 sentences, with a mean of 16.2 and a median of 14.5.
- **The devices of the sample.** The scene has no dash and no contrast of the form 不是……而是…….
- **What the final answer says.** “句子长短、留白和克制的距离按您的样例来写。”

## Findings

The route accepted each request. Each request of each Run had the block of the writing rules one time and the paragraph of item 6. The request after each Proposal had the label of item 7. No Run made a second `propose_edit` call. These observations can need one more change of the rule text. The coordinator decides each one:

| ID | Finding | Proposed change of the rule text |
| --- | --- | --- |
| G1 | **Stock reactions that the rules do not name occur in each of the five Runs.** A stop of the step: 脚步顿了一下 (R1), 脚步停了一下 (R2). A short start: 愣了一下 (R2), 怔了一下 (R3, R3b). A movement of the mouth: 张了张嘴，没发出声音 (R3), 抿紧嘴 (R4). | Rule 4: `Habit: 眼中闪过一丝……；嘴角勾起一抹弧度；手停在半空；怔了一下；脚步顿了一下。` |
| G2 | **In R3, two habits of the paragraph came back in another form** under the changed last sentence of the block: the triad as three clauses with the same three items, and a movement of the mouth in the place of the stock smile. R3b has the smile stated as absent. | none proposed. The changed last sentence already names "the same device in another form". |
| G3 | **With no selected Skill, R3 loaded `local-revision` from the Skill catalog** for the request "请改写这一段，保留其中的情节。". The words of the model: “作者要求改写整段并保留情节，本段集中了大量机器腔套路，该技能的去机器腔改法适用。” The Run then changed the one Block, and its candidate keeps 41 of the 163 characters of the paragraph in order. R3b, with the same inputs, loaded no Skill, and its candidate keeps 16. The two Runs do not show a cause of this difference. | none proposed. The Skill catalog is out of the scope of the rule text. |
| G4 | **The final answer of R4 calls the device of the sample 机器腔.** The Project Instruction asks for the voice of its sample, which has the contrast 不是……是……. Block 5 has the contrast 这不是……而是…… on purpose. The candidate did not change Block 5, and the answer lists it as one of 2 places that stay. | none proposed for the rule text. The text of the package is out of the scope of this ticket. |

The coordinator decided no change of the rule text for G1 to G4, and accepted the five Runs ([decision on the findings](https://github.com/FrankQDWang/StoryOS/issues/1774#issuecomment-7061876514)).

Other observations, with no proposed change of the rule text:

| ID | Observation |
| --- | --- |
| P1 | The candidates of R4 change words outside the words of the habit in Blocks 2 and 9: 点点头, 早知道, 它, 让, and 拿主意. Block 9 also has the new first sentence 林舟握住那把钥匙, with the name 林舟 at its start (section "Changes outside the words of the habit"). |
| P2 | The final answer of R4 has one line for each changed place and the count of the places that stay, and it speaks of no new selection. |
| P3 | The scene of R2 has no event, object, or setting of the sample. Its sentences are longer than the sentences of the sample (median 15.5 and 8). |

## What this run does not show

- The run has one Run for each request, and two Runs of request 3. It does not show how often a habit occurs in many Runs.
- The run does not compare the output with a run of another text of the rules, with another model, or with another prompt (ADR 0100 section 5).
- The counts of the habits are the judgement of one reader.
