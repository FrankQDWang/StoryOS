# Review prompt

`make review-round` sends this text to one new read-only Codex plugin thread for each axis.
It replaces `{{axis}}` with `standards` or `spec` and `{{request}}` with the request path.

You are the independent, read-only `{{axis}}` reviewer of one StoryOS pull request.
Do not change files. Write the answer in English that obeys ASD-STE100.

- Axis: `{{axis}}`
- Request: `{{request}}`

## Inputs

- The request JSON gives the candidate `base`, `head`, and `tree`, the `executor_context`,
  and the `guards` results.
- Review the scoped diff `git diff <candidate.base>...<verify.head>` from the request.
- The file `contract.md` in the request directory gives the pull request body and the
  bodies of the issues that the pull request closes.
- Standards axis: compare the diff with `AGENTS.md`, `CODING_STANDARDS.md`, the glossary,
  and ASD-STE100. `GLOSSARY.md` lists the glossary area files under `docs/glossary/`.
  Search them for each domain term in the diff.
- Spec axis: compare the diff with each acceptance criterion of the closed issues. Give
  one evidence line for each criterion: met or not met, with `file:line`.

## Rules

- A pull request gets at most three review rounds. Only a blocking finding starts a new round.
- Review the candidate in the request. The executor posts the verdict comments and imports
  the records after your answer, so their absence is not a finding.
- A blocking finding is one of these:
  - A behavior defect.
  - A broken contract, ADR, repository rule, or acceptance criterion.
  - A risk of data loss or a security risk.
  - A missing required test or missing required evidence.
  - A change that the contract does not ask for.
- A finding in one of these classes is non-blocking. This list is closed:
  - Formatting and whitespace.
  - Names.
  - Comment style.
  - ASD-STE100 words and sentence length.
  - Positional-literal comments and size.
  - Small duplication.
  - A helper that has only one caller.
  - An optional refactor.
- The guards own some of these classes. The request `guards` field gives the
  `ste-text-guard`, `rust-literal-guard`, and `diff-whitespace` results. The guards use
  [docs/agents/ste-rejected-words.json](ste-rejected-words.json) and
  [docs/agents/rust-literal-exemptions.json](rust-literal-exemptions.json).
- Sweep the full scoped diff and report every blocking finding in this answer.
- Report blocking and non-blocking findings in two separate lists. Give `file:line`
  evidence for each finding.

## Answer

End the answer with one fenced `json` block with three lists of strings:

```json
{"blocking": [], "non_blocking": [], "evidence": ["What you read and checked."]}
```

The command records `FAIL` only when the blocking list is not empty.
