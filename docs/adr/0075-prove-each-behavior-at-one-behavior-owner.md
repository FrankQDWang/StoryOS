---
status: accepted
---

# Prove Each Behavior at One Behavior Owner

The repository cleanup specification of phase 3 ([#1307](https://github.com/FrankQDWang/StoryOS/issues/1307)) owns this ADR. Ticket [#1308](https://github.com/FrankQDWang/StoryOS/issues/1308) adds this ADR, the shared HTTP test harness, and the Profile Probes. Ticket [#1309](https://github.com/FrankQDWang/StoryOS/issues/1309) adds the Smoke Check table and removes the smoke-only HTTP files. Ticket [#1310](https://github.com/FrankQDWang/StoryOS/issues/1310) moves the kept HTTP files to the harness. Ticket [#1311](https://github.com/FrankQDWang/StoryOS/issues/1311) removes the invalid and boundary golden-wire fixtures. This ADR partly supersedes [ADR 0042](0042-derive-release-1-contract-artifacts-from-one-operation-registry.md) for the fixture triple.

## Context

The HTTP tests under `apps/web/test/node-postgresql/` had approximately 31,000 lines in 65 files. Most of these lines proved behavior that another layer already proved:

- The Rust PostgreSQL adapter tests prove the SQL semantics of each project command. The table-driven `command_sequence_tests/` suite proves replay, retry, rollback, and damaged evidence for each command. Many HTTP files proved these semantics again, through the same SQL row edits.
- Many HTTP files were near copies of each other, and approximately 40 files copied the same helpers: Server start, Project creation, Command Challenge steps, problem reading, and Worker settlement.
- A change to a command or to a helper needed a change in many files.

The generated golden-wire corpus had three fixtures for each of the 48 operations: positive, invalid, and boundary. The tests read only some positive fixtures and two other fixtures.

## Decision

- **Behavior Owner.** Each behavior has one Behavior Owner, and another layer does not repeat its proof:

  | Layer | Owns |
  | --- | --- |
  | `storyos-core` and `storyos-application` unit tests | The pure rules of a command: its outcome for each input. |
  | `storyos-adapter-postgres` tests, with `command_sequence_tests/` | The SQL semantics: the stored records, replay, retry, rollback, and damaged or historical stored evidence. |
  | `storyos-server` unit tests | The HTTP answer of an error that an HTTP test cannot make without a change to stored rows. |
  | HTTP tests under `apps/web/test/node-postgresql/` | The wire contract: the Smoke Check table, the Profile Probes, and the cross-cutting files of host, session, Origin, Challenge rate, streams, Snapshot, takeover, diagnostics, Worker flows, exports, and process cuts. |
  | Browser-source tests | The Web Client. |
  | Exact-dist journeys | An end-to-end path through the packaged release. |

- **Smoke Check table.** One HTTP test file holds one Smoke Check row for each operation of the generated Route Catalog, keyed by operation ID. A row has an arrange step from the harness, an input, and the expected result: a success, or a declared refusal code when a Profile Probe or a kept file proves the success path. A project command row also proves that a request with no session and a request with no Command Challenge proof are refused. A row reads its response through the Web Client function of its operation. A stream row proves only that the stream opens and closes. A coverage test compares the row keys with the Route Catalog, so a new operation fails until it gets a row. The table runs in the `http-main` phase.

- **Profile Probes.** One HTTP file keeps the complete HTTP semantics of each settlement profile of [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md), of Author Undo compensation, and of export admission:

  | Path | Operation | File |
  | --- | --- | --- |
  | `Structural` | `updateChapter` | `update-chapter-http.integration.test.ts` |
  | `ChapterSelection` | `setCurrentChapter` | `set-current-chapter-http.integration.test.ts` |
  | `ActivityOnly` | `updateProject` | `update-project-http.integration.test.ts` |
  | `ActionOnly` | `rejectProposalOperations` | `reject-proposal-operations-http.integration.test.ts` |
  | `AuthoritativeRevision` | `acceptProposal`, and the Author Edit test of `project-http` | `accept-proposal-http.integration.test.ts` |
  | Author Undo compensation | `undoLatestAuthorAction` | `undo-latest-author-action-http.integration.test.ts` |
  | Export admission | `exportHumanReadableManuscript` | `readable-export-admission-http.integration.test.ts` |

- **Probe content.** A probe proves these behaviors through HTTP:
  - Replay: an exact retry returns the first response and body.
  - Refusal: a changed request with the same key and nonce gives `challenge_invalid`, another session cannot send the command (the Challenge route answers 404 with no owner identity, and the command route refuses the Challenge of the owner with `challenge_invalid`), and the command refuses an input that its rules refuse.
  - Conflict: a stale expected head gives the conflicted outcome. `acceptProposal` has no conflicted outcome since [ADR 0064](0064-accept-a-proposal-on-an-earlier-base-by-replacing-its-target-text.md), so the Author Edit test of `project-http` proves the conflict of the `AuthoritativeRevision` profile. An export command is admit-only and has no conflicted outcome.
  - Historical acknowledgement: an exact retry returns the first acknowledgement after later changes to the Project and after a Server restart.
- **No row edits in a probe.** A probe does not change the stored rows of a command, its acknowledgement, or its evidence with SQL. The adapter tests own damaged and historical stored evidence. The harness resets the Command Challenge rate window of a test Project before each Challenge, as specification #1307 requires. This reset is test setup: it lets one test send more commands than the rate class permits in one minute, and no assertion reads it. The Challenge rate files own the rate class behavior. A `storyos-server` unit test owns the HTTP answer of [ADR 0032](0032-accept-historical-acknowledgement-unavailable.md): `409 historical_acknowledgement_unavailable` for historical absence and `503 project_store_unavailable` for damaged evidence.
- **Exception: the altered candidate.** The `acceptProposal` test of `acceptance-refusal-http.integration.test.ts` changes a stored candidate with SQL and proves the `altered_candidate` outcome and its replay through HTTP. The recovery seed (`scripts/seed-recovery-acceptance.sh`) runs this test to retain one altered-candidate Acceptance condition, and no adapter test changes a stored candidate. This test stays until an adapter test owns the outcome and the seed makes the condition in another way.
- **Acknowledgement hold.** The `updateProject` probe holds the one HTTP test of the acknowledgement hold: a held response returns the acknowledgement that the command captured at settlement.
- **Shared HTTP test harness.** `apps/web/test/support/http-harness.ts` holds the helpers of the HTTP tests: Server start, Project, Volume, and Chapter creation, Command Challenge and send in one call, problem code reading, and Worker settlement. It builds on `node-integration.ts` and `acceptance.ts`. A test file starts one Server for the file when its tests allow it. A test file does not copy these helpers.
- **Golden-wire.** The operation artifacts declare one positive golden-wire fixture for each operation. The fixture triple builder and the invalid and boundary builders go. A test that read an invalid or boundary fixture uses inline values, or goes when a Behavior Owner already proves its behavior. The fixture corpus digest covers the 48 positive fixtures. This replaces the fixture triple of ADR 0042. The protocol profile changes, so a database activated with the earlier release identity refuses the package with `identity_mismatch`.

## Considered options

- Keep each HTTP file as a full route test. Rejected: the files proved the adapter semantics again, and a change to a command or a helper needed a change in many files.
- Prove damaged evidence at each layer. Rejected: an HTTP test can damage evidence only through SQL row edits, and `command_sequence_tests/` already proves it for each command.
- Remove all HTTP semantic tests and keep only the Smoke Check table. Rejected: each settlement profile has its own HTTP replay and acknowledgement path, so each profile keeps one complete HTTP proof.
- Keep the invalid and boundary fixtures for a later client. Rejected: no test read them, and the generator can add them again when a reader needs them.

## Consequences

- A new test goes to the Behavior Owner of its behavior. A reviewer can refuse a test that proves a behavior again at another layer.
- A new operation gets a Smoke Check row. A new settlement profile gets a Profile Probe.
- A change to a shared HTTP helper is in one file.
- The HTTP test layer becomes approximately half of its earlier size.
