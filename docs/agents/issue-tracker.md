# Issue tracker: GitHub

Issues and the StoryOS design map live in the public GitHub repository `FrankQDWang/StoryOS`. Use the `gh` CLI for tracker operations.

## Conventions

- Create, read, edit, comment on, label, assign, and close issues with the corresponding `gh issue` commands.
- Infer the repository from the configured Git remote when possible; otherwise pass `--repo FrankQDWang/StoryOS` explicitly.
- Before mutating an issue, read its current body, labels, assignees, native dependencies, and current resolution-evidence comment. Tracker state may have changed concurrently.
- In human-facing text, refer to an issue by its linked title, never by a bare issue number.
- One task has one execution owner. Claim work by assigning the issue before doing it.

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub shares one number space across issues and pull requests. If an ambiguous number must be resolved, try `gh pr view <number>` and then `gh issue view <number>`.

## Publishing and fetching

- Publish StoryOS tickets as GitHub issues in `FrankQDWang/StoryOS`.
- Fetch a ticket by reading its current body, labels, assignees, native dependencies, exact `main` baseline, and tracked contracts named by the body.

## Ticket sizing

These repository constraints preserve Matt ticket publication and add StoryOS review-size limits:

- Size each ticket so its expected diff follows the change-size and generated-artifact rules in [CODING_STANDARDS.md](../../CODING_STANDARDS.md). Split a larger ticket before publication.
- Before publication, validate that the approved ticket graph is acyclic and that every blocking edge reflects a real dependency. Run independent dependency-ready tickets in parallel.

## Current map operations

These operations apply to Wayfinder decision tickets. Implementation tickets published by the `to-tickets` skill follow [Delivery](delivery.md).

- **Current map:** [Map the StoryOS Editor-First Product and Production Delivery Contract](https://github.com/FrankQDWang/StoryOS/issues/1) is the repository's permanent design-map entry point and the sole issue labelled `wayfinder:map`.
- **Map body:** maintain Destination, Current product contract, Current design index, Current evidence, Current planning frontier, Issue-native execution contract, and Completion gate as a living current-state view. Edit these sections in place as the product contract advances.
- **Contract ownership:** each current design topic has one owning tracked file or section named in the map's Current design index. Cross-references link to that owner. Each requirement and implementation surface has one current issue owner.
- **Original owner:** when an accepted topic needs correction or extension, reopen its owning issue and edit its body in place. Create a new issue only for a genuinely ownerless domain question.
- **One current answer:** an open issue has no resolution answer. A closed issue has a positive current-contract body and exactly one evidence-only resolution comment. Decision requirements do not live in correction, supersession, checkpoint, or historical-precedence comments.
- **Child ticket:** create a positive current-state question with exactly one type label: `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling`, or `wayfinder:task`. Link it to the current map, or to a map area parent when the map has 90 or more sub-issues, using GitHub's sub-issues API.
- **Native map children:** only Wayfinder decision tickets and map area parents are native sub-issues of the map. Specifications, stage parents, implementation tickets, and triaged review findings are not map children. The map body indexes them in Current design index or Current planning frontier. GitHub limits one parent to 100 sub-issues. Closed sub-issues also count toward that limit.
- **Map area parent:** keep the last 10 map sub-issue places for map area parents only. When the map has 90 or more sub-issues, put a new ownerless decision ticket under a map area parent. A map area parent is a native map child that groups the decision tickets of one design area. A design area is a group of related topics in Current design index, for example Context and disclosure. Create a map area parent only when the first decision ticket of its area needs it. Do not move earlier decision tickets.
- **Map area parent rules:** a map area parent is not a question. It has no type label, no Claim, and no resolution. It stays open. The map body lists it in Current planning frontier. Frontier queries skip the map area parent and include its sub-issues as map decision tickets. A map area parent also has the 100 sub-issue limit. When it is full, create the next map area parent for the same area in a reserved map place. When any new map area parent needs a reserved map place and none remains, stop and ask the author.
- **Blocking:** use GitHub's native issue dependencies. Add blockers through `repos/FrankQDWang/StoryOS/issues/<child>/dependencies/blocked_by` using the blocker's numeric database `id`, not its issue number or GraphQL node ID.
- **Frontier:** native dependencies define the ready set of open, unassigned questions whose blockers are resolved. Independent questions can proceed in parallel; shared contract edits have one owner.
- **Read before claim:** before claim, read the current map, current `main`, owning tracked contracts, and affected downstream issue bodies. Align scope, Requirement ownership, wording, ordering, and native dependencies so the selected issue is the sole owner of its question.
- **Claim:** assign the selected frontier issue. Then record its Contract revision and exact `main` Baseline in a claim comment.
- **Resolve:** update the owning tracked contract, add a resolution comment that links the exact files and commit, refresh the map's current-state sections, and close the child.
- **Review:** route every newly sharp design problem to its existing owner first. When no owner exists, create one focused sub-issue of the map, or of a map area parent when the map has 90 or more sub-issues, add its real dependencies, and refresh downstream issue bodies and dependencies.
- **Charting:** create the required issues first and wire sub-issue and blocking relations in a second pass, producing the dependency-ready frontier.

## Workflow skills in each client

[Delivery](delivery.md) gives the steps that deliver a specification and its tickets. The steps use Matt Pocock skill names, for example `to-spec`. Claude Code and Codex load the same skill bodies. Start a skill with the command of your client:

| Client | Skill source | Start a skill |
| --- | --- | --- |
| Claude Code | `mattpocock-skills` plugin | Type `/mattpocock-skills:<name>`, for example `/mattpocock-skills:to-spec`. |
| Codex | User skills in `~/.agents/skills/` | Type `$<name>` in the prompt, for example `$to-spec`, or select the skill from `/skills`. |

- In Claude Code, the plugin marks `wayfinder`, `to-spec`, `to-tickets`, `implement-spec`, `triage`, and `retro` as user-invoked. The agent cannot start them itself. When a workflow step uses one of these skills, the agent asks the author to type the command.
- In Claude Code, the Matt `code-review` skill comes from the `mattpocock-skills` plugin. Claude Code also has a different `code-review` skill. For the two-axis Standards and Spec review, use `/mattpocock-skills:code-review`.
- A subagent is a separate agent context that the client starts for one role, for example an executor. The two clients can start subagents. When a client starts a subagent, the subagent shares the files of its parent. Give each writing subagent its own worktree.
