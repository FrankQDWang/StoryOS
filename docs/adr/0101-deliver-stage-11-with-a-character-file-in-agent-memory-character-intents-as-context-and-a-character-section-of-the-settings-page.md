---
status: accepted
---

# Deliver Stage 11 with a Character File in Agent Memory, Character Intents as Context, and a Character Section of the Settings Page

The author decided this on 2026-11-21 and 2026-11-22 in conversation, after the release of Stage 10. [ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) says that the author decides again about Stage 8 and Stage 9 after that release. This ADR records the order of the work after Stage 10. It adds one stage identity, Stage 11, and it sets the scope of that stage. It changes no requirement of Stage 8 or Stage 9.

## Context

The third problem of ADR 0096 is the plot: a model stays inside the known facts, or it adds people and places with no control. After the release of Stage 10, the author said how the plot of a novel grows. It grows from the characters, as a garden or a coral reef grows, and not from a plan. The writer stands in the place of each character and asks what that person does next. Most of the time, a person acts by ordinary logic.

The author gave these conditions:

- The work starts with no request of the author. The author can have no professional training, and the attention of the author is limited. A start by the author stays possible.
- StoryOS does not simulate a world. The author finds a simulation not possible and its result not good. A character only says what it wants to do. The manuscript is the one truth, and the author decides what the manuscript gets.
- The time is the time of the story, for example the three days that pass in the story. It is not only the moment at which a Chapter is complete.
- Each character has its own context: what the character did, saw, and heard, what the character wants, and what the character hides.

The survey of `main` at `9c8d304b` found these facts:

- Agent Memory has entries of the type character ([ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)). The capture of a Chapter and the consolidation pass write them with no action of the author. A Project has one entry for each character, and the text of a character entry has at most 1,500 characters.
- The entries of the calibration run of Stage 7 ([record](../research/stage7-memory-calibration-2026-11-17.md)) have 126 to 365 characters. They have the view of the storyteller, and they join the manuscript with the plan that the author told in a Conversation. They do not say what the character knows.
- When the author tells the assistant to remember a fact of a character, the entry of that character becomes an entry of the author. Background work cannot change an entry of the author. Thus the entry gets no later Chapter.
- The Stage 8 views of characters, relationships, and the timeline have no data, because ADR 0094 deferred the structured fiction records.

Three paper trials on the real route used the sample novel of `fixtures/memory-calibration/`, with no change of the product:

- **Intents from a file.** The coordinator wrote the file of three characters by hand. Each character got only its file and the question what it does in the next three days of the story. Each answer was specific and in character, and the three independent answers met in one scene. Each character was careful, and the secrets of the characters came from the plan of the author, not from the manuscript. The raw evidence is on the tag `evidence/character-perspective-trial-2026-11-22`.
- **A file from the Chapters.** The model read seven Chapters and wrote the file of the same three characters. The part "what you experienced" had no fact that the character could not know. Each file named a fact outside that limit in a negative form ("you do not know that …"). The questions that the model wrote for the author were the secrets of the plan of the author, and the model did not guess the answers. The raw evidence is on the tag `evidence/character-file-build-trial-2026-11-22`.
- **Intents and questions from a built file.** Each character got only the file that the model built in the second trial, with no heading for the author. Each character gave a short intent that used only facts of its file, and at most three questions to the author. The questions were about the facts that only the author knows, and the model did not guess the answers. The raw evidence is on the tag `evidence/character-intent-pass-trial-2026-11-22`.

When the coordinator named the file "automatic", the author answered that it is the Memory mechanism, and that it is not a new mechanism.

## Decision

### 1. The order after Stage 10

The author agreed to this order on 2026-11-21:

1. **Stage 11: Character intents.** This ADR sets its scope.
2. **The base for more than one Provider.** The Host keeps the conversation record for a Provider that has no state on its server ([the decision ticket](https://github.com/FrankQDWang/StoryOS/issues/1406)). Section 6 of ADR 0096 stays as it is.
3. **A smaller Stage 9.** The list of the plan comes first, then the Subruns. The triggers come last or not at all.
4. **Stage 8.** The author intends to withdraw Stage 8 in its present form.

The number of a stage is its identity, and it is not its place in the order (ADR 0096). This ADR decides only item 1. A later ADR records each other item when its work starts. Until then, Stage 8 and Stage 9 keep their requirements, and they stay on EXECUTION HOLD.

### 2. The means of Stage 11

- StoryOS does not simulate. No function of Stage 11 moves the story forward, and no function changes the manuscript or Agent Memory from an intent. A change of the manuscript by the assistant stays a Proposal.
- A character proposes, and the author decides. An intent is a suggestion for the plot. It is not a fact of the story, and it is not an instruction.
- Each character gets only its own file when it forms an intent. It does not get the manuscript, the Memory of the Project, or the file of another character.
- The work is a part of Agent Memory. It uses the entries of the type character, the capture of a Chapter, and the consolidation pass. Stage 11 adds no structured character record, and the rule "no setup before writing" of ADR 0096 applies: the author fills no character sheet.
- The mechanisms are instructions to the model and bounded background work, as in [ADR 0100](0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md).

### 3. The character file

The character file is the Memory of one character. It has two parts, and each part is one entry of the type character with the name of the character as its title.

- **The part of the author.** It holds what the author told the assistant to remember about the character, for example a secret that the manuscript does not have. Its origin is the request of the author. Background work never changes it (ADR 0094 section 6). When the author tells the assistant to remember a fact of a character, the assistant writes it into this part, and it creates the part when the Project does not have it. A request of the author no longer makes the part of the background work an entry of the author.
- **The part of the background work.** The capture of a Chapter and the consolidation pass keep it current. It holds the view of the character: who the character is, what the character did, saw, and heard, in the order of the story, what the character wants at the end of the manuscript, and how the character speaks. A decision about the character that the author states in a Conversation goes into this part only when the character knows it.
- **The limit of knowledge.** The part of the background work holds no fact that the character did not see and that no person told the character. It does not use the negative form ("the character does not know that …"), because that form gives the fact to the reader of the file.
- **The length.** The limit of a character entry increases from 1,500 characters. The first value is 4,000 characters. One recorded live run on the sample manuscript measures how the file of a character grows with each Chapter, and the ticket of that run sets the value before the release. The instructions tell the model to keep the recent events in detail and to make the early events short.
- A Project has at most one part of each kind for each character. The rule "one entry for each character" of ADR 0094 section 8 becomes "one entry of the author and one entry of the background work".
- The rule of a conflict of ADR 0094 section 7 stays: when the manuscript is against the part of the author, background work sets a conflict flag and changes no text.

### 4. The intent pass

- An intent pass is background work for one character. It sends one model request. The input is the two parts of the file of that character and no other data. The request has no Tool that reads, searches, or writes.
- The reply has two items. The first is the intent: what the character wants to do in the next period of story time, from the moment at which the file ends, in two or three sentences. The model selects the length of the period that fits the story. A character that does nothing says so. The second is at most three questions to the author about facts that the character must know and that the file does not have. The model asks, and it does not guess an answer.
- An intent pass starts for a character after the file of that character changed. A Chapter in which the character occurs changes the file through the capture and the consolidation pass. A character that does not occur keeps its last intent, and the record says at which Chapter that intent was made.
- The pass runs with no request of the author, as the capture does, and ordinary writing does not wait for it. The Host limits the rate of the passes of a Project.
- An intent pass is for a character that has the part of the background work. A character that only the author described, and that the manuscript does not have, gets no pass.
- The Host keeps the newest result of each character with the part of the background work, as it keeps a conflict flag (ADR 0094 section 7). A result changes no text of the file. A later change of that part removes the result, and the next pass gives a new one. A result is data of background work: it has no Instruction Authority, and it grants no permission.

### 5. The use of an intent

- The writing workspace shows no intent and no question. The author decided this on 2026-11-22.
- The Memory Index of a Run of the author has the newest intent in the line of each character. The Host instructions say that an intent is a suggestion from the file of the character alone, that it is not a fact, and that the author decides. The assistant uses an intent when it continues or rewrites a scene with that character. When the author asks what a character wants, the assistant answers from the intent and the file.
- A capture and a consolidation pass do not get the intents, so that an intent does not become a fact of Memory.
- The questions are for the author only. No Run gets them. The author reads them on the settings page and answers in a Conversation when the author wants. The answer goes into the file by the rules of section 3.

### 6. The settings page

The author approved a sketch of this page on 2026-11-22.

- The settings page of a Project gets a navigation on its left side with three items: "项目说明", "记忆", and "人物". One item is selected, and the page shows its section.
- The section "人物" has a search field ("搜索人物") and the list of the names of the characters, because a Project can have many characters. The search finds the names that hold the typed characters.
- A selected character shows its name, the file ("档案"), the block "接下来想做什么" with the intent and the line "写到《<title>》时的想法", which names the Chapter at which the intent was made, and the block "助手还不清楚的事" with at most three questions. A block with no content is not shown.
- The file shows the part of the author first, with the origin line "你让助手记住的", and then the part of the background work with its origin line. Each part keeps the delete control and the conflict flag of a Memory Entry (ADR 0094 section 9).
- The section "记忆" no longer has the group "人物". Its other groups do not change.
- The page stays a place to read. It gets no control that edits a file, that starts an intent pass, or that answers a question. The author answers in a Conversation.

### 7. Not in Stage 11

- A simulation of the story, and a function that writes a scene from an intent with no request of the author.
- A list of the intents in the writing workspace, and a notice to the author.
- A small floating assistant that reminds the author, and voice. The author named them on 2026-11-22 as a possible later form.
- A pass that looks at the whole Memory again, as the dreaming of a general agent does. The author wants it after Stage 11.
- Two parts for an entry of another type, for example a relationship.
- An edit control for a Memory Entry, and a control that sets the intent pass on or off.
- A structured record of a character, a relationship, or the timeline. ADR 0094 keeps them deferred.

## Considered options

- **A list of the intents in the writing assistant panel.** The coordinator recommended it. Refused by the author: the intents are context for the assistant, and the author reads them on the settings page or asks the assistant.
- **A simulation in which the characters act and the story moves.** Refused by the author: not possible, and the result is not good.
- **A new record of a character apart from Agent Memory.** Refused: the author said that the file is the Memory mechanism. ADR 0094 deferred the structured records because the author found them too complex.
- **The intent as a part of the consolidation pass.** Refused: that pass has each entry of the Project, so an intent would use facts that the character cannot know.
- **One entry for each character, as today.** Refused: a request of the author makes the entry an entry of the author, and then the file gets no later Chapter. A rule that lets background work change the words of the author is against ADR 0094.
- **Questions from the capture of a Chapter.** Not selected: the capture and the consolidation pass would get one more output each. The intent pass already has the file, and the third trial shows that it finds the questions.

## Consequences

- The delivery continuation and the release baseline get the new stage and its place in the order. `Todos.md` gets the two later ideas of section 7. This ADR adds no Requirement.
- The specification of Stage 11 follows. With it, the delivery continuation gets the Requirements, the author journey, and the evidence rows of Stage 11, the glossary gets the new terms, and the proof owner covers Stage 11 with a new binding of the release sources.
- The request mapping gets its next revision, and the Agent Plan route gets its next Registration. The new instructions of the capture and of the consolidation pass, the request of the intent pass, and the intents in a Run of the author each need a live run on the real route.
- ADR 0094 changes in three places: the limit of a character entry, the rule of one entry for each character, and the effect of a request of the author on an entry of background work of the type character. The other rules of ADR 0094 stay.
- Each intent pass is one model request with no request of the author. The cost of the background work of a Project increases with the number of the characters that occur in the new Chapters.
- No database holds data ([ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md)), and a deployment has no upgrade edge. Thus Stage 11 has no step for the character entries of an earlier release: a Project of a Stage 11 deployment gets the new form from its first capture.
- The settings page gets its first navigation. The exact-dist journeys of the settings page change with it.
