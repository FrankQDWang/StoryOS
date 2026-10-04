# Node process cut checkpoint

Source review is complete: five tests in all three files. Three KEEP, two MERGE; zero direct candidate lines. No support-only files occur in this directory. Mutations remain pending.

| File | Source rows | Decision |
|---|---|---|
| apply-author-edit-process-cut.integration.test.ts | 179 | KEEP: durable Challenge/committed Outcome across Server restart and bounded database interruption |
| project-export-admission-process-cut.integration.test.ts | 104,180 | MERGE unclaimed restart into retained Archive claim/reclaim sequence before claim-only |
| readable-export-admission-process-cut.integration.test.ts | 106,182 | MERGE unclaimed restart into retained readable claim/reclaim sequence before claim-only |

- All three files were read in full. Their public comparisons include browser reload, Adapter Outcome query and real pinned-export HTTP tests.
- Do not describe controlled Server stops as in-flight crashes. Author Edit delivery loss is thrown after the complete response is read; GET-first ordering is explicitly scripted. PostgreSQL pause plus a two-second client abort proves no fabricated committed response while unavailable, not a server timeout bound.
- Export claim-only exits normally after persisting a claim. TTL zero makes it reclaimable. The two output families have separate claim/settlement paths and different published representations, so neither replaces the other.
- The retained export tests must receive the exact earlier unclaimed restart assertions before the separate tests are removed. Current direct removable lines remain zero.
- Receipt counts use /0$/ and could accept 10; Archive output checks only ZIP magic and a root shape. Keep the distinct recovery transition, not an invented claim of exact Receipt count or complete ZIP validation.
- Shared phase preparation is part of execution: Author Edit uses reset-challenge after the existing controlled fixture; each export phase reloads the fixture. Preserve these dependencies in any later targeted mutation run.
- No process-cut test ran during this source review. No product changes, temporary mutation or database process remains.
