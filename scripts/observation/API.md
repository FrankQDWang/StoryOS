# Local supervision query API, version 1

Run `make observe-start`, then GET `http://127.0.0.1:3754/api/v1/runs`.
`make observe-status` includes the query container; `make observe-stop` removes it.
`make observe-rebuild` replaces projection facts in the same database; queries
resume without a service restart. The tracked Python source runs in the pinned
Python container. It mounts only the observation data directory, read-only.
No ignored prototype input is needed. The service retains only bounded health state.

Every response has `version: 1`, `read_only: true`, and UTC `queried_at`.
Successful responses include `projection_modified_at`, the database file write
time, not proof of collector progress. Collection can lag or retain deleted source
records until rebuild. Missing JSON
facts are null. HTTP success does not establish source freshness or process life.

| GET path under `/api/v1` | Result and parameters |
| --- | --- |
| `/runs` | Root `items`, `total`, `offset`, `next_offset`; `q` searches run ID, Issue and profile across all retained roots; `status` matches the retained status. Use `status=running` for unfinished roots, not proven live processes. `sort` is `newest` (default), `oldest`, or `duration`. |
| `/runs/<run>` | `record`, retained `reason`, `has_graph`, and relative report `evidence`. No graph means unavailable membership, not zero files. |
| `/runs/<run>/files` | Paginated `node_states` file rows: node and graph identity, path, selected flag, state, duration, producer. Order is path then node ID. |
| `/runs/<run>/attempts` | Paginated actual `node_attempts`, ordered by start then attempt ID. Selection reason and execution scope retain their JSON text representation. |
| `/requests` | Paginated request ID, run ID, Issue, outcome, UTC, profile, quality and allowlisted evidence; optional `run` filter. Order is UTC descending then evidence path. |
| `/violations` | Paginated findings from the existing `violations` view. Each row has rule, disposition, run, source evidence, and available root context. A prevented request has no root start. |
| `/runs/<run>/graph` | Retained graph definition and projected node states. A missing graph is null, not an empty execution claim. |
| `/runs/<run>/cost` | One root's cost and exclusive stages. An attributed root also returns Issue profile totals, exclusive stage totals, request counts, and one Issue blocked-wait total. |
| `/compare?left=<run>&right=<run>` | Existing comparison quality, run scope summaries, and paginated node differences. The two run IDs must differ. |

All lists accept `limit` (1–100, default 50) and `offset` (0–1000000).
The comparison difference list accepts `limit` up to 500. Its `next_offset`
pages through all retained nodes. Each page recomputes the current comparison.
`next_offset: null` ends the list. Each response uses one SQLite read transaction;
separate pages are live reads, not one historical snapshot. A client must apply
membership/order changes explicitly and restart paging when refreshing its list.
Root timestamp sorting removes ISO date/time separators from retained UTC text;
ties use run ID, in the same direction. Missing values sort first ascending and
last descending. Duration order is descending with run ID descending as tie breaker.
Search is case-insensitive literal substring, at most 128 characters.

Root rows expose quality, Issue, profile, status, start/actual start/end/heartbeat,
duration, actual-start flag and cache status/producer. These are retained facts,
not computed execution evidence. File state is the existing projection state;
pending, blocked and cached are not actual file attempts. A grouped pass never
establishes file success or duration. Missing attempts mean unknown execution.
Request reuse is separate from a new root; daily cache hits retain their producer.
No sum or minimal-selection claim is derived by this API.
The `violations` view supplies all six existing rule categories. Its evidence
path points to the retained report or request. The API does not serve that file.
The comparison uses the same query definitions as the provisioned dashboard.
It reports descriptive deltas when build state, scope, definitions, or other
required facts do not establish comparable evidence.

Only run IDs with 1–128 ASCII letters, digits, underscores or hyphens, starting
with a letter or digit, are accepted. Evidence references are relative to
`target/verification/`; the API serves no files. An Agent can inspect the referenced
local report and its recorded scope before proposing work. No SQL, path, command,
retry or report-write input is accepted. Unknown/duplicate parameters return 400.
Missing routes/runs return 404; write methods return 405; unavailable, busy or
over-budget database queries return 503; responses over 1 MiB return 413.
Errors carry an `error` code instead of result fields. Query work has a five-second
SQLite budget and a one-second lock timeout. Requests are serial with a three-second
socket timeout and a 2048-character target limit. The container is capped at
128 MiB and 0.25 CPU. Only loopback Host and the local Grafana Origin are accepted;
other origins return 403. These limits do not authorize any verification execution.

## Independent health

GET `/api/v1/health` accepts no parameters. Each of `collector`, `query` and
`grafana` has its own `status`, `checked_at` and `age_seconds`. Status is `ok`,
`unavailable`, or `stale`. `reported_status` preserves the last observed status
when its timestamp can be read. An age above 30 seconds, or a future timestamp,
is stale. Missing or invalid timestamps are unavailable. HTTP 200 only means
the cached health response is readable; inspect each component and its age.

A background probe samples every ten seconds plus bounded query time. Query
health reads the observation database identity and record count with a two-second
SQLite budget. Grafana health reads its local `/api/health` with a two-second
timeout and a 4 KiB response cap. Collector health reads at most 64 KiB from
`target/observation/data/collector.json`. The collector replaces this file after
each collection attempt, including errors. `pending` is its observed backlog,
not inferred from process existence. One component failure cannot suppress others.

The probe atomically replaces `target/observation/health/probe.json`; it retains
one sample, not history. `storage` reports whether that write succeeded.
`probe_checked_at` identifies the last cycle. The raw file is not loaded as a
fresh sample after restart. Query shutdown ends the probe. Stop retains source
records; start resumes collection, and rebuild replays those same records.

`make observe-start` first moves existing Grafana runtime data into
`target/observation/grafana` with Grafana stopped for a consistent copy. Later
starts reuse this directory. It includes Grafana SQLite and sidecars; it is not
the product database. Grafana warning logs rotate at 1 MiB with two-day retention
under that directory. Collector and query containers retain no Docker logs;
their bounded health records retain current diagnostics. The existing SQLite
plugin, dashboards and advisory alert rules remain provisioned from tracked input.

For Agent handoff, record the health response timestamp and component ages, then
GET the relevant run, file and attempt pages. Inspect only their allowlisted
references under `target/verification`. Report unknowns and collection lag before
suggesting action. Periodic probing does not schedule an Agent, admit a test,
retry work, or correct state. Ordinary shell activity and remote CI stay outside
coverage. Controlled fixtures and container smoke prove these health boundaries;
they do not prove natural failures or recovery.

## Approved App

Run `make observe-start` and open
`http://127.0.0.1:3749/a/storyos-supervision-app?theme=light`.
`make observe-build` assembles only tracked App inputs into
`target/observation/plugins/storyos-supervision-app`. Stop and start observation
after source changes to reload the query process. Use a hard browser reload
when replacing an existing plugin build; Grafana caches its module for one hour. Existing dashboards and alerts
stay available. The [interaction contract](INTERACTION.md) binds the approved reference.

GET `/api/v1/overview` accepts no parameters. It returns all unfinished root
counts, roots with a recent non-future heartbeat, and the projection's heartbeat
threshold. The last 24-hour start count and duration sum use only `run_cost`
actual roots whose actual start (or retained start fallback) is in that UTC window.
Any missing root duration makes the sum null. Empty windows have zero starts and
zero cost. Nested stages and reused requests are excluded. Each response has its
own query timestamp; summary and list reads need not represent one snapshot.

History uses server search and 50-row pages. Overview shows 12 recent roots and
up to 100 unfinished roots, with an explicit history link direction on truncation.
Same-tab session storage retains page settings, scroll and applied row membership.
A poll can update row facts; only Update applies changed order or membership.
The health page reads only `/health`, independently of run-list reads. It retains
the last observation on failure and adds elapsed browser time to each reported
component age. A small browser/container clock offset cannot make fresh API ages
stale. The API owns future-timestamp detection. Page scroll and observations survive
same-tab navigation and reload; poll results retain focused links and controls.

The run drawer reads the run, file, attempt and request endpoints. It follows
100-row pages, up to 10000 rows per section; larger results require Agent paging.
Separate page reads do not claim snapshot isolation. New membership requires
Update Range. Known facts refresh in place. File evidence requires a matching
node ID and graph digest; stage results and durations never become file results.
Multiple retained file attempts have their own durations summed only when all
have an end and known duration, and the projected file state is known. The
reconciled projection owns the file result; raw attempts do not override unknown.

The hash route retains page, run, level and file node ID. Same-tab storage retains
the last four visited runs, each level's scroll, and file filters and order.
Close or Escape restores source-row focus. Contextual Back follows the drawer
hierarchy; browser Back follows navigation history. Diagnostic links open a new
tab; closing that tab returns to the retained source. Evidence paths are displayed
only when they match the API allowlist. An Agent must inspect those local records
and their timestamps; the drawer grants no execution or repair authority.
