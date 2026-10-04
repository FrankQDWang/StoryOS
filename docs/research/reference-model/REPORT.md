# Reference Model Report

## Conclusion

All four requested stages meet the outcome-count criterion at product base `479224809cdaae997cda51cb8853e3fafa242b65`. The counted corpus has no unexplained compared-value difference. It retains five implementation defects and eleven contract questions. Product code is unchanged.

The generated public contracts contain 123 effect/reason branches for the primary typed commands: 99 reached at least 20 times; 24 have explicit public-boundary reachability limits. Two further foundation-only outcomes are recorded separately: Acceptance NoEffect and ProposalRevised StructuralReshapeConflict. Missing rules were not filled with product implementation behavior.

The deliverables are the executable [harness and replay guide](../../../prototypes/reference-model/README.md), [outcome ledger](coverage.json), [counted-evidence manifest](evidence-manifest.json), [findings](FINDINGS.md), and [progress log](PROGRESS.md). They form a replayable behavior baseline; the known defects remain visible when replayed.

## Findings by severity

| Severity | ID | Observed difference | Minimal replay |
| --- | --- | --- | --- |
| P2 | D-001 | Project create/update/archive omit Author Action. After a prose edit and Project rename, Undo skips the rename and removes the earlier prose. | Replay stage, `project-undo`, seed 403 |
| P2 | D-002 | Create Chapter exact retry returns 503 after an Editor Session opens. | Replay stage, `chapter-after-session`, seed 401 |
| P2 | D-003 | Create Editor Session exact retry returns a later base after an edit. | Replay stage, `session-after-edit`, seed 402 |
| P2 | D-004 | Proposal Undo returns an empty authoritative payload under the unchanged real Revision ID. Canonical prose remains intact. | Proposal stage, `proposal-undo`, seed 353 |
| P2 | D-005 | Withdrawal Undo exact retry changes its Activity position to zero. | Proposal stage, `withdrawal-undo`, seed 354 |
| P3 | A-001–A-006, A-008–A-012 | Visibility, initialization, refusal priority, Undo availability, validation timing, automatic Current Chapter choice, and foundation/wire outcome gaps need explicit contracts. A-007 was a model error and is retired. | Individual seeds, steps, sources, and static comparisons in [contract questions](CONTRACT-QUESTIONS.md) |

[FINDINGS.md](FINDINGS.md) gives the contract and code locations for every implementation defect. The minimal commands are executable through the same database wrapper. No bug fix, migration, generated artifact change, existing test change, Issue, or PR was made.

## Stage evidence

| Stage | Main execution | Typed branches at 20+ | Explained limits | Result |
| --- | --- | ---: | ---: | --- |
| 1. Structure and Current Chapter | Seeds 100–119; interrupted first run retained, seed 114 restarted in a fresh database | 43 | 5 | Live trees, ranks, revision increments, explicit selection, allocation, and no-change observations compared |
| 2. Author Edit and Undo | Direct edits 220–239; Proposal/Draft cases 320–339 and 360–379; refusal state checks 380–399 | 18 | 1 | All six edit effects and all five Undo effects reached; reason counts in ledger |
| 3. Proposal lifecycle | 20 full seeds 320–339, with 19 shuffled scenarios per seed | 37 | 16 | Real AgentRun/fake-destination production, edit, accept, reject, withdraw, replan, and both reopen paths compared |
| 4. Idempotency and writer | Seeds 420–444 and real-clock expiry seed 450 | 1 | 2 | Takeover applied 25 times; post-Admission compare failure unreachable in this serialized implementation |

The minimum reached count is 20 for stages 1–3 and 25 for the typed takeover result. [REACHABILITY.md](REACHABILITY.md) explains each limit and gives its owning source. The two foundation-only gaps are present in `coverage.json`; they are not hidden by the generated-schema denominator.

Setup outcomes also exceed 20: the corpus contains 1,516 successful Project creates, 617 current-writer Session creates, 25 read-only Session creates, and 444 admitted AgentRuns. The primary Proposal batches alone produced 380 independent real AgentRuns. Auxiliary inline Proposals used public query Anchor proofs.

Stage 4 additionally records:

- 25 successful takeovers, 25 stale-writer refusals, and 25 stale-takeover refusals.
- 25 changed-body, wrong-nonce, and new-key/old-nonce refusals each; 25 changed-digest Challenge conflicts.
- 25 observations of shared capacity 10 and Author Edit capacity 120; 25 exact Challenge retries after each exhausted class. All counted windows were stable.
- 20 distinct pending challenges rejected after their real expiry, all with HTTP 422 `challenge_invalid` and no Receipt.
- 20 distinct committed Author Edit acknowledgements replayed byte-for-byte after expiry, with the final prose unchanged.
- At least 20 immediate exact retries for every exercised command kind. Delayed retries preserve the known D-002, D-003, and D-005 failures.

## Difference accounting

The counted corpus has 51 failed comparison assertions: D-001 once, D-002 26 times, D-003 once, D-004 twice, and D-005 21 times. The separate allocation audit finds missing Project actions in 1,516 Create, 46 Update, and 186 Archive Receipts, deduplicated by Receipt ID.

The [writer replay audit](writer-replay-audit.json) verifies that all 25 writer-batch Chapter failures are 503 `project_store_unavailable`. The [Draft binding replay audit](draft-binding-replay-audit.json) verifies that all 20 batch Undo replay differences have two 200 responses and change only `effect.project_activity_position` to zero. Classification therefore uses response evidence, not a failed label alone.

Earlier failed probes are retained. Their corrected model errors include an invented initial revision, incomplete update shapes, whitespace-title assumptions, an absent ID used as a stale Head, the AgentRun status, an incorrect Draft query route, an omitted optional frontier, incomplete conflict objects, reservation release assumptions, and an unsupported candidate-edit shape. They do not supply successful coverage. The initial long structure run's HTTP disconnect and database startup readiness failures remain failed operational evidence; a later pass does not rewrite them.

## Independence and replay

The model uses independent list operations, text replacement with computed UTF-16 offsets, and Proposal state axes. A seed selects scenario order, identities, text, positions, and a structure walk. Targeted templates select rare results. No product classifier, adapter, or fake-output constant is imported as an expected result.

Opaque IDs, public prior-state control values, and fake candidate bytes are inputs. The model derives transitions from the contracts. For example, acceptance must apply the exact candidate supplied to it; an empty-authority result must preserve the previous canonical body. Product source was inspected for launch mechanics, reachability proofs, and defect locations.

D-001 is checked by a separate Project allocation audit and the minimal Undo counterexample. The other action counters track the structure/edit history after Project creation; their passes do not approve missing Project actions. Contract-ambiguous cases admit only the documented alternatives and preserve invariant checks. The benchmark does not assert unspecified refusal priority or initialization values.

Replay reconstructs the same semantic sequence with new database identities and fresh challenges. Evidence omits cookies and nonce values. Raw acknowledgement comparison happens in memory. A managed process PASS means execution completed; the difference list and this classification determine conformance.

## Environment and artifact scope

- Branch/worktree: `codex/reference-model` at `/Users/frankqdwang/.codex/worktrees/reference-model/StoryOS`.
- Product base: `479224809cdaae997cda51cb8853e3fafa242b65`.
- Package source: `0dfe51840212a5001dc5439f1996c690405cd56e`, with the same product files as the base.
- Web manifest SHA-256: `fef133446adce7fe76872021f705b9ee5044fe91d3d2f37f1badb8fa82a512ac`.
- Python 3.14.8; PostgreSQL `postgres:16-alpine`, image `sha256:c05eced0bdb41ea9b95a656472a6aa4d50cad0d8a2e33d14eb1c53fd6204f2ae`.
- Database lifecycle exclusively through `scripts/dev-postgres.sh run`; Server port dynamically allocated.
- Package built with `make release-package`. No `make verify-local` or full test suite was run. The harness has no self-tests and no default build/verification hook.
- All tracked changes are under `prototypes/reference-model/` and `docs/research/reference-model/`.

The primary command denominator follows the four requested stages and includes every generated effect and reason for those commands. Auxiliary Draft/assistance setup paths are retained; this is not a Cartesian-product proof of every optional DraftRetry metadata combination, browser interaction, provider, or concurrency schedule.
