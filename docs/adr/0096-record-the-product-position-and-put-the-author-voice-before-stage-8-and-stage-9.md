---
status: accepted
---

# Record the Product Position and Put the Author Voice Before Stage 8 and Stage 9

The author decided this on 2026-11-16 in conversation, during the delivery of Stage 7 ([#365](https://github.com/FrankQDWang/StoryOS/issues/365)). This ADR records for whom StoryOS is, which problems come first, and the release order after Stage 7. It adds one stage identity, Stage 10, and it sets no scope for that stage. It changes no requirement of Stage 7, Stage 8, or Stage 9.

## Context

The author asked if a general coding agent, for example Claude Code or Codex, makes StoryOS unnecessary. The author writes long fiction with general models today, and names three problems:

1. The prose reads as machine-written.
2. The model loses the facts of a long novel.
3. The model cannot decide when to make the world larger. It stays inside the known facts, or it adds people and places without control.

The destination of the design map and `GOAL.md` already say that StoryOS is an editor-first system for Discovery Writing, that the editor works with no AI, and that no outline or character sheet is necessary. They do not say which problem of the author comes first, and they do not say where the product must be better than a general agent.

The release order of the [delivery continuation](../foundation/storyos-product-delivery-continuation.md) is Stage 5 through Stage 9. Stage 8 delivers views inside the Agent transcript, and Stage 9 delivers plans, Subruns, and proactive work. These are abilities of an agent, and a general agent has them or gets them. No stage works on the first problem of the author.

## Decision

### 1. For whom the product is

- The first user is the author of this repository. The target user is an author with no professional training who does Discovery Writing: the author writes in order, with no outline, and the plot and the characters grow during the writing.
- StoryOS is not an automatic writing machine. The author gives the instructions, and their quality decides the quality of the novel. The product asks for the ideas of the author, and not for professional skill.
- A plot point can come from a discussion of the author with the assistant, from the author alone, or from the judgement of the assistant about the characters, the events, and the history of the story. The author decides each one, and each change of the manuscript by the assistant stays a Proposal.
- The editor comes first because in fiction the text is the product. The author must see the text and own it. In software, the code is not the product, so a general agent can hide it. That form does not fit fiction.

### 2. No setup before writing

A function that asks the author for a character card, an outline, a beat, or a setting before the author can write is refused. This rule decides between two designs of one function, and it is the reason for the capture of Agent Memory from the manuscript in [ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md). The existing scope exclusion of the delivery continuation ("no mandatory outline or character-sheet setup") stays, and this rule also covers a later function.

### 3. The order of the problems

The first problem is the machine voice. The second problem is the memory of a long novel. The third problem is the decision when to make the world larger, and it comes after the first two. The author keeps the design of the plot.

### 4. Where the product must be better than a general agent

StoryOS does not compete on the abilities of an agent. It uses three properties that need an editor that owns the manuscript:

1. **The origin of the text.** StoryOS can find which text the author typed and which text came from a Proposal, and what the author made of an accepted Proposal. The code today has no origin record for each Block. It records the command kind of each Chapter revision, it keeps the identity of a Block across revisions, and it keeps the candidate text of each Proposal, also of a rejected one. A function that needs the origin builds the derivation, or a record, in its own ticket.
2. **A place beside the text.** StoryOS sees each Author Edit and each decision on a Proposal, and it can put a mark at an exact place of the manuscript.
3. **Work with no request.** StoryOS can do background work for the Project when the author asks nothing, as the capture of Stage 7 does.

A new function is first when it works on problem 1 or problem 2 and uses one of these properties.

### 5. The order after Stage 7

- A new stage, **Stage 10: Author voice**, comes after the release of Stage 7 and before Stage 8 and Stage 9. The number is the identity of the stage, and it is not its place in the order. Stage 8 and Stage 9 keep their identities, their parents, and their requirement identifiers.
- Stage 10 works on the first problem. Its scope is not decided. A kickoff with the author decides the scope, as the kickoff of Stage 7 did, and then a specification and a parent issue follow. The candidates are in [`Todos.md`](../../Todos.md).
- Stage 8 and Stage 9 stay on EXECUTION HOLD. After the release of Stage 10, the author decides again about each of them: deliver it, change it, or withdraw it. Until that decision, their requirements stay as they are.

### 6. The model for production

The Volcengine Agent Plan route ([ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md), [ADR 0048](0048-use-the-deployment-model-destination-by-default.md)) stays the route of development: it is cheap, it has the Responses form, and it lets the full loop work. ADR 0048 already says that this route is a personal test route. The author will use another model for production, and rates the Claude series and the Gemini series as the best for Chinese prose, and the GPT series below them. This ADR selects no vendor. A later ADR selects the vendor, the account, and the cost, and a new Model Registration follows ([ADR 0059](0059-pin-a-dated-provider-model-version-in-each-model-registration.md)). That ADR also says how the new destination fits the explicit model policy of Stage 9 (`S9-REQ-006`).

> Amended by [ADR 0103](0103-deliver-a-smaller-stage-9-with-a-plan-list-of-a-run-and-a-subrun-of-one-level-that-the-parent-waits-for.md) section 1 on 2026-11-23: Stage 9 delivers no model policy, and the ADR that selects the vendor decides a model policy.

## Considered options

- **Keep the order Stage 8, then Stage 9.** Refused: these stages add abilities of an agent, and the first problem of the author stays open for two more stages. The author would continue to write with a general agent.
- **Withdraw Stage 8 and Stage 9 now.** Refused: the decision needs the experience of Stage 7 and Stage 10 in real writing. The hold costs nothing.
- **Put the author-voice work into Stage 7.** Refused: Stage 7 has an approved ticket graph in delivery, and the scope of the voice work is not decided.
- **Give the new stage the number 8 and move the others.** Refused: the stage numbers are identities in requirement identifiers, ticket titles, and evidence.
- **Compete with a general agent on its abilities.** Refused: a small product cannot keep up with them, and they do not work on the problems of section 3.

## Consequences

- The delivery continuation and the release baseline get the new order and a short section for Stage 10. The design map gets the same order in its stage table and its frontier.
- Stage 10 has no requirement, no journey, and no evidence row until its specification. The proof owner has nothing to cover for it until then.
- The proof of this position is use: the author writes the author's own novel in StoryOS. A trial that writes one scene with StoryOS and with a general agent can support the kickoff of Stage 10.
- `Todos.md` at the root holds the ideas that are not built now. It is not a contract.
- The Stage 8 refresh that [ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) names for S8-06 waits for the decision of section 5.
