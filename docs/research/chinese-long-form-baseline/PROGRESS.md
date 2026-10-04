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
