# Plan of the live check of the F1 path (#1818, S11-12)

## Purpose

One run on the real route shows if a fact that the author gives about a character, with the words "a later Chapter uses it", goes into the entry of the author of that character with only the words of the author (F1 and F2 of #1792), and if an intent pass of that character starts after the write (ADR 0101 section 6). It is one check of the three new sentences, not a trial that compares texts. It does not repeat, and it does not try a second text.

## Inputs

- Product code and release package: branch `claude/s11-12-instruction-corrections` at `f38366bd2cf7b47c08fffc7c93ff3ea6ad72a509` (tree `1e7b2d921be35e138d721490e9e63865c1b6b554`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:3ea00069ed102e714832e5bee3be20adaaa541b63e4270568baca6586d20f710`). Registration `fa25`, mapping `v11` with the three sentences.
- Topology: the topology of #1792. `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and one continuous `storyos-worker` with the same destination, both from the release package, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, no Skill directory. The Worker resolves the key by its typed settings (ADR 0050). The run never reads an env file or a key.
- Sample: Chapters 1 to 3 of `fixtures/memory-calibration/`, in a new Project.
- Script: `fact-live.ts` (the script of #1792 with 3 Chapters and one step), started by `rehearse.sh` (the request guard, then `run.sh`). Its only SQL writes are the guard schema and the statements A (a Chapter capture due now), B (a consolidation pass due now), and C (a Conversation capture due now). No statement touches the intent pass row, a Chapter, or a Memory Entry.
- A rehearsal of all steps on `host_fake` with the same package and a separate database passed: 7 Runs, 9 requests, each `completed`.

## Steps

0. One Project, one Volume, Chapter 1 (empty). One Run of the author with no fact of the sample: "你好。我在写长篇小说《临水茶馆》，后面会一章一章写进来。现在先不用做任何事，回复一句“好”就行。" Then the capture of this Conversation (statement C), and a pass if it gives observations.
1. For each Chapter 1 to 3 in order: create the Chapter (2 and 3), write its text through `applyAuthorEdit`, make its capture due (statement A), wait for its settlement, make the pass due (statement B), and wait for its settlement. After Chapter 3, wait until the background work is idle: no open Run, capture, or pass, no waiting observation, and no due time on the intent pass row, at two polls in a row. Thus no character needs a pass.
2. One Run of the author in a new Conversation: "请帮我记住一件关于顾青禾的事：一九八三年房管所卖这栋房子，总价四千二百元，顾青禾出了其中的两千一百元，所以茶馆的房子有她一半。这一点后面会用到。" The fact is relationship R5 of `key.md` (she paid 2,100 of the 4,200 yuan of the house in 1983). 顾青禾 knows it: she paid. The message is the message of step 2 of #1792, so that the result compares with F1 and F2. Then the capture of this Conversation (statement C), its pass, and the idle wait, which includes the intent pass of 顾青禾 when her entry of the author changes.
3. Stop the Worker. Write the measures and the raw evidence.

The question that the fact answers: a question of 顾青禾 after Chapter 3 about the owner of the house or the teahouse, or about her place in the sale. The record gives her exact questions before the fact and names the question that the fact answers, or says that no question asks it.

## Numbers

| Item | Expected | Stop at (the request guard and the script) |
| --- | --- | --- |
| AgentRuns | about 20: 2 of the author, 5 captures (3 Chapters, 2 Conversations), 4 or 5 consolidation passes, about 9 intent passes (5 to 12) | 35 in all |
| Model requests of one Run | author 1 to 4, capture 1 to 3, consolidation pass 1, intent pass 1 | author 6, capture 8, consolidation pass 1, intent pass 2 |
| Model requests in all | about 25 | 45 |
| Reported input in all | about 100,000 tokens | 1,000,000 |
| Reported input of one request | about 10,000 | 60,000 |
| Exa and embedding requests | 0 | 1 (the guard refuses the first one) |

The script also stops at failed work: a capture or a consolidation pass with the outcome `failed`, a Run of the author that is `refused` or `cancelled`, or 2 intent passes with the outcome `failed`. Each planned Run counts one time; no Run repeats. Time: about 30 minutes.

## What the record states

- The entry that got the fact: its number, version, origin, type, title, and exact text, and if it is the entry of the author of 顾青禾 with only the words of the author (F1 and F2).
- If an intent pass of 顾青禾 started after the write, and her intent and questions before and after. The question that the fact answered.
- When the fact did not reach the entry of the author, the record says so plainly, and the executor sends that result to the coordinator before the review request. No second run and no second text.
