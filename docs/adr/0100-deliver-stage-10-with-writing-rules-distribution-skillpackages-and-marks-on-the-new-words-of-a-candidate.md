---
status: accepted
---

# Deliver Stage 10 with Writing Rules, SkillPackages of the Distribution, and Marks on the New Words of a Candidate

The author decided this on 2026-11-21 in conversation, at the kickoff of Stage 10 that [ADR 0096](0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) requires. This ADR sets the scope of Stage 10. It changes no requirement of another stage.

## Context

ADR 0096 gives Stage 10 the first problem of the author: generated prose reads as machine-written. It left the scope to a kickoff. `Todos.md` had four candidates: examples only from the text of the author, learning from the changes of the author, marks on machine phrases in the text, and the production model.

The kickoff survey of `main` at `1cb48136` found these facts:

- The Host instructions of a tool-loop request have one sentence about prose: "Write in the language of the manuscript."
- The Project Instruction exists. Its limit is the Context item limit of 32,000 tokens, so sample passages fit in it.
- SkillPackages work ([ADR 0089](0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md)), and no package comes with the product: the Skill directory of a deployment is empty until its operator fills it.
- The editor shows a Proposal candidate below its target Block with no mark on the changed words. The Web has no view decoration.
- No record gives the origin of each Block, so examples from the text of the author need a derivation first.

The coordinator first proposed a larger scope: a blind trial of models, a derivation of the origin of the text, automatic examples, and the learning of preferences from the edits and the rejections of the author. The author refused it as too complicated.

A research run of the same day read about 45 Chinese-language sources and the prompt guides of three model vendors ([report](../research/machine-voice-patterns-chinese-fiction-2026-11-21.md)). Its findings for this decision:

- The patterns that the sources name most are a small number of classes: the negative-flip frame (不是……而是……), the dash as a frame, an ending that states the meaning, stock similes, groups of three, explained emotion, connective words, stock body reactions, weak adverbs, dialogue in one voice, and a conflict that ends at once.
- A list of forbidden phrases has a limited effect. It can cause the phrase, and the model moves to the next stock phrase.
- A rule works better when it states the wanted behaviour and says when a device is appropriate.
- Examples direct the model most. Thus the sample passages of the author must come before general rules.

## Decision

### 1. The means of Stage 10

- Stage 10 works with instructions to the model that the author controls, and with an easy examination of each change by the author.
- StoryOS infers no preference of the author automatically. No function of Stage 10 takes a preference from an edit, from a rejection, or from the text of the author with no request of the author. The author finds such a function hard to control and too complex. The capture of Stage 7 that records a preference that the author states in a message stays as it is ([ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)).
- The author and the assistant work in small pieces, as a person and a coding agent do. The product helps the author to examine each change, to see how an addition continues the text, to find a conflict with the earlier story, and to give the intent of the author to the model clearly.

### 2. The scope

1. **Writing rules in the Host instructions.** Each tool-loop request has a short section of default rules for prose, with about ten rules for the classes of the research. Each rule states the wanted behaviour and says when a device is appropriate, with a small number of Chinese phrases as examples of the habit. The section is not a list of forbidden phrases. The rules are defaults: the author request, the Project Instruction, a sample passage that they give, and the instructions of a Skill come before them. This is one exception to the order of [ADR 0089](0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md), in which the Host instructions come before the instructions of a Skill. The exception is only for the style of the prose: the default rules are Host instructions, and they say themselves that a different style in the instructions of a Skill applies. The text of the rules in the mapping says so, and the request tests have the exact text. A Skill still gives no permission, no function, no disclosure right, and no authority (ADR 0089 section 4). A change of the section gets a new mapping revision and a new Registration, as the change of the Host instructions for the Memory rules did (the amendment of 2026-11-18 to [ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md)).
2. **The Project Instruction stays the one place for the lasting demands and the sample passages of the author.** Stage 10 adds no record, no control, and no automatic capture for them.
3. **SkillPackages of the StoryOS distribution.** The repository tracks a directory of standard SkillPackages, and the release package has a copy of it. Three packages are in the scope:
   - A local revision: the assistant finds a small number of the most obvious machine-voice places in the Chapter and gives a Proposal for each.
   - A conflict check: the assistant compares a text with the earlier Chapters and with Agent Memory, and it reports each conflict. It changes no text.
   - An agreement before a scene: the assistant states in a few lines what the scene will do, and it writes after the answer of the author. This package is the least sure of the three. Its ticket is the last of the SkillPackage tickets, and the author can remove it from the ticket graph with no change of this ADR.
4. **Marks on the new words of a Proposal candidate.** The editor underlines the new words of a candidate and puts a small tick where words were removed with no replacement. The target Block has no mark until the pointer is on a mark; then the old words of that mark get a background. The author chose this form (variant B) from a prototype on 2026-11-21. The prototype is on the tag `evidence/proposal-change-highlight-prototype-2026-11-21`.

### 3. How the SkillPackages of the distribution reach a deployment

- The one Skill Source stays the deployment (ADR 0089). The operator sets `STORYOS_SKILLS_DIR` to the directory of the release package that has the SkillPackages of the distribution, or to another directory. An absent setting still gives no Skill.
- The packages are files. They are not compiled into the binaries, which ADR 0089 refused.
- A package of the distribution is a standard SkillPackage with no StoryOS-specific file. Its name is ASCII, as the format requires. Its description is in Chinese, because the author reads it in the Skill picker.

### 4. The defaults of the marks

The author confirmed these defaults on 2026-11-21:

- A candidate has no mark when its target Block is empty, and when almost all of its text is new.
- The pointer on a mark shows the old words of that mark in the target Block. The target Block has no mark at other times.
- The marks use the grey-brown colours of the workspace, and no new colour.
- When the author changes the text of a candidate, the marks follow the text.

### 5. Not in Stage 10

- Automatic examples from the text of the author, and learning from the changes of the author. Section 1 refuses them.
- A second model that reviews a draft of the assistant. The author named it as a possible later mechanism.
- Marks on machine phrases in the editor.
- A trial that compares models, prompts, or products. The author wants no experiment now.
- The production model. Section 6 of ADR 0096 stays as it is.
- An entry in which the author provides or manages own SkillPackages. ADR 0089 deferred it.
- A control that sets the writing rules or the marks on or off.

## Considered options

- **The larger scope of the first proposal.** Refused by the author: too complicated, and an automatic inference of preferences is hard to control.
- **A long list of forbidden phrases in the Host instructions.** Refused: the research shows a limited effect, and the list becomes old.
- **A new place for sample passages.** Refused: the Project Instruction has the room, and the author already writes there.
- **SkillPackages compiled into the binaries, or a default Skill directory with no setting.** Refused: ADR 0089 keeps the operator as the one who provides Skills, and a default directory changes the context of each Run of each existing test and deployment.
- **Marks on the old words and the new words (variant C), or the two texts as one paragraph (variant D).** Refused by the author after the prototype: variant B keeps the book look of the manuscript.

## Consequences

- This ADR changes the scope sentences of the delivery continuation and of the release baseline, and `Todos.md`. It adds no Requirement.
- The specification of Stage 10 follows (ADR 0096). With it, the delivery continuation gets the Requirements, the author journey, and the evidence rows of Stage 10, and the proof owner covers Stage 10 with a new binding of the release sources.
- The request mapping gets its next revision, and the Agent Plan route gets its next Registration. The new instruction text needs one live run on the real route, and each SkillPackage of the distribution needs one live run that shows its effect.
- The repository gets a directory of product SkillPackages beside the test packages of `fixtures/skill-packages/`. The release package gets a directory of SkillPackages beside its Web directory.
- The Web gets its first view decoration. It is in the view only: the document, the Local Edit Journal, the commands, and the copied text do not change.
- The author as the operator sets `STORYOS_SKILLS_DIR` one time to get the packages of the distribution.
