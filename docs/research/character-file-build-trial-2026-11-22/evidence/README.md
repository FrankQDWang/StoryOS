# Paper trial: the model builds the experience file of a character (2026-11-22)

This directory is the raw evidence of one live run on the Volcengine Agent Plan route. It is the second evidence record of a design talk with the author, before a specification. It is not a ticket, and no product code changed. The first record is on the tag `evidence/character-perspective-trial-2026-11-22`.

## Question

In the first trial, the coordinator wrote the experience file of each character by hand. This trial asks one thing: when the model reads the Chapters by itself, does it write an experience file that keeps the knowledge limits of the character? A fact that the character did not see and that no person told the character must not be in the file.

## Method

- Sample: Chapters 1 to 7 of `fixtures/memory-calibration/`. Chapters 1 to 6 are the files of the fixture with no change. `chapter-07.md` in this directory is Chapter 7 with its last paragraphs removed: the text stops in the night of the Wednesday, before the telephone call to 赵德明. This is the same point of story time as in the first trial.
- One Project holds the 7 Chapters. The deployment has no Skill directory, no embedding destination, and no Exa key. The Project has no Memory Entry, because no capture ran.
- Each file of `characters/` is the author message of one author Run in a new Project Conversation. The message tells the model to read Chapters 1 to 7 and to write the file of one character (顾青禾, 程立, 赵德明) with five rules about the knowledge limits and with seven headings. Two headings are for the author: 【拿不准】 (not sure if the character knows) and 【待作者补充】 (a thing that the character knows and that the text does not state; a question only, no guess).
- `trial.ts`, `run.sh`, `rehearse.sh`, and `request-guard.sql` come from the first trial. The request guard permits 10 model requests for each Run and 30 in all. A rehearsal on `host_fake` came first.
- The release package is the package of `main` at `9c8d304b395206bf9decfce4a9d9f3ef9fe1e663` (mapping `v10`, Registration `fa24`).

## Result

- 3 author Runs, each `completed` with 3 model requests: the `read_chapter` calls, the file, then `finish`. 9 requests and 235,546 input tokens in all. No refused call, no Proposal, and no background Run. The Run of 顾青禾 read 6 Chapters with the tool; Chapter 7 was the current Chapter of the request.
- `run/answer-*.md` has the text of each Run. `run/wire/` has each request payload. `run/measures.json`, `run/model-attempts.json`, `run/tool-calls.json`, and `run/runs.json` have the rows. `run/guard-log.json` has the log of the request guard.

The coordinator compared each sentence of the three files with the text of Chapters 1 to 7:

- The part "what you experienced" of each file has no fact that the character could not know. The file of 赵德明 is short and says that he is in two scenes only.
- Each file names facts outside the limit in a negative form. The file of 顾青禾 says that she does not know of the message that 知遥 sent to her father in the night. The file of 程立 says that he was not there when 知遥 quarrelled with her father on the telephone. The heading 【拿不准】 of 赵德明 names the price, the letter of intent, the quarrel, and the carpenter. A reader who gets the whole file learns these facts.
- Omissions in the file of 顾青禾: the sentence of 沈明川 in her presence about "the things after the funeral", and the empty cupboard on the next morning (the file has it as a question to the author).
- Small errors: the order of two actions in Chapter 1; the state "closed again" of the teahouse, which was open again in Chapter 6; one added phrase in the file of 赵德明 ("those words were only for 怀山").
- The questions under 【待作者补充】 are the secrets of the plan of the author (`fixtures/memory-calibration/key.md`): why one half, the woman of the photograph, the box, and the money of 1983. The model asked and did not guess. The file of 赵德明 does not have the key, because the cut text does not have it.

## Limits

- One Run for each character shows no variance.
- The model read 7 short Chapters in one request. The trial does not show the result on a long manuscript, where a file must grow Chapter by Chapter.
- The trial did not give the built files to the question of the first trial.
- The check of each sentence is the reading of the coordinator.
