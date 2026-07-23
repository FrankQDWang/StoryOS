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
- The current body plus its exact tracked-contract baseline is the execution contract. A resolution comment records evidence only: exact commit, pull request, verification, and closure.

## Current map operations

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

## Issue-native implementation

- **Single current issue:** after planning closes, the current map contains exactly one open implementation issue. Its body is the complete contract for that implementation slice.
- **Required body:** include Contract revision, exact `Baseline: main@<commit>`, stable Requirement IDs, exact Authoritative inputs, Goal, Scope, owning modules, data flow, acceptance criteria, author journey, red tests, deterministic fault points, migration and generated artifacts, final verification, PR gate, and merge gate.
- **Execution input:** execute from the locked current implementation issue, its exact Baseline commit, the applicable `AGENTS.md` files at that commit, and the tracked files explicitly named by the issue.
- **Contract revision:** a contract change produces a refreshed issue body, a new Baseline, a new Contract revision, and a new claim lock before implementation resumes.
- **Completion:** merge the implementation PR into `main`, record the exact merge commit and verification evidence in the resolution comment, refresh the current map, and close the issue.
- **Continuation:** create the next implementation issue from the resulting `main` only after its predecessor completes. The new issue becomes the map's sole open implementation issue.
- **Audit:** closed issues preserve one current contract plus exact commits, PRs, and verification evidence for completed work. Repository history retains earlier text without making it an execution input.
