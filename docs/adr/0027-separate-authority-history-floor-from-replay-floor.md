# Separate the Authority History Floor From the Replay Floor

Earlier Manuscript Structure Transitions have Activity without Authoritative Commit or Author Action records. The repair starts complete settlement after one Authority History Floor and writes a fresh Canonical Query Snapshot at that boundary. It does not invent missing causal records, and it does not raise Replay Generation. Replay Floor remains the compaction or archival cursor bound. Using Barrier to rewrite history was rejected because Barrier is an Author Undo disposition, not a migration of past Activity.
