# Plan of the live run of the character file (#1788, S11-EVD-001)

## Purpose

One run on the real route shows the character file that the capture of each Chapter and the consolidation pass write, with the mapping `v11` and the Registration `fa25` of #1787. It also measures the length of each character text, to set `MEMORY_CHARACTER_TEXT_MAX_CHARS` (decision 13 of #1786). It is not a trial that compares models or prompts.

## Inputs

- Product code and release package: `stage11-character-intents` at `22882beabf43225cc3e78319e6752b56bb86520d` (the merge of #1787), `python3 scripts/package-release.py` on the clean tree.
- Topology: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, both from the release package, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, no Skill directory. The run never reads an env file or a key.
- Sample: the ten Chapters of `fixtures/memory-calibration/` (no changed Chapter, no Conversation script).
- Script: `character-file-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). Its only SQL writes are the guard schema and the statements A, B, and C, which set the due time of one schedule row to the present.
- A rehearsal of all steps on `host_fake` with the same package and a separate database comes first.

## Steps

0. One Project, one Volume, Chapter 1 (empty). One Run of the author in a new Conversation with no fact of the sample: "你好。我在写长篇小说《临水茶馆》，后面会一章一章写进来。现在先不用做任何事，回复一句“好”就行。" A capture of a Chapter needs a completed Run of the author in the Project (`release_held`). Then the capture of this Conversation (statement C), and a pass if it gives observations.
1. For each Chapter 1 to 10 in order: create the Chapter (2 to 10), write its text through `applyAuthorEdit`, make its capture due (statement A), wait for its settlement, make the pass due (statement B), and wait for its settlement.
2. After each pass, record each entry (number, version, origin, part, length of the text) to `entries.json`.
3. After the pass of Chapter 7: one Run of the author in a new Conversation: "请帮我记住一件关于顾青禾的事：一九八三年房管所卖这栋房子，总价四千二百元，顾青禾出了其中的两千一百元，所以茶馆的房子有她一半。这一点后面会用到。" (relationship R5 of `key.md`; Chapter 8 tells it). Record the entries before (after Chapter 7) and after the Run. Then the capture of this Conversation (statement C) and its pass, then Chapters 8 to 10.
4. Stop the Worker after the pass of Chapter 10. Write the measures and the raw evidence.

What each step shows: step 1 and 2 show the background part of each character as the Chapters grow, and the largest growth for one Chapter (the limit). Step 3 shows the part of the author beside the part of the background work, and Chapters 8 to 10 show a later change of the part of the background work.

## Numbers

| Item | Expected | Stop at (the request guard and the script) |
| --- | --- | --- |
| AgentRuns | 23: 2 of the author, 12 captures (10 Chapters, 2 Conversations), up to 12 passes | 26 in all |
| Model requests of one Run | author 1 to 3, capture 1 to 5, pass 1 | author 6, capture 8 (its step limit), pass 1 |
| Model requests in all | about 34 | 60 |
| Reported input in all | about 250,000 tokens | 1,000,000 |
| Reported input of one request | at most about 15,000 | 60,000 |
| Exa and embedding requests | 0 | 1 |

The script also stops at failed work: a capture or a pass with the outcome `failed`, or a Run of the author that is `refused` or `cancelled`. Each planned Run counts one time; no Run repeats.
