# Local supervision acceptance and Agent handoff

This guide covers US40. US41 anomaly and analysis presentation and Rust speed
work remain separate. Use the [API contract](API.md) and
[approved interaction contract](INTERACTION.md). Observation cannot admit,
execute, retry, or correct verification work.

## Agent read-only handoff

1. Record the checkout, run ID, visible symptom, and UTC observation time.
2. GET `http://127.0.0.1:3754/api/v1/health`. Record `queried_at`, each component's
   status, `checked_at`, age, and collector backlog. HTTP 200 is not overall health.
3. GET `/api/v1/runs/<run>`, then its `/files` and `/attempts` pages and
   `/api/v1/requests?run=<run>`. Follow `next_offset`; each page is a live read.
4. Read only returned allowlisted references under `target/verification/`.
   Compare run, candidate, scope, producer, actual attempts, and timestamps.
   Missing historical facts stay unknown; projection time is not process life.
5. Report facts, unknowns, and collection lag before proposing a change. Obtain
   execution authorization through the existing ticket flow. The health probe
   does not schedule an Agent or perform diagnosis and repair.

Example read: `curl --fail --max-time 10 http://127.0.0.1:3754/api/v1/health`.
Use the documented run ID grammar; do not supply SQL or arbitrary paths.

## Reproduce a tracked-source build

Use a clean committed candidate. Run these commands from its repository root:

```sh
mkdir -p target/observation
snapshot="$(mktemp -d "$PWD/target/observation/source-check.XXXXXX")"
git archive HEAD | tar -x -C "$snapshot"
make -C "$snapshot" observe-build
```

Compare `module.js`, `style.css`, and `plugin.json` under that snapshot's
`target/observation/plugins/storyos-supervision-app` with `make observe-build`
output from the same commit. The archive contains no ignored prototype, runtime
state, or installed dependencies. Python and Node are the build prerequisites.
Keep the disposable snapshot under `target/observation`; no worktree is needed.

## Integrated acceptance

| Boundary | Reproducible evidence |
| --- | --- |
| Bounded API and real record mapping | `make verify-targeted CHECK=verification-observation-tests`; labelled HTTP fixtures cover history pages, missing/legacy data, actual attempts, grouped results, reuse, empty data, refusal, and independent health. Compare a natural run with its original report. |
| Lifecycle and source retention | Hash retained reports; run `make observe-stop`, `make observe-rebuild`, `make observe-start`, and `make observe-status`; compare hashes and observe catch-up to zero. Stop retains records and projection data. |
| Containers, dashboards, and alerts | `make observe-smoke` uses disposable labelled data, actual loopback services, datasource queries, and the existing alert provisioning. It does not execute product tests. |
| Chrome navigation | Inspect overview and history, search all history, change order and page, open summary, files, file evidence and diagnostics; use Back, Escape, close and reload. Filters, focus and scroll remain usable. |
| Stable reading | Let polling update facts. New membership and order require explicit application. Verify failed reads retain evidence and the last success. |
| Health | Stop observation during a controlled lifecycle check. The loaded page retains timestamps and becomes stale. Restart clears the connection error after a successful read. Components may still report independent failures. |
| Edge states | Label empty, legacy, missing, running, interrupted, failed and reused fixtures explicitly. Do not place them among authoritative reports. UI fixtures prove presentation, not natural transitions. |
| Visual direction | Compare at the same Chrome viewport with the reference hashes. Keep compact unboxed metrics, flat rows, one full-height edge drawer and one summary Close action. |

Retain screenshots, API samples, source hash comparisons, rebuild output, and the
acceptance matrix under `target/observation/`. Retain managed verification reports
under `target/verification/`. Record exact candidate, independent reviews, complete
report, merged-tree equality and tracker result in the ticket Resolution.

Natural intermittent SQLite lock failures can occur independently of a readable
health response. Show the current observation and retain prior UI evidence; do
not claim the cause or recovery mechanism without separate evidence. A fixture
or later successful read does not establish that the underlying fault is fixed.
