---
status: accepted
---

# Require Lifecycle Proof Before Recovery Visibility

StoryOS treats a PostgreSQL base backup and WAL chain as bounded Recovery Copies,
not as an alternate Project history. A restored Project may become readable only
after an inspectable Recovery Visibility Proof shows that recoverable later
Redaction, Tombstone, retention, and availability decisions have been applied.
If the lifecycle range is missing or unverifiable, the Project remains in a
recovery hold instead of exposing an older view as current.

## Considered options

- Opening the newest recoverable database state immediately was rejected because
  it can resurrect content whose later protection decision is absent from that
  state.
- Destroying every backup and WAL segment when a payload is redacted was
  rejected because it breaks the Foundation Recovery Service Profile's stated
  recovery chain and RPO/RTO promise.

## Consequences

- Logical redaction and deletion take effect before physical Recovery Copy
  expiry; the author can see whether physical deletion is still pending.
- A recovery-chain gap becomes an explicit availability failure, not permission
  to treat incomplete recovery as a safe historical replay.
