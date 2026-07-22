---
status: accepted
---

# Allow Policy-Governed Post-Seal Operational Compaction

StoryOS automatically compacts eligible high-volume, non-authoritative Run and
Subrun operational payloads after terminality and the root Subrun Mailbox Seal.
The compaction is policy-versioned and inspectable: it preserves immutable
event, Attempt, Manifest, Receipt, Seal, deduplication, digest, checkpoint or
Snapshot evidence and records the resulting availability gap. It never changes
authoritative creative state, Artifacts, prior context/disclosure history, or
the author-owned deletion boundary.

## Considered options

- Retaining every raw operational payload until the author manually deletes it
  was rejected because it makes long-running ordinary writing depend on manual
  storage operations and leaves no default storage bound.
- Allowing age-based cleanup before terminality and Mailbox Seal was rejected
  because it could erase crash-recovery, at-least-once delivery, or replay
  evidence before the root can prove settlement.

## Consequences

- A compacted historical Run remains inspectable, but an inspector, export,
  replay, or Eval view must expose the known raw-payload gap rather than claim
  byte-for-byte completeness.
- Exact eligibility classes, retention windows, Snapshot materialization,
  archival storage, export/restore behavior, and purge execution are specified
  in [Run Event, Mailbox, Snapshot, Retention, and Archival
  Semantics](../foundation/run-event-mailbox-snapshot-retention-and-archival-semantics.md).
