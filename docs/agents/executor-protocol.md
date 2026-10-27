# Executor protocol

An executor is the session that implements one ticket for a coordinator session. These rules apply to each agent client, for example Claude Code and Codex. The check tiers and the review rules are in [ADR 0070](../adr/0070-verify-locally-with-one-check-command-and-keep-github-checks-cheap.md). The Claim and the comment templates are in [Issue tracker](issue-tracker.md).

## Refresh-gate report

After you read the ticket, its contracts, and current `main`, send this report to the coordinator. Then stop and wait for the reply.

```markdown
- Baseline: <full `main` SHA>, worktree HEAD <SHA>, <clean or dirty>
- Plan: <the approach, and each design choice with its reason>
- Files: <each file to change, with the estimated changed lines>
- Self-tests: <each test to add, and the test that fails before the change>
- Risks: <each risk, and how the plan controls it>
- Coordinator decisions: <each open question, with your recommendation>
- State: no Claim, no code, no pull request
```

While you wait, you can read code and make the plan. Do not write code, claim the ticket, change the tracker, or open a pull request. When the coordinator replies, do the Claim. Then start the work.

## Live model requests

Do not stop to ask before a live model request. The author gave a standing permission. [ADR 0049](../adr/0049-admit-an-author-authorized-runtime-qualification.md) gives the contract of a qualification request.

1. Before the run, write the plan: the purpose and the planned number of requests. Automatic requests, for example a late retrieval or a recovery `Retrieve`, count.
2. Send the requests.
3. Log each request: the time, the request ID, and the result.

Stop and report to the coordinator only for an anomaly. An anomaly is an unexpected status, more requests than the plan, or an unexpected time or cost.

## Decision owners

The coordinator approves a split of a ticket only for its size. The author decides each change of the product, a contract, a number, or a waiver.

## After the review rounds

When round 3 has open blocking findings, follow the coordinator fix procedure that `make review` prints when it refuses a fourth round. Do not start a fourth round.

## Report after check-full

When `make check-full` exits and its result is available, report the result to the coordinator at once. Send the report before any change to the pull request: a push, a body edit, or a comment.

## Final report

End the work with one report to the coordinator. Do not merge the pull request. Give these fields:

- The pull request link.
- The head SHA and the tree SHA.
- The links of the two verdict comments of the last round, and each coordinator review comment.
- The result of the GitHub `verify` check.
- The Check Evidence Line of the head.
- The state of the worktree: clean or dirty.

## Cleanup

Before the session ends, stop each process that you started:

- Each background wait.
- Each Codex broker and app server of a manual Codex call. `make review` stops the brokers of its own review jobs.
- Each development server.
- Each disposable database.

The author's own `ChatGPT.app` and `CodexCLI.app` processes are not yours. Keep them running.
