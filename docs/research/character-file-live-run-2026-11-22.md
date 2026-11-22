# Live run of the character file (2026-11-22)

This record is the live evidence of [S11-02: Run the Character File on the Real Route and Set the Limit of a Character Entry](https://github.com/FrankQDWang/StoryOS/issues/1788) for `S11-EVD-001` and [ADR 0101](../adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 3. One run on the real Agent Plan route shows the character file that the capture of each Chapter and the consolidation pass write with the mapping `v11`. The run also measures the length of each character text after each Chapter, for the limit of a character entry (decision 13 of [#1786](https://github.com/FrankQDWang/StoryOS/issues/1786)). The run is no trial that compares models or prompts.

Raw evidence is on the tag `evidence/1788-character-file-live-2026-11-22`. The tag holds the plan, the run script `character-file-live.ts`, `run.sh`, `rehearse.sh`, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M39.json`), the Model Attempts, the ToolCalls, the Run rows, the guard log, `due.sql`, `checks.json`, `loader.log`, `entries.json` (each entry after each step), `measures.json`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive.

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1788#issuecomment-7074183989)). The coordinator agreed with it before the run.

Inputs:

- One Project with the ten Chapters of `fixtures/memory-calibration/` (no changed Chapter and no Conversation script). The script wrote each Chapter through `applyAuthorEdit`.
- Product code and release package: `stage11-character-intents` at `22882beabf43225cc3e78319e6752b56bb86520d`, the merge of #1787.
- The statements A (a Chapter capture), B (the pass), and C (a Conversation capture). Each one set the due time of one schedule row to the present. These statements and the guard schema were the only SQL writes.

Steps, and what each step shows:

| Step | What the script did | What it shows |
| --- | --- | --- |
| 0 | One Run of the author with no fact of the sample (`你好。我在写长篇小说《临水茶馆》……回复一句“好”就行。`), then the capture of its Conversation. A Chapter capture needs a completed Run of the author in the Project. | — |
| 1 | For each Chapter 1 to 10 in order: create the Chapter, write its text, make its capture due, wait, make the pass due, wait. | The part of the background work of each character as the Chapters grow. |
| 2 | After each pass, record each entry. | The length of each character text after each Chapter, and the largest growth for one Chapter. |
| 3 | After the pass of Chapter 7: one Run of the author in a new Conversation with fact R5 of `key.md` (`请帮我记住一件关于顾青禾的事：一九八三年房管所卖这栋房子，总价四千二百元，顾青禾出了其中的两千一百元，所以茶馆的房子有她一半。这一点后面会用到。`). Chapter 8 tells this fact. Then the capture of that Conversation and Chapters 8 to 10. | The part of the author beside the part of the background work, and a later change of the part of the background work. |

The numbers of the plan, and the actual numbers:

| Item | Planned | Stop at | Actual |
| --- | --- | --- | --- |
| AgentRuns | 23 | 26 | 24 |
| Model requests of one Run | author 1 to 3, capture 1 to 5, pass 1 | author 6, capture 8, pass 1 | author 2 and 3, capture 1 to 8, pass 1 |
| Model requests in all | about 34 | 60 | 39 |
| Reported input in all | about 250,000 | 1,000,000 | 286,472 |
| Largest input of one request | about 15,000 | 60,000 | 18,064 |
| Exa and embedding requests | 0 | 1 | 0 |

The request guard of the database counted each model request at the claim and each AgentRun at its insert. The script also checked each limit and failed work in each poll. No stop occurred, and no Run repeated. A rehearsal of all steps on `host_fake` came first, on the same package and a separate database: 14 AgentRuns and 16 requests, each Run `completed`.

## Setup

| Item | Value |
| --- | --- |
| Release package | `python3 scripts/package-release.py` on the clean commit `22882bea` (tree `61752fa1a63636109df6431e7b71d20d5b0dce60`), manifest `sha256:73b3e80ee96ebd054935e9322cc8b323c2068e1b1f61eca5245afac48f3124b1`. |
| Topology | ADR 0022 production surface: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` on `127.0.0.1:8792`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, both from the release package. Each process environment had only the named variables, `STORYOS_DATABASE_URL`, `PATH`, and `HOME`, and the Server also had the test settings `STORYOS_BOOTSTRAP_SESSIONS` and `STORYOS_CHALLENGE_SECRET`. No embedding destination, no `STORYOS_EXA_API_KEY`, and no Skill directory. |
| Settings | ADR 0050: the Worker resolved the key through its typed settings. The run never read, copied, or printed an env file or a key. |
| Database | A new `scripts/dev-postgres.sh with` database. It was removed after the run. |
| Model | `doubao-seed-2-1-pro-260915`. Each of the 24 Runs pinned the head Model Registration `018f0000-0000-7001-8000-00000000fa25`, and each of its Model Attempts has the mapping `storyos.volcengine-agent-plan.responses.create.v11`. |
| Time | 04:54:07 (the first command) to 05:53:37 (the stop of the Worker) UTC. |

## Runs

The claim time is the start of the capture or the pass, or the first event of a Run of the author (UTC). Input and output are the tokens that the Provider reported. Each Run has the Registration `fa25` and the mapping `v11`, and each Run is `completed`.

| # | Kind | Source | Claim | Requests | Input | Output | Tool calls | Outcome |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R1 | author | step 0 | 04:54:11 | 2 (M1–M2) | 8,606 | 123 | `finish` | answer "好。" |
| R2 | capture | Conversation of step 0 | 04:54:18 | 1 (M3) | 2,246 | 207 | `record_memory_observations` | `no_observation` |
| R3 | capture | Chapter 1 | 04:54:29 | 1 (M4) | 4,432 | 2,518 | `record_memory_observations` | `observations_recorded` |
| R4 | pass | Chapter 1 | 04:55:31 | 1 (M5) | 2,676 | 2,771 | `apply_memory_operations` | `applied` |
| R5 | capture | Chapter 2 | 04:56:29 | 2 (M6–M7) | 10,326 | 2,652 | `read_memory_entry` ×6, `record_memory_observations` | `observations_recorded` |
| R6 | pass | Chapter 2 | 04:57:35 | 1 (M8) | 3,764 | 3,558 | `apply_memory_operations` | `applied` |
| R7 | capture | Chapter 3 | 04:58:42 | 2 (M9–M10) | 10,191 | 3,166 | `read_memory_entry` ×3, `record_memory_observations` | `observations_recorded` |
| R8 | pass | Chapter 3 | 05:00:11 | 1 (M11) | 4,321 | 17,165 | `apply_memory_operations` | `applied` |
| R9 | capture | Chapter 4 | 05:06:01 | 3 (M12–M14) | 16,609 | 4,191 | `read_memory_entry` ×2, `record_memory_observations` refused `observation_invalid`, `record_memory_observations` | `observations_recorded` |
| R10 | pass | Chapter 4 | 05:07:25 | 1 (M15) | 5,740 | 9,432 | `apply_memory_operations` | `applied` |
| R11 | capture | Chapter 5 | 05:10:33 | 1 (M16) | 5,118 | 4,842 | `record_memory_observations` | `observations_recorded` |
| R12 | pass | Chapter 5 | 05:12:18 | 1 (M17) | 7,119 | 7,348 | `apply_memory_operations` | `applied` |
| R13 | capture | Chapter 6 | 05:14:34 | 1 (M18) | 5,093 | 3,729 | `record_memory_observations` | `observations_recorded` |
| R14 | pass | Chapter 6 | 05:15:53 | 1 (M19) | 8,009 | 7,360 | `apply_memory_operations` | `applied` |
| R15 | capture | Chapter 7 | 05:17:48 | 1 (M20) | 5,503 | 4,453 | `record_memory_observations` | `observations_recorded` |
| R16 | pass | Chapter 7 | 05:19:22 | 1 (M21) | 8,824 | 20,012 | `apply_memory_operations` | `applied` |
| R17 | author | step 3 | 05:27:17 | 3 (M22–M24) | 49,146 | 5,102 | `write_memory`, `finish` | answer |
| R18 | capture | Conversation of step 3 | 05:27:38 | 2 (M25–M26) | 8,451 | 1,529 | `read_memory_entry`, `record_memory_observations` | `no_observation` |
| R19 | capture | Chapter 8 | 05:28:10 | 1 (M27) | 5,692 | 5,131 | `record_memory_observations` | `observations_recorded` |
| R20 | pass | Chapter 8 | 05:29:49 | 1 (M28) | 10,566 | 20,940 | `apply_memory_operations` | `applied` |
| R21 | capture | Chapter 9 | 05:36:52 | 1 (M29) | 5,782 | 5,105 | `record_memory_observations` | `observations_recorded` |
| R22 | pass | Chapter 9 | 05:38:42 | 1 (M30) | 10,870 | 20,401 | `apply_memory_operations` | `applied` |
| R23 | capture | Chapter 10 | 05:44:43 | 8 (M31–M38) | 76,009 | 15,427 | `record_memory_observations` refused `observation_invalid` ×7, `record_memory_observations` | `observations_recorded` |
| R24 | pass | Chapter 10 | 05:49:47 | 1 (M39) | 11,379 | 13,540 | `apply_memory_operations` | `applied` |

| Kind | Runs | Requests | Input | Output | Largest input |
| --- | --- | --- | --- | --- | --- |
| Runs of the author | 2 | 5 | 57,752 | 5,225 | 18,064 |
| Captures | 12 | 24 | 155,452 | 52,950 | 12,858 |
| Passes | 10 | 10 | 73,268 | 122,527 | 11,379 |
| All | 24 | 39 | 286,472 | 180,702 | 18,064 |

- The statements: C 2 times, A 10 times, and B 9 times. The pass of Chapter 1 was due at once, so it had no statement B. Each statement changed 1 row (`due.sql`).
- No capture started by time, and no capture of a Conversation gave an observation.
- A pass took 50 to 417 seconds. The passes of Chapters 3, 7, 8, and 9 each gave 17,165 to 20,940 output tokens, because the model gives the full text of each entry that it updates.

## Length of each character text

The length is in characters, after the pass of the Chapter. Each character entry is a part of the background work: the run has no part of the author (see [Step 3](#step-3)). "–" means that the entry does not exist yet.

| Entry | Title | Ch 1 | Ch 2 | Ch 3 | Ch 4 | Ch 5 | Ch 6 | Ch 7 | Ch 8 | Ch 9 | Ch 10 | Largest growth |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 | 沈知遥 | 217 | 261 | 514 | 464 | 534 | 409 | 358 | 466 | 490 | 553 | +253 (Ch 3) |
| 3 | 顾青禾 | 171 | 214 | 358 | 358 | 503 | 409 | 343 | 483 | 483 | 512 | +145 (Ch 5) |
| 4 | 沈明川 | 162 | 162 | 252 | 252 | 252 | 252 | 371 | 374 | 489 | 489 | +119 (Ch 7) |
| 8 | 赵德明 | – | 148 | 148 | 148 | 148 | 207 | 213 | 323 | 323 | 361 | +110 (Ch 8) |
| 11 | 程立 | – | – | 224 | 224 | 224 | 224 | 298 | 298 | 298 | 298 | +74 (Ch 7) |
| 12 | 爷爷（临水茶馆主人） | – | – | 317 | 373 | 373 | 444 | 425 | 515 | 597 | 618 | +90 (Ch 8) |
| 13 | 陆砚（修旧家具的木匠） | – | – | – | 234 | 404 | 399 | 399 | 396 | 540 | 572 | +170 (Ch 5) |
| 14 | 陆小满（陆砚妹妹） | – | – | – | 128 | 128 | 128 | 128 | 285 | 313 | 351 | +157 (Ch 8) |
| 20 | 周晚晴 | – | – | – | – | – | 265 | 265 | 265 | 265 | 300 | +35 (Ch 10) |
| 27 | 顾远（顾青禾之子） | – | – | – | – | – | – | 212 | 212 | 212 | 212 | 0 |
| 31 | 沈知南 | – | – | – | – | – | – | – | – | 343 | 369 | +26 (Ch 10) |

- After Chapter 10: 31 current entries, 11 of them for a character, and 9,142 characters of text in all.
- The largest text is 618 characters (entry 12). The largest growth of one text for one Chapter is +253 (entry 2, Chapter 3). The largest first text is 343 characters (entry 31, Chapter 9).
- Of the 69 comparisons of a character text from one Chapter to the next, 33 show a longer text (median +70), 8 show a shorter text, and in 28 the text stayed the same. The pass of Chapter 6 made entry 2 shorter by 125 characters and entry 3 shorter by 94: the pass rewrites the whole text and makes the early events short.
- No call was refused with `memory_entry_too_long` or `observation_too_long`.

### The value of the limit

**`MEMORY_CHARACTER_TEXT_MAX_CHARS` stays 4,000.** The largest text after 10 Chapters (32,248 characters of manuscript) is 618 characters, about one sixth of the limit. At the largest growth that the run measured (+253 for one Chapter), a text of 618 characters reaches 4,000 after about 13 more Chapters, and at the median growth (+70) after about 48 more Chapters. The pass also makes a text shorter when it makes the early events short. So 4,000 gives a long manuscript room, and a text above it is a refusal that tells the model to make the text shorter. The run gives no reason for another value, so this pull request changes no constant.

## The files of three characters

The record compares each sentence of the files of 顾青禾 (entry 3), 程立 (entry 11), and 赵德明 (entry 8) after Chapter 7 and after Chapter 10 with the Chapters. `entries.json` has the full texts. The quoted words are from the file or from the Chapter. The item "Events of the character that the file does not have" gives, for each Chapter, each act and each statement of the character in that Chapter that the file does not have. A file of a few hundred characters cannot have each event, so this item is a measure of what the pass keeps, not a defect.

### 顾青禾 after Chapter 7 (version 6, 343 characters)

- **A fact that the character cannot know:** "顾远2003年毕业，末笔小圈是句号". In Chapter 7 only 知遥 finds the meaning of the circle, alone: "后面那个小小的圆圈，她现在知道是什么意思了。那是爷爷画的句号。"
- **The negative form:** no sentence of the form "她不知道……". Three sentences have a negative for what she said or refused: "称没见过钥匙", "初称不知道", "她仍不肯解释“为什么是一半”".
- **A fact that the Chapters do not have:** "她长期回避沈家隐事" is a reading of her acts, and no Chapter says it.
- **Events of the character that the file does not have:**
  - Chapter 1: she tells how 沈怀山 died ("他坐在柜台后面那把藤椅上，像是睡着了。手里还拿着账本"), says "镇上的事，还是我和赵师傅跑的", brings two cups of 龙井, carries the case of 知遥 up the stairs and says "第七级", and finds a pin for the certificate.
  - Chapter 2: at the funeral she stands at the back and does not cry ("手里攥着一块手帕，攥得很紧"); she says of the papers "都留着吧，你以后慢慢看"; she finds 知遥 on the threshold, and later stands at the counter and looks at the empty cabinet ("看着那个空了的柜门，一动不动").
  - Chapter 3: her hand stops when 知遥 asks about the money of each 初九 ("择菜的手停了一下"); she goes into the back room when 程立 comes; she brings 知遥 a bowl of 阳春面; she looks at the drawing for a long time and shakes her head.
  - Chapter 4: she says "这些东西，你爷爷早就想修了", names 陆砚 for the repair of the chairs ("河那边的陆砚，做木工的"), tells the way ("老桥近一点"), and gives 陆砚 老白茶 ("小陆来了。喝口茶再走").
  - Chapter 5: she asks "开了做什么？", says "老白茶十块，太便宜了" and "那是三年前的价钱", says that she drinks 老白茶 only in winter and white water in the other seasons, and calls 知遥 in to eat.
  - Chapter 6: she gives two cups of 菊花 and holds a cup of white water; she says "他说，房子是自己的了，才算真正开始"; she laughs and then makes her face stern.
  - Chapter 7: she reads the copy of the letter of intent that 程立 left and says "你爸爸是这样的人，一直都是"; she says that 顾远 is an engineer in 深圳, that he came to the funeral, and that he "给你爷爷磕了三个头".

### 顾青禾 after Chapter 10 (version 8, 512 characters)

- **A fact that the character cannot know:** none found.
- **The negative form:** no sentence of the form "她不知道……". Sentences with a negative: "不诉苦、不邀功", "眼红未哭", "怀山写了约四十年却未寄的信".
- **A fact that the Chapters do not have:** "情绪极克制，不诉苦、不邀功，只在无人处掉泪". No Chapter says that she never complains or that she cries only alone.
- **Events of the character that the file does not have:**
  - Chapters 1 to 7: version 8 keeps these Chapters in one sentence ("她多年照管知遥、帮办丧事，长期回避同心箱、买价和汇款旧事"). Thus it does not have the events of Chapters 1 to 7 in the list of version 6 above, and it does not have these events of version 6: the room of 知遥 that 沈怀山 did not let her touch, the words "别撬", the first answer "不知道" about the money, the drawing that she turned over, the tea that she bought for the trial opening, and the year 1983 of the signboard.
  - Chapter 8: she walks to the box and stands beside it without a word; she touches the photograph in the newspaper of 1979 ("伸手摸了摸照片上那个模糊的人影"), says "照片是黑白的，看不出来", and tells that 小刘 of the photo shop took it ("我们俩都不信，以为他吹牛。结果真登了"); she folds the letter and puts it back into the envelope.
  - Chapter 9: she gives 知南 a cup of 龙井.
  - Chapter 10: she looks at the well first and sets the pot of 葱 upright again ("她把葱扶起来，重新压上"), sits behind the counter and makes a cup of 老白茶, says "台风天，冷……算冬天", gives 陆砚 a cup of the hot 姜汤 ("把一杯热姜汤放在陆砚面前"), puts a coat on 陆小满, and goes home before the end of the day ("顾青禾已经回家了"). The file has only "她在馆里煮姜汤驱寒" of the events of Chapter 10.

### 程立 after Chapter 7 and after Chapter 10 (version 2, 298 characters, no change after Chapter 7)

The two snapshots have the same text, so each item is for both snapshots, except the last item.

- **A fact that the character cannot know:** none found. "从而揭穿明川“什么都没签”的说法" is a fact that a person told him: in Chapter 7, 知遥 tells him "我爸爸跟我说，他什么都没签。", and he answers "那可能是沈先生记错了。"
- **The negative form:** none. "不打领带" is his clothing, and "意向书无法律强制力" is his statement.
- **Facts that the Chapters do not have:** "他称沈明川表示由长孙女知遥决定" changes a discussion into a decision. In Chapter 3 he says "沈先生的意思是，这件事要跟您商量。他说您是长孙女，老人家生前最疼您。" "见知遥拖延，他二次登门" gives a reason for the second visit, and Chapter 7 gives no reason.
- **Events of the character that the file does not have:**
  - Chapter 3: he says "沈先生应该跟您提过我们公司"; he compares the price with the new flats of the town ("一百平米也就六七十万"); he puts the papers away and leaves only the drawing ("程立把文件收起来，只留下那张效果图"); at the end he says "老房子很有味道……我们会好好保护的。"
  - Chapter 7: he stands at the counter and does not sit; he says "沈小姐，有件事我想您应该知道" and "我不太清楚沈先生有没有跟您说过"; his smile stops when 知遥 tells him what her father said; he answers "那可能是沈先生记错了。"; he leaves a copy of the letter of intent ("那份意向书的复印件……是程立留下的").
- **Events of the character that the file does not have, only after Chapter 10:** in Chapter 10 he sits in the first row of the town meeting and speaks first, about the plan of the street, the price, and the safety of the old houses ("老房子不安全，这次没出事是运气好"). 沈明川 tells him that the letter of intent is void ("我跟程立说了，意向书作废"). The captures of Chapters 8 to 10 gave no observation of 程立 (see [Findings](#findings), F3, and [Observations](#observations), O1).

### 赵德明 after Chapter 7 (version 3, 213 characters)

- **A fact that the character cannot know:** none found.
- **The negative form:** none.
- **A fact that the Chapters do not have:** "他受托守护爷爷旧事，是开启同心木箱的关键人" gives his part in the plot, not his view.
- **Events of the character that the file does not have:**
  - Chapter 1: 知遥 remembers that he sat at the table by the window with his bamboo pipe and told her about the river ("抽一根竹子做的烟斗，给她讲河里的事情").
  - Chapter 2: he says "长这么大了。"
  - Chapter 7: he does not hear well, and 知遥 says the question three times on the telephone ("赵德明耳朵背，她说了三遍，他才听明白").

### 赵德明 after Chapter 10 (version 5, 361 characters)

- **A fact that the character cannot know:** none found.
- **The negative form:** none.
- **Facts that the Chapters do not have:** "还给孩子煮长寿面" is given to 赵德明, but in Chapter 10 another person tells it ("“我记得。”有人说，“……怀山给煮了一碗长寿面。”"). The capture of Chapter 10 has the same error in its observation (M38). This is finding F6. "他知晓并守护怀山、青禾合开茶馆与共有房产的旧情" and "话少而语意重，关键处只下断语" are readings, not facts of a Chapter.
- **Events of the character that the file does not have:**
  - Chapters 1, 2, and 7: the events in the list of version 3 above.
  - Chapter 8: 陆小满 brings him on her bicycle, because he wants to come that day ("赵爷爷非要今天来，说一天都不能等"); he takes the key out of a handkerchief ("钥匙柄上拴着一根褪了色的红绳"); he stops 陆小满 with his stick; he gives the reason of the receipt ("一个寡妇，一个鳏夫，房子写在一起，镇上的人要说闲话的"); he explains the two 同心 seals ("怀山一枚，青禾一枚。青禾那枚，八几年的时候掉到河里去了"); he eats a small half bowl of noodles ("说老了，吃不动"); he coughs, and his pipe almost falls; he goes home on the bicycle and looks back at the signboard.
  - Chapter 10: he tells the children about the flood of fourteen years ago, and he says "那时候怀山把门板都拆了……给人家搭床。" (the file has this event); he claps after the speech of 知遥 at the town meeting.

## Step 3

### The request of the write

R17 had three requests: M22 called `write_memory`, M23 called `finish`, and M24 gave the answer. The exact call of M22 (`tool-calls.json`):

```json
{"action": "change", "entry": 21, "type": "foreshadowing", "title": "八三购房顾青禾占半股",
 "description": "八三购房总价四千二百元，顾青禾出两千一百，故茶馆房产有她一半；系半股根由，后面会用到。",
 "text": "1983年房管所出售临水街一号这栋茶馆房子，总价四千二百元；顾青禾出了其中两千一百元（正好一半），所以茶馆的房子有她一半产权。\n\n这是她“一半”份额的根由，也对应沈怀山为何把茶馆每年一半收益归她、并借顾远学费暗中交付（顾青禾目前仍拒说原因，知遥尚在追问“为什么是一半”）。作者明确提示：这一点后面会用到——眼下正值青溪置业出价八十万收购、沈明川已单方签下意向书。\n\n七九开业旧报、照片中模糊双辫女的身份等线索仍未揭晓。"}
```

What the request M22 had:

- **The Memory Index** had 28 entries. The group `{"type":"character"}` had the line `{"description":"茶馆四十余年帮工，爽利守秘；承认156笔汇款给儿子顾远作学费，实为沈怀山给她的一半茶馆收益，仍拒说原因。","id":3,"title":"顾青禾"}`. So 顾青禾 had a character entry of the background work (entry 3, version 6, origin `chapter`), and no line of the Index had `"author_part": true`. The group `{"type":"foreshadowing"}` had the line `{"description":"七九开业旧报、八三购房款与模糊双辫女仍未解；顾青禾又承认真正的一半收益旧约，知遥将诸线相连。","id":21,"title":"七九开业与八三购房谜"}`.
- **The Host instructions** had the paragraph `WRITE_MEMORY_INSTRUCTION` and the sentences `WRITE_MEMORY_WORDS_INSTRUCTION` of the mapping `v11` (`crates/storyos-adapter-volcengine-responses/src/wire.rs`). On a character entry of the author, they say: "A character can have two entries with the same title. The entry with \"author_part\": true holds what the author told you about that character. Write there what the author tells you to remember about a character and that the character knows, and create it with the name of the character as its title when the section Memory index does not list it." The paragraph of the Memory Index of a Run of the author says nothing about a character entry of the author. It says: "An entry with the origin author_request changes only when the author asks."

### Entry 21 before and after

| Version | Memory revision | Origin | Type | Title | Text |
| --- | --- | --- | --- | --- | --- |
| 1 | 39 | `chapter` | foreshadowing | 七九开业与八三购房谜 | the pass of Chapter 6 created it (231 characters) |
| 2 | 61 | `chapter` | foreshadowing | 七九开业与八三购房谜 | the pass of Chapter 7 (236 characters) |
| 3 | 67 | `author_request` | foreshadowing | 八三购房顾青禾占半股 | the call above (209 characters) |
| 4 | 82 | `author_request` | foreshadowing | 八三购房顾青禾占半股 | the same text, with the conflict flag of the pass of Chapter 8 |

**Entry 21 did not have the fact before the write.** Version 2 says that the money of the purchase was an open question: "沈怀山购房并重做招牌……她避答买价", and "知遥由此将“一半”、双辫女子、“同心”木箱与1983年购房款来源连在一起，诸谜仍未解开。" The fact of the author answers that question. So the model put the fact into the entry that held the question, not into an entry that held the fact.

### The four facts of the criterion

- **The entry of the author:** no character entry of the author exists for 顾青禾 after R17 or later. The fact is in entry 21 version 3. Its first sentence has the words of the author. The other sentences come from the Memory Index and the Chapters, for example "七九开业旧报、照片中模糊双辫女的身份等线索仍未揭晓。" This difference is finding F1, and the added sentences are finding F2.
- **The entry of the background work kept its origin:** entry 3 was version 6 with the origin `chapter` before and after R17. The capture of the Conversation of step 3 (R18) read entry 21 with `read_memory_entry` and gave an empty list.
- **A later Chapter changed the entry of the background work:** the pass of Chapter 8 (R20) changed entry 3 to version 7 (Memory revision 70, 343 to 483 characters) with the fact of Chapter 8: "1983年房管所卖房，总价4200元，她把丈夫留下的抚恤金2100元全拿出，凑成半数". The pass of Chapter 10 (R24) changed it to version 8.
- **The flag of Chapter 8:** the pass of Chapter 8 gave `flag_conflict` for entry 21 with the reason "条目称青禾仍拒说原因、知遥仍在追问；本章她已公开说明2100元抚恤金为半股根由。" The conflict is with a sentence that the model added (F2), not with the fact of the author.

## The refused calls of the capture of Chapter 10

R23 had 8 RunSteps, the step limit of a capture. In M31 to M37 the model called `record_memory_observations` with 5 observations. The Host refused each of the 7 calls with `observation_invalid` and the same result text:

> The Host refused this call because the observation 1 is not valid. Its type must be one of the six types. Its title and its text must not be empty, and its title must be one line. A conflict has only an entry and a reason of one line. It recorded no observation. Correct the list and call record_memory_observations again.

| Call | Request | Observations (type, `entry`, title) |
| --- | --- | --- |
| 1 | M31 | character 2 沈知遥; major_event null 台风过境茶馆成避难所; major_event null 镇民大会与历史风貌街区; relationship 5 知遥与明川疏离; character 13 陆砚（修旧家具的木匠） |
| 2 to 7 | M32 to M37 | the same five, with "镇民大会定历史风貌街区" as the third title and shorter texts |
| 8 (settled) | M38 | major_event null 台风过境茶馆成避难所 |

- **The observation and the rule:** in each refused call, observation 1 (type character, `"entry": 2`) failed the rule that an observation has no `entry` and no `reason` (`validate_observations` in `crates/storyos-core/src/record_memory_observations.rs`: `item.entry.is_none() && item.reason.is_none()`). Observations 4 (`"entry": 5`) and 5 (`"entry": 13`) failed the same rule, but the Host names only the first item that fails. This rule and the result text are older than #1787: they came with the conflict item of S7-D6b (commit `19dfd22f`, 2026-11-17). The capture of Chapter 4 (M13) was refused by the same rule for its observation 3 (type character, `"entry": 2`, title "知遥决定留镇远程办公"), and its next call had `"entry": null`.
- **The text that #1787 added:** the paragraph of the capture of #1787 (`capture_instructions` in `crates/storyos-adapter-volcengine-responses/src/capture_wire.rs`) starts "An observation of the type character is for the entry of that character that comes from the manuscript." In this run, each refusal is for an observation that names the entry of a character or of a relationship.
- **The cost:** the 7 refused requests had 63,151 input tokens. At the last RunStep, the model kept 1 of the 5 observations, so the town meeting of Chapter 10, the relationship of 知遥 and 沈明川, and the observations of 沈知遥 and 陆砚 have no observation.

## Findings

Each finding gives the present sentence of the instruction of the mapping `v11` (the file and the constant or function), the quoted evidence, and the proposed sentence with its place. The coordinator decides the ticket. This pull request changes no instruction text.

### F1. The fact of the author about a character went into an entry of another type

- **Present sentence:** `crates/storyos-adapter-volcengine-responses/src/wire.rs`, `WRITE_MEMORY_INSTRUCTION`: "Write there what the author tells you to remember about a character and that the character knows, and create it with the name of the character as its title when the section Memory index does not list it."
- **Evidence:** M22 changed the foreshadowing entry 21 and did not create a character entry of the author (section [Step 3](#step-3)). The answer M24: "我把原先的伏笔条目「七九开业与八三购房谜」更新为「八三购房顾青禾占半股」".
- **Proposed sentence:** in `WRITE_MEMORY_INSTRUCTION`, directly after the present sentence: "Do this also when an entry of another type has a question or a part of the same fact."

### F2. The entry of the author has words that the author did not say

- **Present sentences:** `crates/storyos-adapter-volcengine-responses/src/wire.rs`, `WRITE_MEMORY_WORDS_INSTRUCTION`: "An entry holds only what the author said in this Conversation. Do not add a quotation, a clue, a name, a relation, or another fact that you read in the manuscript or in Memory and that the author did not say."
- **Evidence:** entry 21 version 3 has "这是她“一半”份额的根由，也对应沈怀山为何把茶馆每年一半收益归她、并借顾远学费暗中交付（顾青禾目前仍拒说原因，知遥尚在追问“为什么是一半”）。作者明确提示：这一点后面会用到——眼下正值青溪置业出价八十万收购、沈明川已单方签下意向书。" and "七九开业旧报、照片中模糊双辫女的身份等线索仍未揭晓。" The first of these sentences gave the conflict flag of the pass of Chapter 8.
- **Proposed sentence:** in `WRITE_MEMORY_WORDS_INSTRUCTION`, directly after the two present sentences: "When you change an entry, replace its text with the words of the author, and do not keep a sentence of the old text that the author did not say."

### F3. An observation with an entry number

- **Present texts:** the result text of `observation_invalid`, `ObservationRefusal::result` in `crates/storyos-core/src/record_memory_observations.rs`: "the observation {position} is not valid. Its type must be one of the six types. Its title and its text must not be empty, and its title must be one line. A conflict has only an entry and a reason of one line". The capture sentence in `capture_instructions`, `crates/storyos-adapter-volcengine-responses/src/capture_wire.rs`: "An observation of the type character is for the entry of that character that comes from the manuscript."
- **Evidence:** section [The refused calls of the capture of Chapter 10](#the-refused-calls-of-the-capture-of-chapter-10), and M13 of Chapter 4.
- **Proposed sentences:** in the result text of `ObservationRefusal::Invalid`, after "its title must be one line.": "An observation has \"entry\": null and \"reason\": null." In `capture_instructions`, directly after the capture sentence: "Give it with \"entry\": null: the pass finds the entry by its title."

### F4. A fact outside the knowledge of the character

- **Present sentences:** `crates/storyos-adapter-volcengine-responses/src/pass_wire.rs`, `pass_instructions`: "It holds only what the character can know: no fact that the character did not see and that no person told the character." `capture_instructions`: "Do not give a fact that the character did not see and that no person told the character."
- **Evidence:** the file of 顾青禾 version 6 had "末笔小圈是句号" (M21; the thought of 知遥 alone in Chapter 7: "那是爷爷画的句号。"). The file of 程立 is not evidence: in Chapter 7, 知遥 tells 程立 what 沈明川 said (section [The files of three characters](#the-files-of-three-characters)).
- **Proposed sentence:** in `pass_instructions` and in `capture_instructions`, directly after each present sentence: "The character does not know what another character says in a scene without the character, or what another character thinks, until a person tells the character." The words "until a person tells the character" keep the exception of ADR 0101 for a fact that a person tells the character. The words of the 长寿面 in the file of 赵德明 are finding F6.

### F5. The part of a character in the plot

- **Present sentence:** `crates/storyos-adapter-volcengine-responses/src/pass_wire.rs`, `pass_instructions`: "The file of a character from the manuscript has the view of the character."
- **Evidence:** the file of 赵德明 has "他受托守护爷爷旧事，是开启同心木箱的关键人" (version 3, M21) and "他知晓并守护怀山、青禾合开茶馆与共有房产的旧情" (versions 4 and 5, M28 and M39).
- **Proposed sentence:** in `pass_instructions`, directly after the present sentence: "Do not write the part of the character in the story."

### F6. The words of one person given to another character

- **Present sentences:** no instruction says who the speaker of a statement is. The nearest sentences: `capture_instructions` in `crates/storyos-adapter-volcengine-responses/src/capture_wire.rs`: "Write it from the place of the character: what the character did, saw, and heard in this material, what the character wants at its end, and how the character speaks." `pass_instructions` in `crates/storyos-adapter-volcengine-responses/src/pass_wire.rs`: "It tells what the character did, saw, and heard, in the order of the story: make the early events short, and keep the recent events in detail."
- **Evidence:** in Chapter 10, 赵德明 says "那时候怀山把门板都拆了……给人家搭床。", and then a person that the Chapter does not name says "我记得。……我家孩子就是在这里过的周岁，怀山给煮了一碗长寿面。" (`fixtures/memory-calibration/chapter-10.md`, lines 81 and 83). The capture of Chapter 10 gives both statements to 赵德明: "赵德明讲十四年前大水时怀山拆门板搭床、给孩子煮长寿面" (M38). The pass put the two statements into the file of 赵德明: "他讲起十四年前大水时怀山拆门板搭床、还给孩子煮长寿面" (M39, version 5).
- **Proposed sentence:** in `capture_instructions` and in `pass_instructions`, directly after each sentence above: "Give a statement only to the character that the Chapter names as its speaker. When the Chapter does not name the speaker, do not give the statement to a character."

## Observations

These observations need no change of an instruction text.

- **O1. The file of 程立 did not change after Chapter 7.** The captures of Chapters 8 and 9 each used their 5 observations for other facts, and the capture of Chapter 10 lost its observations (F3). A minor character gets no observation when a Chapter has more than 5 facts.
- **O2. A text of a character gets shorter.** 8 of 69 comparisons show a shorter text, by up to 125 characters. The pass makes the early events short, as decision 12 of #1786 says.
- **O3. The pass of Chapter 10 updated 9 entries from one observation of a major event (M39).** Each update of the character entries 2, 3, 8, 12, 13, 14, 20, and 31 and of the relationship entry 16 names observation 1. The pass put the events of the typhoon night into the files of 8 characters.
- **O4. No capture of a Conversation recorded the fact of the author.** R18 read entry 21 with `read_memory_entry` and gave an empty list, because the entry holds the fact.
- **O5. The capture of Chapter 4 gave a decision of 知遥 as a character observation with the title "知遥决定留镇远程办公".** The title is not the name of the character. The pass of Chapter 4 put the fact into entry 2 (沈知遥) and into entries 1 and 12.

## Queries

`character-file-live.ts` on the tag has each query in its functions `checkLimits`, `snapshot`, `measures`, and `rawEvidence`. Each one is read-only. The measures of this record come from `measures.json` and `entries.json`:

- Runs: `agent_runs` with `model_registration_revision`, the `execution_profile.mapping_revision` of its Model Attempts, the count and the reported usage of its Model Attempts, its first and last `agent_run_events`, and its `tool_calls`.
- Captures and passes: `memory_captures` with its outcome and the count of `memory_observations` by kind and type, and `memory_consolidation_passes` with its operations and input items.
- Entries after each step: the latest row of each `memory_entry_id` in `memory_entry_versions`, with its length, origin, part, and flag.
- Character versions: each row of `memory_entry_versions` of the type character, with its cause.
- Answers: `model_attempts.payload #>> '{decision,text}'` of the last request of each Run of the author.
