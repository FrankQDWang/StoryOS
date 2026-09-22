# Local supervision query API, version 1

Run `make observe-start`, then GET `http://127.0.0.1:3754/api/v1/runs`.
`make observe-status` includes the query container; `make observe-stop` removes it.
`make observe-rebuild` replaces projection facts in the same database; queries
resume without a service restart. The tracked Python source runs in the pinned
Python container. It mounts only the observation data directory, read-only.
No ignored prototype input is needed. The service stores no logs or runtime state.

Every response has `version: 1`, `read_only: true`, and UTC `queried_at`.
Successful responses include `projection_modified_at`, the database file write
time, not proof of collector progress. Collection can lag or retain deleted source
records until rebuild. Health and backlog probes belong to US40-B. Missing JSON
facts are null. HTTP success does not establish source freshness or process life.

| GET path under `/api/v1` | Result and parameters |
| --- | --- |
| `/runs` | Root `items`, `total`, `offset`, `next_offset`; `q` searches run ID, Issue and profile across all retained roots; `status` matches the retained status. Use `status=running` for unfinished roots, not proven live processes. `sort` is `newest` (default), `oldest`, or `duration`. |
| `/runs/<run>` | `record`, retained `reason`, `has_graph`, and relative report `evidence`. No graph means unavailable membership, not zero files. |
| `/runs/<run>/files` | Paginated `node_states` file rows: node and graph identity, path, selected flag, state, duration, producer. Order is path then node ID. |
| `/runs/<run>/attempts` | Paginated actual `node_attempts`, ordered by start then attempt ID. Selection reason and execution scope retain their JSON text representation. |
| `/requests` | Paginated request ID, run ID, Issue, outcome, UTC, profile, quality and allowlisted evidence; optional `run` filter. Order is UTC descending then evidence path. |

All lists accept `limit` (1–100, default 50) and `offset` (0–1000000).
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
