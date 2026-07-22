---
status: accepted
---

# Require Snapshot Resync at Replay-Generation Boundaries

StoryOS bounds every Project Activity cursor to one replay generation. When a
cursor predates that generation's replay floor, the Server returns the existing
`activity_cursor_too_old` outcome and the client reauthorizes a fresh canonical
Snapshot before resuming. StoryOS does not retain or guess an indefinite
cross-generation cursor translation.

## Considered options

- Perpetual verified cursor migration was rejected because its retained mapping
  state would grow with every compaction generation and still needs a separate
  safe resync result when any mapping is unavailable.
- Silently resetting a cursor to the current stream was rejected because it
  could hide skipped or reordered historical activity.

## Consequences

- A compaction or archival boundary publishes an inspectable replay generation,
  replay floor, and fresh Snapshot; the old generation's closing position
  remains evidence, not a live cursor map.
- The editor owns a smooth resync experience, but must make the generation
  boundary legible rather than representing the new Snapshot as uninterrupted
  byte-for-byte replay.
