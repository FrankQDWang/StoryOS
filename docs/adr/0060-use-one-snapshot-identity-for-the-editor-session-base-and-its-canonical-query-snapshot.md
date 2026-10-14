---
status: accepted
---

# Use One Snapshot Identity for the Editor Session Base and Its Canonical Query Snapshot

On 2026-10-14, the author selected the identity rule of an Editor Session base snapshot. The decision was made on [Use one snapshot id for the base snapshot and the persisted snapshot at writer takeover](https://github.com/FrankQDWang/StoryOS/issues/1151).

## Context

[Fence Stale Writers and Resync across Replay Generations](https://github.com/FrankQDWang/StoryOS/issues/111) required that the Editor Session base snapshot and the Canonical Query Snapshot "remain distinct identities". The implementation did not keep this rule:

- Five paths that write a new base use one snapshot id for the base row and for the persisted Canonical Query Snapshot. These paths are Editor Session open, Current Chapter selection, the Authoritative Revision settlement of an author edit, the Structural Authority Settlement, and the Current Chapter compensation.
- When the Local Edit Journal has no activity cursor, the Web Client reads the Project Activity Stream with the snapshot id of the Editor Session base.
- Writer takeover was the only path that used two ids. After a takeover, the activity read with the base id found no Canonical Query Snapshot and returned 404. The editor became read-only and showed that the activity stream cannot synchronize ([#1151](https://github.com/FrankQDWang/StoryOS/issues/1151)).

## Decision

- The base snapshot of an Editor Session is a persisted Canonical Query Snapshot. The base row and the Canonical Query Snapshot have the same snapshot id.
- Each new base gets a new snapshot identity. The paths that make a new base are Editor Session open, Current Chapter selection, an Authoritative Revision settlement, a Structural Authority Settlement, the Current Chapter compensation, and writer takeover. Each path writes the base row and the Canonical Query Snapshot with that identity in the same transaction.
- This decision replaces the rule of #111 that the two snapshots "remain distinct identities" and that the implementation must "not collapse them". The other #111 rules stay: a takeover gives the winner a new base identity, and the earlier Snapshots stay unchanged.

## Considered options

- Persist the takeover base as a second Canonical Query Snapshot with a different id at the same Activity position. Rejected: two Canonical Query Snapshots then have one position, and the client still reads activity with the base id.
- Keep two ids, and make the Web Client write the activity cursor from the Canonical Query Snapshot that it reads after a takeover. Rejected: the base id then stays unreadable for each other reader, and the five other paths already use one id.

## Consequences

- After a takeover, the activity read with the base id of the winner succeeds, also when the Local Edit Journal has no activity cursor, for example on a new Server origin.
- The `resulting_snapshot_id` of a takeover Receipt is the same as the snapshot id of the base of the winner.
- A takeover persists one Canonical Query Snapshot, as before.
