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

For daily dependency inspection, use `python3 scripts/tracker_query.py <number>`.
Add `--format json`, `--relation blocking`, or `--page 2` as needed. The query
reads all native relationship pages, then prints at most eight related tickets
within 16 KiB and 80 lines. It reports exact totals and omitted counts. Long titles
are clipped with their character count. Unknown states stay unknown; a failed read
returns nonzero with unknown dependency state. Concurrent page drift requires a
fresh query; this is not a transactional tracker snapshot. Bodies remain available
through the printed contract command. This summary does not replace the real
execution-contract reads or the Claim rules below.

## Ticket sizing and execution

These repository constraints preserve Matt ticket publication and add StoryOS review-size limits:

- Size each ticket so its expected diff follows the current review-size and generated-artifact rules in `AGENTS.md`. Split a larger ticket before publication.
- Before publication, validate that the approved ticket graph is acyclic and that every blocking edge reflects a real dependency. Run independent dependency-ready tickets in parallel.

## Current map operations

These operations apply to Wayfinder decision tickets. Implementation tickets published by the `to-tickets` skill follow Matt feature delivery below.

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
- **Refresh gate:** before claim, read the current map, current `main`, owning tracked contracts, and affected downstream issue bodies. Align scope, Requirement ownership, wording, ordering, and native dependencies so the selected issue is the sole owner of its question.
- **Claim:** assign the selected frontier issue, then record its Contract revision, exact `main` Baseline, and SHA-256 of the UTF-8/LF-normalized issue body in a claim comment.
- **Resolve:** update the owning tracked contract, add a resolution comment that links the exact files and commit, refresh the map's current-state sections, and close the child.
- **Review:** route every newly sharp design problem to its existing owner first. When no owner exists, create one focused sub-issue of the map, or of a map area parent when the map has 90 or more sub-issues, add its real dependencies, and refresh downstream issue bodies and dependencies.
- **Charting:** create the required issues first and wire sub-issue and blocking relations in a second pass, producing the dependency-ready frontier.

## Matt feature delivery

### Workflow skills in each client

The steps below name Matt Pocock skills by their skill name, for example `to-spec`. Claude Code and Codex load the same skill bodies. Start a skill with the command of your client:

| Client | Skill source | Start a skill |
| --- | --- | --- |
| Claude Code | `mattpocock-skills` plugin | Type `/mattpocock-skills:<name>`, for example `/mattpocock-skills:to-spec`. |
| Codex | User skills in `~/.agents/skills/` | Type `$<name>` in the prompt, for example `$to-spec`, or select the skill from `/skills`. |

- In Claude Code, the plugin marks `wayfinder`, `to-spec`, `to-tickets`, `implement-spec`, `triage`, and `retro` as user-invoked. The agent cannot start them itself. When the workflow reaches one of these skills, the agent asks the author to type the command.
- Claude Code also has a built-in `/code-review` command that is not the Matt skill. For the two-axis Standards and Spec review, use `/mattpocock-skills:code-review`.
- A subagent is a separate agent context that the client starts for one role, for example an implementer or the merger. Both clients provide subagents. Starting a subagent does not isolate its files: give each writing subagent its own worktree.

### Delivery steps

1. **Specify and split.** When Wayfinder has resolved the required decisions, use the `to-spec` skill, then the `to-tickets` skill. The parent owns the complete acceptance contract. Present the tracer-bullet breakdown, test seams, and real blocking edges for user approval. Publish approved tickets as native children with native blocking edges and `ready-for-agent`. Each ticket must fit one implementation context and the review-size limits.
2. **Coordinate.** Use the `implement-spec` skill for the whole specification. Record one coordinator and one integration branch from an exact current `main` baseline. The coordinator owns shared tracker updates; a merger subagent alone changes the integration branch. Give each implementer an independent branch and worktree based on the integration tip. Verify the base before edits and preserve unrelated changes when correcting it.
3. **Claim the frontier.** Dispatch independent ready tickets concurrently. Before edits, read the current parent, ticket, native dependencies, applicable instructions, and named tracked contracts. Assign the ticket and record its owner, worktree, branch, Contract revision, exact main baseline, integration base commit, and SHA-256 hashes of the UTF-8/LF-normalized parent and ticket bodies. These inputs form the execution contract. A changed contract requires refreshed bodies, revision, base, and Claim before work resumes.
4. **Implement and integrate.** Each implementer uses the `tdd` skill at approved public seams, runs applicable repository checks, and merges the latest integration tip before reporting completion. The merger integrates the result; the coordinator records an Integration comment on the child: acceptance evidence, branch commit, integration commit and tree, command results, and any unresolved scope. Integration is provisional delivery, not final main acceptance.
5. **Advance dependencies.** A native blocker can be satisfied for implementation by a closed accepted ticket or by complete Integration evidence in this specification. For the latter, verify the evidence commit is an ancestor of the current integration tip, required behavior is present, and the dependent contract permits integration delivery. Keep the native edge and child open until final PR delivery. Record the evidence in the dependent Claim. Product release, external acceptance, and explicit main-only gates still require their specified final evidence. Unknown, failed, or stale evidence blocks dispatch. The tracker query reports GitHub state; it does not infer integration readiness.
6. **Open the aggregate PR.** After the first child merges into integration, open a draft PR with the `pr` skill. Link the full parent and child titles and include `Closes #<issue>` for the specification and every delivered child. Maintain scope and evidence as integration advances. Do not add closing references for unmet product stage gates or other specifications.
7. **Review the complete result.** Once all tickets are integrated, run separate read-only Standards and Spec reviews of the complete specification diff. Each reviewer runs in a fresh context of an agent tool that is different from the implementer's tool. The current example when Claude Code implements is the Codex plugin, with one fresh read-only thread for each axis; when Codex implements, use a different agent tool or a separate Claude Code session. The `code-review` skill gives the two-axis structure. The executor writes and imports the two review records. Use one implementer to fix review findings. Follow [Candidate review and admission](verification.md#candidate-review-and-admission) for current required GitHub `verify`, review records, targeted checks, and any requested complete run. Refresh applicable evidence after fixes, then mark the PR ready. Required checks and independent reviews must pass before merge.
8. **Resolve on main.** The coordinator, not an implementer, checks the review and verification evidence and merges the aggregate PR with an ordinary merge commit. Then it synchronizes `main` and runs `make verify-tracker`. GitHub may close linked issues on merge; read back their native state. Closed state alone does not prove acceptance. Evaluate each child and every parent user story on the final main tree. Record one final Resolution per issue with PASS or FAIL, exact commit and tree, linked PR and requirement evidence, commands and results, and retained out-of-scope items. After verifying the final Resolution, explicitly close any issue that remains open and has passed acceptance. Reopen any auto-closed issue whose acceptance fails or is unverified. Parent acceptance includes all child evidence and must not claim another owner's acceptance. Refresh the current map after verified delivery.
9. **Retrospect and clean up.** Use the `retro` skill before clearing the work context. Record actionable environment findings in their owning tracker or contract. Remove implementation worktrees and temporary branches only after their work and evidence are retained. Preserve unrelated work and historical evidence.

Ticket bodies contain Parent, What to build, observable acceptance criteria, and native blockers. Name stable Requirement IDs, authoritative inputs, and interfaces when the parent delegates them. Use an expand-contract sequence only where a wide refactor cannot remain a complete passing vertical slice.

Closed tickets retain their current contract and exact delivery evidence. Current instructions follow this workflow; immutable historical records describe the workflow used at their recorded commit.
