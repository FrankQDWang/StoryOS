# Agent Plan live hybrid search run (2026-11-17)

This record is the live evidence of [Show Each Search by Meaning of a Run in the Panel, and Record a Live Hybrid Search](https://github.com/FrankQDWang/StoryOS/issues/1600), group 2, for [ADR 0095](../adr/0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md). It records two live runs on the real Agent Plan route.

- The first run had a sample of three short Chapters. The background work made the vectors with no action of the author, but in each of its two Runs the model read the Chapters and did not call the hybrid search. That run is a finding (section [First run: a finding](#first-run-a-finding)).
- The second run had a sample of 20 Chapters of about 3,000 characters each. The background work made 123 vectors in 44 seconds with no action of the author. Then one author message asked for a passage in words that the passage does not have. The model called `hybrid_search_manuscript`, the result had the passage first, and the final answer named the Chapter 旧债 of the passage.

Raw evidence of the two runs: the plans, the wire payload of each model request, the Model Attempts, the Embedding Attempts, the ToolCalls, the guard log, the Server and Worker logs, the sample and its scripts, and two panel screenshots are on the tag `evidence/1600-live-hybrid-search-2026-11-17`. The tag has no database dump and no Project Export Archive. On the tag, the files are under `docs/research/agent-plan-live-hybrid-search-2026-11-17/`, in `evidence-run1/`, `evidence-run2/`, and `helpers/`.

## Setup

| Item | Value |
| --- | --- |
| Product code | `stage7-memory` at `9252769e540dd9779991e109b11624d17af2cdf0` (tree `455c1222`), with S7-C2b2b (#1647), on the branch `claude/s7-c3c-live-hybrid-search`. No product text changed before or between the runs. |
| Release package | `make generate-persistence-catalog` on that clean head, web manifest `sha256:77fd85b17f826c1d5c94b3690a84b5b98202c9295d5b2605925c2d4eb9deffd8`. The same build for the rehearsal and the two runs. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` on `127.0.0.1:8750`, and one continuous `storyos-worker`. Each had `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` and `STORYOS_EMBEDDING_DESTINATION=volcengine_agent_plan`, and a process environment with only the named variables, `PATH`, and `HOME`. Both came from the release package. |
| Settings | ADR 0050: the Worker resolved the key from the env file of the main checkout, which it found from its working directory in the worktree. The runs never read, copied, or printed that file or a key. |
| Database | For each run, a new `scripts/dev-postgres.sh up --interactive` database with the controlled fixture. The only direct writes were the guard schema `live_guard`. All reads during a run went through a read-only connection. The guard and the database were removed after each run. |
| Model | `doubao-seed-2-1-pro-260915`, head Model Registration `018f0000-0000-7001-8000-00000000fa22`, mapping `storyos.volcengine-agent-plan.responses.create.v8`, grant scope `project_writing_assistance_v5`. Each request declared twelve Tools, with `hybrid_search_manuscript` after `search_manuscript`. |
| Embedding | `doubao-embedding-vision-251215`, Embedding Registration `018f0000-0000-7001-8000-0000000eb002`, mapping `storyos.volcengine-agent-plan.embeddings.multimodal.v1`, grant scope `manuscript_embedding_v1`, request bounds `embedding_request_v1`, 1,024 dimensions. |
| Author surface | The packaged Web client with the bootstrap session `session-a`. First run: the built-in browser of the Claude desktop app. Second run: Chapters 1 and 2 in the built-in browser, and Chapters 3 to 20, R0, and the author message in Google Chrome driven by Playwright (`type-sample.mjs` and `panel.mjs` on the tag), which types into the same editor and composer. The editor sent `createChapter` and `applyAuthorEdit`; nothing wrote the text with SQL. |

## Plans

Each plan was a comment on the ticket before the first request of its run: [first plan](https://github.com/FrankQDWang/StoryOS/issues/1600#issuecomment-6944979416) and [second plan](https://github.com/FrankQDWang/StoryOS/issues/1600#issuecomment-6945504870). The coordinator confirmed the numbers.

| Run | Model requests (planned / maximum) | Embedding requests (planned / maximum) | Exa | Guard caps (model / Exa / embedding) | Final count |
| --- | --- | --- | --- | --- | --- |
| First | 5 / 8 | 7 / 9 | 0 | 8 / 0 / 9 | 6 model, 6 embedding, 0 Exa |
| Second | 6 / 10 | 125 / 129 | 0 | 10 / 0 / 129 | **4 model, 124 embedding, 0 Exa** |

Request guard: the `live_guard` triggers of the [record of 2026-11-14](agent-plan-live-compaction-2026-11-14.md), and one more trigger of the same form, `BEFORE INSERT` on `storyos.embedding_attempts`. The store writes the Model Attempt and the Embedding Attempt in the claim transaction before the I/O, so each trigger counts at the claim, and above its cap it refuses the claim, so nothing goes out. No cap was reached. After each run the four triggers and the schema `live_guard` were dropped, and a read showed no schema and no trigger.

## The first Run R0 and the binding

In the code of this head, the index work of a Project needs its embedding use binding, and the first Run of the author makes that binding. A new binding marks each live Chapter with no quiet time. So in each run the author admitted one Run R0 with the message "准备检索。" in the panel and cancelled it with the control 取消任务 before any Worker ran. R0 is the first Run of the author, it makes the binding, and it makes no model request. Rows of the second run, read before the Worker started:

- R0 `01a1e921-7736-7ad2-95b3-d7f8eb2dd19d`: status `cancelled`, kind `author`. Model Attempts of the Project: 0.
- Binding rows (`project_external_use_binding_revisions`), each `operator_quota`: the model binding with Model Registration `fa22`, the embedding binding with Embedding Registration `eb002` and Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`, and the Exa tool binding.
- 20 rows of `passage_index_work`, one for each Chapter, each with Embedding Registration `eb002`, mark generation 1, completed generation 0, and `not_before` `10:50:46.968Z`, the time of the admission of R0. The first run had the same rows for its three Chapters.
- A rehearsal with `host_fake` for the model and the embedding destination came before the first run, with the same steps. R0 kept its binding after the cancel, the marks had no wait, and the Worker made the 6 vectors at its first poll.

## Second run

### Sample

A script with a sentence bank and the fixed seed 1600 made the filler (`evidence-run2/make-sample.py` on the tag). One paragraph was written by hand. The text is not a work of the author and not a copy of a published text. Volume 第一卷 has 20 Chapters and 123 paragraphs with 58,268 characters (58,371 with the line feeds). Each Chapter has 5 to 7 paragraphs of 381 to 592 characters. Two adjacent paragraphs with their line feed have at least 827 characters, which is above the passage limit, so the cut `storyos.passage-cut.v1` gives one passage for each paragraph: **123 passages**. The SHA-256 is of the paragraphs of the Chapter joined with a line feed, in UTF-8, and the column Characters counts that text with its line feeds.

| # | Title | Paragraphs | Characters | SHA-256 |
| --- | --- | --- | --- | --- |
| 1 | 春网 | 5 | 2862 | `sha256:7bbc8cddf270fb18a35abb4db560e6eb39bf72b9b95cdfc6d45c35bec958a747` |
| 2 | 修船 | 5 | 2914 | `sha256:ac8ff344ad2d597760da58be1cf4905307bba539ce2a4c9f2d0221eba992eaed` |
| 3 | 盐价 | 7 | 2976 | `sha256:6e34b31326ace72d8097e7a113757bf8b6ee5ddd291a3324b8c8c627b75e8f15` |
| 4 | 祠堂 | 7 | 2970 | `sha256:5415bea4f6c0897b11eecd5d5eb9ab238f14927578369b950bb3d90eaf503aa3` |
| 5 | 晒场 | 6 | 2925 | `sha256:39582018075b9fd6136123d34222ecdab4b4384efd07cec4891d47cbc94c6eec` |
| 6 | 换工 | 7 | 2982 | `sha256:496fc63b8dca89ab5b8adcd4776ad8e2f1d5335d640a6bf9cd03e93581f06780` |
| 7 | 学堂 | 5 | 2864 | `sha256:f5b36ec4f015bee8efe5cabe1d9b7adbdea436819864a3b131ac9b2100f65210` |
| 8 | 雨季 | 5 | 2882 | `sha256:2f531914235e66b3208cffb412588c482283a6f734375c26c032cc43be27619d` |
| 9 | 账本 | 7 | 2967 | `sha256:a6b0a3b58764f66c2521125bcdacb2fb83ec60ba286e6aa98f17bde2e4707569` |
| 10 | 新桥 | 7 | 2945 | `sha256:884af4729024fd2bb5298d61f1802bd8d228fb7b45b1c72151c6c738feb1c78a` |
| 11 | 腌菜 | 7 | 2945 | `sha256:1a4eb713159c3cc4a673e7ed2010b9ebaca99eac772cb96edfed75c234cde9f8` |
| 12 | 旧债 | 6 | 2785 | `sha256:3aa6a6746d4a4377cdc126632b07741685506c22bec9316af267263b2e7017f9` |
| 13 | 米铺 | 6 | 2908 | `sha256:56434e4d07983e39739585c829884438e1b382c0cf9e650145c97e63d854391b` |
| 14 | 嫁妆 | 7 | 2987 | `sha256:73ae1b7bfe3996e23c1f4e02e7b4176f9de78266a081a65d69fd14054f650972` |
| 15 | 木匠 | 5 | 2851 | `sha256:2f9d8d7c86daf0d5d194dd1fc15f6bc217bdd71e8e261acfac29b8cc86a3393a` |
| 16 | 鱼市 | 7 | 2970 | `sha256:67144320be6511822d58798ead9e707d5e279a5bf31c84731d8cc4d37976d30b` |
| 17 | 冬至 | 6 | 2877 | `sha256:28003426e0693aea12c44e2f9988721a4bed3c6e09ca2841a16d653a065dabfc` |
| 18 | 分家 | 6 | 2888 | `sha256:3f46afd4502c848313584d7dadf16cfa330d4781d008ec34bfc17f21ee3a7451` |
| 19 | 过年 | 7 | 2987 | `sha256:e877cb91fdfc0732e6f37c2451adbb34af73284d348fb385e40a502b86e6698c` |
| 20 | 开春 | 5 | 2886 | `sha256:0934243dcb07d5d2d1d9b6bd72a3aa6afffa7c1952df5b00d40ce7f64ae031d4` |

The passage to find is paragraph 4 of Chapter 12 旧债, one passage of 381 characters. No Chapter title names its event. It has no two-character word of either author wording. The filler has none of 灯, 光, 亮, 照, 火, 雾, 夜, 红, 回来, and 方向. The complete text of Chapter 12 旧债:

> 钱掌柜的米铺新进了一批米，价钱比上个月便宜了一些。镇上今年添了三户新人家，后山的菜地一下子显得窄了不少。林雪蹲在门槛上剥豆子，小桃在一旁补衣服，两人有一句没一句地说着话。学堂的路被雨水冲出了几道沟，刘先生和孙木匠扛着锄头去填平。镇上今年添了三户新人家，米铺一下子显得窄了不少。米铺的路被雨水冲出了几道沟，林舟和小桃扛着锄头去填平。阿满蹲在门槛上剥豆子，杜大娘在一旁补衣服，两人有一句没一句地说着话。何三叔在镇东的石桥碰见了林雪，两人站着聊了好一会儿家常。林雪说起年轻时去县城做工的事，说那时候一个月只能吃上两回肉。渡口的路被雨水冲出了几道沟，孙木匠和刘先生扛着锄头去填平。河湾今天格外热闹，卖菜的和买菜的挤成一团，说话声一阵高过一阵。刘先生想起阿满前几天托他办的事，赶紧从怀里摸出那张写着字的纸条。孙木匠的手艺在镇上数一数二，谁家打柜子都要排队等他。小桃家的鸡跑到了何三叔家的菜地里，啄坏了一畦青菜。钱掌柜的米铺新进了一批米，价钱比上个月便宜了一些。何三叔把旧船板拆下来，挑出还能用的几块，堆在墙角晾着。老周把半罐猪油背回家，放在灶台边上，又去井边打了两桶水。

> 集市边上新开了一家铺子，门口挂着黄纸写的招牌，字写得歪歪扭扭。刘先生在学堂里教孩子们认字，念书声隔着两条街都听得见。孙木匠说话慢，可是办事从来不出错，钱掌柜最信得过的就是他。杜大娘在祠堂前的空地碰见了林舟，两人站着聊了好一会儿家常。孙木匠的手艺在镇上数一数二，谁家打柜子都要排队等他。刘先生把三斤粗盐背回家，放在灶台边上，又去井边打了两桶水。林舟一边走一边盘算，明天要先去米铺，再去找老周商量修房子的事。刘先生把账本翻来覆去看了三遍，还是对不上那几文钱。孙木匠在镇东的石桥碰见了刘先生，两人站着聊了好一会儿家常。钱掌柜的米铺新进了一批米，价钱比上个月便宜了一些。林舟蹲在门槛上剥豆子，杜大娘在一旁补衣服，两人有一句没一句地说着话。刘先生想给林舟送点一卷麻绳，又怕人家不收，在门口转了好几圈。孙木匠的手艺在镇上数一数二，谁家打柜子都要排队等他。林雪把半罐猪油背回家，放在灶台边上，又去井边打了两桶水。孙木匠托人从县城捎来一卷麻绳，说是给小桃家办喜事用的。何三叔想起杜大娘前几天托他办的事，赶紧从怀里摸出那张写着字的纸条。阿满托人从县城捎来半罐猪油，说是给孙木匠家办喜事用的。

> 何三叔的院子里种着几棵枣树，秋天一到，枣子落了满地。祠堂前的空地的路被雨水冲出了几道沟，刘先生和杜大娘扛着锄头去填平。刘先生一早就去了河湾，想赶在人多之前把事情办完。钱掌柜的米铺新进了一批米，价钱比上个月便宜了一些。何三叔把旧船板拆下来，挑出还能用的几块，堆在墙角晾着。老周说话慢，可是办事从来不出错，钱掌柜最信得过的就是他。林雪说话慢，可是办事从来不出错，钱掌柜最信得过的就是他。何三叔蹲在门槛上剥豆子，老周在一旁补衣服，两人有一句没一句地说着话。杜大娘蹲在门槛上剥豆子，阿满在一旁补衣服，两人有一句没一句地说着话。小桃说今年的收成比去年强一些，至少不用再向钱掌柜赊账了。杜大娘蹲在门槛上剥豆子，林雪在一旁补衣服，两人有一句没一句地说着话。何三叔在后山的菜地碰见了林舟，两人站着聊了好一会儿家常。老周蹲在门槛上剥豆子，孙木匠在一旁补衣服，两人有一句没一句地说着话。刘先生一早就去了米铺，想赶在人多之前把事情办完。到了晌午，刘先生在后山的菜地的树荫下吃了两个饼子，喝了一碗凉茶。到了晌午，林舟在镇东的石桥的树荫下吃了两个饼子，喝了一碗凉茶。

> 那年秋汛最凶的一晚，河面上白茫茫一片，站在码头上连对岸的柳树都望不到。出港的七条渔船，到了该进港的时辰，一条也不曾靠岸。男人们都在船上，镇上只剩些老人和孩子，大伙挤在祠堂门口，谁也说不出话，只听得见河水一下一下拍着石阶。秦婆婆不去祠堂，她从柴房里抱出一捆浸过松脂的芦苇，扛到老码头尽头的石墩上点着，又一捆接一捆地往上添。孙木匠的小儿子跑来帮她搬柴，她叫孩子把柴房里剩下的芦苇都抱过来，一根也别留，自己守在石墩旁边，寸步不离，整整守到后半宿。那一团跳动的红在水汽里时隐时现，像有人在岸边不停地招手。风一阵紧过一阵，好几回眼看就要被吹灭，她就用身子挡着风，再添上一捆。过了很久，远处终于传来了橹声，先是一条，接着是两条、三条，船上的人顺着那团红，一条一条摸进了港湾。最后一条船靠岸时，天边已经发白，秦婆婆的眉毛被烤焦了一半，手背上也起了泡，她却只问了一句：都齐了吗？

> 孙木匠的手艺在镇上数一数二，谁家打柜子都要排队等他。刘先生的院子里种着几棵枣树，秋天一到，枣子落了满地。林舟说今年的收成比去年强一些，至少不用再向钱掌柜赊账了。何三叔蹲在门槛上剥豆子，小桃在一旁补衣服，两人有一句没一句地说着话。林雪说今年的收成比去年强一些，至少不用再向钱掌柜赊账了。阿满蹲在门槛上剥豆子，孙木匠在一旁补衣服，两人有一句没一句地说着话。林舟一边走一边盘算，明天要先去学堂，再去找老周商量修房子的事。老周说今年的收成比去年强一些，至少不用再向钱掌柜赊账了。刘先生说起年轻时去县城做工的事，说那时候一个月只能吃上两回肉。杜大娘托人从县城捎来几张草纸，说是给林舟家办喜事用的。小桃一早就去了集市，想赶在人多之前把事情办完。何三叔想起杜大娘前几天托他办的事，赶紧从怀里摸出那张写着字的纸条。林舟一边走一边盘算，明天要先去后山的菜地，再去找孙木匠商量修房子的事。林舟想起孙木匠前几天托他办的事，赶紧从怀里摸出那张写着字的纸条。到了晌午，小桃在盐仓的树荫下吃了两个饼子，喝了一碗凉茶。镇上今年添了三户新人家，后山的菜地一下子显得窄了不少。

> 集市的老槐树下常有几个老人下棋，一盘棋能下一个下午。到了晌午，老周在渡口的树荫下吃了两个饼子，喝了一碗凉茶。老周蹲在门槛上剥豆子，小桃在一旁补衣服，两人有一句没一句地说着话。孙木匠的手艺在镇上数一数二，谁家打柜子都要排队等他。林舟算了算手里的钱，只够买三斤粗盐，别的只好等下个月再说。钱掌柜一早就去了集市，想赶在人多之前把事情办完。镇上的人都知道林雪脾气好，谁家有活忙不过来，总会先去喊他。钱掌柜和何三叔为了半罐猪油的价钱争了几句，最后各让一步，算是谈妥了。镇上今年添了三户新人家，米铺一下子显得窄了不少。何三叔把一包茶叶背回家，放在灶台边上，又去井边打了两桶水。钱掌柜说今年的收成比去年强一些，至少不用再向钱掌柜赊账了。米铺的路被雨水冲出了几道沟，刘先生和杜大娘扛着锄头去填平。渡口的老槐树下常有几个老人下棋，一盘棋能下一个下午。米铺的老槐树下常有几个老人下棋，一盘棋能下一个下午。钱掌柜蹲在门槛上剥豆子，杜大娘在一旁补衣服，两人有一句没一句地说着话。到了晌午，何三叔在后山的菜地的树荫下吃了两个饼子，喝了一碗凉茶。孙木匠想起何三叔前几天托他办的事，赶紧从怀里摸出那张写着字的纸条。

### Index work

The Worker started at 10:51Z, and its first log line is at 10:51:08Z. Its first Embedding Attempt was at 10:51:08Z and its 123rd at 10:51:48Z. The poll through the read connection saw 74, 106, and then 123 vectors at 10:51:52Z. Each Embedding Attempt has the cause `passage_index` and the settlement `vector`, and each of the 20 rows has its completed generation equal to its mark generation. The set of the 123 `passage_digest` values in `passage_vectors` is equal to the set of the SHA-256 of the 123 paragraphs of the sample, so each passage came in exactly as the sample has it.

### Live request log

The author message went in a new Conversation of Chapter 1 春网 at 10:52:05Z: "书里有没有谁在大家看不清方向时，为还没回来的人照亮归途？在哪一章？" Each row is one claim that the guard counted (UTC). Usage is the input / output tokens that the Provider reported. Each later model request continued the response before it with `previous_response_id`.

| # | Time | RunStep | Request ID | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | 10:52:09 | 0 | `resp_021794912729605ab257ece25bd0fea81c82ca5c15ec8fdf3e9f7` | required | two calls: `hybrid_search_manuscript` and `search_manuscript` | 8,115 / 287 |
| E124 | 10:52:16 | 0 | (embedding) | | the question of the hybrid call: `vector` | 47 / – |
| M2 | 10:52:16 | 1 | `resp_0217949127371354f170dda172df2cc79a65077ac74d404f0ed7b` | required | `read_chapter` 12, Blocks 3 to 5 | 12,022 / 617 |
| M3 | 10:52:28 | 2 | `resp_021794912748989e0d15f89dc5eb44ad1ad600579228b792e236a` | required | `finish` | 13,835 / 130 |
| M4 | 10:52:34 | 3 | `resp_0217949127548914c9833b195a1cac5bbbd18e9e4e5ff83731b8a` | none | final answer | 14,015 / 221 |

Total: 47,987 reported input tokens and 1,255 output tokens for 4 model requests (plan: about 55,000 for 6). The first request was 8,115, not about 6,000: the Working text of 春网 (2,858 characters) is about 4,000 tokens of Chinese text. No request was near the limit of 120,000.

The calls of M1 (ToolCalls of the Run, each `settled`):

- `hybrid_search_manuscript` with `question` "有人在雾中或黑夜里提着灯，为还没回来的人照亮回家的路" and `terms` `["灯","灯笼","照亮","归途","雾"]`.
- `search_manuscript` with `query` "灯": `{"blocks":0,"blocks_returned":0,"complete":true,"matches":0,"query":"灯"}`.

The model called the two Tools in the same step, so it did not wait for an empty keyword result. No term of the hybrid call occurs in the sample, so the exact-term list was empty and the order came from the vector list only.

### Function declaration and Tool result as the route got them

Each request declared the Tool as `hybrid_search_manuscript` with the description and the parameters of S7-C3b (`question`: "The question in ordinary words."; `terms`: "The exact words or phrases that the passages can have, with the same letter case. The list can be empty."; both required, `strict: true`). The wire payload of M2 (`evidence-run2/evidence/wire/M2.json` on the tag) sent the result as a `function_call_output` in a `tool_result` tag with `state="ok"`. The result (12,086 bytes) starts with these lines:

```text
{"passages":8,"passages_returned":8,"terms":["灯","灯笼","照亮","归途","雾"]}
{"chapter":12,"chapter_id":"01a1e91b-dab8-71c3-8309-6e74d35a3a2f","title":"旧债"}
[block 4]
那年秋汛最凶的一晚，河面上白茫茫一片，站在码头上连对岸的柳树都望不到。……她却只问了一句：都齐了吗？
{"chapter":1,"chapter_id":"01a1e911-8b8d-7db2-a29d-d9d72f14e6c6","title":"春网"}
[block 4]
……
```

The passage of 旧债 is the first of the 8 passages. The other 7 are filler passages of 春网 (2), 分家, 账本, 冬至, and 过年 (2). The result has no first line about the search by meaning, so the question had a vector.

### Final answer and panel

The final answer (M4) named the Chapter and the person: "有的。这个人是**秦婆婆**，故事在**第十二章《旧债》**（该章第 4 段）。" It then told the event in its own words and quoted the last question of the passage. The panel showed the answer and then these lines (`evidence-run2/run1-panel.png` and `evidence-run2/run1-panel.txt` on the tag):

```text
按意思在全书中搜索了「有人在雾中或黑夜里提着灯，为还没回来的人照亮回家的路」
在全书中搜索了「灯」
查阅了章节「旧债」
```

The Run completed at 10:52:40Z. Its Conversation capture was due at 11:22:40Z (`CONVERSATION_QUIET_TIME`). The Worker stopped at 10:52:54Z, so no capture request went out.

### Usage of the embedding requests

| Cause | Requests | Reported input tokens |
| --- | --- | --- |
| `passage_index` | 123 | 47,068 in total, 320 to 475 for each request |
| `search_question` | 1 | 47 |

Each passage request has one context line with the Volume and Chapter titles and then the passage text.

## First run: a finding

The first run had three Chapters 渡口, 集市, and 潮夜, each of two paragraphs with a scene separator, 107 to 113 characters for each Chapter and 6 passages. The target was the second paragraph of 潮夜. The Worker made the 6 vectors from 10:20:34Z to 10:20:39Z (6 Embedding Attempts, each `vector`, 403 input tokens in total). Then the author sent two wordings, each in a new Conversation of 渡口.

| # | Time | Run, RunStep | Request ID | `tool_choice` | Result | Usage |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | 10:21:12 | 1, 0 | `resp_0217949108727044f170dda172df2cc79a65077ac74d404995640` | required | `read_chapter` 2 and `read_chapter` 3 | 3,724 / 258 |
| M2 | 10:21:19 | 1, 1 | `resp_02179491087946236a85968bdbb8a949a5bf709eb7d71bc31afc8` | required | `finish` | 4,558 / 173 |
| M3 | 10:21:25 | 1, 2 | `resp_0217949108851662f58812a10f9c6b8f25a472cef1ad3e52d8830` | none | final answer: 潮夜, 阿婆 | 4,781 / 120 |
| M4 | 10:21:55 | 2, 0 | `resp_021794910915648841fc2298faf73f2fc5d524f9c961e71174b1a` | required | `read_chapter` 2 and `read_chapter` 3 | 3,736 / 239 |
| M5 | 10:22:01 | 2, 1 | `resp_02179491092124134cf8297fe89ff93b17b5141a0abd4069b7034` | required | `finish` | 4,554 / 238 |
| M6 | 10:22:09 | 2, 2 | `resp_021794910930091888862656ddb6bf8fba16f467f9d03d95dd561` | none | final answer: 潮夜, 阿婆 | 4,843 / 120 |

Run 1 had the first wording. Run 2 had the second wording: "请按意思在全书中找一找：有没有一段写某个人在视线模糊的时刻，替尚未返航的人指明方向？告诉我它在哪一章。" Each request declared `hybrid_search_manuscript`. Both answers were correct. The Chapter list gave the length of each Chapter, so the model read the two Chapters that were not the Working text. For a manuscript of that size, a read is the correct choice: the sample was the defect, not the model and not the product. The coordinator then decided on a second sample that a Run does not read whole. The Worker stopped at 10:22:39Z, before the capture times of 10:51:28Z and 10:52:14Z, and the database was removed with its capture rows.

## Initial values in the run

| Value | Initial value | Measure in the second run |
| --- | --- | --- |
| `PASSAGE_CHARACTER_LIMIT` (S7-C2b1) | 800 | Paragraphs of 381 to 592 characters gave 123 passages, one for each paragraph, and no cut inside a paragraph. One passage request was 320 to 475 tokens. |
| `PASSAGE_CUT_REVISION` | `storyos.passage-cut.v1` | Each index row recorded it. |
| `EMBEDDING_TEXT_TOKEN_LIMIT` (S7-C2a1) | 8,192 | The largest request was 475 tokens, 6 percent of the limit. |
| `EMBEDDING_REQUEST_TIMEOUT` (S7-C2a1) | 30 s | 123 requests settled in 40 seconds, about 0.33 seconds for each with the loop. No request came near the limit. |
| `EMBEDDING_DIMENSIONS` (S7-C2a1) | 1,024 | Each of the 124 responses had a vector that the store kept. |
| `PASSAGE_INDEX_QUIET_TIME` (S7-C2b2) | 600 s | Not used: the run had the case of a new binding, which has no wait. No Chapter changed after the binding. |
| `PASSAGE_INDEX_LEASE` (S7-C2b2) | 60 s | One Chapter of 5 to 7 requests took about 2 seconds, well inside the lease. |
| `CONVERSATION_QUIET_TIME` | 1,800 s | Not used: the Worker stopped before the capture was due. |
| `PASSAGE_RESULT_LIMIT` (S7-C3a) | 8 | 8 of 123 passages came back, the target first. The result added about 3,900 input tokens to M2. |
| `PASSAGE_LIST_LIMIT` (S7-C3a) | 40 | The vector list took the 40 nearest of 123 passages. The exact-term list was empty. |
| `PASSAGE_FUSION_CONSTANT` (S7-C3a) | 60 | With an empty exact-term list, the fused order is the vector order, so this run gives no measure for the constant. |
| `PASSAGE_TERM_LIMIT` and `PASSAGE_TERM_MAX_CHARS` (S7-C3a) | 8 and 64 | The model gave 5 terms of 1 or 2 characters. |

This run gives no reason to change a value, and no change is proposed. One observation for later work: the model called `search_manuscript` and `hybrid_search_manuscript` in the same step, so the order "search_manuscript first" of the Tool description was not followed literally. The two calls were cheap, and the result was correct.

## Key check

The Server and Worker logs (2,255 and 845 lines in the second run, 239 and 1,095 in the first run), each wire payload, each JSON file, the scripts, and the plans of the tag have no `Authorization` header, no bearer token, no `x-api-key`, no e-mail address, and no home path. The matches of a search for long tokens are only digests, identities, response references, and names of settings. The Credential References are `settings:STORYOS_AGENT_PLAN_API_KEY` and `settings:STORYOS_EXA_API_KEY`.
