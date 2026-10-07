# Chinese long-form baseline progress

## Resume here

- Status: complete. Three part reports, ranked final report, and reproducible evidence are committed.
- Baseline: `origin/main@479224809cdaae997cda51cb8853e3fafa242b65`.
- Branch: `codex/chinese-long-form-baseline`.
- Worktree: `/Users/frankqdwang/.codex/worktrees/chinese-long-form-baseline/StoryOS`.
- Date: 2026-10-05 (Asia/Singapore).
- Resume: read REPORT.md and the part reports. Product defects are recorded only; no implementation or issue follow-up is authorized by this task.

## Scope and decisions

Only `prototypes/chinese-long-form-baseline/` and this report directory may change.
Do not change product code, migrations, generated contracts, or existing tests.
Do not open a pull request or change issues or other worktrees. Use only
`scripts/dev-postgres.sh run` for database lifetime and dynamic service ports.
Do not run complete verification or add harness tests. Commit progress after
 each completed step. Retain failed observations and separate measured values,
static operation counts, inferred complexity, and unavailable measurements.

The earlier Issue 76 report measured a disposable editor and synthetic schema.
This study extends it with current product command paths and Chinese corpora at
30,000, 300,000, 1,000,000, and 3,000,000 characters (1,500 chapters at the largest
scale). Corpus size units and all count profiles will be explicit.

The research skill delegates primary-source word-count research to a background
agent. It owns only its report and related candidate-count files. The parent owns
measurement, integration, progress, and commits.

## Evidence log

### Step 1: scope and baseline

- `git fetch origin main`: baseline unchanged at the SHA above.
- Created the isolated managed worktree and the requested branch.
- `make verify-status BASE=origin/main`: pending, empty changed set; recommended
  `verify-policy`. No product or test changes exist. Full verification and policy
  integration are outside this research task; do not enroll the harness.
- Read root operating instructions, domain and verification guides, parallel
  resource ownership, root glossary, and the prior Issue 76 report.
- Main checkout was clean at initial inspection; no source files there changed.

## Planned delivery

1. Seeded workload, public command route map, and reproducible runner.
2. Four-scale operation observations, SQL plans/buffers, transport and journal counts.
3. Word-count primary sources, corpus comparisons, precise candidate, golden vectors.
4. UTF-16 end-to-end observations and counterexamples or bounded no-finding result.
5. Three part reports, ranked findings in `REPORT.md`, and final scope audit.

### Step 2: corpus and product-backed smoke

- `make release-package` passed in 302.69 s at `cc61461d`; no full tests ran.
  Package manifest: `sha256:4295347e71c0374f1283d2dff8c417bcdaf6fb6f72e43a97e335bf8bfee3ae7a`.
- `python3 prototypes/chinese-long-form-baseline/generate.py` generated exact scalar
  budgets with seed 20261005. The largest corpus has 1,500 Chapters and 34,626 Blocks.
  Counts include paragraph LF separators; titles do not enter the budget.
- A real 30,000-scalar Project imported through Create Project, Create Volume,
  Create Chapter, Set Current Chapter, and Apply Author Edit. No prose SQL inserts.
  Get Statistics confirmed exactly 30,000 scalars.
- Product smoke passed: Project read 7 SQL calls; tree 12; Chapter read 8;
  statistics 9; current-Chapter and manuscript search 9 each; session read 8.
  These are isolated endpoint costs, before full Web action measurement.
- The temporary database uses pg_stat_statements and auto_explain. The extension
  needs a database restart. Docker changed its dynamic host port, so the harness
  now reads that port again. Attempt 01 records the initial connection failure.
- Attempt 02 used UUID v4 for an idempotency key; the public contract requires v7.
  The harness now allocates v7-shaped time/random identities.
- Attempt 03 tried editing a new Chapter without selecting it. It received 503
  and an empty-UUID database error. Preserve this as a protocol sequence probe,
  not proof that the ordinary editor loses text. The normal path now switches
  the Current Chapter before the edit. No product fix was made.
- Quota preparation resets only this disposable Project's challenge counters,
  outside measured actions, as existing business fixtures do. Import duration
  excludes no API calls but is not a production quota-throughput claim.
- Next: complete structural, Undo, export, Proposal, Web and UTF-16 observations;
  then run all four scales and retain compact raw evidence.

### Step 3: count definitions and native Word observations

- Completed `WORD-COUNT.md`, first-party source links/access dates, the public-domain
  digital-edition excerpt and provenance, and 19 Unicode golden vectors.
- The machine has Microsoft Word 16.89.1. Its native `compute statistics` API
  measured all four exact corpora, the public-domain excerpt, and the golden inputs.
  All temporary unsaved documents closed after measurement.
- At 3,000,000 current scalars, Word reports 2,957,724 words (-1.4092%).
  The Unicode 16.0.0 L/N scalar candidate reports 2,609,036 (-13.032133%).
- Complete current algorithms for WPS and the studied novel platforms were not
  available from the consulted first-party material. Their exact deltas stay
  unknown; independent profile sensitivity is labelled separately.
- Repeat native Word with `WORD_COUNTS=1 sh prototypes/chinese-long-form-baseline/run.sh`.
- Expanded operation run found a database startup race after restart (attempt 04).
  Added an explicit `pg_isready` barrier; no product or database schema change.

### Step 4: complete small-scale operation and coordinate run

- All 19 isolated HTTP/Worker operations and 14 ordinary Web observations completed
  at 30,000 scalars. The extra post-structure reload probe produced a recorded
  `needs_attention` outcome. This is a product-visible observation, not a harness
  timeout or a text-equality failure. No product change was made.
- The ordinary reload before structural mutations succeeded. The subsequent
  create/reorder/delete/export sequence followed by reload made the editor read-only.
  The runner keeps both outcomes and continues from each independent scope.
- Browser preparation now uses public Create Editor Session, Take Over Project
  Writer, and Set Current Chapter commands, then seeds the exact issued session
  reference in sessionStorage. The browser journal starts empty. This separates
  book-size measurement from setup recovery and preserves real product input.
- Six UTF-16 observations passed: insert, supplementary-Han replacement,
  decomposed-letter replacement, ZWJ-family replacement, full-width punctuation
  replacement, and reload. The latest probe includes regional-indicator and VS16
  emoji. Each compares expected text, DOM, public GET Chapter, and stored UTF-8.
- `COMPLEXITY.md` completes the primary-source operation map and superlinear
  inventory. `TEXT-COORDINATES.md` records the earlier pilot and owning source map;
  update its measurements from the final four-scale evidence.
- `python3 prototypes/chinese-long-form-baseline/summarize.py` derived 34 operation
  rows and actual-plan scan visits from the completed small-scale run.
- Calibration attempts 05–09 retain resource startup, request-shape, and Web
  recovery observations. The main apparatus now archives every prior run directory.
- Next: freeze the apparatus, run the single command at all four scales, inspect
  the counts and retained failures, and write the scale and final reports.

### Step 5: first four-scale evidence and controlled-plan preparation

- Completed 136 operation observations across all four sizes. Preserved the raw
  counters and actual plans under `evidence/default-run/` with SHA-256 hashes.
- All 24 Unicode observations passed: six edits/reloads per size; the probe has
  27 scalars and 36 UTF-16 units. The post-export reload was read-only at every size.
- Default statistics buffer counts are non-monotonic: 1,806 / 691,924 / 33,351 /
  133,148. Actual plans show stale estimates and repeated joins at 300,000 scalars.
  Do not treat this as an unconditional measured quadratic growth rate.
- Add a paired statistics request before and after explicit ANALYZE, outside
  measured actions. Preserve the default run separately. Repeat other actions
  after ANALYZE to make the planner-state assumption explicit.
- Capture public tree order and export headings for the observed lexical sort;
  capture response positions for the post-export session recovery mismatch.
- Add actual Web Proposal open/accept and populated-Volume deletion measurements.
  Drain pending browser traffic before counter reset to keep sample attribution clear.
- The first command completed database measurements but its final count step
  included `corpus-manifest.json` in a glob. Narrowed that glob to numeric corpus
  names. Existing native Word evidence is intact. No product changes were made.

### Step 6: controlled-plan calibration and final apparatus

- The 30,000-scalar controlled run confirmed the ANALYZE pairing and repeated all
  six Unicode checks. Its additional Proposal probe stopped after the production
  UI had already opened the single-Block candidate. The harness incorrectly
  expected a multi-location navigation link. This is an apparatus selector error.
- Measure the actual assistant Inspect action as Web Proposal open, then click
  the visible Accept control. Preserve the failed attempt; do not alter product UI.
- Final run uses the corrected count-file glob and captures tree/export order,
  canonical/session/chapter positions, actual Web Proposal actions, and the
  specified refusal when deleting a populated Volume.

### Step 7: final four-scale measurement and evidence bundle

- Sampler source `9bcc6967` completed all four managed database runs. Each scale
  has 38 operation windows: 36 without a recorded error/refusal, one expected
  `nonempty_volume` Refused result, and one failed post-export recovery.
- `evidence/final/` retains 152 rows, actual plans, inter-window plans, IndexedDB
  counts, tree/export order, planner state, response positions, and SHA-256 hashes.
  The summarizer now exposes counters even for failed/refused rows and adds the
  IndexedDB table. This post-processing change does not alter measured operations.
- All 24 coordinate observations passed exact expected/DOM/API/PostgreSQL equality.
- Confirmed generation-2 recovery mismatch: the Chapter position is one ahead of
  the canonical Snapshot after export. Numeric tree ranks also sort as text at
  every scale; the export retains that order.
- Observed the old 1,903 count label after saving the 1,930-scalar Unicode body at
  every scale. Record as a UI refresh defect candidate, not a count-profile change
  or a proved statistics-API contract violation. No refresh duration was measured.
- Web actions that remount the editor can continue after the recorded saved-state
  window. Proposal Acceptance counters are lower bounds for the full journey;
  deferred bootstrap plans are retained separately. Do not silently add setup
  intervals to another operation's counters or call the window full end-to-end.
- Final paired statistics scan visits before/after ANALYZE are 63,056/842,
  7,963/7,967, 26,599/26,603, and 79,728/79,732. The earlier default-run bad plan
  remains evidence; the paired run does not erase planner variability.
- All owned database leases completed cleanup. The one-command run is now in its
  optional native Word repeat; its previous native Word observations remain valid.

### Step 8: final reports, count repeat, and scope audit

- Completed `SCALE-ENVELOPE.md`, `WORD-COUNT.md`, `TEXT-COORDINATES.md`, and
  conclusion-first `REPORT.md`. `COMPLEXITY.md` holds the complete scoped source
  inventory and distinguishes measured plans from application complexity.
- The one-command final run exited 0, including native Word and 19 golden inputs.
  Native Word and independent corpus counts match the earlier observations when
  matched by input SHA-256. Console: `evidence/final-controlled-console.log`.
- Evidence checks confirmed all 152 operation windows, the 4 expected refusals,
  the 4 recorded recovery failures, all 24 exact text comparisons, and all 19
  golden expected property values. Both evidence manifests match every file.
- Checked local report links, source path/line existence, and `git diff --check`.
  The branch diff from the fixed baseline contains only the two authorized
  directories. Generated evidence is classified separately from apparatus/prose.
- WPS and current platform exact Unicode algorithms/deltas remain explicit
  research gaps. First-party sources do not define a complete reproducible rule;
  no account/publishing flow was used. Independent profiles are not vendor clones.
- Web remount windows remain identified lower bounds with separate deferred plans.
  These limits are stated in the reports, not hidden by a generic PASS label.
- No product, migration, generated contract, existing test, main, issue, or other
  worktree was changed. No PR, complete test suite, or `make verify-local` was run.
- Delivery is this research branch. Its reports and evidence are complete; pushing
  the branch publishes only the authorized research artifacts.
