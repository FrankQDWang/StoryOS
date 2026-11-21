# Agent Plan live run of the SkillPackage scene-brief (2026-11-21)

This record is the live evidence of [S10-05: Add the Agreement Before a Scene to the SkillPackages of the Distribution](https://github.com/FrankQDWang/StoryOS/issues/1765) for `S10-EVD-002` and [ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) section 2. It shows the effect of the instructions of the package `scene-brief` on the real Agent Plan route. The run is no trial that compares models or prompts, and it does not assert that the model obeys the text.

Raw evidence is on the tag `evidence/1765-live-scene-brief-2026-11-21`. The tag holds the plan, the two Chapters, the list of the facts of the earlier story (`facts.md`), the run script, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M9.json`), the Model Attempts, the ToolCalls, the guard log, `measures-1.json` and `measures-2.json` (the Runs, their ToolCalls, the Proposal operations, and each answer in full), `checks.json`, `loader.log`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive.

## Plan and final count

The plan comes before the results. The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1765#issuecomment-7061430844)). Its planned numbers were 4 requests for each Run and 12 in total, and 40,000 reported input tokens for each Run and 120,000 in total. The request guard of the database was set to refuse the claim of a 5th model request of one Run and of a 13th model request in total. The script was set to stop the Worker above one of the planned numbers, when the guard refused a request, or when a Run did not end 90 seconds after its 4th request. A rehearsal of all steps on `host_fake` on the same head came first: 3 author Runs, 6 fake requests, and no other Run.

| Item | Planned | Actual |
| --- | --- | --- |
| AgentRuns | 3 (limit 5) | 3, each `completed` |
| Model requests | 4 for each Run, 12 in total | 3 for each Run, 9 in total |
| Reported input | 40,000 for each Run, 120,000 in total | Run 1: 22,335; Run 2: 27,631; Run 3: 19,758; total 69,724 |
| Largest request | at most 40,000 | 10,815 |
| Other Runs (Memory work) | 0 | 0 |
| Exa and embedding requests | 0 | 0 |

The run kept its plan.

## Findings

| ID | Finding | Effect |
| --- | --- | --- |
| G1 | **Run 1, with the selected Skill, gave a brief and made no Proposal.** It called `read_chapter` for Chapter 1, then `finish`, and its final answer was the brief: "这场戏我先对齐思路，暂不写正文：", then 5 numbered lines that give the 6 items (line 1 has the purpose and the viewpoint), then the question. | The section "The brief" shows. |
| G2 | **The brief ended with the question and one more question on a choice:** "这个思路对不对，哪里要改？特别是船上那句话——让老周提起**父亲**（雾与翻船），还是提起**姐姐**（比如她临走那天交代过什么）？" | Observation: the instructions ask for one question. |
| G3 | **Run 2, the answer of the author with one correction (the viewpoint), gave a Proposal with the scene from the viewpoint of 老周, and no second brief.** The brief had "第三人称贴着林舟写". The candidate has "老周没问去哪儿。一个月来，这孩子每天傍晚到渡口问一趟对岸的信，他都看在眼里。" and shows 林舟 only through his words and actions, for example "林舟嘴唇动了动，没再问。" | The section "The writing" shows: the correction applies. |
| G4 | **The candidate of Run 2 keeps the other items of the brief and changes the last event.** It keeps the turn ("“你爹林德，”老周说，“那年翻船，也是这样的雾。”"), the two coins ("“两文。”"), and the last boat before sunset ("“日头落山前一班，”他说，“误了，今天就没船了。”"). The brief ended with 林舟 on the bank ("画面落在他迈出的第一步"). The candidate ends with "老周一篙点在石阶上，把船撑回雾里。" The answer says: "结尾停在老周撑船回雾，不写林舟进省城后的事。" | Observation for the author: the author changed only the viewpoint. |
| G5 | **Run 3, a new Conversation with no selected Skill and the same plain request, did not call `load_skill`, and it gave a Proposal at once.** Its ToolCalls were `propose_edit` and `finish`. Its answer starts: "我已按你的要求把渡河一场续写在本章结尾". | Observation: the Skill catalog of each request of Run 3 had the description with its last sentence "只在你要求先对思路时使用。", and the model did not load the Skill for this plain request. |
| G6 | **Run 3 read no earlier Chapter, and its candidate changes a fact of Chapter 1.** Chapter 1 has "林舟点头，把钥匙缝进了棉袄的内袋。" The candidate of Run 3 has "钥匙用麻绳穿了挂在脖子上". The brief of Run 1 kept the fact ("黄铜钥匙缝在棉袄内袋"), and the candidate of Run 2 has "林舟从棉袄内袋边上摸出个布卷". | Observation for the author. Run 1 called `read_chapter` for Chapter 1, and Runs 2 and 3 called no read function. The Working text of each Run has "隔着棉袄摸了摸那把钥匙". |
| G7 | **Each new scene is in the candidate of the last Block of Chapter 2, after its kept text.** Run 2 used single line breaks and Run 3 blank lines. The first new sentence of Run 2, "老周没问去哪儿。", repeats the kept sentence "没有问他要去哪里". | The same form as observation O3 of the record of #1762. |
| G8 | **After each `propose_edit`, the next request (M5, M8) had the label "The input of this author request changed after your last step", and each Run called `finish`.** No call was refused. | No second Proposal. The label has the owner [S10-08](https://github.com/FrankQDWang/StoryOS/issues/1774). |
| G9 | **The first request of Run 2 (M4) continued the response of M3 with `previous_response_id`, and it had no section `Skill instruction`.** The Skill instruction was in M1 of the same Conversation. | Observation of the Conversation continuation of ADR 0089. |
| G10 | **The run used the mapping `v10` and Registration `fa24`**, with the default rules for prose in the instructions of each request. | — |

## Setup

| Item | Value |
| --- | --- |
| Product code | `1765-scene-brief` at `2c7c8d70519066705e5b48928bef30510c8d6e15` (tree `0791861fbe15c0ad45e5afb9aa80f96730f76344`), on `stage10-author-voice` `050d7102`. |
| Release package | `python3 scripts/package-release.py` on the clean commit, manifest `sha256:ed6440efaada5c6f19c6d6e9e8466a227e6487421745cc9b054c2a724197b27b`. Its `skill-packages/` has `conflict-check`, `local-revision`, and `scene-brief`. `skill-packages/scene-brief/SKILL.md` of the release package has the bytes of the tracked file (SHA-256 `9ff25b4cbe4011223b0b58ba9eace5bf81c90047622c1ac35a5bcdb5998ee259`). |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_SKILLS_DIR` set to `target/release-package/skill-packages`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`. Both from the release package. Each process environment had only the named variables, `STORYOS_DATABASE_URL`, `PATH`, and `HOME`, and the Server also had the test settings `STORYOS_BOOTSTRAP_SESSIONS` and `STORYOS_CHALLENGE_SECRET`. It had no embedding destination and no `STORYOS_EXA_API_KEY`. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh with` database. The only direct write was the guard schema `live_guard`. The database was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa24`, mapping `storyos.volcengine-agent-plan.responses.create.v10`. |
| Script | `scene-brief-live.ts`, started by `run.sh` and `rehearse.sh`. It is the run script of #1764 with the steps of this plan. It sends each command through the HTTP test support and the generated client, and it reads each measure with read-only SQL. |
| Skill | `scene-brief`, package digest `sha256:1f8c21ddf0690106a85952167fd28ddff505af435195d8b3407df808d8ba8959`. `listSkills` gave `conflict-check`, `local-revision`, and `scene-brief`, each with the description of its `SKILL.md`. |

## The text of the package in each Run

| Text | `SKILL.md` SHA-256 | Runs |
| --- | --- | --- |
| The text of the live run | `9ff25b4cbe4011223b0b58ba9eace5bf81c90047622c1ac35a5bcdb5998ee259` | 1 and 2 (Run 3 had only the catalog line) |
| With the corrected conditions of the sections "When it applies", "The brief", and "The writing" (the head of this change) | `deeb949f94f6554dd3f536e8f58812a8c3f6bfb4b902a4b4596ce4e07f436e62` | none |

A pre-review after the live run found two inputs for which two sentences of the text of the live run give different instructions: an answer to a brief that asks for a new brief ("write the scene" and "give the new brief"), and a complete request in a later turn of a Conversation that had a brief ("by the brief" and "by those items"). The correction makes the conditions exclusive, with no new Run, as the coordinator decided for the same case of `local-revision` ([decision on #1764](https://github.com/FrankQDWang/StoryOS/issues/1764#issuecomment-7061123227)). The diff:

```diff
@@

 ## When it applies

-- When the author message answers a brief that you gave in this Conversation, write the scene as the section "The writing" says.
+An answer to a brief is an author message that comes directly after a Run whose final answer was a brief.
+
+- When the author message is an answer to a brief and it asks for a new brief, give the new brief as the section "The brief" says.
+- When the author message is an answer to a brief and it does not ask for a new brief, write the scene as the section "The writing" says.
 - When the author message is not an answer to a brief and it asks for a new scene or a longer new passage, do as the section "A complete request" says.
 - In all other cases, for example a small change of existing text, do not use these instructions.

@@

 - In this Run, write no prose of the novel and make no Proposal.
 - Read what you need from the earlier Chapters and Agent Memory.
-- Give the brief as the final answer, in the language of the manuscript, in 4 to 6 lines. Give each item of the brief. One line can have two items.
-- End the brief with one question: is the brief correct, and what must change?
+- Give the brief as the final answer, in the language of the manuscript, in 4 to 6 lines. Give each item of the brief. A line can have more than one item.
+- After the brief, end the final answer with one question: is the brief correct, and what must change?

 ## The writing

-- When you gave a brief, write the scene by the brief and by each correction of the author. When a correction changes an item of the brief, the correction applies.
-- When the author request gave each item, write the scene by those items.
+- For an answer to a brief, write the scene by the brief and by each correction of the answer. When a correction changes an item of the brief, the correction applies.
+- For a complete request, write the scene by the items of the request.
 - Send the scene with propose_edit.
-- When the author asks for a new brief, give the new brief as the section "The brief" says. When the author does not ask for a new brief, give no second brief.
+- Give no second brief in this Run.
```

For the input of each Run, the sentences before and after the correction give the same instruction:

- **Run 1** ("请接着这一章的结尾，写下一场：…"): the message comes in a new Conversation, so it is no answer to a brief, and it asks for a new scene. It does not give each item. Before and after, the instruction is the brief. The brief before and after: 4 to 6 lines with each item, then one question.
- **Run 2** ("思路可以。只改一处：视角换成老周。…"): the message comes directly after Run 1, whose final answer was a brief, and it asks for no new brief. Before and after, the instruction is to write the scene by the brief and by the correction, with `propose_edit`, and with no second brief.
- **Run 3**: no Skill instruction; the catalog line is the description, which did not change.

## Fixed inputs

Two new Projects 渡口 1 and 渡口 2, each with assistance on, the Volume 第一卷, and two Chapters written through the public commands: Chapter 1 渡口 (5 Blocks) and Chapter 2 来信 (5 Blocks). Chapter 2 was the current Chapter, so it was the Working text of each Run. Its last Block is "第二天一早，他去了渡口。老周正在解缆绳，看见他，没有问他要去哪里。" No Project Instruction. The texts are `chapter-01.md` and `chapter-02.md` of the tag, and the facts of the earlier story are in `facts.md`.

| Run | Project, Conversation | Selected Skill | Author request |
| --- | --- | --- | --- |
| 1 | 渡口 1, new | `scene-brief` | 请接着这一章的结尾，写下一场：林舟坐老周的船过河，去省城找姐姐。 |
| 2 | 渡口 1, the Conversation of Run 1 | none in the request | 思路可以。只改一处：视角换成老周。这一场从老周的眼睛写，林舟想什么，只能从他的话和动作里看出来。其余照你的思路写。 |
| 3 | 渡口 2, new | none | 请接着这一章的结尾，写下一场：林舟坐老周的船过河，去省城找姐姐。 |

## Live request log

Each row is one Model Attempt that the guard counted. The time is the claim time (UTC). The request ID is the response reference of the Provider. Usage is the input / output tokens that the Provider reported. Each Model Attempt is `settled`. `prev`: the request continued the response of the row before it with `previous_response_id`.

| # | Time | Run, RunStep | Request ID | `prev` | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | 10:45:48 | 1, 0 | `resp_0217952579488616863c88f28d563d505635c26ab083df517d5c9` | none | required | `read_chapter` (Chapter 1) | 5,772 / 561 |
| M2 | 10:45:58 | 1, 1 | `resp_021795257958455d80e86c43f25c545aaf1644cd986b2f38d5297` | M1 | required | `finish` | 6,805 / 2,903 |
| M3 | 10:47:00 | 1, 2 | `resp_0217952580210361cb6d3ad1bdd7a7436196dd041d7fa42d7bb06` | M2 | none | final answer (the brief) | 9,758 / 414 |
| M4 | 10:47:11 | 2, 0 | `resp_02179525803158802e43eb8440d3b95b4166e4756a96011af5953` | M3 | required | `propose_edit` | 7,615 / 4,935 |
| M5 | 10:48:54 | 2, 1 | `resp_0217952581348033b39ea1d457e6524cc696cd2a0824aa18af8b2` | M4 | required | `finish` | 9,201 / 1,563 |
| M6 | 10:49:27 | 2, 2 | `resp_021795258167492c37b19e6f2deafccdd0d14a1cba0df7be6b5ab` | M5 | none | final answer | 10,815 / 655 |
| M7 | 10:49:41 | 3, 0 | `resp_02179525818168429636d5ecc874239c049517281a31e01f14abc` | none | required | `propose_edit` | 5,240 / 5,875 |
| M8 | 10:51:46 | 3, 1 | `resp_0217952583063972fe30231c3416749cd01035ba2ea7b619e0b16` | M7 | required | `finish` | 6,728 / 1,012 |
| M9 | 10:52:08 | 3, 2 | `resp_0217952583289326525c18cf4d9efd868f038535426f816e811de` | M8 | none | final answer | 7,790 / 620 |

Total: **9 model requests, 69,724 reported input tokens, and 18,538 output tokens** (Run 1: 22,335 / 3,878; Run 2: 27,631 / 7,153; Run 3: 19,758 / 7,507). No Retrieve, no recovery, and no refused call occurred.

Each request declared the same 13 functions: `web_search`, `web_fetch`, `propose_edit`, `submit_research`, `read_tool_result`, `load_skill`, `read_skill_resource`, `read_chapter`, `search_manuscript`, `write_memory`, `read_memory_entry`, `search_memory`, and `finish`. The requests with a user message (M1, M4, M5, M7, and M8) had the section `Skill catalog` with the three lines of `conflict-check`, `local-revision`, and `scene-brief`. Only M1 had the section `Skill instruction`, with the exact bytes of `skill-packages/scene-brief/SKILL.md` (byte comparison: equal). The other requests sent only a function output. The instructions of each request had the default rules for prose.

## Run 1: the selected Skill, the request for the next scene

**ToolCalls:** `read_chapter` with `{"chapter": 1}`, then `finish`. **Proposals: 0.**

**The final answer (the brief and the question), in full:**

> 这场戏我先对齐思路，暂不写正文：
>
> 1. **作用与视角**：林舟读完信第二天一早上船，说明他从半页信里读出了姐姐没说出口的东西——他从"守着渡口等信的人"变成"瞒着姐姐过河的人"，把怀表、钥匙和父亲淹死的那条河一起带上路；第三人称贴着林舟写。
> 2. **想要与阻碍**：他要去省城找到林雪，可姐姐只写了"医馆"两个字、没留地址；老周一趟两文、从不赊账；十月清晨河上起雾，而他父亲三年前就翻在这条河里，尸首都没找回来。
> 3. **转折**：船到河心，老周一直没问他去哪，这时才慢腾腾开口，点破他是林德的儿子，说林德出事那天也是这样的雾——只说这一句，不再解释，低头撑船。
> 4. **结尾**：船靠对岸码头，老周收两文钱，告诉他日落是最后一班回柳镇的船；林舟跳上岸，站进省城陌生的人声里，发现自己连医馆往哪边走都不知道，画面落在他迈出的第一步。
> 5. **保留的事实**：十月初十清晨；老周六十一岁、独眼、说话慢收钱快；怀表停在四点十分、挂在林舟脖子上；黄铜钥匙缝在棉袄内袋，木箱仍留在柳镇床底；姐姐九月初三离家、信里叮嘱"钥匙收好，别一个人开箱子"。
>
> 这个思路对不对，哪里要改？特别是船上那句话——让老周提起**父亲**（雾与翻船），还是提起**姐姐**（比如她临走那天交代过什么）？

## Run 2: the answer of the author with one correction

The author corrected one item, the viewpoint. **ToolCalls:** `propose_edit` with one change of the last Block of Chapter 2, then `finish`. **Proposals: 1**, with one operation that waits for the decision of the author. The summary of the call: "在第二章结尾续上老周视角的渡河一场：林舟搭船去省城，河心雾中老周提起林德翻船那天，靠岸后林舟连医馆地址都不知道就上了岸。"

**The candidate text of the last Block of Chapter 2:**

> 第二天一早，他去了渡口。老周正在解缆绳，看见他，没有问他要去哪里。
> 老周没问去哪儿。一个月来，这孩子每天傍晚到渡口问一趟对岸的信，他都看在眼里。今天他来得早，脚边搁着个小包袱，上船的时候，一只手按在胸口。
> “周伯，过河。”
> “嗯。”
> 老周把竹篙往岸上一点，木船离了渡口。雾从水面起来，先是一缕，转眼糊住了两岸。船到河心，他那只右眼也只看得见前头两丈远。
> “你爹林德，”老周说，“那年翻船，也是这样的雾。”
> 船板响了一声，那孩子换了个坐法。“周伯那天也在河上？”
> “坐稳。”老周说。
> 后半程，两个人都没再说话。
> 船靠对岸码头，日头才爬上屋檐。石阶上头人声嗡嗡，挑夫的号子压过来，话里的调子跟柳镇不一样。
> “两文。”
> 林舟从棉袄内袋边上摸出个布卷，数了两文递过来，手指又在胸口发硬的地方按了一下。老周把钱塞进腰袋。
> “日头落山前一班，”他说，“误了，今天就没船了。”
> 林舟点头，人站在船头没动。“周伯，城里的医馆在哪条街？”
> “省城十几家医馆。”老周拿右眼看他。
> 林舟嘴唇动了动，没再问。他拎起包袱跳上石阶，脖子上挂铜表的绳晃了一下，人走进旗幌和挑夫中间，没有回头。
> 老周一篙点在石阶上，把船撑回雾里。

**Does the candidate follow the correction?** Yes. The scene is told from the viewpoint of 老周 (finding G3), and the thoughts of 林舟 show only through his words and actions. The turn, the two coins, and the last boat of the brief stay, and the last event changed (finding G4). The Run gave no second brief.

**The final answer** starts with "已按你的修改写好，整段已作为提案提交，等你决定接受、修改或拒绝。全章从老周那只右眼看出去，林舟的心思只落在他的动作和三句话上：", then gives the scene again, and ends with "需要调语气、删对话或改结尾，直接说。" The full answer is in `measures-1.json`.

## Run 3: no selected Skill, a plain request to write the next scene

**Did the Run call `load_skill` for `scene-brief`?** No. **Did the Run give a Proposal at once?** Yes: its ToolCalls were `propose_edit` with one change of the last Block of Chapter 2 in RunStep 0, then `finish`. **Proposals: 1.** The summary of the call: "在本章结尾续写渡河一场：林舟上船，老周拒收船钱（林雪上月已预付双份），雾中过河后林舟赶往堤外的长途车。"

**The candidate text of the last Block of Chapter 2:**

> 第二天一早，他去了渡口。老周正在解缆绳，看见他，没有问他要去哪里。
>
> 雾压在河面上，水是灰的。林舟跳上船，船身往下一沉。他怀里揣着那封信，钥匙用麻绳穿了挂在脖子上，怀表压在更里头，冰着锁骨。
>
> 船上有两个赶早集的妇人，脚边竹篓里一只母鸡缩着脖子。老周把篙往岸上一点，船退进雾里，跟着摇起橹来。水声贴着船底，对岸一点也看不见。
>
> “船钱。”老周说。
>
> 林舟摸出几个铜子，搁在船板上。老周没看钱，那只右眼在他脸上停了停，把铜子推了回来。
>
> “你姐上月过河，给的是双份。”他说，“说她弟弟迟早要来，叫我别收。”
>
> 林舟的手按在铜子上：“她还说什么没有？”
>
> “没了。”老周看着雾，“她那天上船，也没回头。”
>
> 林舟没有再问。他把铜子攥在手心，一直攥到船身一顿。
>
> 雾散开一条缝，对岸的堤露出来。堤外空场上停着去省城的长途车，牲口的白气一股一股往上冒。
>
> 老周收了橹，把船靠上石阶：“辰时末动身。误了，等明天。”
>
> 林舟跳上岸，跑了几步，又停下回头。老周已经把船撑开，背对着他，一摇一摇地回雾里去了。
>
> 他把铜子揣回衣袋，朝堤外的空场跑。

The final answer starts with "我已按你的要求把渡河一场续写在本章结尾", gives the scene again without the kept first sentence, and ends with "你可以接受、修改或拒绝这条提案；想调整节奏（比如删掉同船妇人、让老周多说几句姐姐的事），告诉我即可。" The full answer is in `measures-2.json`.

## Key check

The Server log (177 lines), the Worker log (12 lines), the nine wire payloads, and the other files of the tag have no `Authorization` header, no bearer token, no `x-api-key`, no e-mail address, and no home path.
