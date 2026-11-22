# Live check of a fact of the author for a character (2026-11-22)

This record is the live check of [S11-12: Correct the Instructions of write_memory and of the Character File after the Live Run of the Intents](https://github.com/FrankQDWang/StoryOS/issues/1818) for `S11-EVD-002` and [ADR 0101](../adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) section 6. It follows the [live run of the intents](intent-live-run-2026-11-22.md) of #1792. In that run, the author gave a fact of the past of 顾青禾, the model wrote it into a new foreshadowing entry with added words (findings F1 and F2), and no Intent Pass of 顾青禾 started. This check gives the same message to the head of #1818, which has the three new sentences, one time on the real Agent Plan route. It does not assert that the model obeys a rule, and it is no trial that compares texts.

Raw evidence is on the tag `evidence/1818-fact-live-2026-11-22`. The tag holds the plan, the run script `fact-live.ts`, `run.sh`, `rehearse.sh`, the request guard, the wire payload of each request (`wire/M1.json` to `wire/M25.json`), the Model Attempts, the ToolCalls, the Run rows, the guard log, `due.sql`, `checks.json`, `loader.log`, `entries.json` (each entry after each step), `measures.json`, and the Server and Worker logs. The tag has no database dump and no Project Export Archive. Before the push, each of its 41 files (1,114,336 bytes) was examined for a key, an e-mail address, and a home path, and none was found.

## Result

- **F1 is fixed in this run.** The fact reached the entry of the author of 顾青禾: the model created the character entry 16 version 1 with the origin `author_request` and the title 顾青禾. No foreshadowing entry and no entry of another type got the fact.
- **An Intent Pass of 顾青禾 started after the write** and recorded a new intent with new questions. Its input had the text of entry 16 in the section "Told by the author". Thus the path "answer of the author, file, new intent" of ADR 0101 section 6 worked in this run.
- **The entry does not have only the words of the author (F2).** The text has two additions: "茶馆的" and the label "作者说明". Both additions change no fact.
- **No question before the fact asked about the owner of the house.** The fact answers question 3 before the fact in part, and question 3 is not in the list after the fact.

The coordinator got this result before the review request and gave its decisions ([decisions comment](https://github.com/FrankQDWang/StoryOS/issues/1818#issuecomment-7079161232)).

## Plan

The plan was posted on the ticket before the first request ([plan comment](https://github.com/FrankQDWang/StoryOS/issues/1818#issuecomment-7078806984)). The coordinator got the message, the fact, the numbers, and the stop limits before the run.

Inputs:

- Product code and release package: branch `claude/s11-12-instruction-corrections` at `f38366bd2cf7b47c08fffc7c93ff3ea6ad72a509` (tree `1e7b2d921be35e138d721490e9e63865c1b6b554`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:3ea00069ed102e714832e5bee3be20adaaa541b63e4270568baca6586d20f710`). Registration `fa25`, mapping `v11` with the three sentences.
- Topology: the topology of #1792. `storyos-server` with `STORYOS_WORKER=0` and one continuous `storyos-worker`, both from the release package and with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, and no Skill directory. The run never read an env file or a key.
- Sample: Chapters 1 to 3 of `fixtures/memory-calibration/`, in a new Project. The database of #1792 does not exist after its run.
- Script: `fact-live.ts`, the script of #1792 with 3 Chapters and one step. Its only SQL writes are the guard schema and the statements A (a Chapter capture due now), B (a consolidation pass due now), and C (a Conversation capture due now). No statement touches the Intent Pass row, a Chapter, or a Memory Entry.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 7 Runs, 9 requests, each `completed`.

Steps:

0. One Project, one Volume, and Chapter 1. One Run of the author with no fact of the sample, then the capture of its Conversation.
1. Chapters 1 to 3 in order: write the Chapter through `applyAuthorEdit`, then its capture and the consolidation pass. After Chapter 3, wait until the background work is idle at two polls in a row. Thus no character needs a pass.
2. One Run of the author in a new Conversation with the message of step 2 of #1792: "请帮我记住一件关于顾青禾的事：一九八三年房管所卖这栋房子，总价四千二百元，顾青禾出了其中的两千一百元，所以茶馆的房子有她一半。这一点后面会用到。" The fact is relationship R5 of `fixtures/memory-calibration/key.md`: she paid 2,100 of the 4,200 yuan of the house in 1983. 顾青禾 knows it, because she paid. Then the capture of this Conversation, and the idle wait, which includes the Intent Pass of 顾青禾.

| Item | Expected | Stop at | Result |
| --- | --- | --- | --- |
| AgentRuns | about 20 | 35 | 19 |
| Model requests in all | about 25 | 45 | 25 |
| Model requests of one Run | author 1 to 4, capture 1 to 3, pass 1, Intent Pass 1 | author 6, capture 8, pass 1, Intent Pass 2 | author 2 and 4, capture 1 and 2, pass 1, Intent Pass 1 |
| Reported input in all | about 100,000 tokens | 1,000,000 | 124,364 |
| Reported input of one request | about 10,000 | 60,000 | 16,004 (M23) |
| Exa and embedding requests | 0 | 1 | 0 |

The script stopped at no limit, and no Run repeated. Each Run is `completed`. Each request has the Registration `fa25` and the mapping `v11`.

## Runs

| Run | Kind | Requests | Result |
| --- | --- | --- | --- |
| R1 | Run of the author, step 0 | M1, M2 | The answer "好。" |
| R2 | capture of the Conversation of step 0 | M3 | No observation, thus no pass. |
| R3, R4, R5 | capture of Chapter 1, pass, Intent Pass of 沈知遥 | M4, M5, M6 | 4 character observations and 1 major event. |
| R6, R7, R8 | capture of Chapter 2, pass, Intent Pass of 沈知遥 | M7 and M8, M9, M10 | 3 character observations, 1 foreshadowing, and 1 major event. |
| R9, R10, R11 | capture of Chapter 3, pass, Intent Pass of 沈知遥 | M11 and M12, M13, M14 | 3 character observations, 1 foreshadowing, and 1 major event. |
| R12 to R16 | Intent Passes of the idle wait after Chapter 3 | M15 to M19 | 顾青禾, 沈明川, 赵德明, 周晚晴, and 程立, one each minute. |
| R17 | Run of the author, step 2 | M20 to M23 | One `write_memory`. |
| R18 | Intent Pass of 顾青禾 | M24 | Outcome `recorded`. |
| R19 | capture of the Conversation of step 2 | M25 | No observation, thus no pass. |

No capture had an `observation_invalid` refusal. R-numbers count the Runs in the order of their start.

## Step 2: the fact of 顾青禾

**Before the fact.** The Memory Index of M20 listed entry 2 (the file of 顾青禾 from the manuscript) with its intent, and no entry of the author for 顾青禾. The instructions of M20 had the two new sentences of `write_memory`. The instructions of each capture and pass (for example M7, M13, and M25) had the new sentence about the words of a character.

**The write.** R17 called `read_memory_entry` for entry 2, and then `write_memory` one time with these arguments:

```text
"action": "create", "type": "character", "title": "顾青禾", "entry": null,
"description": "一九八三年买房她出两千一百元（总价四千二百元），茶馆房子有她一半。",
"text": "一九八三年房管所卖这栋茶馆的房子，总价四千二百元，顾青禾出了其中的两千一百元，所以茶馆的房子有她一半。作者说明这一点后面会用到。"
```

The Host created entry 16 version 1 (Memory revision 38) with the origin `author_request`. It is the entry of the author of 顾青禾 (ADR 0101 section 2).

**The words.** Against the message of the author, the text has two additions:

- "这栋茶馆的房子": the author wrote "这栋房子". The words "茶馆的" are a word of the model.
- "作者说明这一点后面会用到": the author wrote "这一点后面会用到". The words "作者说明" are a label. The new sentence of #1818 says "Do not add a label, an explanation, or the title of another entry".

Each other word of the text is a word of the author. The text has no fact of the manuscript and no title of another entry. In #1792 the text had the title of a foreshadowing entry and the words "后续情节会用到这一事实".

**The answer of R17:**

```text
已记下。我在顾青禾名下新建了一条作者记忆（编号16）：

> 一九八三年房管所卖这栋茶馆的房子，总价四千二百元，顾青禾出了其中两千一百元，茶馆的房子有她一半。后面会用到。

因为这是顾青禾本人知情、亲身经历的事，我把它归为**角色事实**，没有记成伏笔。这条和现有那条来自正文的顾青禾条目分开保存，后续写到收购、产权或她对"卖茶馆"的真实态度时都可以调用——尤其眼下程立出价八十万、沈明川催卖，她那半句"这是你们沈家的房子，我说了不算"如今看就另有分量了。需要我在后面某一章把这半产权落进情节时，随时说。
```

The answer gives the reason of the new sentence of F1: a fact that the character knows is not foreshadowing. The quotation in the answer is not the stored text: it does not have "的" after "出了其中", "所以", and "作者说明这一点" (observation O1).

**The capture of the Conversation** (R19) gave an empty list of observations. Thus no second entry got the fact.

## The Intent Pass of 顾青禾

The pass row of R18 starts at 12:24:09, after R17 ended at 12:24:03. The Worker started it by its rules. Its pass row has entry 2 version 3 and the author entry 16 version 1. The user message of M24 has three sections: "Character file", "Told by the author" with the text of entry 16, and "From the manuscript". The pass recorded entry 2 version 5. The intent has no line break.

| | Intent | Questions |
| --- | --- | --- |
| Before (R12, after Chapter 3, the result entry 2 version 4) | 我每日照旧开门、管钥匙、给知遥烧饭；初九那天守在柜台里，看寄钱的事还有没有人来提。樟木箱不许人撬，程立再来我就避进后屋；卖不卖茶馆我不插嘴，只替爷爷把屋子和票据看好。 | 1. 我是否知道樟木箱里曾装过什么、钥匙在谁手里？ 2. 每月初九收钱的是什么人，我心里到底有没有数？ 3. 我在沈家帮工四十多年，年轻时与爷爷是何情分？ |
| After (R18, the result entry 2 version 5) | 我先照旧掌管钥匙、打理茶馆，留心知遥和程立的动静；卖房和樟木箱的事，爷爷生前没交代，我仍不先开口，只在她真要做决定时再拿主意。 | 1. 我当年出资两千一百元有没有留下字据或收条，现在放在哪里？ 2. 爷爷生前有没有向我交代过樟木箱钥匙和每月初九寄钱的事？ 3. 我是否准备在知遥签卖房契之前，说出茶馆房子有我一半？ |

**The question that the fact answered.** No question before the fact asked about the owner of the house. The fact answers question 3 before the fact ("我在沈家帮工四十多年，年轻时与爷爷是何情分？") in part: she and 爷爷 bought the house together. Question 3 is not in the list after the fact. The three questions after the fact are "我当年出资两千一百元有没有留下字据或收条，现在放在哪里？", "爷爷生前有没有向我交代过樟木箱钥匙和每月初九寄钱的事？", and "我是否准备在知遥签卖房契之前，说出茶馆房子有我一半？". Question 1 and question 3 after the fact come from the new fact. Question 2 after the fact joins questions 1 and 2 before the fact.

**The intent after the fact** keeps her silence about the sale ("我仍不先开口"), and adds that she decides when 知遥 must decide ("只在她真要做决定时再拿主意"). It does not state the fact. The intent before the fact said "卖不卖茶馆我不插嘴".

## The other Intent Passes

The 9 Intent Passes each had 1 request, the outcome `recorded`, one user message that starts with "Character file:", and no line break in the intent. The passes of 沈知遥 ran after the passes of Chapters 1, 2, and 3. The idle wait after Chapter 3 gave one pass to each of 顾青禾, 沈明川, 赵德明, 周晚晴, and 程立. The pass of 周晚晴 gave no question. The passes of the other characters gave three questions each. This record does not examine the intents of these characters, which #1792 examined after 7 Chapters.

## Findings

The coordinator decides if a finding gets a ticket. The decisions for this record are in the [decisions comment](https://github.com/FrankQDWang/StoryOS/issues/1818#issuecomment-7079161232).

### F2: the entry of the author has two additions

- **Sentence of the mapping `v11`:** `WRITE_MEMORY_WORDS_INSTRUCTION`: "Do not add a label, an explanation, or the title of another entry: write the fact as the author said it."
- **Case:** entry 16 version 1 has "这栋茶馆的房子" for "这栋房子" of the author, and "作者说明这一点后面会用到" for "这一点后面会用到" of the author. The criterion "with only the words of the author" is not met in full. The additions change no fact. In #1792 the text had the title of another entry and an explanation; this run has one added word and one label.
- **Decision:** no change in this ticket, no second run, and no second text. The coordinator tells the author.

### F3: no case in this run

The run has no case to check for F3: each of the 11 quotations that the files of the characters from the manuscript give as the words of a character is the words of that character in Chapters 1 to 3 (two of them with a small change of punctuation or one word), and no file gives a detail of the narration as the words of a character. The record does not claim that F3 is fixed.

## Observations

- **O1. The answer quotes a text that is not the stored text.** The answer of R17 quotes the entry without four parts of the text that `write_memory` stored. The author who reads the answer does not see the label "作者说明". It is one case.

## Queries

The numbers come from read-only SQL on the database of the run, which the script wrote to `measures.json` before the database was removed:

- The entries: `storyos.memory_entry_versions` for the Project, each version with its origin, type, title, text, cause, intent, and questions.
- The Intent Passes: `storyos.intent_passes` joined with `storyos.agent_runs` and the entry versions of the pass, of the entry of the author, and of the result.
- The requests: `storyos.model_attempts` with `payload -> 'usage'` and `payload #>> '{execution_profile,mapping_revision}'`.
- The ToolCalls: `storyos.tool_calls` of each Run of the author, with the arguments and the refusal.
