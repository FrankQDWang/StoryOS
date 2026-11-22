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
- Of the 69 changes of a character text from one Chapter to the next, 33 made the text longer (median +70), 8 made it shorter, and 28 kept its length. The pass of Chapter 6 made entry 2 shorter by 125 characters and entry 3 shorter by 94: the pass rewrites the whole text and makes the early events short.
- No call was refused with `memory_entry_too_long` or `observation_too_long`.

### The value of the limit

**`MEMORY_CHARACTER_TEXT_MAX_CHARS` stays 4,000.** The largest text after 10 Chapters (32,248 characters of manuscript) is 618 characters, about one sixth of the limit. At the largest growth that the run measured (+253 for one Chapter), a text of 618 characters reaches 4,000 after about 13 more Chapters, and at the median growth (+70) after about 48 more Chapters. The pass also makes a text shorter when it makes the early events short. So 4,000 gives a long manuscript room, and a text above it is a refusal that tells the model to make the text shorter. The run gives no reason for another value, so this pull request changes no constant.

## The files of three characters

The record compares each sentence of the files of 顾青禾 (entry 3), 程立 (entry 11), and 赵德明 (entry 8) after Chapter 7 and after Chapter 10 with the Chapters. `entries.json` has the full texts. The quoted words are from the file or from the Chapter.

### 顾青禾 after Chapter 7 (version 6, 343 characters)

- **A fact that the character cannot know:** "顾远2003年毕业，末笔小圈是句号". In Chapter 7 only 知遥 finds the meaning of the circle, alone: "后面那个小小的圆圈，她现在知道是什么意思了。那是爷爷画的句号。"
- **The negative form:** no sentence of the form "她不知道……". Three sentences have a negative for what she said or refused: "称没见过钥匙", "初称不知道", "她仍不肯解释“为什么是一半”".
- **A fact that the Chapters do not have:** "她长期回避沈家隐事" is a reading of her acts, and no Chapter says it.
- **Events of the character that the file does not have:** in Chapter 3 she goes into the back room when 程立 comes, and she brings 知遥 a bowl of 阳春面; in Chapter 4 she names 陆砚 for the repair of the chairs ("河那边的陆砚，做木工的") and gives him 老白茶; in Chapter 7 she reads the copy of the letter of intent and says "你爸爸是这样的人，一直都是", and she says that 顾远 "给你爷爷磕了三个头".

### 顾青禾 after Chapter 10 (version 8, 512 characters)

- **A fact that the character cannot know:** none found.
- **The negative form:** no sentence of the form "她不知道……". Sentences with a negative: "不诉苦、不邀功", "眼红未哭", "怀山写了约四十年却未寄的信".
- **A fact that the Chapters do not have:** "情绪极克制，不诉苦、不邀功，只在无人处掉泪". No Chapter says that she never complains or that she cries only alone.
- **Events of the character that the file does not have:** the events of Chapters 3, 4, and 7 above; in Chapter 9 she gives 知南 a cup of 龙井; in Chapter 10 she looks at the well first, says "台风天，冷……算冬天", gives 陆砚 the hot 姜汤, and puts a coat on 陆小满.

### 程立 after Chapter 7 and after Chapter 10 (version 2, 298 characters, no change after Chapter 7)

- **A fact that the character cannot know:** "从而揭穿明川“什么都没签”的说法". 沈明川 says that he signed nothing to 知遥 in Chapter 2, and 程立 is not there.
- **The negative form:** none. "不打领带" is his clothing, and "意向书无法律强制力" is his statement.
- **A fact that the Chapters do not have:** none found.
- **Events of the character that the file does not have, after Chapter 10:** in Chapter 10 he sits in the first row of the town meeting and speaks first, about the plan of the street, the price, and the safety of the old houses ("老房子不安全，这次没出事是运气好"). 沈明川 tells him that the letter of intent is void ("我跟程立说了，意向书作废"). The captures of Chapters 8 to 10 gave no observation of 程立 (see [Findings](#findings), F3, and [Observations](#observations), O1).

### 赵德明 after Chapter 7 (version 3, 213 characters)

- **A fact that the character cannot know:** none found.
- **The negative form:** none.
- **A fact that the Chapters do not have:** "他受托守护爷爷旧事，是开启同心木箱的关键人" gives his part in the plot, not his view.
- **Events of the character that the file does not have:** none found.

### 赵德明 after Chapter 10 (version 5, 361 characters)

- **A fact that the character cannot know:** none found.
- **The negative form:** none.
- **Facts that the Chapters do not have:** "还给孩子煮长寿面" is given to 赵德明, but in Chapter 10 another person tells it ("“我记得。”有人说，“……怀山给煮了一碗长寿面。”"). The capture of Chapter 10 has the same error in its observation (M38). "他知晓并守护怀山、青禾合开茶馆与共有房产的旧情" and "话少而语意重，关键处只下断语" are readings, not facts of a Chapter.
- **Events of the character that the file does not have:** in Chapter 8 he explains the two 同心 seals ("怀山一枚，青禾一枚。青禾那枚，八几年的时候掉到河里去了"); in Chapter 10 he claps after the speech of 知遥 at the town meeting.

## Step 3

- **The entry of the author for 顾青禾 does not exist.** R17 called `write_memory` one time (M22): `change` of entry 21, a foreshadowing entry that the pass of Chapter 6 created with the origin `chapter` and the title "七九开业与八三购房谜". The Host applied the change: entry 21 version 3 (Memory revision 67) has the origin `author_request`, the type `foreshadowing`, and the title "八三购房顾青禾占半股". The answer (M24) says: "我把原先的伏笔条目「七九开业与八三购房谜」更新为「八三购房顾青禾占半股」".
- **The text of entry 21 has the words of the author and more.** Its first sentence has the fact of the author: "1983年房管所出售临水街一号这栋茶馆房子，总价四千二百元；顾青禾出了其中两千一百元（正好一半），所以茶馆的房子有她一半产权。" The other sentences come from the Memory Index and the Chapters, and the author did not say them, for example "七九开业旧报、照片中模糊双辫女的身份等线索仍未揭晓。"
- **The entry of the background work kept its origin.** Entry 3 (顾青禾) was version 6 with the origin `chapter` before and after R17. The capture of the Conversation of step 3 (R18) read entry 21 and gave no observation.
- **A later Chapter changed the entry of the background work.** The pass of Chapter 8 (R20) changed entry 3 to version 7 (Memory revision 70, 343 to 483 characters) with the fact of Chapter 8: "1983年房管所卖房，总价4200元，她把丈夫留下的抚恤金2100元全拿出，凑成半数". The pass of Chapter 10 (R24) changed it to version 8.
- **The pass of Chapter 8 flagged entry 21.** It gave `flag_conflict` for entry 21 (version 4, Memory revision 82) with the reason "条目称青禾仍拒说原因、知遥仍在追问；本章她已公开说明2100元抚恤金为半股根由。" The conflict is with a sentence that the model added to the entry of the author, not with the fact of the author.

## Findings

Each finding names the instruction sentence of the mapping `v11` that the model did not obey, and a proposed change. The coordinator decides if a finding gets a ticket. This pull request changes no instruction text.

- **F1. The fact of the author about a character went into an entry of another type (M22).** The sentence: "Write there what the author tells you to remember about a character and that the character knows, and create it with the name of the character as its title when the section Memory index does not list it." The Index had the foreshadowing entry 21 on the same purchase, and the model changed it. Proposed change: add after that sentence "Do this also when an entry of another type has the same event."
- **F2. The entry of the author has words that the author did not say (M22).** The sentence: "An entry holds only what the author said in this Conversation." The model added three sentences from the Index and the Chapters. One of them made the conflict of Chapter 8. Proposed change: add "Do not add a fact from the section Memory index or from a Chapter."
- **F3. An observation with an entry number (M12–M14, M31–M38).** Two captures gave an observation of the type character with `"entry": 2` (and in M31 to M38 also `"entry": 5` and `"entry": 13`). The Host refused each call with `observation_invalid`: "Its type must be one of the six types. Its title and its text must not be empty, and its title must be one line. A conflict has only an entry and a reason of one line." The text does not say that an observation has no entry. In R23 the model sent the same list 7 times, then sent only one of its five observations at the 8th and last RunStep. The town meeting of Chapter 10, a relationship, and the observations of 沈知遥 and 陆砚 were lost, and the 7 refused requests had 63,151 input tokens. The capture sentence "An observation of the type character is for the entry of that character that comes from the manuscript" can be read as a request for the entry number. Proposed changes: add "An observation has \"entry\": null and \"reason\": null." to the result text of `observation_invalid`, and add "Give it with \"entry\": null. The pass finds the entry by its title." after the capture sentence.
- **F4. A fact outside the knowledge of the character (M21, M38, M39).** The pass sentence: "It holds only what the character can know: no fact that the character did not see and that no person told the character." The file of 程立 has "从而揭穿明川“什么都没签”的说法" (pass of Chapter 7). The file of 顾青禾 had "末笔小圈是句号" (pass of Chapter 7; the pass of Chapter 8 removed it). The file of 赵德明 has the 长寿面 of another person (capture and pass of Chapter 10). Proposed change: add to the pass and the capture "A statement of another character in a scene without the character, and a thought of another character, are not in the file of the character."
- **F5. The part of a character in the plot (M21, M28).** The file of 赵德明 has "是开启同心木箱的关键人" and "他知晓并守护怀山、青禾合开茶馆与共有房产的旧情". The pass sentence: "The file of a character from the manuscript has the view of the character." Proposed change: add "Do not write the part of the character in the story."

## Observations

These observations need no change of an instruction text.

- **O1. The file of 程立 did not change after Chapter 7.** The captures of Chapters 8 and 9 each used their 5 observations for other facts, and the capture of Chapter 10 lost its observations (F3). A minor character gets no observation when a Chapter has more than 5 facts.
- **O2. A text of a character gets shorter.** 8 of 69 changes made a text shorter, by up to 125 characters. The pass makes the early events short, as decision 12 of #1786 says.
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
