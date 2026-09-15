---
status: accepted
---

# Bound Active Journal Reads With Retained History

IndexedDB version 4 adds an active-partition index to the existing intent, payload-chain, and submission-group stores. Version 3 data upgrades in one version-change transaction. The Project Scope, Editor Session, writer generation, primary keys, payloads, and command identities stay the same. Unsupported older formats still enter local recovery. This replaces the version 3 storage choice in ADRs 0015 and 0017; their editor, language, and recovery decisions remain in effect.

The 2400-item guard bounds the active working set. Before another input, a complete collected prefix can leave that index only after Journal validation and exact collection-fence checks. One strict transaction retains every row under its original primary key, stores each retained collection fence under its identity, and advances a boundary linked to the last retained record, group, and fence. Active reads validate this boundary. A failed transaction retains the previous complete state. Unresolved work and uncollected payloads keep their active entries.

Historical identities and settlement proof remain inspectable through the existing partition indexes and primary keys. Their lifetime count no longer enters each input read. This is a storage access change within one Journal, not a new partition, writer takeover, Archive state, deletion rule, or retention period. Actual quota, corruption, and persistence failures still stop input. Clearing history would remove required proof, raising the limit would only postpone the defect, and loading all lifetime history would make the active path unbounded.
