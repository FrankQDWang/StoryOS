# Paper trial: what a character wants to do next (2026-11-22)

This directory is the raw evidence of one live run on the Volcengine Agent Plan route. It is an evidence record of a design talk with the author, before a specification. It is not a ticket, and no product code changed.

## Question

The author wants the plot to grow from the characters. The trial asks one thing: when the model gets only the experience file of one character, does it give an intent for the next three days of story time that is specific, in character, and useful for the author?

## Method

- Sample: the original novel of `fixtures/memory-calibration/` (Chapters 1 to 7). The point of story time is the night of the Wednesday in Chapter 7, after the quarrel and before the telephone call to 赵德明.
- The coordinator wrote three experience files by hand: `characters/01-gu-qinghe.md`, `characters/02-cheng-li.md`, and `characters/03-zhao-deming.md`. Each file has only what that character experienced or was told, in the text of Chapters 1 to 7. Two files have facts from the plan of the author that the text does not have at that point (`fixtures/memory-calibration/key.md`, rows R5, F1, and R6). The files mark these facts.
- Each file is the author message of one author Run, in a new Project Conversation of one Project with one blank Chapter. The deployment has no Skill directory, no embedding destination, and no Exa key. Thus the model gets the Host instructions of the tool loop and the file, and no manuscript.
- `trial.ts`, `run.sh`, `rehearse.sh`, and `request-guard.sql` come from the scripts of the tag `evidence/1765-live-scene-brief-runs-4-6-2026-11-21`. The request guard permits 4 model requests for each Run and 12 in all.
- The release package is the package of `main` at `9c8d304b395206bf9decfce4a9d9f3ef9fe1e663` (mapping `v10`, Registration `fa24`).

## Result

- 3 author Runs, each `completed` with 2 model requests (the answer, then `finish`). 6 requests and 42,834 input tokens in all. No refused call, no Proposal, and no background Run.
- `run/answer-*.md` has the final answer of each Run. `run/wire/` has each request payload. `run/measures.json`, `run/model-attempts.json`, `run/tool-calls.json`, and `run/runs.json` have the rows. `run/guard-log.json` has the log of the request guard.

## Limits

- One Run for each character shows no variance.
- The files are hand-written. The trial does not show that the product can make such a file from the manuscript.
- The message tells the model that a person usually acts by ordinary reasons. The intents are careful, and this instruction is one possible cause.
