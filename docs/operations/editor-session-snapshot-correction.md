# Editor Session Snapshot position correction

Owner: [Create an Editor Session and Persist One Pending Intent](https://github.com/FrankQDWang/StoryOS/issues/106).

## Scope

Earlier session creation stored position 0 even when the Project had committed
Activity. A new session now captures its Chapter, current Head, and Activity
position from one database statement snapshot. The session base and canonical
Snapshot use that same captured position in the existing transaction.

This changes future session creation only. It adds no schema, migration, data
backfill, or activation-time repair. Keep the existing Release 1 bootstrap and
compatibility checks. Session creation does not advance Project counters or
grant a later session the current writer generation.

## Existing records

Keep existing Snapshots, Sessions, revisions, counters, and browser Journal
bytes. Position 0 can be valid. A position below the current Project counter
can describe a valid historical Snapshot. Neither value alone identifies a
bad record; do not replace it with the current counter.

The Web Client keeps its exact Chapter/base binding and complete Journal
validation. A mismatch leaves the editor in read-only recovery and sends no
author command. Do not clear IndexedDB or Session storage to bypass that
result. A separately requested new Session uses the corrected creation path,
but remains an observer when another Session is the current writer. Writing
then requires the existing explicit takeover and recovery rules; this patch
does not transfer pending work or promise transparent repair.

Existing admitted settlement can advance a Session base under its original
contract. This is not permission to relabel historical Snapshot evidence.
Any future stored-data repair requires a separately approved forward-repair
contract from the PostgreSQL storage owner before it runs.

## Verification

The real HTTP test creates a Project, Volume, and Chapter, then checks position
3, exact retries, observer creation, and retained legacy records. The browser
test checks that a valid zero-position base opens and that a binding mismatch
preserves every Journal store without a command or implicit Session change.
Only isolated test databases contain the injected legacy wrong stamp.
