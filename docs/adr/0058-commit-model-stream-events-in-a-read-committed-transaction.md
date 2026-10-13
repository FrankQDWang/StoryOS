---
status: accepted
---

# Commit Model Stream Events in a Read Committed Transaction

On 2026-10-13, the author selected a lower isolation level for each stream commit. The selection was made at [Retry a stream commit after a serialization failure instead of stopping the stream](https://github.com/FrankQDWang/StoryOS/issues/1137). This ADR amends [ADR 0054](0054-stream-the-agent-plan-response-and-commit-events-at-64-or-500-ms.md). Each stream commit is no longer a serializable Project transaction.

## Context

The S4-04b live probe found that a stream commit can fail with SQLSTATE `40001` while another Run of the same Project does Worker work. The Model Gateway then stopped the stream. On the Volcengine Agent Plan route, a stream that closes before its terminal event is not retrievable ([ADR 0057](0057-retrieve-the-stored-response-of-an-unknown-agent-plan-create.md)). Thus the outcome became unknown.

A deterministic probe on a disposable database found the cause. The two Runs share no row. The predicate locks of serializable snapshot isolation (SSI) cause the conflict:

- With small tables, PostgreSQL scans `agent_runs` and `model_attempts` sequentially. Each serializable transaction then holds a relation-level SIRead lock on these tables.
- With index scans, the stream commit holds page-level SIRead locks on the B-tree leaf pages of `agent_runs_pkey`, `model_attempts_pkey`, and `model_attempts_one_role`. One leaf page holds the rows of all Runs of the Project. The work of the other Run inserts an Attempt row into these pages.
- The Run-scoped queries of the other Run scan `model_attempts_pkey` by its Project prefix. Thus they read the Attempt row of the streaming Run, and the stream commit updates that row.

These two read-write dependencies make a cycle. PostgreSQL then cancels the stream commit as the pivot when it commits. A narrower query does not remove the cycle, because the lock granularity causes it.

## Decision

- Each stream commit runs in a read committed Project transaction, with the same row-level security scope.
- It locks the fenced Run row with `FOR UPDATE`. Read committed examines the fence predicate again on the current row version. Thus a takeover or a durable cancellation still stops the exchange.
- It locks the target Attempt row with `FOR UPDATE` before it merges the events. Thus the merge reads the current payload, and no concurrent update is lost.
- A transaction outside SSI cannot be a pivot. Thus this cycle cannot cancel a stream commit.
- The work phases, the dispatch claim, and the record of an observation stay serializable.
- A serialization failure or a deadlock of a stream commit is contention. The stream sink runs a contended commit again after a short delay, a bounded number of times. A commit that stays contended keeps its events for the next commit. Only a different failure, or contention after a bounded number of commits, stops the exchange.

## Considered options

- Retry alone was rejected. With serializable commits, each commit can meet the same cycle again while the other Run works.
- A narrower serializable query was rejected. A relation lock or a leaf page lock still covers the rows of other Runs.
- An explicit row lock in a serializable transaction was rejected. SSI still records the reads of the transaction.

## Consequences

- A stream commit can wait for the row lock of its own Run or Attempt. The Model Gateway phases lock the Run row before they write the Attempt row, and the stream commit uses the same order.
- A deterministic test overlaps a stream commit with the cancellation duties of another Run of the same Project. With a serializable stream commit, the test fails with `40001`.
- A serializable transaction that read the Attempt row can still fail with a write conflict. Its Worker runs that work again.
