# Chinese long-form baseline apparatus

From the repository root, on this research branch:

```sh
WORD_COUNTS=1 sh prototypes/chinese-long-form-baseline/run.sh
```

This builds the clean product package if absent, generates four seeded corpora,
runs each scale in its own `scripts/dev-postgres.sh run` database, and writes
operation observations, SQL execution plans, corpus counts, Unicode observations,
and a derived table under `out/`. It does not run the test suite. Without
`WORD_COUNTS=1`, the independent Unicode profiles run but native Word does not.
The native Word option needs Microsoft Word; other runs need the locked repository
Node toolchain, installed Chrome, Python with Unicode 16.0.0, and Docker.

`SCALES=30000` selects the small calibration run. `SKIP_BROWSER=1` selects only
the public HTTP operations. Default execution measures all four scales and Web.
Every new observation moves an existing scale directory to an attempt directory.
Failed attempts remain there. The harness has no self-tests and is not a build
or verification input extension.

The generator uses original Chinese sentence templates, dialogue, 20–160-character
paragraph targets, indented paragraphs, and a small Latin/digit component. Five
Chapters total exactly 10,000 Unicode scalars. The four corpora share prefixes.
Chapter titles do not enter the size; one LF between paragraphs does. This is a
controlled prose-shaped workload, not a natural-language quality sample.

Create Project, Create Volume, Create Chapter, Set Current Chapter, and Apply
Author Edit load every paragraph through the public generated client. The
disposable database's ordinary controlled fixture remains present. Only Project
challenge quota counters are reset outside operation samples. No authoritative
manuscript, Proposal, or Receipt is inserted by measurement SQL.

PostgreSQL instrumentation installs pg_stat_statements in the temporary database
and preloads it by restarting that owned container. Its newly assigned port is
read again. auto_explain records actual plans with buffers and no node timing.
Only the runtime role enters SQL-call totals. Administrative probes do not.
Returned/affected SQL rows are separate from scan-row visits in actual plans.
EXPLAIN averages per-loop row values, so aggregated scan visits can carry its
rounding. Buffer counts are accesses, not unique pages or byte totals.

Each Web phase has a new browser journal and a publicly prepared writer session.
The harness sets only its server-issued active-session reference in sessionStorage.
It measures the packaged production UI, real input, IndexedDB APIs, and reload.
Browser response bodies and IndexedDB API observations include the completed
foreground action and its triggered reads. They are distinct from isolated HTTP
operation counts. A stopped Worker runs once for each export or fake-adapter
Proposal setup; no external model or provider is called.

The report branch retains compact evidence. To derive it from a completed run:

```sh
python3 prototypes/chinese-long-form-baseline/summarize.py docs/research/chinese-long-form-baseline/evidence/final
```

Use `PROGRESS.md` in the report directory to resume this task and distinguish
calibration failures from the final run. Timing is reference-only on this shared
machine. Corpus identity, public result kind, SQL calls, bytes, and text equality
are the main observations.
