# Reference model

This one-off Python harness compares a contract model with the packaged StoryOS Server, Core, PostgreSQL, and fake destination through public HTTP. It has no product imports, SQL writes, self-tests, or default build hooks.

The fixed product base is `479224809cdaae997cda51cb8853e3fafa242b65`. The product package was built at `0dfe51840212a5001dc5439f1996c690405cd56e`; its product files are the same base. The complete findings, outcome ledger, and evidence manifest are in `docs/research/reference-model/`.

## Run

Use the isolated worktree. Build the repository-owned release package if it is absent or the product candidate changed:

```sh
cd /Users/frankqdwang/.codex/worktrees/reference-model/StoryOS
make release-package
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage bootstrap --seed 1
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage structure --seed 100 --count 4 --replay --output target/reference-model/structure.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage edits --seed 220 --count 20 --replay --output target/reference-model/edits.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage proposals --seed 320 --count 4 --replay --output target/reference-model/proposals.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage proposals --case draft_binding --seed 360 --count 20 --replay --output target/reference-model/draft-binding.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --seed 420 --count 5 --replay --output target/reference-model/replay.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case expiry --seed 450 --replay --output target/reference-model/expiry.json
```

The database command owns creation and cleanup. The Server binds port zero. Managed runs are serial and source-frozen: do not edit or commit tracked-source scope while one is active. Keep runtime outputs in ignored `target/reference-model/`, then copy evidence and commit progress after exit. Keep batches bounded; the retained first long structure run lost an HTTP connection before completion.

The expiry case waits for the real returned five-minute deadline. It checks 20 distinct expired pending challenges and 20 distinct committed Author Edit acknowledgements after expiry.

## Replay a finding

```sh
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case project-undo --seed 403 --replay --output target/reference-model/D-001.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case chapter-after-session --seed 401 --replay --output target/reference-model/D-002.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case session-after-edit --seed 402 --replay --output target/reference-model/D-003.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage proposals --case proposal-undo --seed 353 --replay --output target/reference-model/D-004.json
scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage proposals --case withdrawal-undo --seed 354 --replay --output target/reference-model/D-005.json
```

A seed and case reconstruct the semantic sequence with new server identities and fresh challenges. Evidence is not a curl transcript: cookies and nonce values are intentionally absent. Exact retry comparisons use the original bytes and nonce only in process memory.

The script reports `coverage`, `differences`, and every public request/response. A managed command's process PASS only means that execution completed; conformance comes from the difference list and the classified report. Known baseline defects remain visible. Never change the model expectation to erase them.

Rebuild the retained count report with:

```sh
python3 prototypes/reference-model/aggregate.py
```

Its manifest selects counted evidence explicitly; failed initial probes remain retained but do not supply successful model coverage.

## Oracle and generation

- Structure: independent ordered lists, live sibling ranks, relative Tree Revision increments, allocation counts, explicit Current Chapter selection, and no-change Activity.
- Author Edit: seeded strings and UTF-16 positions; compute the resulting text from the input units. Verify canonical queries, action/Commit allocation, and compensation.
- Proposal: shuffled independent lifecycle scenarios, seeded candidate edits, fresh fake-destination AgentRuns, independent state axes, exact candidate acceptance, and zero-authority decisions.
- Mixed input: preserve every source unit and digest; check unchanged Blocks/candidates, Draft closure, compensation, and exact source binding.
- Replay: compare raw acknowledgements, mutate key/body/nonce bindings, hand writer authority to a second Session, and measure the two independent Challenge capacities.

The generator randomizes scenario order, identities, edit positions/text, and the structure walk. Targeted templates deliberately select rare outcomes. It is not a uniform arbitrary-command fuzzer.

Returned opaque identities, prior public state, and fake candidate bytes are inputs to the model. Transition rules come from AGENTS, GLOSSARY, ADRs, foundation contracts, and generated public schemas. Product source was read only for launch mechanics, reachability, and defect locations. Initial revision conventions, automatic Chapter selection, and other unresolved rules are listed as contract questions instead of guessed expectations.

D-001 is audited separately for Project Action allocation. Existing sequence checks track the structure/edit history after Project creation; they must not be read as approval of missing Project actions. The benchmark describes this fixed base and is not a proof of all input combinations, browser behavior, external providers, or concurrency schedules.
