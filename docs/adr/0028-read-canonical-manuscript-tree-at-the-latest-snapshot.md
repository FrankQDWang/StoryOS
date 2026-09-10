# Read the Canonical Manuscript Tree at the Latest Snapshot

A Canonical Manuscript Tree query returns live structure only with the latest Canonical Query Snapshot that committed in the same Core Transition as those facts. A stale Snapshot is a resync, not a historical tree. Persisting a tree copy per Snapshot, or embedding the tree in the Snapshot payload, was rejected because a Snapshot is a reading boundary, not a second history of the hierarchy.
