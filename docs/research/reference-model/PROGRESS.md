# Reference Model Progress

## Goal and fixed source

Compare an independent contract model with Release 1 Server, Core, and PostgreSQL through public HTTP. Complete four stages: structure and Current Chapter; Author Edit and Undo; Proposal lifecycle from a fake-destination AgentRun; idempotency, Command Challenge, and writer takeover. Each contract outcome needs 20 observed hits or a documented reachability limit. Classify each difference and retain minimal replay evidence.

- Base: `479224809cdaae997cda51cb8853e3fafa242b65` (`origin/main`, fetched 2026-10-05).
- Branch: `codex/reference-model`.
- Worktree: `/Users/frankqdwang/.codex/worktrees/reference-model/StoryOS`.
- Only `prototypes/reference-model/` and `docs/research/reference-model/` may change.
- No product changes, existing test changes, generated changes, PRs, Issues, main changes, or complete verification runs.
- Database lifecycle: `scripts/dev-postgres.sh run`; dynamic service ports.
- Model expectations come only from repository contracts. Product source may supply launch and transport mechanics and defect locations, never expected state transitions.

## Step 1: Isolated baseline

Done: read repository rules and domain entry; fetched origin/main; created the isolated worktree and required branch. The primary checkout was clean and remained on main. Initial `make verify-status BASE=origin/main` reported stale historical evidence. No complete run was started.

Decision: use a small Python standard-library harness with independent state, seeded commands, HTTP transport, and retained JSON traces. Keep it outside default build and verification discovery. Do not add harness self-tests. Contract gaps remain explicit instead of inferred from implementation.

Next: inventory the public command contracts, prepare the existing release package, and execute one Empty Project to Volume to Chapter chain.

## Stage status

| Stage | Status | Evidence |
| --- | --- | --- |
| 1. Structure and Current Chapter | Started | Baseline only |
| 2. Author Edit and Undo | Pending | None |
| 3. Proposal lifecycle | Pending | None |
| 4. Idempotency and writer | Pending | None |

## Step 2: First HTTP chain

Done: built the existing release package with `make release-package` (278 seconds; no tests). Package source is `0dfe5184`; product sources equal the fixed base. The model uses Python standard-library HTTP, its own seeded UUIDv7 inputs, and a list-based tree model. The first successful seed is 1: Empty Project, Create Volume, Create Chapter. It compared empty state, command effects, one Commit and one Author Action per structure change, and the public tree query. No differences remained in this chain.

Evidence: `bootstrap.json`; managed database run `5bb9e87e01d844398f8a22b0da83657f` passed. The first database setup hit the existing readiness limit and cleaned up; the second setup succeeded. No foreign container was changed.

Model correction M-001: the first model assumed initial Tree Revision zero without a source. The HTTP input was refused with 400. `bootstrap-initial-model-error.json` preserves it. The contract defines increments but no initial number; the model now accepts an opaque initial query baseline and computes later increments independently. The zero-valued generated schema versus positive-only Server parser is a separate contract discrepancy to investigate.

Decision: execution writes evidence to ignored `target/reference-model/`, then copies it to the report directory after the managed command exits. Writing the first trace straight into tracked-source scope caused a `source-changed` observation. That run is retained, not relabeled PASS. Nonce and cookie values are omitted from traces.

Next: expand structure scenarios and retain a complete outcome/reachability matrix, then implement Author Edit and Undo.

## Step 3: Structure outcome survey

Done: seed 10 exercised all 48 generated structure/Project/Current Chapter effect variants as separate HTTP scenarios. A seeded random walk now adds mixed create, rename, reorder, and delete commands. The model compares full live trees, relative tree revisions, command outcome/reason, Receipt allocation, Author Action sequence, and Activity stability for non-applied results.

The first survey is retained as `structure-first-survey.json`. The initial interrupted survey is `structure-initial-model-errors.json`. No product source changed.

Model corrections: M-002 used an empty update body although the generated schema requires both title and order. The generator now submits the unchanged current title and rank. M-003 treated a whitespace title as invalid, but the contract only requires 1 to 1024 UTF-8 bytes. Empty titles now probe schema refusal and are not counted as Core refusal outcomes.

Classified contract limits (full report pending):
- A-001: no contract rule states whether an archived Project permits Canonical Manuscript Tree reads. Observed 404 after archive; `getProject` remains readable. Do not infer a 200 requirement.
- A-002: initial Tree Revision and Project Revision numbers are not specified. Tree Revision is read as an opaque initial baseline. The Project query does not expose Project Revision; the generator probes positive 1 and validates later known increments.
- A-003: `empty_project` and `invalid_chapter_join` priority is unspecified. A public empty Project has no live target; the exercised sequence reaches invalid join. Keep the unreachable empty branch in the coverage ledger.
- A-004: generated structure revision/order strings permit zero, while `structure_admission.rs:302` requires positive values. The initial zero probe returned 400. This is schema/transport disagreement, not a zero-to-one model expectation.
- R-001: `invalid_title` Core effects cannot pass public title validation: the Server and Core use the same byte limits. Exercise the public refusal and document the unreachable Core variant.

Next: collect 20-hit structure evidence and complete replay cases; prepare the Author Edit model.

## Step 4: Retained structure batch and Author Edit chain

The 20-seed structure run was interrupted by `RemoteDisconnected` during Create Chapter in seed 114. Seeds 100 through 113 completed; seed 114 is partial. All 723 created Projects and observed commands were retained, with zero model differences. The minimum reached outcome count is 14. This is partial evidence, not a completed batch. Evidence: `structure-partial-100-114.json.gz` and its log; managed run `b2b43f2f4b284bae8694daaded6d9a81` failed after 561 seconds. The owned database and Server were stopped. The preceding launch also hit the database readiness limit; no product or resource script was changed.

New finding to verify: successful Create Project, Update Project, and Archive Project return no Author Action. Manuscript State Machine section 3.2 requires one for each successful author-owned Core Transition. The current structure sequence comparison starts after Project creation; it does not excuse this missing allocation. Add a separate allocation audit and minimal replay instead of changing the contract model to match it.

Next: run the Author Edit/Undo smoke chain, then use a fresh database for six structure seeds (114 through 119) and aggregate actual counts. Keep all earlier failed evidence. The Author Edit model calculates text from input units independently and uses returned IDs only as opaque future preconditions.

## Step 5: Author Edit and Undo smoke

Seed 200 passed the revised direct-edit comparison: no effect, invalid selection, same-Scope stale Head conflict, four independent UTF-16 replacements (including Chinese and supplementary characters), frontier mismatch, wrong target Head, and one successful compensation. `edits-smoke.json` retains the public evidence; managed run `98a33de474bf45698a227dcc8ca98d51` passed.

M-004: a random absent Revision is not a stale same-Scope Revision. It failed Admission with 409. The generator now retains a real prior Revision before a successful edit and submits that old Head; this reaches the contracted `stale_authoritative_head` Receipt. A foreign `target_refs` value is a pre-Admission 422, not the Core target-mismatch outcome. `edits-first-survey.json` retains the original probes.

A-005: after the newest manual edit is compensated, a second undo reaches the correct prior frontier but returns `wrong_target_head`: the first compensation created a new Head. Section 10.1 specifies routing but does not define direct-edit eligibility after a later compensation. The model records both possible typed outcomes for this unresolved contract case and continues to require exact text for any compensation. Core location: `crates/storyos-core/src/undo_latest_author_action.rs:134`. This is not proof of a complete multi-step undo experience.

The transport can now compare exact Challenge retry results and exact command acknowledgement bytes, without writing nonce or cookie values into evidence. It remains separate from model expectations.

## Step 6: Structure count threshold

Seeds 114 through 119 completed on a new database. `structure-114-119.json.gz` retains the evidence; managed run `7e9efe8f889443418e7c5143da22cc14` passed in 145 seconds. Aggregate only actual counts from this run and the retained partial run. `stage-1-coverage.json` lists all 48 effect/reason variants: 43 reached at least 20 times; four invalid-title Core branches are stopped by public title validation, and the empty-Project branch is stopped by the invalid live-Chapter join. Every reachable variant passed the structure model's current checks. This does not resolve the recorded contract gaps or the separate Project Author Action allocation finding.

The second run also compared 1,098 exact Challenge retries and successful command replay bytes across the covered command kinds, with no mismatch. Final per-command replay counts still belong to Stage 4.

Next: exercise the independent Proposal axes model through a real AgentRun and the packaged fake destination; finish Author Edit outcome coverage and minimal findings.

## Step 7: Proposal production and lifecycle smoke

The public path now creates its own assistance binding with Update Project Assistance, creates an AgentRun with 202 Accepted, waits for its fake-destination settlement, and reads the resulting Proposal. No fixture or SQL writes were added. The initial 404 assistance query means that no binding exists; the schema-valid zero-revision initialization probe succeeds. Its absent-binding revision convention needs clearer prose contract documentation. The earlier claim that a controlled fixture was necessary was disproved before any fixture work.

Seed 300 exercised Proposal editing, Acceptance, rejection/reopen, and withdrawal/reopen through HTTP. All exact Challenge and command retries matched. Evidence: `proposals-first-survey.json`; run `b725626d10ac4fdf946e06eb0f5ae9ad` completed. Startup probes are retained separately.

A-006: a candidate edit is already valid at its next public query, with a new Validation Receipt bound to its new Revision. Section 7.2 says the edit resets validation to pending; section 7.4 permits separate Core validation. HTTP does not expose the interval or specify when that validation may run. The model now checks the fresh validation identity and valid plain-text candidate rather than assuming a query must catch pending. The adapter writes that validation in `author_edit_proposal.rs:431-502`; record this timing ambiguity, not an unsupported claim that the old Receipt was reused.

Next: verify conflict/replan and negative lifecycle results, then reach 20-hit counts. The first 202 assertion failure was a harness mistake and is retained in `proposal-agent-run-accepted.json`.

## Step 8: Three minimized behavior differences

`replay-minimals.json` confirms three implementation defects, recorded with contract and code locations in `FINDINGS.md`: Project changes are absent from Author Undo order; Create Chapter exact replay returns 503 after opening an Editor Session at its Activity position; Create Editor Session replay returns a later base after an edit. The last two were invisible to immediate retry checks. No product fix was made.

The normal Stage 4 smoke also proved current-writer fencing, usable takeover, refusal of stale takeover, unchanged old Author Edit acknowledgement after takeover, changed-digest Challenge conflict, the 10-Challenge shared capacity, and the separate 120-Challenge Author Edit capacity. It is `replay-first-survey.json`; its known Create Chapter replay failure is retained. The minimized run `5bde3e30923240d6b34a91e408322ba2` completed in 24 seconds.

Proposal conflict/replan and ordinary negative lifecycle smoke passed in `proposals-lifecycle-smoke.json`. Additional conflict and refused branches, mixed-ownership Draft creation, and the 20-hit batches remain to run.

## Step 9: Complete mixed-intent preservation

Seed 302 now creates two Blocks through one public Author Edit, generates a Proposal for the first Block, and submits one structured replacement spanning that candidate and the second authoritative Block. It receives RefusedToDraft, preserves every attempted unit and its independently recomputed payload digest, and changes neither authoritative Blocks nor the candidate. Draft close followed by Author Undo restores the open Draft. `draft-smoke.json` retains the evidence; run `b49ea67447c142c88fc7edf56bf4cc1b` completed in 22 seconds with zero differences.

The first Draft query used the wrong URL and stopped in the harness. The route now comes directly from the generated route catalog. The multi-Block model checks semantic Blocks and identity; it treats the separate body string as an opaque display projection for this setup.

Next: finish negative Proposal branches and run the 20-hit Author Edit/Undo and Proposal batches. The no-frontier Undo probe uses the unmodified command-owned fixture's empty action history; all novel changes in the main scenarios still enter through HTTP.

## Step 10: Expanded Proposal survey

Seed 303 exercised 15 independent Proposal scenarios, including producer withdrawal, changed-Head conflicts, stale Revision refusals, mixed Draft preservation, and close/reopen. Evidence: `proposals-expanded-survey.json.gz`. Three candidate probes exposed model assumptions: a Proposal target with no expected Proposal Head is malformed at Admission; an unknown operation is a typed target mismatch; Replan Undo uses a registered Proposal handler. Keep these observations and use a well-formed no-target stale-ownership request next. Do not count the initial expected Barrier assertion as a product defect.

A-007: section 10.1 names typed Proposal edit, rejection, and withdrawal handlers but does not explicitly map Replan. The observed Replan compensation has no authoritative Commit and returns an empty projected authoritative body. The routing and response projection need separate contract/schema review.

Next: verify Acceptance reversal, then collect bounded 20-seed batches.

Step 10 follow-up: the empty fixture frontier is an omitted optional property, not explicit null. Corrected the harness lookup. Seed 220 reached unsupported-intent refusal with zero differences before that lookup stopped the run; evidence is retained as `edits-empty-frontier-probe.json.gz`.

## Step 11: Direct Author Edit count threshold

Seeds 220-239 completed with zero differences. The run reached 20 content-unchanged results, 20 invalid-selection refusals, 20 unsupported-intent refusals, 20 stale-Head conflicts, 80 applied edits, 20 frontier conflicts, 40 wrong-Head conflicts, 20 direct compensations, and 20 no-frontier results. Evidence: `edits-220-239.json.gz`; managed run `5d22458b0d2045ed88d4d92cf81905c1` completed in 125 seconds. All 361 exact Challenge retries and 341 immediate command retries were stable.

A parallel launch was refused by the repository resource budget before database startup. Keep subsequent managed runs serial. Next: verify the remaining Proposal-derived edit and Undo outcomes and then collect their 20-hit counts.

## Step 12: Reversal and full lifecycle smoke

Seed 304 passed all 17 scenarios with zero differences. This includes a public Acceptance, later Author Edit, compensation, and UndoAcceptance reaching ReversalRequired without changing current prose. It also reaches Proposal-head conflict and typed Proposal-target mismatch through well-formed requests. Thus target mismatch is reachable through Proposal targets even though the direct-author target check is pre-Admission. All 171 immediate Challenge and command retries were stable. Evidence: `proposals-304.json.gz`; run `d1910035c0b64f4b86cc82ecd8cd65b4`, 67 seconds.

Next: add two-Operation ordered and atomic Bundle negative selections, then run 20 complete Proposal seeds. The fake destination prompt switches are input-fixture mechanics; dependency and Bundle expectations remain from the Manuscript State Machine contract.

## Step 13: Ordered and Bundle selections

Seed 310 passed 19 scenarios with zero differences, including missing required dependencies and incomplete atomic Bundle closure for both Accept and Reject. The generator identifies a selected Operation by its declared Block identity, not incidental query order. Evidence: `proposals-310.json.gz`; run `684c6a3493bf4a08b87dc559c8bc6b35`, 96 seconds. Added standalone replay modes for the zero-revision schema discrepancy and Replan Undo observation.

Next: check the new Reversal Proposal frontier, collect Proposal counts, and verify real five-minute Challenge expiry without clock injection.

Step 13 follow-up: seed 311 confirms that the new Reversal Proposal action becomes a Barrier for the next Undo. Its first Revision has no exact restoration handler. The request returns unavailable/barrier with stable exact retry and no authority change. Evidence: `reversal-barrier-311.json.gz`; managed run `a72a7d666617458fbddf6d2f57c2b323`.

## Proposal batch 320-323

Managed exit: 0. Differences: 0. Evidence: `proposals-320-323.json.gz` and `proposals-320-323.log`. Counts include only recorded calls.

## Proposal batch 324-327

Managed exit: 0. Differences: 0. Evidence: `proposals-324-327.json.gz` and `proposals-324-327.log`. Counts include only recorded calls.

## Proposal batch 328-331

Managed exit: 0. Differences: 0. Evidence: `proposals-328-331.json.gz` and `proposals-328-331.log`. Counts include only recorded calls.

## Proposal batch 332-335

Managed exit: 0. Differences: 0. Evidence: `proposals-332-335.json.gz` and `proposals-332-335.log`. Counts include only recorded calls.

## Proposal batch 336-339

Managed exit: 0. Differences: 0. Evidence: `proposals-336-339.json.gz` and `proposals-336-339.log`. Counts include only recorded calls.

## Step 14: Proposal count threshold

Five bounded batches completed seeds 320-339: 380 independently created AgentRuns, 20 hits for each reached lifecycle reason, 20 mixed Draft compensations, 20 Acceptance reversals, and 20 Reversal Proposal Barriers. All batches have zero differences. Each batch has its own committed compressed trace and managed log.

Next: verify two remaining Author Edit/Undo reasons through complete ordered-source proof and Draft expansion/compensation. Also minimize the empty authoritative payload in Proposal Undo responses as possible D-004. The expiry probe now retains 20 distinct committed commands, so its post-expiry retries are not repetitions of one settlement.

Step 14 follow-up: the first source-binding probe reached ownership_changed. Its expected object omitted the schema-required current Head, now included. The later inline AgentRun selected a change but opened no Proposal because the original pending Operation still reserved that Block after withdrawal. Closure and Operation resolution are independent. The setup now rejects the old Operation before creating the next reservation; no product expectation was changed. The initial probe is retained as `draft-binding-first-probe.json.gz`.

Step 14 follow-up: seed 351 reached Draft expansion through a public Anchor proof. Its derived candidate is structured and rejects the legacy plain candidate-edit shape. The attempted Undo then compensated expansion itself, so later assertions were model errors, not source-binding failures. The probe now changes the derived Proposal Head through withdrawal plus its registered compensation. It needs no candidate payload conversion. Evidence is retained as `draft-binding-structured-edit-probe.json.gz`. A later Undo at a rejection frontier returned Barrier; add an isolated replay against the section 10.1 rejection-reopen contract.

## Step 15: Remaining Undo reasons reached

Seed 352 reaches ownership_changed and source_binding_changed through HTTP. Draft expansion uses an Anchor read from an AgentRun-produced inline Proposal. Withdrawal compensation appends a new derived Proposal Head; the later expansion Undo refuses exact source binding and leaves the Draft closed. These state comparisons pass.

New implementation difference D-005: immediate exact retry of the withdrawal compensation changes effect.project_activity_position from 8 to 0. Both statuses are 200; every other response field matches. The first settlement stores the position in its payload as authoritative_applied, but replay only reads that payload position for proposal_revised and otherwise falls back to zero. Locations: undo_withdrawal.rs:162-173 and undo_latest_author_action.rs:1230-1246. Evidence: `draft-binding-352.json.gz`, run `0c9a07dd81a64ad4a4ff51dab6fcde5d`.

Next: minimize D-004 and D-005 separately; sample the two remaining reasons 20 times while retaining this known replay difference. Rejection Undo availability remains a contract question because section 10.1 names reopening but also permits an explicit Barrier for an unregistered handler.

## Step 16: Five isolated implementation differences

Seed 353 independently confirms D-004, the empty authoritative payload in Proposal Undo response. Seed 354 independently confirms D-005, withdrawal Undo retry changing only Activity position to zero; it also shows D-004. Seed 355 records A-010, the rejection Undo registration/Barrier question, with no guessed failure. All three compressed traces and source/contract locations are committed. No product source changed.

Next: reach 20 ownership_changed/source_binding_changed cases, then complete writer, challenge binding, rate-limit, and expiry sampling.

## Step 17: Author Edit and Undo threshold

Seeds 360-379 reached ownership_changed and source_binding_changed 20 times each. All state checks passed. The 20 exact-retry differences are individually audited in `draft-binding-replay-audit.json`: both HTTP statuses are 200 and only effect.project_activity_position changes to zero, the same D-005. Evidence: `draft-binding-360-379.json.gz`; run `54b4ded917914b72962298563570c01b`, 80 seconds. SourceUnavailable remains unreachable without missing retained source evidence; no destructive retention or SQL mutation is authorized.

Next: finish the Stage 4 sampling and real-clock expiry proof, then build the final outcome ledger and report.

## Step 18: Writer and binding smoke

Seed 410 passes writer takeover, stale writer refusal, stable old Author Edit replay after takeover, changed-body and wrong-nonce refusal, new-key/old-nonce refusal, changed-digest Challenge conflict, and both independent rate classes. Its sole difference is the already minimized D-002 delayed Create Chapter retry. Evidence: `replay-410.json.gz`; run `14c40005989141f7a6f282a1f8eae201`, 16 seconds. Added an exact Challenge retry after each exhausted rate class to prove that settled challenge identity does not need fresh capacity.

Next: run 25 bounded writer/rate seeds; count only stable-window capacity observations, then verify real expiry.

## Writer and Challenge batch 420-424

Managed exit: 0. D-002-shaped differences: 5. Other differences: 0. Evidence: `replay-420-424.json.gz` and `replay-420-424.log`. Final replay audit must confirm response status and changed fields before classification.

## Writer and Challenge batch 425-429

Managed exit: 0. D-002-shaped differences: 5. Other differences: 0. Evidence: `replay-425-429.json.gz` and `replay-425-429.log`. Final replay audit must confirm response status and changed fields before classification.

## Writer and Challenge batch 430-434

Managed exit: 0. D-002-shaped differences: 5. Other differences: 0. Evidence: `replay-430-434.json.gz` and `replay-430-434.log`. Final replay audit must confirm response status and changed fields before classification.

## Writer and Challenge batch 435-439

Managed exit: 0. D-002-shaped differences: 5. Other differences: 0. Evidence: `replay-435-439.json.gz` and `replay-435-439.log`. Final replay audit must confirm response status and changed fields before classification.

## Writer and Challenge batch 440-444

Managed exit: 0. D-002-shaped differences: 5. Other differences: 0. Evidence: `replay-440-444.json.gz` and `replay-440-444.log`. Final replay audit must confirm response status and changed fields before classification.

## Step 19: Writer and Challenge count threshold

Seeds 420-444 completed all 25 writer, digest/body/nonce binding, capacity, and post-capacity exact-Challenge retry cases. Both rate classes reached 25 stable-window capacity observations; no extra count run is needed. `writer-replay-audit.json` confirms all 25 delayed Chapter retry differences are HTTP 503/project_store_unavailable, the same D-002. No other difference occurred.

The run output now records candidate package source, manifest/server digests, and harness commit. This separates the fixed contract baseline from a later candidate package. Added usage and reachability documents. Next: one foundation-only structural Proposal reachability probe and the final real-clock expiry run.

## Step 20: Foundation-only outcome inventory

Seed 356 records HTTP 422 for a structural primitive with an explicit Proposal target, without changing prose. A-011 explains why the foundation's StructuralReshapeConflict sub-outcome cannot be selected through this current wire profile. A-008 separately records missing Acceptance NoEffect. Neither missing branch is silently removed from the report.

Created the explicit counted-evidence manifest. Future result files record completed seeds separately from the requested count, so partial runs cannot look complete from their header alone. Next: the real-clock expiry run and final ledger/report.
