# Issue tracker: GitHub

Issues and Wayfinder maps for this repository live in the public GitHub repository `FrankQDWang/StoryOS`. Use the `gh` CLI for tracker operations.

## Conventions

- Create, read, edit, comment on, label, assign, and close issues with the corresponding `gh issue` commands.
- Infer the repository from the configured Git remote when possible; otherwise pass `--repo FrankQDWang/StoryOS` explicitly.
- Before mutating an issue, read its current body, labels, assignees, dependencies, and comments. Tracker state may have changed concurrently.
- In human-facing text, refer to an issue by its linked title, never by a bare issue number.
- One task has one execution owner. Claim work by assigning the issue before doing it.

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub shares one number space across issues and pull requests. If an ambiguous number must be resolved, try `gh pr view <number>` and then `gh issue view <number>`.

## Publishing and fetching

- When a skill says to publish to the issue tracker, create a GitHub issue in `FrankQDWang/StoryOS`.
- When a skill says to fetch a ticket, read the issue body, labels, assignees, dependencies, and resolution comments.

## Wayfinding operations

- **Map:** one issue labelled `wayfinder:map`. Its body contains Destination, Notes, Decisions so far, Not yet specified, and Out of scope.
- **Child ticket:** create an issue with exactly one type label: `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling`, or `wayfinder:task`. Link it to the map using GitHub's sub-issues API. If sub-issues are unavailable, use a map task list and put `Part of <linked map title>` in the child body.
- **Blocking:** use GitHub's native issue dependencies. Add blockers through `repos/FrankQDWang/StoryOS/issues/<child>/dependencies/blocked_by` using the blocker's numeric database `id`, not its issue number or GraphQL node ID. If native dependencies are unavailable, put a `Blocked by:` line in the child body.
- **Frontier:** the map's open, unassigned child issues with no open blockers, in map order.
- **Claim:** assign the selected frontier issue to the current developer before investigation or mutation.
- **Resolve:** add the decision or result as a resolution comment, close the child, and append only a linked one-line gist to the map's Decisions so far. The full decision lives in exactly one child issue.
- **Charting:** create all issues first, wire sub-issue and blocking relations in a second pass, then stop without resolving a newly opened ticket in the same charting session.
