# Chinese long-form baseline progress

## Resume here

- Status: active. Step 1 (scope and baseline) complete.
- Baseline: `origin/main@479224809cdaae997cda51cb8853e3fafa242b65`.
- Branch: `codex/chinese-long-form-baseline`.
- Worktree: `/Users/frankqdwang/.codex/worktrees/chinese-long-form-baseline/StoryOS`.
- Date: 2026-10-05 (Asia/Singapore).
- Next: read owning contracts and public command entry points; build the seeded corpus and disposable measurement runner.

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
