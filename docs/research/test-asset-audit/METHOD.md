# Audit method and reason codes

Mode: Test Quality Review. All source locations refer to baseline `479224809cdaae997cda51cb8853e3fafa242b65`.

The presentation follows PR 927, `Removed test assertions and reasons`: one reason legend followed by per-file rows. A test with several input cases retains all distinct cases in its explanation; generated test.each cases get separate rows. Assertion overlap does not justify deletion of a test with another unique scenario.

| Code | Meaning |
|---|---|
| D1 | Same behavior and input already observed by the named retained test. |
| D2 | Static output, literal, type-level fact, or generator shape; no distinct observable product regression. |
| D3 | The oracle can pass after the intended protection is removed because another failure masks it. Named coverage owns the meaningful behavior. |
| D4 | A test checks a test-only reimplementation with no product consumer. Named coverage owns the product behavior, not that dead helper. |
| M1 | Merge the named unique case into an existing public-boundary test, then remove the redundant test. |
| L1 | Move the scenario to its public boundary with a discriminating oracle; current setup does not prove its intended protection. |
| K1 | A concrete distinct input or invariant has no equivalent assertion in the compared tests. |
| K2 | A distinct executable boundary or failure mode is not proved at another layer. |

KEEP evidence states a concrete failure and the closest competing test, including why it does not cover that failure. Coverage here is a source comparison unless mutation evidence is linked. Later directory review can revise any verdict. DELETE D2/D4 does not claim that another test checks dead code or a static implementation shape. If the random self-check selects such a row, record the limitation or failed coverage, do not replace the sample or invent a kill.

## Line estimates

`remove` in each data file is a closed source-line interval for immediate DELETE recommendations. Full-file removal includes imports and local helpers only when every test in that file is deleted. MERGE and MOVE have zero immediate savings until replacement coverage exists. Cross-file support cleanup is counted only in the support directory. Intervals must be unioned per file; do not sum overlapping spans. Final top-20 and directory summaries use these unions.

## Mutation protocol

Freeze the complete DELETE population and choose 30 without replacement with a saved random seed. For each: run the named covering test on clean sources, save a minimal product defect patch, run that test alone, restore exact bytes in finally, and run the restored test. A compile error, unrelated setup failure, timeout, zero selected tests, skipped test, or a failure only in the proposed removed test is not a kill. Save all outputs and classify misses and blocked runs. Revise all verdicts that depend on the invalidated assumption. Use only the audit worktree and `scripts/dev-postgres.sh run` for database checks.
