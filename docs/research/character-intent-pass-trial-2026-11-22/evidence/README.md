# Paper trial: intents and questions from a file that the model built (2026-11-22)

This directory is the raw evidence of one live run on the Volcengine Agent Plan route. It is the third evidence record of a design talk with the author, before a specification. It is not a ticket, and no product code changed. The earlier records are on the tags `evidence/character-perspective-trial-2026-11-22` and `evidence/character-file-build-trial-2026-11-22`.

## Question

The first trial used files that the coordinator wrote by hand. The second trial let the model write the file of each character from the Chapters. This trial asks one thing: when a character gets only the file that the model built, does it give a short intent and good questions to the author?

## Method

- Each file of `characters/` is the author message of one author Run in a new Project Conversation. The Project has one Chapter with no story text (`blank-chapter.md`), so the request has no text of the manuscript.
- The message has two rules, the file, and two tasks. The file is the answer of the second trial for the same character, from the heading 【你是谁】 to the line before the heading 【拿不准】. The two headings for the author (【拿不准】 and 【待作者补充】) are not in the message. No other change was made: the file of 顾青禾 still has one sentence in the negative form.
- Task 1: what the character wants to do in the next period of story time, in two or three sentences and at most 150 characters. The model selects the length of the period. Task 2: at most three questions to the author about facts that the character must know and that the file does not have, with no guess.
- `trial.ts`, `run.sh`, `rehearse.sh`, and `request-guard.sql` are the files of the first trial with no change. The request guard permits 4 model requests for each Run and 12 in all.
- The release package is the package of `main` at `9c8d304b395206bf9decfce4a9d9f3ef9fe1e663` (mapping `v10`, Registration `fa24`).

## Result

- 3 author Runs, each `completed` with 2 model requests: the answer, then `finish`. 6 requests and 36,309 input tokens in all. No ToolCall that reads, no Proposal, and no background Run.
- `run/answer-*.md` has the text of each Run. `run/wire/` has each request payload.

The coordinator compared each answer with the file of its character:

- Each intent is specific and in the voice of the character, and each fact that it uses is in the file. 顾青禾 prepares the teahouse for the typhoon of the radio report and does not speak about the sale first. 程立 waits two or three days and then telephones. 赵德明 waits for dry weather, comes to the teahouse, and wants to say what he held back.
- The questions are about the facts that only the author knows: if 顾青禾 gave money for the house in 1983 and if a paper records her half; the limit of the authority of 程立 and if he saw the signature; what 赵德明 held back two times and if 怀山 gave a thing into his care. The plan of the author (`fixtures/memory-calibration/key.md`) has the answers. The model did not guess them.
- The sentence in the negative form in the file of 顾青禾 had no effect on her answer.

## Limits

- One Run for each character shows no variance.
- The request was a Run of the author with the Host instructions of the tool loop. A request of the product for this work has its own instructions and one function.
- The check of each answer is the reading of the coordinator.
