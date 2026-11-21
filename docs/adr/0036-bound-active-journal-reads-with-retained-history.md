---
status: accepted
---

# Bound Active Journal Reads With Retained History

IndexedDB version 4 adds an active-partition index to the existing intent, payload-chain, and submission-group stores. Version 3 data upgrades in one version-change transaction. The Project Scope, Editor Session, writer generation, primary keys, payloads, and command identities stay the same. Unsupported older formats still enter local recovery. This replaces the version 3 storage choice in ADRs 0015 and 0017; their editor, language, and recovery decisions remain in effect.

The 2400-item guard bounds the active working set. Before another input, a complete collected prefix can leave that index only after Journal validation and exact collection-fence checks. One strict transaction retains every row under its original primary key, stores each retained collection fence under its identity, and advances a boundary linked to the last retained record, group, and fence. Active reads validate this boundary. A failed transaction retains the previous complete state. Unresolved work and uncollected payloads keep their active entries, except a closed group (amendment of 2026-11-21 below).

Historical identities and settlement proof remain inspectable through the existing partition indexes and primary keys. Their lifetime count no longer enters each input read. This is a storage access change within one Journal, not a new partition, writer takeover, Archive state, deletion rule, or retention period. Actual quota, corruption, and persistence failures still stop input. Clearing history would remove required proof, raising the limit would only postpone the defect, and loading all lifetime history would make the active path unbounded.

The editor input bound (240) can stop input while an Author Edit waits for Challenge admission (ADR 0038). During a wait, and while records that follow a group exist, the bound also counts the unsettled Journal records (ADR 0065). Then the Editor Session writing controller also cancels the Challenge wait, as it does when the editor closes. Thus, no save occurs after input stops. The frozen group and the records that follow it stay in the Local Edit Journal and recover after a reload. The editor tells the author why input stopped.

IndexedDB version 5 keeps each version 4 store, index, and row. Its upgrade writes only the schema version in one version-change transaction, and a version 3 Journal gets the version 4 index in the same transaction. Version 5 lets an intent record carry the optional binding `follows_submission_group`. The editor writes it for input during a Challenge wait (ADR 0065). A reader that finds a binding that names no earlier group enters local recovery.

A Journal that cannot upgrade keeps its earlier version and each row, and it enters local recovery. The [Web Editor Session contract](../foundation/web-editor-session-synchronization-and-recovery-semantics.md#32-immutable-intent-records) owns the record shape.

[ADR 0099](0099-remove-the-compatibility-with-earlier-local-edit-journal-versions.md) amends this record: the Web has no upgrade from an earlier Journal version.

## Amendment 2026-11-21: the collected prefix leaves before each input

The author decided this rule on 2026-11-21 from the measurement of [#1721](https://github.com/FrankQDWang/StoryOS/issues/1721) ([#1756](https://github.com/FrankQDWang/StoryOS/issues/1756)). Before each input, the complete collected prefix leaves the working set. It does not wait for the 2400-item guard. The prefix is the groups from the start of the working set that are settled and collected, each with its exact collection fence, up to the first group that is not. The validation, the strict transaction, the boundary, and the retained history do not change. The 2400-item guard still bounds the active working set. A collection occurs one time for each save. Retirement ends at the last group of the collected prefix. Thus, the working set keeps the first group that is not collected and each group after it, also a collected group, and these groups count toward the 2400-item guard.

## Amendment 2026-11-21: a closed group leaves with the collected prefix

The coordinator decided this rule on 2026-11-21 from the reproduction and the recommendation of [#1758](https://github.com/FrankQDWang/StoryOS/issues/1758). It replaces the prefix rule of the amendment above. A settled group that the collection does not take stopped the retirement for the life of the partition, and input stopped at the 2400-item guard.

- **A closed group** is a settled group that the collection does not take and that no later work needs in the working set:
  - A group with the effect `no_effect` of the Chapter of the session base, when the Journal proves that it had no effect. Its settlement, its receipt, and its payload chain pass their checks. When its head is the head of the durable base, its final text and Blocks are equal to the durable base. Otherwise, an applied group of the same Chapter that starts from the same head comes after it, and its final text and Blocks are equal to the checkpoint of its payload chain.
  - A group with the effect `no_effect` of another Chapter, when its settlement, its receipt, and its payload chain pass the same checks, and its final text and Blocks are equal to the checkpoint of its payload chain. The checkpoint is the text of that Chapter at the head of the request, and the receipt says that the head did not change. The session base names another Chapter, so the rule makes no comparison with it.
  - A group with the effect `proposal_revised`, when no record after the prefix targets the Revision that the group revised.
  - An applied group with no collection fence, when a collected applied group comes after it in the prefix.
- **The prefix** is the groups from the start of the working set that are collected or closed, up to the first group that is neither. The prefix ends at its last collected group, because the working boundary needs that group. Thus, a closed group leaves with the next collected group. Each payload chain of the prefix ends inside the prefix. A chain can continue from one closed group into a later group of the prefix, as the chain rule before this amendment wrote it.
- **A closed group leaves with its payload.** The strict transaction retains each of its rows, as it retains a collected row. The group gets no retained collection fence.
- **Not closed:** a group with zero authority of another Chapter whose first record follows a group, because the active records of the editor include it in each Chapter; a group with the effect `conflicted` or `refused`, a group with the effect `no_effect` that the Journal does not prove, and a group with the settlement `outcome_query_rejected_no_admission` or `outcome_query_requires_reconfirmation`. Such a group can have text of the author that did not save. It stays in the working set, and each group after it stays.

The validation, the boundary, and the 2400-item guard do not change. Retirement ends at the last collected group of the prefix. Thus, the working set keeps the first group that is neither collected nor closed and each group after it, and the closed groups after the last collected group.
