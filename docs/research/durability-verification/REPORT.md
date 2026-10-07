# StoryOS durability verification

## Conclusions

D1 holds in the executed local matrix. D2, D3, and D4 fail. The three findings
below have deterministic public-interface reproducers. No acknowledged prose
loss, duplicate authoritative effect, or PostgreSQL deadlock was observed.
These results describe the tested command families and schedules, not every
possible StoryOS execution.

The second complete 16-case round found no new failure class. Six minimized
replays then reproduced the same findings. The 38 retained executions include
one first-round follow-up probe error; its completed replacement is retained.
No product fix is included.

## Findings, by severity

### F1 — P1: old writer recovery can commit after takeover

An Author Edit has a committed Admission but no Core effect. The Server stops,
or its database connection ends, while Core waits for the Head lock. A second
Editor Session takes over and receives writer generation 2. The old session's
public outcome GET then returns `committed` and writes `Authoritative AOLD`.
Authoritative Commit count changes from 0 to 1 after the takeover acknowledgement.

The same result occurs without a process cut: takeover commits while the old
Core transaction waits. The old request returns 503 after release, then outcome
reconciliation commits it. A fresh stale-writer command correctly returns 412.
Thus the defect is the recovery of an already admitted command.

The winner's next edit returns `conflicted / stale_authoritative_head`: the
old recovery updates the old session's base, not the winner's base. A fresh
Editor Session and another explicit takeover restore writing. The Project is
not permanently unwritable in these cases.

Root cause at baseline:

- `crates/storyos-adapter-postgres/src/author_edit.rs:103` joins the Admission
  to its historical writer-generation row. The Core query does not require
  that generation to be the latest. Compare the first-use check at line 306.
- `crates/storyos-adapter-postgres/src/author_edit_outcome.rs:290` checks the
  stored Admission against the stored Challenge, without a current-writer check.
- `crates/storyos-adapter-postgres/src/author_edit_admission_recovery.rs:48`
  resumes Core with that Admission.
- `crates/storyos-adapter-postgres/src/author_edit_settlement.rs:355` rebinds
  only the old Editor Session Snapshot through its historical generation.

Minimal schedules: `takeover-server` and `takeover-concurrent`. Hold the Chapter
Head; send the old edit; observe its Head-lock wait; for the Server variant,
kill and restart Server; acknowledge takeover; release the Head; GET the old
outcome. The database-cut variant replaces SIGKILL with `pg_terminate_backend`.
Evidence: [Server cut](evidence/minimal/takeover-server.json),
[concurrent](evidence/minimal/takeover-concurrent.json),
[database cut and recovery control](evidence/round-2/takeover-database.json).

### F2 — P2: legal concurrency and exact retry expose serialization failures

Two authenticated sessions submit legal Update Project requests at revision 1.
One PostgreSQL gate holds the Project row. The driver observes the first runtime
connection waiting on that row, then a second runtime connection waiting in the
same lock queue. Release lets the first request commit. The second returns HTTP
503 `project_store_unavailable`, with PostgreSQL reporting
`could not serialize access due to concurrent update`.

Replacing the second request with an exact retry of the first produces the
same 503. Only one Admission and one Receipt exist. A later sequential retry
returns the full original acknowledgement. The defect is acknowledgement and
availability failure, not a duplicate effect. It also repeats after Server restart.
An Author Edit concurrent with a rename returns `author_edit_store_unavailable`
through the same missing transaction-retry class; its earlier acknowledged edit
survives and reconciliation can finish the interrupted edit.

Root cause at baseline:

- `crates/storyos-adapter-postgres/src/lib.rs:486` starts SERIALIZABLE before
  command Challenge consumption. Row-lock waiting does not refresh its snapshot.
- `crates/storyos-adapter-postgres/src/update_project.rs:23` executes once.
  Its Project lock at line 74 and error return at line 57 do not restart the
  transaction after a serialization failure.
- `crates/storyos-adapter-postgres/src/lib.rs:405` locks Challenge and idempotency
  rows. In the duplicate schedule, the changed row fails before the settled
  exact-retry branch can read the original result.
- `crates/storyos-server/src/update_project.rs:255` maps the error to 503.
  The application caller delegates once at
  `crates/storyos-application/src/update_project.rs:124`.

Minimal schedules: `concurrent-rename`, `concurrent-retry`, and
`concurrent-rename-restart`. No author edit or Editor Session setup is needed
for the minimum rename failure. Evidence: [two sessions](evidence/minimal/concurrent-rename.json),
[exact retry](evidence/minimal/concurrent-retry.json),
[after restart](evidence/minimal/concurrent-rename-restart.json).

### F3 — P2: CreateEditorSession exact retry changes its acknowledgement

Create one writer Editor Session and retain its command. Open another session
and take over. Replaying the exact first create command returns `read_only`,
`observed_writer_generation: 2`, and `superseded_by_takeover`; its original reply
was `current_writer`, generation 1. Restarting Server does not restore the
original acknowledgement. Counts do not change on replay.

Root cause at baseline:
`crates/storyos-adapter-postgres/src/editor_session.rs:108` keeps only the existing
session ID for exact retry. Line 117 calls the normal live `read_session` query.
Lines 201–209 select the latest writer generation and current base Snapshot.
This is new-command reply drift, not ADR 0032's pre-capture historical exception.

Minimal schedule: `session-replay`. It requires only create, takeover, and exact
replay; the minimized version has no intervening Author Edit.
Evidence: [original and replay, before and after restart](evidence/minimal/session-replay.json).

## Invariant × method matrix

“Holds” means the named oracle passed in these local executions. “Fails” means
one counterexample disproves the invariant. Every cell has executed evidence.

| Invariant | Deterministic concurrent schedule | Process or connection interruption |
| --- | --- | --- |
| D1: acknowledged Author Edit persists | **Holds.** `concurrent-author` keeps the acknowledged seed prose and its exact Receipt reply while another session renames the Project. | **Holds.** `durable`, `lost-ack`, and six `cut-*` cases preserve prior acknowledged prose; recovery adds exactly one new Commit. |
| D2: exact retry returns the original acknowledgement once | **Fails, F2/F3.** Overlapping exact retry gets 503; session creation replay changes its writer result. No duplicate effect observed. | **Fails, F3.** After restart, the original session command still returns the changed writer result. Direct Author Edit recovery and sequential exact reply checks pass. |
| D3: concurrent legal commands preserve availability | **Fails, F2.** Two connections deterministically produce one 503. Further writing succeeds; no deadlock was observed. | **Fails, F2.** `concurrent-rename-restart` reproduces the same two-session 503 after restart. Injected disconnects themselves are not classified as this defect. |
| D4: takeover fences the old writer | **Fails, F1.** The blocked old request fails, but its subsequent outcome GET commits after takeover. | **Fails, F1.** Both Server and backend cuts retain an Admission that can commit after takeover. Fresh stale commands return 412. |

## Evidence and execution

Date: 2026-10-05, Asia/Singapore.
Product baseline: `479224809cdaae997cda51cb8853e3fafa242b65`.
Package source: `d82ccf6ab8abeceb8ce35695d609b2cc6c5be321` (only the initial
research progress file differs from the product baseline).
Branch: `codex/durability-verification`.

The Server package was built once with `make release-package` in 177.06 seconds.
Its SHA-256 is `46d20bd488a100435a37980b61601177181bafe643cc2b569292737af8e47842`.
The Storage binary SHA-256 is
`337f64a15a441d5fc78b4720a83a4e207e0f00561c2cba97af613a9ddf83e561`.
Host: Darwin arm64; Node 24.16.0. PostgreSQL: 16.15, aarch64 Alpine, with
`fsync`, `synchronous_commit`, and `full_page_writes` enabled. Database default
isolation is READ COMMITTED; the affected command transactions use SERIALIZABLE.

Each case starts a fresh database through `scripts/dev-postgres.sh run` and
starts the packaged Server at `127.0.0.1:0`. Restarts reuse that case's allocated
port and local session configuration. Two test bootstrap handles identify the
same fixture User; two Editor Sessions exercise writer takeover. The multiple
bootstrap mapping flag permits these local handles and disables bootstrap
issuance; it injects no command outcome or fault. The Worker is disabled.
No production database, remote service, or external model is used.

Only HTTP commands, public generated HTTP client encoding, PostgreSQL reads and
locks, backend termination, and process signals drive the cases. Setup does not
change domain rows with SQL. No product fault hook, trigger, migration change,
internal Rust call, sleep coordination, or harness self-test is used.

| Case family | Barrier and observed result |
| --- | --- |
| `durable` | Receive Author Edit acknowledgement; SIGKILL/restart; terminate runtime backends/restart; chapter state, complete command reply, and counts remain equal. |
| `lost-ack` | Consume and discard an HTTP reply, then kill Server. Outcome GET and exact retry recover the same response with one Commit. |
| `cut-admission-{server,database}` | Hold that command's idempotency row; observe consumption waiting; cut. No new Admission or effect is visible. Retry applies once. |
| `cut-core-{server,database}` | Hold Chapter Head; observe Core waiting after Admission commits; cut. Prior prose stays intact. Outcome GET settles once. |
| `cut-commit-{server,database}` | First stop at Head; acquire idempotency gate; release Head; observe final idempotency UPDATE waiting after Core effect writes; cut. All uncommitted effect rows roll back; recovery settles once. |
| `concurrent-rename`, `concurrent-retry`, `concurrent-rename-restart` | Observe both runtime lock waits before release. One request succeeds, one gets 503. |
| `concurrent-author` | Acknowledge seed edit; block another edit at Head; rename from the other session; release. Seed reply and prose survive; later writing succeeds. |
| `session-replay` | Two-session takeover precedes exact replay; full reply changes, counts do not. Repeat after restart. |
| `takeover-{server,database,concurrent}` | Takeover acknowledgement precedes old outcome reconciliation. Old recovery writes one Commit after generation 2 is current. |

The evidence records backend PIDs, wait reasons, blocker PIDs, visible query
text, full command replies, state/count oracles, and PostgreSQL errors. Long
activity queries are truncated by PostgreSQL; the final pre-COMMIT UPDATE is
short enough to identify directly. Monitor deadlines fail the probe; they never
release a scheduling barrier. Separate PostgreSQL connections own the gates
and observe the two runtime connections.

The observed Update Project lock order is Challenge/idempotency then Project.
The Project lock does not repair its old SERIALIZABLE snapshot. F2 is a
serialization failure, not proof of a lock-order deadlock. Author Edit has a
separate Admission commit and Core commit; F1 crosses that durable boundary.
The `lost-ack` case discards a delivered response. It does not claim a precise
cut inside Server serialization between COMMIT and the first response byte.
Host power loss and PostgreSQL process crashes were not part of this connection
and Server-interruption experiment.

Rounds: [round 1](evidence/round-1), [round 2](evidence/round-2),
[minimized replays](evidence/minimal). Round 1 has 9 completed positive/control
cases, 6 completed failure cases, and 1 follow-up fetch error after F1 already
occurred. That first fetch error has no proved product cause. Round 2 uses fresh
HTTP connections and has 9 completed positive/control cases, 7 completed failure
cases, no probe error, and no new finding. All six minimum replays exit 1 as
expected. `concurrent-author` exits 0 for its D1 oracle while also recording F2.
The first-round failed record is preserved.

## Replay and contracts

See [replay instructions](../../../prototypes/durability-verification/README.md)
and [step-by-step progress](PROGRESS.md). Each minimized case has a recorded
command. Exit 1 means a reproduced invariant failure; exit 2 means a probe error.

The controlling contracts are `GLOSSARY.md` (Command Acknowledgement, Command
Idempotency Fence, Author Command Admission, and Editor Session), ADRs 0004,
0012, 0013, 0031, 0032, and 0041, and:

- `docs/foundation/postgresql-project-storage-isolation-and-migration-contract.md:628`
  requires commit before acknowledgement; line 656 requires whole-transaction
  retry; line 694 requires the immutable original exact-retry outcome.
- `docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md:134`
  defines takeover and fencing; its Admission recovery matrix requires changed
  bindings to receive reconfirmation rather than new authority.

All changes are new research or prototype files. Product code, migrations,
generated contracts, existing tests, and default verification remain unchanged.
No full test suite or `make verify-local` ran. No PR or GitHub Issue was created.
