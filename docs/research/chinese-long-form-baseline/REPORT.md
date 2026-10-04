# Chinese long-form baseline

Date: 2026-10-05. Product baseline: `origin/main@479224809cdaae997cda51cb8853e3fafa242b65`.

## Conclusion

A 3,000,000-scalar, 1,500-Chapter Chinese manuscript loads through StoryOS's public
commands and supports the measured ordinary editor operations. It also exposes two
functional failures: numeric Chapter/Volume order becomes lexical order, including
in readable exports; and a writer-generation-2 session becomes read-only after
export and reload. Both occur at the smallest 30,000-scalar scale. No text-coordinate
mismatch was found in 24 full Web-to-database observations across the four scales.

Large-book costs are not confined to opening the book. A Chapter save triggers
whole-book statistics in Web; tree actions transfer all Chapters; Undo and write
constraints inspect history; readable-export admission contains a quadratic Chapter
lookup. SQL call counts alone hide this work. Actual plans also show strong
sensitivity to PostgreSQL statistics, so this baseline retains both default and
explicit-ANALYZE observations.

At the current 3,000,000-scalar count, Word 16.89.1 reports 2,957,724 words, while
the Unicode letters/numbers candidate reports 2,609,036. These are different
writing measures, not conversion constants. The existing product profile is unchanged.

## Findings by severity

### High: numeric tree ranks are sorted as text

Observed on all four sizes. In the 15-Chapter book, Chapters return as
`1, 10, 11, 12, 13, 14, 15, 2, 3, 4, 5, 6, 7, 8, 9` immediately after import.
At ten or more Volumes the same problem affects Volume order. The public response
then renumbers this incorrect order as consecutive ranks. A correct ordinal title
alone cannot restore the public rank used by the UI.

Evidence: `evidence/final/*-tree-order.json` retains generated IDs in import order
and the complete public tree. Actual plans in `*-tree.json.gz` show Sort Keys
`((volume.tree_order)::text)` and `((chapter.tree_order)::text)`.
`crates/storyos-adapter-postgres/src/manuscript_tree.rs:73` and `:91` select
`tree_order::text`; `:84` and `:103` order by the output name. Lines 112–129 assign
new public ranks from the resulting vectors. The stored order column is numeric
(`crates/storyos-adapter-postgres/migrations/0014_create_volume.sql:11`). The live sibling-rank contract is in
`GLOSSARY.md:51` and `docs/adr/0018-keep-canonical-sibling-order-distinct-from-physical-tree-storage.md:7`.

The export uses this same tree order (`crates/storyos-adapter-postgres/src/readable_export.rs:274`;
`crates/storyos-application/src/readable_export.rs:140`). After the measured move of
Chapter 1 to the end of the first Volume, the 15-Chapter export starts
`2, 11, 12, 13, 14, 15, 1, 3` instead of `2, 3, 4, 5, 6, 7, 8, 9`.
`*-export-result.json` retains every exported heading and the content hash.

Source-derived consequence, not separately executed: rename passes the returned
Chapter rank back to Update Chapter (`apps/web/src/chapter-tree-actions.tsx:61`,
`:99`), whose physical reorder logic compares that value with storage order
(`crates/storyos-adapter-postgres/src/update_chapter.rs:155`). Rename/move operations
can therefore change order according to the wrong rank. This is more than a visual
sort issue. The baseline records it without changing product code.

### High: completed export prevents writable session recovery

Ordinary reload before export succeeds at every size. The measured structural
create/reorder/delete sequence and readable export then finish. Reloading the same
Project produces `needs_attention` and `contenteditable=false` at every size.
This is a retained product result, not a browser timeout. The observed session uses
public writer takeover, generation 2, with a fresh local journal.

| Scale | Chapter activity position | Snapshot position | Editor after reload |
| ---: | ---: | ---: | --- |
| 30,000 | 79 | 78 | `needs_attention`; contenteditable=`false` |
| 300,000 | 486 | 485 | `needs_attention`; contenteditable=`false` |
| 1,000,000 | 1543 | 1542 | `needs_attention`; contenteditable=`false` |
| 3,000,000 | 4563 | 4562 | `needs_attention`; contenteditable=`false` |

The failure response trace contains successful Chapter, session, tree, and Snapshot
queries, then stops before the second session validation. The Chapter's Project
activity position is one ahead of the Snapshot. Web requires equality at
`apps/web/src/editor-session.ts:357`–`:364`. Export settlement increments the scope
counter and adds an operational event (`crates/storyos-adapter-postgres/src/readable_export_work.rs:279`
and `:297`) without writing a new canonical Snapshot. Get Chapter reads the current
counter (`crates/storyos-adapter-postgres/src/chapter_query.rs:48`); Get Tree locates
the latest canonical Snapshot (`crates/storyos-adapter-postgres/src/manuscript_tree.rs:36`).
The recorded values and this path identify the mismatch.

Evidence: `*-web-session-recovery-after-structure.json.gz` contains HTTP positions
and counters; the companion `*-failure.json` contains visible text, editor state,
and journal census. For a small reproduction, run `SCALES=30000 sh prototypes/chinese-long-form-baseline/run.sh`.
This reproduction includes the preceding structural sequence.
It is not a claim about generation-1 startup, arbitrary crash recovery, or text loss.

### Medium: a small visible edit includes whole-book work

The isolated author edit updates one Chapter. The same production Web edit cycle
requests statistics (`apps/web/src/manuscript-statistics.tsx:41`, `:60`). Statistics
load every live Chapter payload and traverse every scalar
(`crates/storyos-application/src/manuscript_statistics.rs:66`, `:103`). Thus a
one-character change can add `O(N)` work as the manuscript grows, although the
statistics HTTP response stays near 1.3 KB. Web open, Current Chapter changes,
and structure changes also trigger this work.

The full [scale report](SCALE-ENVELOPE.md) and [counter matrix](evidence/final/TABLE.md)
separate endpoint counters from Web observation windows. Proposal Acceptance can continue workspace rehydration
after the saved-state boundary; its Web counters are a lower bound, with deferred
query plans retained separately. All-node tree responses also
grow with Chapters and Volumes. Collapsing a Volume does not avoid its Chapter map
(`apps/web/src/manuscript-tree.tsx:184`). Chapter reorder/delete read all live Chapter
identities (`crates/storyos-adapter-postgres/src/update_chapter.rs:58`;
`crates/storyos-adapter-postgres/src/delete_chapter.rs:91`).

### Medium: plans and retained history enlarge single-operation costs

The default 300,000-scalar statistics query visits about 5.99 million scan rows,
while the 1,000,000-scalar query visits 26,599. Actual plans show repeated joins
under stale estimates; this is not a monotonic capacity curve. The final run pairs
statistics before/after ANALYZE and states that planner assumption for all following
samples. Both populations remain available.

Separate observed paths affect single-Chapter commands: an Admission query repeats
scope scans on save; Proposal Acceptance expands historical revision/membership
joins; Undo inspects retained Author Actions. The default 3,000,000-scalar Undo plan
scans 4,531 forward actions. Activity constraints also read Project history during
small writes. These findings have query/node counts and `path:line` in
[the actual-plan analysis](COMPLEXITY.md#observed-sql-plans-for-chapter-local-operations). They are not all prose scans.

### Medium: export and local-history paths have superlinear work

Readable-export admission searches the Chapter-fact vector once for each tree
Chapter. At 1,500 Chapters it requires 1,125,750 ID comparisons for complete matching
vectors. This exact source count is `C(C+1)/2`, in addition to full text copying
(`crates/storyos-application/src/readable_export.rs:149`).

Local journal reads also inspect retained history outside the bounded working set.
Acceptance reconstruction, per-Operation candidate reconstruction, Proposal-location
rendering, and diff cleanup have multiplicative or quadratic paths. The
[complete scoped inventory](COMPLEXITY.md#superlinear-and-multiplicative-paths)
lists each trigger, bound, and source. The four book sizes do not measure long-lived
journal history, dense Proposals, or one huge Block; those cases remain source findings.

### Observed candidate: the visible count can lag a saved edit

The Unicode insertion produces a saved 1,930-scalar Chapter in DOM, API, and
PostgreSQL. In the same action window the statistics label still shows 1,903.
The statistics response precedes the edit response. This is a refresh-timing
observation, separate from count-profile choice and text preservation. The run
does not measure how long the stale label remains or prove permanent failure.

The panel depends on save state, Current Chapter, and tree revision, but not on
Authoritative Revision (`apps/web/src/manuscript-statistics.tsx:41`, `:60`). It
requests no required watermark. The statistics application checks a required
watermark only when supplied (`crates/storyos-application/src/manuscript_statistics.rs:87`).
The projection contract permits lag; this is a UI freshness defect candidate,
not a proved violation of the statistics API's freshness contract. Evidence is
`*-web-unicode-insert.json.gz` beside `*-coordinates.json`.

### Decision input: the Chinese count needs an explicit profile name

The current label counts all Unicode scalars, including whitespace, line feeds,
punctuation, combining marks, and emoji components. The researched candidate counts
only Unicode 16 letters/numbers, without normalization. On the 3,000,000-scalar
synthetic manuscript it is 13.032% lower; on the public-domain excerpt it is 17.368%
lower. Native Word is 1.409% and 3.707% lower respectively. The variation rules out a
single percentage conversion.

[WORD-COUNT.md](WORD-COUNT.md) provides first-party links with access dates, all
four-scale and public-domain comparisons, the precise Unicode algorithm, and 19
golden vectors. WPS and the current Qidian/Fanqie/Jinjiang sources do not establish
complete reproducible Unicode algorithms. Their exact deltas remain unmeasured;
independent sensitivity profiles are not substituted for them. The author retains
the choice of product count. No existing profile changed.

## Text-coordinate result

[TEXT-COORDINATES.md](TEXT-COORDINATES.md) records six exact comparisons at each size:
insert; supplementary-Han replacement; decomposed-letter replacement; ZWJ emoji
replacement; fullwidth punctuation replacement; and reload. Expected text equals
DOM text, public Get Chapter, and decoded authoritative PostgreSQL text in all 24.
The probe includes supplementary Han, combining marks, flags, VS16, skin tone,
fullwidth Latin/digits, and ideographic space. Web selects UTF-16 ranges; Core maps
those ranges to UTF-8 byte boundaries; PostgreSQL preserves the supplied sequence.
OS IME and Proposal coordinate routes are not part of these observations.

## Evidence and handoff

Run from this isolated research branch:

```sh
WORD_COUNTS=1 sh prototypes/chinese-long-form-baseline/run.sh
```

Deliverables:

- [Scale envelope](SCALE-ENVELOPE.md), [source complexity inventory](COMPLEXITY.md),
  [all operation counters](evidence/final/TABLE.md), and [IndexedDB counts](evidence/final/INDEXEDDB.md).
- [Count research](WORD-COUNT.md), [native measurements](word-count-evidence.json),
  and [golden vectors](../../../prototypes/chinese-long-form-baseline/count-golden.json).
- [Text coordinates](TEXT-COORDINATES.md), with four retained exact-string artifacts.
- [Progress and decisions](PROGRESS.md), [apparatus](../../../prototypes/chinese-long-form-baseline/README.md),
  default and controlled plan evidence, SHA-256 manifests, and command logs.

All changes are confined to the requested prototype and report directories on
`codex/chinese-long-form-baseline`. Product code, migrations, generated contracts,
existing tests, main, issues, and other worktrees were not changed. No PR was opened.
The release package build and targeted apparatus execution provide the evidence;
`make verify-local` and full tests were not run.
