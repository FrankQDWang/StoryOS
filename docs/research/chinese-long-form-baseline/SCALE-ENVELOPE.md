# Chinese long-form scale envelope

Date: 2026-10-05. Product snapshot: `479224809cdaae997cda51cb8853e3fafa242b65`.

## Conclusions

All four seeded manuscripts loaded through the real public commands. The largest
has 3,000,000 displayed Unicode scalars, 1,500 Chapters, 30 Volumes, and 34,626
Blocks. The production Web run exercised open, edit/save, Chapter switch, search, Undo,
create/reorder/delete of empty structure, and small-Proposal open/Acceptance.
A completed operation window is not a proof that its product result is correct.

Two author-visible failures occur before a capacity limit: the tree orders numeric
storage ranks as text, and a completed readable export leaves a writer-generation-2
session unable to reopen as writable. Both reproduce at 30,000 scalars. The final
report ranks these findings and gives their exact reproductions.

A fixed Chapter edit is not independent of book size in the observed Web flow.
Statistics traverse all live text after the edit. Tree operations return all
Chapters. Undo and database activity constraints can inspect retained history.
Export has a source-proven quadratic Chapter join. A small response or a fixed
SQL statement count does not bound this work.

## Reproduction and evidence identity

```sh
WORD_COUNTS=1 sh prototypes/chinese-long-form-baseline/run.sh
```

This is the one command for generation, all four database leases, production Web
measurements, text-coordinate observations, count profiles, and derived tables.
Omit `WORD_COUNTS=1` when native Microsoft Word is unavailable. Dependencies and
scope are in the [apparatus README](../../../prototypes/chinese-long-form-baseline/README.md).
No harness self-tests or complete repository tests were run.

The product package was built with `make release-package` at `cc61461d`. Its product
files equal the stated baseline. The Web manifest SHA-256 is
`4295347e71c0374f1283d2dff8c417bcdaf6fb6f72e43a97e335bf8bfee3ae7a`.
The final measurement source is `9bcc6967`; each `*-import.json` records the full
source ID, measurement-script hash, package metadata, PostgreSQL version, and IDs.

Environment: macOS 15.6.1, Mac16,1, 10 logical CPUs, 32 GiB RAM, Node 24.16.0,
Python 3.14.8 with Unicode 16.0.0, Chrome 154.0.8037.93 through Playwright, PostgreSQL
16.15 in Docker. Each database uses `scripts/dev-postgres.sh run`; HTTP and database
host ports are allocated dynamically. The Worker is stopped except for explicit
one-shot work. Proposal generation uses the product's controlled fake adapter.
There is no external model request. Other host work was active. Docker reports
10 CPUs and 16,819,609,600 memory bytes available to its daemon; this is not measured
peak consumption. Exact environment and image identity are in
`evidence/final/environment.json`.

The [earlier Issue 76 study](../representative-writing-path-performance-and-storage-growth-envelope.md)
used a disposable editor, a 120-Chapter/2.4-MB synthetic book, and synthetic storage
models. This study extends the scale and uses current public product commands,
real production Web, real schema, actual query plans, and Chinese text. Its counts
are a new population; the old timings and storage projections are not pooled here.

## Corpus and setup

| Current scalar count | Chapters | Volumes | Blocks | Body UTF-8 bytes |
| ---: | ---: | ---: | ---: | ---: |
| 30,000 | 15 | 1 | 352 | 89,312 |
| 300,000 | 150 | 3 | 3,442 | 893,206 |
| 1,000,000 | 500 | 10 | 11,557 | 2,977,186 |
| 3,000,000 | 1,500 | 30 | 34,626 | 8,931,648 |

Seed `20261005` controls the original Chinese sentence/dialogue templates,
paragraph lengths, indentation, and Chapter lengths. The largest corpus has a
72-scalar median paragraph; Chapters range from 951 to 3,214 scalars. Five Chapters
sum to exactly 10,000. The four corpora share prefixes. Size includes LF between
Blocks and excludes titles. The scalar count from public Get Statistics must
match the requested scale. Corpus hashes are in `evidence/final/corpus-manifest.json`.
This controls prose shape; it is not a natural-language quality sample.

Import uses Create Project, Create Volume, Create Chapter, Create Editor Session,
Set Current Chapter, and Apply Author Edit. Every paragraph enters through the
public generated client. Instrumentation SQL installs extensions, reads evidence,
and resets only disposable Project challenge quotas outside measured actions.
No manuscript text, Revision, Receipt, or Proposal is inserted with administrative SQL.

Each Web phase starts with an empty IndexedDB journal and a server-issued session
prepared by Create Editor Session, Take Over Project Writer, and Set Current Chapter.
The only browser bootstrap write is the issued session reference in sessionStorage.
The resulting writer generation is 2. Real browser insertion, selection, UI actions,
and reload follow. Book text grows with scale; long-lived browser history and dense
Proposal sets are not varied. The API save changes one character in the same first
Chapter at every scale. The current-Chapter search/session API samples initially
use the last imported Chapter, whose size varies with the seeded corpus. Web input
uses the same second Chapter; Web switching and coordinate probes use the third.

Samples run sequentially, so later samples include preceding setup history and a
few extra characters. The import row is the exact starting scale. An API-created
small Proposal is accepted and undone before the Web phase. Web Proposal setup is
outside the open/accept samples. Deletion of a populated Volume is a separate
expected Refused outcome; ordinary Delete Volume measures an empty Volume.

## Counter meanings

- SQL calls: `pg_stat_statements.calls` for `storyos_runtime`, including transaction
  and scope statements. Challenge issuance belongs to each public write. Admin
  reset, marker, and inspection queries are excluded.
- SQL rows: rows returned or affected, not all rows examined.
- Scan row visits: sum of actual relation Scan nodes' returned and filter/recheck
  removed rows, multiplied by loops. Parent join outputs are not added again.
  PostgreSQL reports per-loop averages, so this derived sum carries its rounding.
- Buffers: shared-buffer hit/read accesses, not unique pages or bytes. These warm
  sequential runs are not a cold-cache benchmark. Statement details also retain
  dirtied/temp blocks and WAL bytes.
- Response bytes: decoded UTF-8 HTTP response bodies. Counts exclude headers,
  compression, TLS, and database-to-Server payload transport. Web samples include
  requests completed within the defined observation window; isolated API rows are
  a different boundary. Asynchronous rehydration can continue after a window.
- IndexedDB reads: returned records or keys from instrumented get/getAll/cursor APIs;
  repeated reads count again. Stored records: a separate per-store census after
  the action, excluding the census from instrumentation. API-only rows have no
  browser journal measurement, not zero journal cost.
- Time: raw `elapsed_ms` is reference-only. Browser samples wait for network and
  render quiescence; the result is not keystroke-to-paint latency. Instrumentation
  and concurrent host tasks affect it. No latency percentile or SLA is inferred.

Each `.json.gz` combines the observation, individual SQL counters, HTTP calls,
IndexedDB details where applicable, and actual auto_explain plans. The plain
`operations.json` is a derived index. `SHA256.json` authenticates retained files.
Web windows end after the action-specific saved/visible condition, network idle,
and two render frames. They do not prove that every later asynchronous effect is
finished. In particular, Proposal Acceptance can remount the workspace after the
saved Revision changes; its statistics panel is absent at the recorded boundary.
The next pre-sample drain can finish bootstrap outside the measured window.
At 1,000,000 and 3,000,000 scalars, eight and seven actual plans occur between
Acceptance END and the next action START.
Treat this row as a measured lower bound, not a complete end-to-end Acceptance cost.
Inter-window actual plans are retained separately for review; they are not silently
added to counters from another boundary.

The final command's managed-run PASS means the apparatus completed; product
failures remain explicit rows and are not converted into successful operations.

## Four-scale operation matrix

The [complete counter table](evidence/final/TABLE.md) covers every operation at
all four sizes. Its cells contain SQL calls, SQL returned/affected rows, actual-plan
scan visits, shared hits, shared reads, and response bytes. The
[IndexedDB table](evidence/final/INDEXEDDB.md) covers every Web action with returned
records, writes, and stored-record totals. [Outcome evidence](evidence/final/operations.json)
retains Refused and failed outcomes alongside their counters.

| Scale | Import HTTP requests | Runtime SQL calls | SQL returned/affected rows | Shared hits | Import seconds (reference) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 30,000 | 110 | 3,345 | 3,848 | 31,575 | 8.9 |
| 300,000 | 1,059 | 32,635 | 40,157 | 571,358 | 109.5 |
| 1,000,000 | 3,523 | 108,690 | 134,020 | 1,599,656 | 367.0 |
| 3,000,000 | 10,563 | 325,990 | 401,849 | 3,879,548 | 1009.9 |

Import counters include fixed Project/startup work. They do not include the administrative quota resets.

Cell: SQL calls / returned-or-affected rows / plan scan row visits / buffer hits / buffer reads / response bytes.

| Operation | 30,000 | 300,000 | 1,000,000 | 3,000,000 |
| --- | --- | --- | --- | --- |
| create-chapter | 71 / 60 / 1040 / 312 / 0 / 1958 | 71 / 60 / 978 / 401 / 0 / 1962 | 71 / 60 / 570 / 407 / 0 / 1965 | 71 / 60 / 1613 / 452 / 0 / 1966 |
| create-volume | 65 / 55 / 995 / 270 / 0 / 1843 | 65 / 55 / 664 / 334 / 0 / 1847 | 65 / 55 / 1083 / 348 / 0 / 1851 | 65 / 55 / 1643 / 407 / 0 / 1852 |
| delete-chapter | 68 / 73 / 1123 / 326 / 0 / 1860 | 68 / 208 / 828 / 673 / 0 / 1864 | 68 / 558 / 1101 / 1419 / 0 / 1867 | 68 / 1558 / 3186 / 3459 / 0 / 1868 |
| delete-populated-volume | REFUSED; 47 / 38 / 731 / 186 / 0 / 1661 | REFUSED; 47 / 38 / 363 / 189 / 0 / 1662 | REFUSED; 47 / 38 / 109 / 208 / 0 / 1663 | REFUSED; 47 / 38 / 153 / 216 / 0 / 1663 |
| delete-volume | 68 / 58 / 1122 / 295 / 0 / 1805 | 68 / 58 / 668 / 353 / 0 / 1809 | 68 / 58 / 577 / 359 / 0 / 1812 | 68 / 58 / 1622 / 401 / 0 / 1813 |
| export | 90 / 104 / 2163 / 2348 / 7 / 92716 | 90 / 376 / 9439 / 12246 / 7 / 904589 | 90 / 1083 / 29254 / 33494 / 7 / 3009379 | 90 / 3103 / 87488 / 138014 / 7 / 9023190 |
| open-project | 7 / 5 / 4 / 46 / 0 / 421 | 7 / 5 / 4 / 5 / 0 / 422 | 7 / 5 / 4 / 5 / 0 / 423 | 7 / 5 / 4 / 5 / 0 / 423 |
| proposal-accept | 71 / 125 / 1318 / 897 / 2 / 15282 | 71 / 125 / 1254 / 1001 / 2 / 15285 | 71 / 125 / 1717 / 1103 / 2 / 15288 | 71 / 125 / 4783 / 1233 / 2 / 15288 |
| proposal-open | 11 / 29 / 125 / 84 / 0 / 1946 | 11 / 29 / 55 / 66 / 0 / 1946 | 11 / 29 / 55 / 67 / 0 / 1946 | 11 / 29 / 55 / 90 / 0 / 1946 |
| read-chapter | 8 / 28 / 210 / 85 / 0 / 13923 | 8 / 28 / 827 / 77 / 0 / 13924 | 8 / 28 / 1572 / 100 / 0 / 13925 | 8 / 28 / 4592 / 181 / 0 / 13925 |
| reorder-chapter | 67 / 100 / 1083 / 684 / 0 / 1859 | 67 / 305 / 1139 / 2225 / 0 / 1863 | 67 / 655 / 1185 / 2793 / 0 / 1866 | 67 / 1655 / 3248 / 5041 / 0 / 1867 |
| reorder-volume | 67 / 60 / 1165 / 308 / 0 / 1843 | 67 / 66 / 987 / 440 / 0 / 1847 | 67 / 87 / 615 / 651 / 0 / 1850 | 67 / 148 / 1740 / 1352 / 0 / 1851 |
| save-input | 76 / 124 / 1340 / 1040 / 0 / 15467 | 76 / 124 / 1364 / 1171 / 0 / 15471 | 76 / 124 / 1681 / 1274 / 0 / 15474 | 76 / 124 / 4745 / 1425 / 0 / 15475 |
| search-current_chapter | 9 / 7 / 1186 / 118 / 0 / 1085 | 9 / 7 / 990 / 118 / 0 / 1088 | 9 / 7 / 1571 / 121 / 0 / 1091 | 9 / 7 / 4611 / 240 / 0 / 1091 |
| search-manuscript | 9 / 21 / 842 / 1198 / 0 / 1085 | 9 / 156 / 7967 / 10636 / 0 / 1088 | 9 / 506 / 26603 / 31293 / 0 / 1091 | 9 / 1506 / 79732 / 133353 / 0 / 1091 |
| session-read | 8 / 29 / 508 / 21 / 0 / 16365 | 8 / 33 / 515 / 112 / 0 / 19979 | 8 / 26 / 1558 / 109 / 0 / 14973 | 8 / 36 / 4598 / 217 / 0 / 18043 |
| statistics | 9 / 21 / 842 / 1198 / 0 / 1326 | 9 / 156 / 7967 / 10636 / 0 / 1332 | 9 / 506 / 26603 / 31293 / 0 / 1337 | 9 / 1506 / 79732 / 133353 / 0 / 1338 |
| statistics-before-analyze | 9 / 21 / 63056 / 1849 / 0 / 1326 | 9 / 156 / 7963 / 10982 / 0 / 1332 | 9 / 506 / 26599 / 32339 / 0 / 1337 | 9 / 1506 / 79728 / 136399 / 0 / 1338 |
| switch-chapter | 61 / 51 / 890 / 1403 / 0 / 1943 | 61 / 51 / 669 / 1436 / 0 / 1946 | 61 / 51 / 573 / 1460 / 0 / 1949 | 61 / 51 / 1616 / 1490 / 0 / 1949 |
| tree | 12 / 24 / 100 / 11 / 0 / 2387 | 12 / 161 / 781 / 23 / 0 / 16603 | 12 / 518 / 2552 / 56 / 0 / 53615 | 12 / 1538 / 6109 / 150 / 0 / 159396 |
| undo | 71 / 102 / 1595 / 1008 / 0 / 15627 | 71 / 102 / 4228 / 4762 / 0 / 15633 | 71 / 102 / 10729 / 13690 / 0 / 15639 | 71 / 102 / 31913 / 39100 / 0 / 15639 |
| web-create-chapter | 92 / 108 / 2227 / 1605 / 0 / 5876 | 92 / 380 / 9536 / 11280 / 0 / 20102 | 92 / 1087 / 29333 / 31798 / 0 / 57123 | 90 / 1606 / 12338 / 691 / 0 / 161568 |
| web-create-volume | 86 / 101 / 2155 / 2824 / 0 / 5667 | 86 / 373 / 9660 / 12696 / 0 / 19893 | 86 / 1080 / 29312 / 31789 / 0 / 56915 | 88 / 4600 / 162762 / 267219 / 0 / 165374 |
| web-delete-chapter | 225 / 372 / 5490 / 3395 / 0 / 83868 | 225 / 1187 / 25431 / 23159 / 0 / 126561 | 222 / 2807 / 46235 / 33832 / 0 / 233083 | 214 / 6362 / 132216 / 5967 / 0 / 553653 |
| web-delete-volume | 225 / 349 / 5467 / 3349 / 0 / 76748 | 225 / 1030 / 25286 / 22689 / 0 / 116211 | 214 / 1795 / 45724 / 1465 / 0 / 213909 | 216 / 6355 / 135236 / 136194 / 0 / 546496 |
| web-export | 100 / 111 / 2401 / 2385 / 0 / 93654 | 110 / 390 / 9571 / 12590 / 0 / 906438 | 120 / 1104 / 29387 / 34064 / 0 / 3013478 | 132 / 4631 / 87639 / 271968 / 0 / 9028201 |
| web-input-save | 112 / 204 / 4059 / 4534 / 0 / 35183 | 112 / 474 / 19159 / 22832 / 0 / 35194 | 110 / 674 / 59608 / 32816 / 0 / 35204 | 110 / 1674 / 102797 / 136728 / 0 / 35205 |
| web-open-project | 116 / 213 / 3849 / 2624 / 0 / 64394 | 116 / 757 / 20130 / 22002 / 0 / 92848 | 116 / 2171 / 63707 / 63164 / 0 / 166892 | 116 / 6211 / 190165 / 267842 / 0 / 378456 |
| web-proposal-accept | 232 / 421 / 5830 / 4041 / 0 / 69562 | 228 / 813 / 26038 / 12665 / 0 / 83789 | 217 / 1370 / 19741 / 2541 / 0 / 120812 | 219 / 3411 / 57105 / 3995 / 0 / 226593 |
| web-proposal-open | 25 / 36 / 530 / 124 / 0 / 12328 | 25 / 36 / 1417 / 128 / 0 / 12328 | 25 / 36 / 1117 / 160 / 0 / 12328 | 25 / 36 / 3137 / 279 / 0 / 12328 |
| web-reorder-chapter | 88 / 148 / 2270 / 2016 / 0 / 5773 | 88 / 626 / 10020 / 13257 / 0 / 18667 | 86 / 1181 / 29952 / 2837 / 0 / 55683 | 86 / 3201 / 13977 / 4056 / 0 / 162803 |
| web-reorder-volume | 88 / 108 / 2360 / 1590 / 0 / 5761 | 88 / 386 / 10031 / 11312 / 0 / 18655 | 86 / 613 / 29396 / 759 / 0 / 55671 | 86 / 1693 / 12501 / 1673 / 0 / 162792 |
| web-search | 9 / 21 / 865 / 1217 / 0 / 1085 | 9 / 156 / 8142 / 10809 / 0 / 1088 | 9 / 506 / 26628 / 31311 / 0 / 1091 | 11 / 3006 / 79757 / 266648 / 0 / 2429 |
| web-session-recovery | 116 / 207 / 3887 / 2560 / 0 / 59240 | 116 / 751 / 20146 / 21978 / 0 / 87694 | 116 / 2165 / 63721 / 63143 / 0 / 161738 | 116 / 6205 / 190179 / 267812 / 0 / 373302 |
| web-session-recovery-after-structure | FAIL; 134 / 239 / 3581 / 2872 / 0 / 58703 | FAIL; 134 / 783 / 21760 / 22104 / 0 / 87155 | FAIL; 134 / 2197 / 62921 / 63221 / 0 / 161197 | FAIL; 134 / 6237 / 184349 / 267722 / 0 / 372761 |
| web-switch-chapter | 178 / 280 / 5814 / 2995 / 0 / 74016 | 178 / 824 / 22312 / 22445 / 0 / 102476 | 176 / 1739 / 68950 / 32569 / 0 / 173852 | 169 / 4772 / 130316 / 135897 / 0 / 385414 |
| web-undo | 97 / 168 / 2630 / 2038 / 0 / 50659 | 97 / 168 / 6332 / 5023 / 0 / 50669 | 97 / 168 / 17047 / 14151 / 0 / 50679 | 97 / 168 / 50315 / 40045 / 0 / 52017 |
| web-unicode-insert | 112 / 196 / 4143 / 4368 / 0 / 32083 | 112 / 466 / 19170 / 23904 / 0 / 32094 | 110 / 667 / 59619 / 34376 / 0 / 32104 | 110 / 1666 / 102804 / 136567 / 0 / 32105 |

### IndexedDB observations

Cell: returned records / write calls / stored records. Repeated reads count again. Stored totals are a post-window census.

Web windows with delayed editor remount can be lower bounds. FAIL retains the failed window counts. Per-store details are in operations.json.

| Operation | 30,000 | 300,000 | 1,000,000 | 3,000,000 |
| --- | --- | --- | --- | --- |
| web-create-chapter | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-create-volume | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-delete-chapter | 627 / 4 / 42 | 627 / 4 / 42 | 419 / 3 / 42 | 627 / 4 / 42 |
| web-delete-volume | 627 / 4 / 42 | 570 / 4 / 42 | 465 / 2 / 42 | 627 / 4 / 42 |
| web-export | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-input-save | 109 / 13 / 13 | 109 / 13 / 13 | 109 / 13 / 13 | 109 / 13 / 13 |
| web-open-project | 30 / 6 / 5 | 30 / 6 / 5 | 30 / 6 / 5 | 30 / 6 / 5 |
| web-proposal-accept | 875 / 13 / 42 | 875 / 13 / 42 | 875 / 13 / 42 | 822 / 13 / 42 |
| web-proposal-open | 268 / 0 / 38 | 268 / 0 / 38 | 268 / 0 / 38 | 268 / 0 / 38 |
| web-reorder-chapter | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-reorder-volume | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-search | 0 / 0 / 13 | 0 / 0 / 13 | 0 / 0 / 13 | 0 / 0 / 13 |
| web-session-recovery | 88 / 3 / 13 | 88 / 3 / 13 | 88 / 3 / 13 | 88 / 3 / 13 |
| web-session-recovery-after-structure | FAIL; 1 / 0 / 42 | FAIL; 1 / 0 / 42 | FAIL; 1 / 0 / 42 | FAIL; 1 / 0 / 42 |
| web-switch-chapter | 102 / 4 / 13 | 102 / 4 / 13 | 102 / 4 / 13 | 102 / 4 / 13 |
| web-undo | 83 / 1 / 13 | 83 / 1 / 13 | 83 / 1 / 13 | 83 / 1 / 13 |
| web-unicode-insert | 184 / 13 / 18 | 184 / 13 / 18 | 184 / 13 / 18 | 184 / 13 / 18 |


## Growth by operation and source

The [operation-by-operation source map](COMPLEXITY.md#every-requested-author-operation)
gives each operation's growth and owning `path:line`. It covers Project/tree open,
Chapter navigation, input/save, all six structural edits, both search scopes,
statistics, readable export, Undo, Proposal open/accept, and session recovery.
The [superlinear inventory](COMPLEXITY.md#superlinear-and-multiplicative-paths)
records all identified superlinear paths in those routes, including bounded
working-journal paths and source-only Proposal/diff cases.

`N` is book prose, `C` Chapters, `V` Volumes, `n` affected Chapter prose, and `H`
retained history. These four samples increase `N` and `C` together. The source
separates their causes; the samples alone cannot identify which variable drives a
path. A single small Proposal or fresh journal cannot measure growth in Proposal
Operation count or retained browser history.

Whole-book dependent paths include visible statistics (`O(N)`), Web open and
Current Chapter change through mounted statistics, post-save and post-structure
statistics refresh, all-node tree serialization/rendering, Chapter reorder/delete
identity reads, export, and Proposal-location tree lookup. Search reads all selected
text before limiting matches. Get Project metadata alone is small. Chapter-local
prose reads and writes can still incur history-dependent SQL constraints or poor
query plans. See the actual-plan distinctions below.

The export join does `C(C+1)/2` Chapter-ID comparisons for complete matching vectors:
120 / 11,325 / 125,250 / 1,125,750. This is an exact algebraic count from
`crates/storyos-application/src/readable_export.rs:140` and `:149`, not an instrumented
runtime comparison counter. Export also copies and emits `O(N)` text.

## Planner sensitivity and default observations

The first run used normal import/autovacuum state. Its statistics shared-hit counts
were 1,806 / 691,924 / 33,351 / 133,148; scan visits were 63,056 / 5,989,091 /
26,599 / 79,728. The 300,000-scalar plan repeatedly scanned Blocks and joined
Chapter facts with stale low estimates. The drop at 1,000,000 is a plan change,
not evidence that larger books cost less by contract.

The final run measures statistics once before explicit `ANALYZE`, then again after
it. All other final operation samples follow that ANALYZE. Planner timestamps and
live-row estimates are retained per size. This is a controlled-plan assumption,
not a product fix; autovacuum and later writes can still affect plans. The
[default evidence](evidence/default-run/TABLE.md) remains intact and is not replaced
by the controlled observations. Neither set justifies fitting a universal Big-O
curve to four buffer counts.

| Scale | Scan visits before / after ANALYZE | Shared hits before / after |
| ---: | ---: | ---: |
| 30,000 | 63,056 / 842 | 1,849 / 1,198 |
| 300,000 | 7,963 / 7,967 | 10,982 / 10,636 |
| 1,000,000 | 26,599 / 26,603 | 32,339 / 31,293 |
| 3,000,000 | 79,728 / 79,732 | 136,399 / 133,353 |

The default API examples also distinguish three sources of single-operation growth:
Admission join plan expansion on save, historical revision/membership plan expansion
on Proposal Acceptance, and retained Author Action/activity reads on Undo and write
constraints. Exact nodes and owning source lines are in
[COMPLEXITY.md](COMPLEXITY.md#observed-sql-plans-for-chapter-local-operations). These are not all full-book prose reads.

## Recorded outcomes and research limits

The tree/export order defect, post-export recovery failure, and saved-text versus
visible-count lag observation are detailed in
[REPORT.md](REPORT.md). Ordinary same-browser reload before export succeeds, and
all 24 text-coordinate comparisons are exact. That successful text result does not
turn the later read-only recovery into a success.

The measured workload is one author/Project with realistic small Chapters and a
fresh browser journal. It does not measure annual edit retention, concurrent
writers, OS IME, model latency, WAN latency, cold-cache startup, peak RSS, portable
archive export, or a single enormous Chapter. Those are separate axes from this
four-scale baseline. Source-only multiplicative paths are labeled as such.
