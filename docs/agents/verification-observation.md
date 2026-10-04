# Verification observation

This optional local service displays retained verification records. It is not
part of the [Repository verification](verification.md) main flow. An
implementer can run checks without this service.

Use Docker Engine with Compose 2.24.4 or later and Python 3.13 or later.
`make observe-start` builds the pinned Grafana OSS image and SQLite plugin, imports
retained records, and starts a five-second collector. Open
<http://127.0.0.1:3749/d/storyos-verification>. `make observe-status` shows container
state and current records. `make observe-stop` removes the observation containers;
records and the read model remain. `make observe-rebuild` rebuilds the read model
from retained files. Collection catches up after a restart without test execution.

The collector reads only `target/verification/*/report.json`, step files, and
request files. It writes `target/observation/data/runs.sqlite`. These directories
are ignored source and cache inputs. SQLite is disposable observation, not
admission or test evidence. No executor imports this database. The dashboard shows
root Issue attribution (or unknown), live stages, scope, reason, elapsed time to
the last heartbeat, and heartbeat age. Display elapsed time uses UTC and is an
estimate; recorded monotonic durations remain in the read model. A stale heartbeat
means unknown liveness, not completion. It does not change the reported status.
Legacy and malformed inputs stay in the diagnostics view. Raw records are never
changed; missing historical attribution remains unknown. Deleted input records
stay in the read model until a rebuild. Rebuild does not preserve derived facts.

The public collector accepts `collect`, `watch`, `status`, and `rebuild`, with
`--records` and `--database` for disposable inputs. `watch --interval` accepts
1 to 3600 seconds. Each collection transaction writes at most 200 changed records;
watch catches up on later polls and one-shot commands drain the backlog. Each
input is limited to 16 MiB. Containers have CPU and memory limits. Collection scans
retained paths; volume overhead measurement belongs to the Issue-cost follow-up.

Grafana binds only to loopback and permits anonymous Viewer access. Login and basic
authentication are disabled; its unused administrator password is random on each
start. The provisioned data source is not editable, opens SQLite with `mode=ro`,
and has `attachLimit: 0`. Only the observation directory is mounted, read-only.
The plugin's default internal-database block list remains enabled. The collector
has no network. Grafana has no configured external notification destination, and
analytics is disabled; advisory alert evaluation stays local. Setup needs Internet access to pull images
and the plugin; this does not change product hosting.

Run `make verify-targeted CHECK=verification-observation-tests` for collector
regressions. Run `make observe-smoke` after installation changes. This bounded
check starts an actual disposable managed command and queries the provisioned
Grafana dashboard through the real SQLite plugin on a temporary loopback port.
It removes its containers and temporary database when complete. It does not run
the product suite. `make verify-policy` discovers the collector tests normally.

## Issue cost and advisory rules

The same dashboard shows `issue_cost`, `request_cost`, `stage_cost`, and
`violations`. Root totals include only recorded actual starts. They retain the
profile and final state, including failures and interruptions. Reused reports
and refused requests stay separate. `observed_runs` retains historical and
incomplete observations; missing attribution and duration remain unknown.

Stage intervals partition each root duration. Child intervals replace parent
intervals; concurrent leaf intervals have one `concurrent` cost. Remaining time
is `unclassified`. These are elapsed costs, not CPU measurements. An optional
`blocked_intervals` list contains explicit monotonic start/end pairs. An empty
list records zero; an absent list means unknown. Multiple roots need one explicit
`blocked_clock` identity before their intervals can form an Issue-wide union.
The Issue blocked total repeats across its profile rows; do not sum those cells.
Current executors do not measure user blocking, so their value remains unknown.

Advisory rules detect daily complete dispatch, duplicate candidate starts without
recorded recovery, unassigned starts, overlapping checkout resource use, stale
heartbeats, and comparable runtime growth. Requests do not become executed
violations. A stale heartbeat means unknown liveness. Runtime comparison requires
an explicit recorded `build_state` (`cold` or `warm`), equal effective scope,
policy, tools, execution inputs, runners, host and repository. Missing facts or
samples stay unknown. New reports record build-input presence for Rust debug artifacts, Rust release
artifacts, and Web dependencies. `warm` means all three are present, `cold` means
none are present, and `mixed` remains incomparable. Presence does not claim a
cache hit. Missing build state and candidate facts have explicit comparison
reasons. Historical reports remain unchanged; a baseline still needs three
comparable completed samples. Never edit original reports to add these observations.

`scripts/observation/settings.json` sets the heartbeat and comparison thresholds,
record batch cap, and database size guard. Compose caps CPU and memory. Grafana
refreshes every five seconds and evaluates provisioned rules every ten seconds.
There is no external notification destination. Grafana rules query the same
`violations` view as the dashboard and cannot authorize execution or retries.

Collection prints wall and CPU seconds, peak process RSS and database bytes.
`make observe-smoke` measures 1,000 synthetic retained roots plus one actual
bounded managed command, queries the real dashboard, waits for a firing rule,
restarts observation without restarting that command, and records container CPU,
memory and block I/O. This is synthetic overhead evidence, not a product speedup.
The original records survive observation downtime and rebuild. The database
size guard stops further collection above 256 MiB; it never removes raw records.

## Retained node attempts

Managed command steps bind their attempt ID and parent to a retained graph digest
and node ID. Build and reset observations use `nodes/`; required evidence stages
keep their existing `steps/` records. They retain the selection reason, execution scope, actual child
start, UTC end, monotonic duration, and result. Shared phases and resets use their
own nodes. Cargo build and grouped execution remain separate boundaries. Graph
membership alone never creates an attempt or a file duration. Uninstrumented
members remain unknown after execution; a missing finish is not success.

The collector exposes `run_graphs`, `node_attempts`, and `node_states`. These
read models preserve full membership, pending and unselected nodes, failed
prerequisites, actual attempts, and cache producer links. Complete reuse stays a
request against its original run. Cache hits create no executed node attempts.
Nested step intervals still use the existing exclusive cost calculation; do not
sum node durations as an Issue bill. Queue and resource wait times remain unknown
without an explicit observation. Legacy reports gain no graph or timing facts.
Run `make verify-targeted CHECK=verification-node-tests` for these public command
and SQLite replay regressions.

## Single-run DAG

Open <http://127.0.0.1:3749/d/storyos-run> or use the run link on the main dashboard.
Select the time range, Issue, profile, and run. The workflow shows retained nodes,
including unselected operations. Select a node, then use its inspect link to show
its details and file members. Select `__none` to collapse files. The URL preserves
run, group, and node selection. Topology-derived fixed coordinates prevent refresh
from restarting Grafana layout. Expanding files preserves existing step positions.
The local UI uses a light theme and Simplified Chinese labels. Step names lead;
state and duration are secondary. Raw node IDs and evidence stay unchanged.
A compact step list keeps names readable beside large graphs.
Read-only queries wait at most five seconds for a collector write lock.
Dependency edges show required order. Contains and member edges show grouping only.
Use the node menu, zoom controls, and detail tables to inspect large graphs.

Times use UTC. Unknown timing is not zero. Selected counts include file membership;
executed counts require actual node attempts. Reused observations link their producer
and add no attempts. Legacy runs show unavailable graph data. Evidence paths are
relative to `target/verification/`; they are local records, not served files.

`scripts/verification_observation_dashboard.py` owns the generated `run.json`.
Run `make observe-dashboard` after edits. Observation tests reject generated drift.
`make observe-smoke` also queries collapsed and expanded DAGs through Grafana for
labelled synthetic partial, complete, running, failed, reused, and legacy examples.
These examples prove display behavior, not product execution.
The collector stores SQLite sort files in its owned observation data directory. The
smoke check forces a sort spill and requires a container-collected change after
the initial import; a stale database cannot satisfy that check.

## Two-run comparison and timeline

Open <http://127.0.0.1:3749/d/storyos-compare> or follow the comparison link from
one run. Select left and right retained runs. Stable IDs preserve removed files;
renames appear as removal and addition. Definition changes include dependencies
and membership, but selection is separate. Inspect the definition and edge cells
for details. Column filters narrow the table. Missing graphs stay unavailable.

The UTC timeline gives each actual attempt its own row. Use the time picker to
zoom to the runs. Attempts without both timestamps remain in the detail table;
no end time or file duration is invented. Overlap includes nested attempts and
is not additive cost. Explicit blocked intervals use the recorded run UTC and
monotonic anchor. Missing waits remain unknown. Complete reuse requests point to
the original run and add no attempt; daily reuse retains its producer link.

Duration deltas are descriptive unless scope, definitions, policy, toolchain,
execution inputs, runners, host, repository, and build state match. Even then,
a delta alone does not prove a cause. Observation does not authorize recovery.
`make observe-dashboard` generates both dashboards; `make observe-smoke` queries
both through the real SQLite plugin. Synthetic examples prove display behavior,
not real-candidate recovery. Historical records are never changed.
