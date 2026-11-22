# Plan of the live run of the intent pass (#1792, S11-EVD-002)

## Purpose

One run on the real route shows the intent and the questions that the intent pass writes for each character, and a continuation of the author with the intents in the Memory index (ADR 0101 sections 4 and 5). The fake model does not read the instructions, so only a live run shows these outputs. The run does not assert that the model obeys a rule. It is not a trial that compares models or prompts.

## Inputs

- Product code and release package: `stage11-character-intents` at `1be11bb5d10b90af4227ee33380fbaf009f6fa9a` (the merge of #1791, tree `7c7e93ee4b809ea53016c1d8b4240f1f6e7b19b0`), `python3 scripts/package-release.py` on the clean tree (manifest `sha256:f2827e8d4cdb8464ab66b2e2c14f19ffa46c2430f2b5d29d99f6e86320baf7d1`).
- Topology: `storyos-server` with `STORYOS_WORKER=0` and `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and one continuous `storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, both from the release package, on a new `scripts/dev-postgres.sh with` database. No embedding destination, no Exa key, no Skill directory. The Worker resolves the key by its typed settings (ADR 0050). The run never reads an env file or a key.
- Sample: Chapters 1 to 7 of `fixtures/memory-calibration/` (no changed Chapter).
- Script: `intent-live.ts`, started by `rehearse.sh` (the request guard, then `run.sh`). Its only SQL writes are the guard schema and the statements A (a Chapter capture due now), B (a consolidation pass due now), and C (a Conversation capture due now). No statement touches the intent pass row: each intent pass starts only by the rules of #1790 (the consolidation work first, 1 pass in a minute). No statement writes a Chapter or a Memory Entry.
- A rehearsal of all steps on `host_fake` with the same package and a separate database comes first.

## Steps

0. One Project, one Volume, Chapter 1 (empty). One Run of the author in a new Conversation with no fact of the sample: "你好。我在写长篇小说《临水茶馆》，后面会一章一章写进来。现在先不用做任何事，回复一句“好”就行。" A capture of a Chapter needs a completed Run of the author in the Project. Then the capture of this Conversation (statement C), and a pass if it gives observations.
1. For each Chapter 1 to 7 in order: create the Chapter (2 to 7), write its text through `applyAuthorEdit`, make its capture due (statement A), wait for its settlement, make the pass due (statement B), and wait for its settlement. The script does not wait for the intent passes between the Chapters (coordinator decision (a)). After Chapter 7, the script waits until the background work is idle: no open Run, capture, or pass, no waiting observation, and no due time on the intent pass row, at two polls in a row.
2. One Run of the author in a new Conversation: "请帮我记住一件关于顾青禾的事：一九八三年房管所卖这栋房子，总价四千二百元，顾青禾出了其中的两千一百元，所以茶馆的房子有她一半。这一点后面会用到。" (relationship R5 of `key.md`). Then the capture of this Conversation (statement C), its pass, and the idle wait, which includes the new intent pass of 顾青禾.
3. One Run of the author in a new Conversation: "赵德明接下来想做什么？" Then statement C, the capture, the pass, and the idle wait.
4. One Run of the author in a new Conversation on Chapter 7: "接着第七章写下一场。" Then statement C, the capture, the pass, and the idle wait.
5. Stop the Worker. Write the measures and the raw evidence.

What each step shows:

| Step | What it shows |
| --- | --- |
| 1 | Each intent pass: the character, the Registration and the mapping, the requests, the reported tokens, the outcome, the intent, the questions, a line break in the intent, and a user message with only the section `Character file`. The files of 顾青禾, 程立, and 赵德明 after Chapter 7 (findings F4, F5, F6 of #1788). For each capture, the `observation_invalid` refusals (F3). |
| 2 | The intent and the questions of 顾青禾 before and after the fact of the author, the question that the fact answers, and the entry and version that got the fact (F1, F2). |
| 3 | The complete answer, its agreement with the intent of 赵德明, and no intent pass that the Run starts. The observations of the capture, and if one of them is the intent of 赵德明. |
| 4 | The Memory index of the first request with the intents, each candidate text of the Proposal, the final answer, the character that acts, an action that agrees with an intent, and each event that the Chapters and the file do not support. The observations of the capture. |

## Numbers

| Item | Expected | Stop at (the request guard and the script) |
| --- | --- | --- |
| AgentRuns | about 46: 4 of the author, 11 captures (7 Chapters, 4 Conversations), about 9 consolidation passes, about 22 intent passes (10 to 31) | 60 in all |
| Model requests of one Run | author 2 to 4, capture 1 to 5, consolidation pass 1, intent pass 1 | author 6, capture 8, consolidation pass 1, intent pass 2 (`INTENT_PASS_MAX_RUN_STEPS`) |
| Model requests in all | about 63 | 100 |
| Reported input in all | about 350,000 tokens | 1,000,000 |
| Reported input of one request | about 20,000 | 60,000 |
| Exa and embedding requests | 0 | 1 (the guard refuses the first one) |

The script also stops at failed work: a capture or a consolidation pass with the outcome `failed`, a Run of the author that is `refused` or `cancelled`, or 2 intent passes with the outcome `failed`. Each planned Run counts one time; no Run repeats. Time: about 80 minutes.
