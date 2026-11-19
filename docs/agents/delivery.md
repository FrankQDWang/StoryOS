# Delivery

This document gives the rules to deliver a specification and its tickets. [ADR 0070](../adr/0070-verify-locally-with-one-check-command-and-keep-github-checks-cheap.md) gives the check tiers and the merge gate. The [Issue tracker](issue-tracker.md) gives the `gh` conventions, ticket publication, the Wayfinder map, and the command that starts a workflow skill in each client.

## Roles

- **Author:** decides each change of the product, a contract, a number, a waiver, and each merge of an aggregate pull request into `main`. The author approves the ticket graph of a specification. The [standing approvals](#standing-approvals) need no question. A report to the author follows the Reports rule of `AGENTS.md`. The author approves a ticket that must add more than 1500 counted lines only when the lines above the limit are mechanical or generated.
- **Coordinator:** one session for each specification. It owns the specification, the shared tracker state, and the integration branch. It starts the executor of each chain, answers the questions of the executors, examines the evidence, and merges the ticket pull requests and the aggregate pull request. Before dispatch, it splits each ticket that must add more than 1500 counted lines and that the author did not approve. It approves a split only for its size.
- **Executor:** one session for an ordered chain of tickets, in one worktree. Each ticket has its own branch and pull request. The executor implements each ticket, runs its review and checks, and sends its final report. It does not merge.
- **Reviewer:** a new read-only Codex thread for each review axis. The reviewer is never the agent tool that implements the ticket.

## Standing approvals

The author approved these items for all work. Do them without a question:

- Start the chains of each ready ticket of an approved ticket graph. The approval of the graph also releases its execution, unless the author says otherwise.
- After the merge of a ticket, remove its merged branches. After the last ticket of a chain merges, archive the executor session and remove its worktree.
- Remove a heavy-run slot when the session of its owner file ended.
- Do a fix that a triage decision of the author approved.
- Send a live model request ([Live model requests](#live-model-requests)).

## Branches and pull requests

- The coordinator makes one integration branch for each specification from an exact `main` commit. During the specification, the coordinator can freeze `main`.
- Each ticket branch starts from the current tip of the integration branch. Two chains never change the same files at the same time.
- The ticket pull request goes into the integration branch. Its body starts with `Closes #<ticket>. Parent: #<specification>.`, so that `make review` finds the ticket.
- The aggregate pull request goes from the integration branch to `main`. Its body has a `Closes #<issue>` line for the specification and for each delivered ticket. It has no closing reference for an unmet product stage gate or for another specification.
- Implementation commits go to `main` only through the aggregate pull request. Each merge is an ordinary merge commit, unless the specification requires another method.
- GitHub closes linked issues only for a pull request into the default branch.

## Ticket life cycle

### 1. Specify and split

When Wayfinder has resolved the required decisions, use the `to-spec` skill, then the `to-tickets` skill. The parent owns the complete acceptance contract.

The author owns each product default. Before the approval of the ticket graph, the coordinator shows the author each product behaviour that the specification decides and the author did not, as one set of choices in product words with a recommendation each. The specification body records each answer, and the coordinator publishes only when each choice has an answer. For each new state or position of an AgentRun, the parent has one acceptance line for each author action (an edit of the Chapter or of a candidate, a steering input, a cancel, a pause), or the reason why that action cannot occur in that state. For an invariant that more than one entry can break, the parent lists the entries, and one ticket owns the proof of each entry.

Show the tracer-bullet breakdown, the test seams, and the real blocking edges to the author for approval. Publish the approved tickets as native children with native blocking edges and the `ready-for-agent` label. Each ticket fits one implementation context and the [change size](../../CODING_STANDARDS.md#change-size) rule.

A ticket body has Parent, What to build, observable acceptance criteria, and its native blockers. Name stable Requirement IDs, authoritative inputs, and interfaces when the parent delegates them. Use an expand-contract sequence only when a wide refactor cannot stay a complete passing vertical slice. A criterion names a status, a problem code, or a state value only from the code that gives it. When it names an earlier Run or pass, it names the outcomes that count.

`make check` does not run the exact-dist browser journeys and the recovery drill. A ticket that changes a browser journey, a file that a journey loads, a panel surface, or a wire form requires `make check-full`, and its body says so.

When a specification changes a request form of a real route, the live run is a ticket after the first slice that sends that form. Do not make it the last ticket, because its findings give more tickets.

### 2. Dispatch

The coordinator uses the `implement-spec` skill for the whole specification. A ticket is ready when each blocker ticket merged into the integration branch, that merge commit is an ancestor of the integration tip, and the dependent contract permits integration delivery. A product release gate, an external acceptance, and an explicit `main`-only gate still require their specified final evidence. Keep the native edge until the aggregate pull request merges. Unknown or failed evidence blocks the dispatch.

Before the release, the coordinator puts the ready tickets into chains. At most three chains run at the same time. A chain has the tickets that depend on each other or change the same files, in dependency order. When the author releases the execution, the coordinator starts each chain session in that turn. The brief names the ordered tickets, the integration tip, the files to read, and the items of earlier tickets. The coordinator adds a later ticket to the end of a running chain with one message. Only a ticket that no chain can take gets a new chain, within that limit. When a ticket stops on a question or a failure, the coordinator can move the later tickets of its chain to another chain with one message.

The ticket body is the only owner of its acceptance contract. Before the brief, the coordinator puts each decision that changes the scope or a criterion in the ticket body, and the brief only links to it.

### 3. Read, then claim

The executor reads the current ticket, the current parent, the native dependencies, `AGENTS.md`, [CODING_STANDARDS.md](../../CODING_STANDARDS.md), and the contracts and ADRs that the ticket names. Then it claims the ticket at once, with no plan for approval and no wait for a reply.

To claim, assign the issue and post the [Claim](#claim). When the coordinator changes the parent or the ticket, it tells the executor. The executor reads the changed body and posts a new Claim before it continues.

The coordinator puts each decision in the ticket body first, and then it sends the message. The executor reads the ticket body again before it opens the pull request and before the final report.

Ask the coordinator only for a real decision. A product, contract, number, or waiver question goes to the author through the coordinator.

### 4. Implement

- Use the `tdd` skill at the approved public seams.
- After each product or test edit, run `make check-fast`.
- Commit on the ticket branch.
- Wait for `make check`, `make check-full`, and `make review` in the turn that starts the command. Do not end a turn while one of them runs: the end of a turn can stop the Codex jobs of the session. Wait in steps of at most 2 minutes, because a coordinator message arrives only between steps.
- When the counted lines will exceed the [change size](../../CODING_STANDARDS.md#change-size) limit, stop and report to the coordinator, which splits the ticket.

### 5. Open the pull request

When `make check-fast` and the tests that the ticket adds or changes pass, push the branch and open the pull request into the integration branch. Use the `pr` skill for the body.

### 6. Review

Before round 1, complete each evidence record that an acceptance criterion requires. For a criterion that says "each", name one test case for each member. Give a fake each refusal rule of the real route that the test depends on.

Run the [Codex review](#codex-review) rounds until a round passes. Handle each Sourcery review thread: fix the finding, or reply with the reason. Then resolve the thread.

When a finding says that the contract does not ask for a change, ask the coordinator to change the ticket body before the next round, and post a new [Claim](#claim).

After a round passes, merge the current integration tip into the branch. Then run the required tier one time on that head: `make check`, or only `make check-full` when the ticket or a request requires it. Put its Check Evidence Line in the body. Report the result of `make check-full` to the coordinator before any push, body edit, or comment.

### 7. Final report

Give one report to the coordinator with these items:

- The pull request link.
- The head commit and its tree.
- The Check Evidence Line of the head, and the result of the GitHub `verify` check.
- The links of the two verdict comments of the last round, and each coordinator review comment.
- Each finding that the ticket does not fix, with its owner issue.
- The state of the worktree: clean or dirty.

Write the hand-over notes for later tickets and a retro. After the report, change the ticket only on a new request of the coordinator, and reply to a message with the facts only. After the merge, the coordinator sends the new integration tip in one message. Then start the next ticket at step 3, on a branch from that tip.

### 8. Merge the ticket

The coordinator examines the evidence against the [merge gate](#merge-gate). Then it merges the ticket pull request into the integration branch and posts the [Resolution](#resolution) on the ticket.

After the merge of the first ticket that requires `make check-full`, and again before the aggregate pull request, the coordinator runs `make check-full` on the integration branch.

After each 10 merged tickets and after the last one, the coordinator runs one batch round on the new changes of the integration branch. It has no second round: each blocking finding gets a fix issue or an answer.

### 9. Merge the specification

After the last ticket merges, the coordinator opens the aggregate pull request and puts a Check Evidence Line of its head in the body. Then it merges by the [merge gate](#merge-gate).

Then the coordinator synchronizes `main`. A closed issue alone does not prove acceptance, so the coordinator evaluates each parent user story on the `main` tree and posts the Resolution on the parent. It closes each issue that passed and is still open. It opens again each issue that GitHub closed and that did not pass. Then it refreshes the current map.

### 10. Retrospect and clean up

Use the `retro` skill before the work context is cleared. Record each environment finding that you can act on in its owner issue or contract. Remove the worktrees and the temporary branches only after their work and evidence are kept. Keep unrelated work and historical evidence.

A closed ticket and a historical record keep the workflow of their own commit.

## Check Evidence Line

The three tiers of ADR 0070:

| Command | When |
|---|---|
| `make check-fast` | After each edit, with the Web source guards. The GitHub `verify` check runs it. |
| `make check` | On the head of each pull request. |
| `make check-full` | Only when the ticket or a request requires it, in place of `make check`. |

Each tier prints one line at the end:

```text
Check Evidence Line: tier=check commit=<40-character SHA> tree=<tree SHA> result=PASS time=<UTC time> duration=<seconds>s
```

- Only a line of `make check` or `make check-full` with `result=PASS` is merge evidence.
- `make check` and `make check-full` stop at the start when the working tree has changes. A line with `tree=DIRTY` is not evidence.
- When each changed file is a document under `docs/` or at the root that no code names, `make check` runs only the `make check-fast` steps, and its line ends with `scope=documents`.
- Copy the line into the pull request body exactly. A review round does not need the line.
- Each new head, also a merge of the base branch, needs a new line.
- Start a long tier in the background, and wait for the process to exit. The result is the line, its last output, and not a step that passed.

## Codex review

### Who runs it

- When Claude Code implements the ticket, the executor runs `make review PR=<number>`.
- When Codex implements the ticket, nobody runs `make review`, because Codex cannot review its own work. The coordinator starts a separate read-only Claude Code session. It runs the `code-review` skill of the `mattpocock-skills` plugin, with the [review prompt](review-prompt.md) rules. It posts one verdict comment for each axis with the heading `## <Standards or Spec> review, round <n>: <PASS or FAIL>`. The round rules below apply.

### What `make review` does

`make review PR=<number>` runs one round on the two axes, Standards and Spec. Before it starts, it requires these conditions:

- The worktree is clean and at the head of the pull request.
- No line of the pull request body has the word `pending`.
- The GitHub `verify` check of the head passes. The command waits for it, for `WAIT` seconds (default 1200).

Then it sends the [review prompt](review-prompt.md) and the request to one new read-only Codex thread for each axis, posts one verdict comment for each axis, and links each verdict in the `## Review verdicts` section of the body. It stops the Codex brokers of its own jobs.

### Rounds

- A pull request gets at most three rounds.
- Only a blocking finding fails a round. A finding on format, lint, names, or ASD-STE100 wording never fails a round.
- After a failed round, fix each blocking finding, commit, push, and run `make review` again. Run the required tier only after a round passes: a fix or a base merge replaces the head of an earlier heavy run, and a round beside `make check-full` can lose its Codex job. The [merge gate](#merge-gate) still needs a line for the final head.
- After a passed round, a new round starts only after a change to the diff of the ticket.
- A merge of the base branch after a passed round needs no new round when it does not change the diff of the ticket. Compare `git diff <base>...<head>` before and after the merge.

### After round 3

When round 3 has open blocking findings, do not start a fourth round. Send the findings to the coordinator. The coordinator can accept a small fix:

1. The coordinator reviews the fix in a pull request comment that names the new head.
2. The fix adds a test that fails before the fix.
3. The new head meets the conditions 1 and 2 of the [merge gate](#merge-gate).

Then the coordinator review comment replaces the verdicts of round 3.

### Merge gate

A pull request merges only when these three conditions are true:

1. The GitHub `verify` check of the head passes.
2. A Check Evidence Line of `make check` or `make check-full` with `result=PASS` names the head.
3. The Codex review passes: the last round passed on the two axes, or a coordinator review comment replaced round 3. For an aggregate pull request: each blocking finding of its batch rounds has a merged fix or an answer.

## Templates

Replace each `<...>` field. Write `none` for an empty field.

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

Each worktree owns its source, `target/`, `node_modules/`, release package, and development database. Do not share these directories through symlinks. The coordinator limits the chains to the CPU and memory of the host.

### Heavy runs on a shared host

One heavy run uses most of the host, so before `make check` or `make check-full`, take the one slot. Run `mkdir -p <main checkout>/target/verify-slots`, then `mkdir <main checkout>/target/verify-slots/slot-1`. When `mkdir` fails, the slot is in use: wait. Put an owner file with your session and the command in the slot. After the run, remove the slot with `rm <owner file>` and then `rmdir <slot>`, each as one command. A client can refuse `rm -rf`, and then the slot stays.

With more than one chain, each heavy run, also a review round, waits for the `TURN` of the coordinator to `HEAVY REQUEST <command> head <SHA>`. The answer to a `HOLD` is `ACK HOLD`. Each of these messages names the head.

### Development PostgreSQL

- `scripts/dev-postgres.sh with <command> ...` runs a command with a new database. The command owns a unique database and gives its environment to the child. It removes the container and its anonymous volume on success, failure, or interruption.
- For interactive development, use `up --interactive`, then `reload`, `env`, and `down`. These commands use the canonical checkout path. `down` also removes the anonymous data volume.
- Do not share database environment variables between worktrees.
- The executor names the prefix lead of its store tests after a search for its string, hex, and UUID suffix forms.

### One exact-dist journey file

Change `FILE` to the journey under `apps/web/`. They need a clean tree, Docker, and Google Chrome Stable.

```sh
python3 scripts/package-release.py
FILE=test/browser-exact-dist/production-host.integration.test.ts sh -ec '
  repository_root=$PWD; . scripts/lib/exact-dist.sh; container=storyos-journey-$$
  trap "stop_exact_dist_server; docker rm -fv $container >/dev/null 2>&1 || true" EXIT
  require_release_package; prepare_server_database "$container"; reset_exact_dist_fixture "$container"
  if exact_dist_ai_disabled_journey "$FILE"; then start_exact_dist_server ""; else start_exact_dist_server; fi
  pnpm --dir apps/web exec vitest run --project browser-exact-dist "$FILE"'
```

## Live model requests

[ADR 0049](../adr/0049-admit-an-author-authorized-runtime-qualification.md) gives the contract of a qualification request.

1. Before the run, write the plan: the purpose and the planned number of requests. Automatic requests, for example a recovery `Retrieve`, count. When the ticket names a token limit, the plan also gives the expected reported input of each request and the total. A request with a prior reference has the full conversation as its input. When the total is above the limit, send the plan to the coordinator and wait for the answer.
2. Send the requests.
3. Log each request: the time, the request ID, and the result.
4. Keep the raw evidence on a Git tag, as `AGENTS.md` says. The tag has the wire payloads and the Server and Worker logs, and no second copy of a public read. Before the push, examine each file for a key, an e-mail address, and a home path. The record of the run names the tag.

Stop and report to the coordinator only for an anomaly: an unexpected status, more requests than the plan, or an unexpected time or cost.

The plan comment has each item that the plan criterion names. When a new instruction text did not give its effect, send the findings before the review request. A second run that the ticket does not name needs the answer of the author.

## Cleanup

Before the session ends, stop each process that you started:

- Each background wait.
- Each Codex broker and app server of a manual Codex call.
- Each development server.
- Each disposable database, and each heavy-run slot.

Keep the author's own `ChatGPT.app` and `CodexCLI.app` processes running.
