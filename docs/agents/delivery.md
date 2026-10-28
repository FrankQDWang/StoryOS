# Delivery

This document gives the rules to deliver a specification and its tickets. The rules apply to each agent client, for example Claude Code and Codex. [ADR 0070](../adr/0070-verify-locally-with-one-check-command-and-keep-github-checks-cheap.md) gives the check tiers and the merge gate. The [Issue tracker](issue-tracker.md) gives the `gh` conventions, ticket publication, the Wayfinder map, and the command that starts a workflow skill in each client.

## Roles

- **Author:** decides each change of the product, a contract, a number, or a waiver. The author approves the ticket graph of a specification and each ticket that must add more than 800 lines.
- **Coordinator:** one session for each specification. It owns the specification, the shared tracker state, and the integration branch. It starts one executor for each ready ticket, answers the questions of the executors, examines the evidence, and merges the ticket pull requests and the aggregate pull request. Only the coordinator changes the integration branch. The coordinator approves a split of a ticket only for its size.
- **Executor:** one session for each ticket, in its own worktree and branch. It implements the ticket, opens the ticket pull request, runs the checks and the review, and sends the final report. It does not merge.
- **Reviewer:** a new read-only Codex thread for each review axis. `make review` starts it. The reviewer is never the agent tool that implements the ticket. [Codex review](#codex-review) tells who runs `make review`.

## Branches and pull requests

- The coordinator makes one integration branch for each specification from an exact `main` commit. During the specification, the coordinator can freeze `main`.
- Each ticket has its own branch and worktree from the current tip of the integration branch. Two executors never change the same files at the same time.
- The ticket pull request goes into the integration branch. Its body starts with `Closes #<ticket>. Parent: #<specification>.`, so that `make review` finds the ticket. It carries the evidence of [Check Evidence Line](#check-evidence-line) and [Codex review](#codex-review).
- After the last ticket merges, the coordinator opens one aggregate pull request from the integration branch to `main`. Its body has a `Closes #<issue>` line for the specification and for each delivered ticket. It has no closing reference for an unmet product stage gate or for another specification.
- Implementation commits go to `main` only through the aggregate pull request. Each merge is an ordinary merge commit, unless the specification requires another method.
- GitHub closes linked issues only for a pull request into the default branch. Thus a ticket issue stays open until the aggregate pull request merges.

## Ticket life cycle

### 1. Specify and split

When Wayfinder has resolved the required decisions, use the `to-spec` skill, then the `to-tickets` skill. The parent owns the complete acceptance contract. Show the tracer-bullet breakdown, the test seams, and the real blocking edges to the author for approval. Publish the approved tickets as native children with native blocking edges and the `ready-for-agent` label. Each ticket fits one implementation context and the [change size](../../CODING_STANDARDS.md#change-size) rule.

A ticket body has Parent, What to build, observable acceptance criteria, and its native blockers. Name stable Requirement IDs, authoritative inputs, and interfaces when the parent delegates them. Use an expand-contract sequence only when a wide refactor cannot stay a complete passing vertical slice.

### 2. Dispatch

The coordinator uses the `implement-spec` skill for the whole specification. A ticket is ready when its blockers are satisfied. A blocker is satisfied when its ticket pull request merged into the integration branch and that merge commit is an ancestor of the integration tip, and the dependent contract permits integration delivery. A product release gate, an external acceptance, and an explicit `main`-only gate still require their specified final evidence. Keep the native edge until the aggregate pull request merges. Unknown or failed evidence blocks the dispatch. Run independent ready tickets in parallel.

The brief of an executor names the ticket, the integration branch and its tip, the files to read, and the items that earlier tickets gave to this ticket.

### 3. Read, then claim

The executor reads the current ticket, the current parent, the native dependencies, `AGENTS.md`, [CODING_STANDARDS.md](../../CODING_STANDARDS.md), and the contracts and ADRs that the ticket names. Then it claims the ticket at once. It sends no plan for approval and does not wait for a reply.

To claim, assign the issue and post the [Claim](#claim). GitHub keeps the edit history of each issue body, so the Claim has no copy or hash of the body. When the coordinator changes the parent or the ticket, it tells the executor. The executor reads the changed body and posts a new Claim before it continues.

Ask the coordinator only for a real decision. A product, contract, number, or waiver question goes to the author through the coordinator.

### 4. Implement

- Use the `tdd` skill at the approved public seams.
- After each product or test edit, run `make check-fast`.
- Commit on the ticket branch. Before the final evidence, merge the current integration tip into the branch.

### 5. Open the pull request

Push the branch and open the pull request into the integration branch. Use the `pr` skill for the body. Then run `make check` on the clean head, and put its Check Evidence Line in the body. The GitHub `verify` check runs on each push.

When `make check-full` exits, report its result to the coordinator at once. Send this report before any change to the pull request: a push, a body edit, or a comment.

### 6. Review

Run the [Codex review](#codex-review). Handle each Sourcery review thread: fix the finding, or reply with the reason. Then resolve the thread.

### 7. Final report

End the work with one report to the coordinator. Do not merge the pull request. Give these items:

- The pull request link.
- The head commit and its tree.
- The Check Evidence Line of the head, and the result of the GitHub `verify` check.
- The links of the two verdict comments of the last round, and each coordinator review comment.
- Each finding that the ticket does not fix, with its owner issue.
- The state of the worktree: clean or dirty.

### 8. Merge the ticket

The coordinator examines the evidence against the [merge gate](#merge-gate). Then it merges the ticket pull request into the integration branch and posts the [Resolution](#resolution) on the ticket.

### 9. Merge the specification

After the last ticket merges, the coordinator opens the aggregate pull request and puts a Check Evidence Line of its head in the body. The aggregate pull request has the same merge gate as a ticket pull request, unless the specification gives another review. The coordinator runs the review and merges.

Then the coordinator synchronizes `main` and reads the state of each linked issue. A closed state alone does not prove acceptance. The coordinator evaluates each parent user story on the `main` tree and posts the Resolution on the parent. It closes each issue that passed and is still open. It opens again each issue that GitHub closed and that did not pass. Then it refreshes the current map.

### 10. Retrospect and clean up

Use the `retro` skill before the work context is cleared. Record each environment finding that you can act on in its owner issue or contract. Remove the worktrees and the temporary branches only after their work and evidence are kept. Keep unrelated work and historical evidence.

A closed ticket keeps its contract and its delivery evidence. These rules apply to new work. A historical record tells the workflow of its own commit.

## Check Evidence Line

The three tiers of ADR 0070:

| Command | When | Typical time |
|---|---|---|
| `make check-fast` | After each edit. The GitHub `verify` check runs it. | about 1 minute after a build |
| `make check` | On the head of each pull request. It is the default goal. | about 10 minutes |
| `make check-full` | Only when the ticket or an explicit request requires a complete local run. | about 20 minutes |

Each tier prints one line at the end:

```text
Check Evidence Line: tier=check commit=<40-character SHA> tree=<tree SHA> result=PASS time=<UTC time> duration=<seconds>s
```

- Only a line of `make check` or `make check-full` with `result=PASS` is merge evidence. A line of `make check-fast` is not.
- `make check` and `make check-full` stop at the start when the working tree has changes. A line with `tree=DIRTY` is not evidence.
- Copy the line into the pull request body exactly. The reviewer and the coordinator compare its commit with the head of the pull request.
- Each new head needs a new line. This includes a merge of the base branch.
- Start a long tier in the background, and wait for the process to exit. The line is the last output of the tier. A step that passed is not the result of the run.

## Codex review

### Who runs it

- When Claude Code implements the ticket, the executor runs `make review PR=<number>`. The Codex plugin does the review.
- When Codex implements the ticket, nobody runs `make review`, because Codex cannot review its own work. The coordinator starts a separate read-only Claude Code session. That session runs the `code-review` skill of the `mattpocock-skills` plugin on the pull request, with the [review prompt](review-prompt.md) rules. It posts one verdict comment for each axis with the heading `## <Standards or Spec> review, round <n>: <PASS or FAIL>`. The round rules below apply.
- The coordinator runs the review of the aggregate pull request.

### What `make review` does

`make review PR=<number>` runs one round on the two axes, Standards and Spec. Before it starts, it requires these conditions:

- The worktree is clean and at the head of the pull request.
- The pull request body has a Check Evidence Line of `make check` or `make check-full` with `result=PASS` for the head.
- The GitHub `verify` check of the head passes. The command waits for it, for `WAIT` seconds (default 1200).

Then it sends the [review prompt](review-prompt.md) and a request to one new read-only Codex thread for each axis. The request has the base and head commits, the Check Evidence Line, the scoped diff command, the pull request body, and the bodies of the closed issues. The command posts one verdict comment for each axis, and it adds a link to each verdict in the `## Review verdicts` section of the pull request body. It changes only the list lines of that section. The command retries GitHub network errors, and it stops the Codex brokers of its own jobs.

### Rounds

- A pull request gets at most three rounds.
- Only a blocking finding fails a round. A finding on format, lint, names, or ASD-STE100 wording never fails a round.
- After a failed round, fix each blocking finding, commit, push, run `make check`, put the new line in the body, and run `make review` again.
- After a passed round, a new round starts only after a change to the diff of the ticket. The command refuses a round when the tree did not change.
- A merge of the base branch after a passed round needs no new round when it does not change the diff of the ticket. Compare `git diff <base>...<head>` before and after the merge. Send the Check Evidence Line of the new head and the GitHub `verify` check to the coordinator. The coordinator accepts the merge with this evidence.

### After round 3

When round 3 has open blocking findings, do not start a fourth round. Send the findings to the coordinator. The coordinator can accept a small fix:

1. The coordinator reviews the fix in a pull request comment that names the new head.
2. The fix adds a test that fails before the fix.
3. `make check` passes on the new head, and its Check Evidence Line is in the body.
4. The GitHub `verify` check of the new head passes.

Then the coordinator review comment replaces the verdicts of round 3.

### Merge gate

A pull request merges only when these three conditions are true:

1. The GitHub `verify` check of the head passes.
2. A Check Evidence Line of `make check` or `make check-full` with `result=PASS` names the head.
3. The Codex review passes: the last round passed on the two axes, or a coordinator review comment replaced round 3.

## Templates

Replace each `<...>` field. Write `none` for an empty field. Write the comment in ASD-STE100.

### Claim

```markdown
## Claim

- Owner: <executor session> for <coordinator session>, worktree `<absolute path>`, branch `<branch>`
- Contract: the current issue body and the current parent body
- Baseline: `main` at `<full SHA>`, integration base `<full SHA>` on `<integration branch>`
- Dependencies: <each blocker, its merge commit, and the ancestor check>
- Delivery: a pull request into `<integration branch>` for #<issue>. The coordinator merges it. The aggregate pull request closes the issue.
```

### Resolution

```markdown
## Resolution: <PASS or FAIL>

- Merge: PR #<number> merged into `<branch>` at `<short SHA>`. The PR head was `<short SHA>`.
- Evidence: `<Check Evidence Line of the head>`. GitHub `verify`: <result and run link>.
- Review: round <n>, Standards <result> <link>, Spec <result> <link>. <Each coordinator review comment>.
- Criteria: <each acceptance criterion: met or not met, with the file, test, or command>.
- Out of scope: <each item that stays, with its owner issue>.
```

On the parent, the Merge line names the aggregate pull request and its merge commit and tree on `main`, and the Criteria line covers each user story and each child ticket.

## Parallel resources

Each worktree owns its source, `target/`, `node_modules/`, release package, and development database. Do not share these directories through symlinks. The coordinator limits the number of executors to the CPU and memory of the host.

### Heavy runs on a shared host

Before `make check` or `make check-full`, take one of three slots. Run `mkdir -p <main checkout>/target/verify-slots`, then `mkdir <main checkout>/target/verify-slots/slot-<N>` with N from 1 to 3. When `mkdir` fails, the slot is in use: try the next slot, or wait. Put an owner file with your session and the command in the slot. Remove the slot after the run.

### Development PostgreSQL

- `scripts/dev-postgres.sh with <command> ...` runs a command with a new database. The `run` form is the same. The command owns a unique database and gives its environment to the child. It removes the container and its anonymous volume on success, failure, or interruption. `make check` uses the `with` form.
- For interactive development, use `up --interactive`, then `reload`, `env`, and `down`. These commands use the canonical checkout path. `down` also removes the anonymous data volume.
- Do not share database environment variables between worktrees.
- The old `storyos-dev-postgres` container stays. The new commands do not use or remove it. Move the data that you need before the author removes that container.

### One exact-dist journey file

These commands run one journey file. Change `FILE` to the journey under `apps/web/`. They need a clean tree, Docker, and Google Chrome Stable.

```sh
python3 scripts/package-release.py
FILE=test/browser-exact-dist/production-host.integration.test.ts sh -ec '
  repository_root=$PWD; . scripts/lib/exact-dist.sh; container=storyos-journey-$$
  trap "stop_exact_dist_server; docker rm -fv $container >/dev/null 2>&1 || true" EXIT
  require_release_package; prepare_server_database "$container"; reset_exact_dist_fixture "$container"
  if exact_dist_ai_disabled_journey "$FILE"; then start_exact_dist_server ""; else start_exact_dist_server; fi
  pnpm --dir apps/web exec vitest run --project browser-exact-dist "$FILE"'
```

To see the authority oracle counts, put `STORYOS_STAGE1_AUTHORITY_ORACLE=1` before the second command. The oracle compares the totals of all journeys. Thus with one journey it fails, and the `+` lines of its diff show the real counts.

## Live model requests

Do not stop to ask before a live model request. The author gave a standing permission. [ADR 0049](../adr/0049-admit-an-author-authorized-runtime-qualification.md) gives the contract of a qualification request.

1. Before the run, write the plan: the purpose and the planned number of requests. Automatic requests, for example a late retrieval or a recovery `Retrieve`, count.
2. Send the requests.
3. Log each request: the time, the request ID, and the result.

Stop and report to the coordinator only for an anomaly: an unexpected status, more requests than the plan, or an unexpected time or cost.

## Cleanup

Before the session ends, stop each process that you started:

- Each background wait.
- Each Codex broker and app server of a manual Codex call. `make review` stops the brokers of its own jobs.
- Each development server.
- Each disposable database, and each heavy-run slot.

The author's own `ChatGPT.app` and `CodexCLI.app` processes are not yours. Keep them running.
