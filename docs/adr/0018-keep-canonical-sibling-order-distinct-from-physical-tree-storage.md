---
status: accepted
---

# Keep Canonical Sibling Order Distinct from the Physical Tree Storage Key

Create Volume and Create Chapter acknowledgements, new create Activity events, and exact replay must report Canonical Sibling Order: the 1-based rank of the new live node among live siblings in current Authoritative manuscript structure. The physical `tree_order` column remains a sparse unique storage key that includes tombstones and is never reused. This Issue is independent of structural Commit, Author Action, and Snapshot settlement.

## Considered options

- Treating the physical storage key as public `order` was rejected because deletion keeps the row, unique indexes include tombstones, and the Canonical Manuscript Tree already enumerates live siblings as `1..N`. After create A, B, C, remove B, and create D, those two numbers diverge.
- Projecting historical v1 create Events into Canonical Sibling Order was rejected because current structural creates do not persist a Snapshot at that Activity position, so a later reader cannot prove the live-sibling rank at that time.
- Raising Replay Generation was rejected because Replay Generation is a compaction or archival epoch with a replay floor, not an event-schema meaning change. Historical v1 bytes stay immutable. New settlements keep `event_kind` `volume_created` or `chapter_created` and use a new `event_schema` in which `order` means Canonical Sibling Order.
- Rewriting historical HTTP acknowledgements was rejected. Exact retry stays byte-stable, including Create Volume responses that stored no order and answered `"1"`. New settlements persist Canonical Sibling Order in the Receipt and replay that immutable result.

## Consequences

A later reorder or deletion does not change an immutable create acknowledgement. Readers must interpret v1 create Activity `order` as the stored storage key, not Canonical Sibling Order. This decision does not add a structural Commit, Author Action, or Snapshot seam.
