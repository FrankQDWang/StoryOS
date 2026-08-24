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

## Ticket sizing and execution

These repository constraints preserve Matt ticket publication and add StoryOS review-size and serial-execution limits:

- Size each ticket so its expected diff follows the current review-size and generated-artifact rules in `AGENTS.md`. Split a larger ticket before publication.
- Before publication, validate that the approved ticket graph is acyclic and that every blocking edge reflects a real dependency. Execution is serial: claim and work exactly one unblocked ticket at a time, including when several tickets are unblocked.

## Current map operations

These operations apply to Wayfinder decision tickets. Implementation tickets published by `/to-tickets` follow Matt feature delivery below.

- **Current map:** [Map the StoryOS Editor-First Product and Production Delivery Contract](https://github.com/FrankQDWang/StoryOS/issues/1) is the repository's permanent design-map entry point and the sole issue labelled `wayfinder:map`.
- **Map body:** maintain Destination, Current product contract, Current design index, Current evidence, Current planning frontier, Issue-native execution contract, and Completion gate as a living current-state view. Edit these sections in place as the product contract advances.
- **Contract ownership:** each current design topic has one owning tracked file or section named in the map's Current design index. Cross-references link to that owner. Each requirement and implementation surface has one current issue owner.
- **Original owner:** when an accepted topic needs correction or extension, reopen its owning issue and edit its body in place. Create a new issue only for a genuinely ownerless domain question.
- **One current answer:** an open issue has no resolution answer. A closed issue has a positive current-contract body and exactly one evidence-only resolution comment. Decision requirements do not live in correction, supersession, checkpoint, or historical-precedence comments.
- **Child ticket:** create a positive current-state question with exactly one type label: `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling`, or `wayfinder:task`. Link it directly to the current map using GitHub's sub-issues API.
- **Blocking:** use GitHub's native issue dependencies. Add blockers through `repos/FrankQDWang/StoryOS/issues/<child>/dependencies/blocked_by` using the blocker's numeric database `id`, not its issue number or GraphQL node ID.
- **Frontier:** native dependencies form a serial chain with exactly one open, unassigned direct sub-issue whose blockers are closed.
- **Refresh gate:** before claim, read the current map, current `main`, owning tracked contracts, and affected downstream issue bodies. Align scope, Requirement ownership, wording, ordering, and native dependencies so the selected issue is the sole owner of its question.
- **Claim:** assign the selected frontier issue, then record its Contract revision, exact `main` Baseline, and SHA-256 of the UTF-8/LF-normalized issue body in a claim comment.
- **Resolve:** update the owning tracked contract, add a resolution comment that links the exact files and commit, refresh the map's current-state sections, and close the child.
- **Review:** route every newly sharp design problem to its existing owner first. When no owner exists, create one focused direct sub-issue, insert it at its exact serial position, and refresh downstream issue bodies and dependencies.
- **Charting:** create the required issues first and wire sub-issue and blocking relations in a second pass, producing one frontier issue.

## Matt feature delivery

- **Route:** when Wayfinder has resolved enough decisions to describe one build specification, run `/to-spec`, then `/to-tickets`, then one fresh `/implement` context per ticket. Wayfinder decision tickets produce decisions. Implementation tickets produce deliverables.
- **Specification:** `/to-spec` publishes the overall specification Issue as the parent. The parent owns the complete scope and acceptance contract. Its child tickets own implementation.
- **Ticket publication:** `/to-tickets` presents the proposed tracer-bullet breakdown and blocking edges to the user before publication. Publish the approved tickets as native children of the specification Issue, add native blocking edges, and apply `ready-for-agent`.
- **Tracer bullet:** each implementation ticket delivers one narrow, complete, independently verifiable path through the layers it needs. It must fit one fresh context and the repository review-size limits. Use an explicit expand-contract sequence only for a wide refactor that cannot keep all required tests passing as a vertical slice.
- **Serial frontier:** publish the approved dependency graph. Claim and execute exactly one unblocked ticket at a time. Close the current ticket before claiming the next frontier ticket.
- **Required ticket body:** include Parent, What to build, observable acceptance criteria, and native blockers. Name stable Requirement IDs, authoritative inputs, and interfaces when the specification delegates them to that ticket. Include only decision-rich code from a prototype when prose cannot state the decision precisely.
- **Claim:** before edits, read the current ticket, parent specification, exact `main`, applicable `AGENTS.md` files, and every tracked contract named by either Issue. Assign the ticket, then add one claim comment with the Contract revision when one exists, exact `main` Baseline, and SHA-256 of the UTF-8/LF-normalized ticket body. The assignment and comment together claim the ticket.
- **Execution contract:** the claimed ticket body, parent specification body, exact Baseline, applicable `AGENTS.md` files at that Baseline, and tracked contracts named by either Issue form the execution contract. A contract change requires an updated ticket body, current Baseline, updated Contract revision when one exists, and a new claim comment before implementation resumes.
- **Completion:** merge the ticket PR into `main`, synchronize `main`, run complete verification, record the exact merge commit, tree, command, and result in the ticket Resolution, refresh the current map, and close the ticket.
- **Parent completion:** after all required child tickets close, evaluate every parent user story against current `main`. Add one parent Resolution that records PASS or FAIL and linked evidence for each user story, lists the child Issues and PRs, final commit and tree, complete verification command and result, and retained out-of-scope items. Close the specification Issue when every user story passes.
- **Audit:** closed tickets preserve their current contract plus exact commits, PRs, and verification evidence. Repository history retains earlier text without making it an execution input.
