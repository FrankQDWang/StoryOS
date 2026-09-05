---
status: accepted
---

# Stage a Pinned Export Source at Admission

Export admission must freeze the exact facts that later Worker settlement may read. StoryOS therefore copies those facts at admission into one Application-owned Pinned Export Source, bound to the operation's source Snapshot locator. The Worker renders or packs only from that source. It must not reconstruct admitted state from live Project rows, from Canonical Query Snapshot metadata, or from Activity position alone.

## Considered options

- Reading current Project rows after Snapshot availability was rejected. Canonical Query Snapshot stores identity, Activity position, profiles, and lifetime. It does not store the exportable records. Live tree rows, Heads, and Archive families can change after admission, so a delayed export can name Snapshot `N` while its bytes describe `N+1`.
- Raising Worker transaction isolation alone was rejected. A later `REPEATABLE READ` can make a later state internally consistent. It cannot restore the admitted state.
- Reconstructing the admitted view from Activity position was rejected. Current mutable rows do not retain every older tree, Head, and Archive-family value. Replaying every exportable family from Activity would invent a second history authority.

## Consequences

- Human-readable admission stores manuscript facts. Archive admission stores the complete exportable families. Both are the same Pinned Export Source type.
- The source is unavailable when its Snapshot is missing or expired. Settlement then fails. Live Project state is never a fallback, including for operations admitted before this source exists.
- After settlement, the source may be discarded. A `ready` export keeps its output bytes. A later Archive may include other still in-progress Pinned Export Source records. It must not nest a copy of the source that it is packing.
- This decision does not change public export transport, status vocabulary, manuscript format, or Archive byte profiles. It does not resume Stage 3.
