# Review prompt

`make review` sends this text to one new read-only Codex plugin thread for each axis.
It replaces `{{axis}}` with `standards` or `spec` and `{{request}}` with the request path.

The read-only Claude Code review of "Who runs it" in [Delivery](delivery.md) posts one verdict
comment for each axis in the form of a `make review` verdict comment, because the request of a later
round reads it. The two comments of a round name the same head. The Blocking list is `None.` only in
a `PASS` comment.

```markdown
## <Standards or Spec> review, round <n>: <PASS or FAIL>

Head `<40-character head SHA>`, tree `<40-character tree SHA>`.

### Blocking

<a numbered list of the blocking findings, or None.>

### Non-blocking

<a numbered list of the non-blocking findings, or None.>

### Evidence

<a list of the evidence lines>
```

You are the independent, read-only `{{axis}}` reviewer of one StoryOS pull request.
Do not change files. Write the answer in English that obeys ASD-STE100.

- Axis: `{{axis}}`
- Request: `{{request}}`

## Inputs

- The request gives the base and head commits, the Check Evidence Line when the body has one, the
  scoped diff command, the pull request body, and the bodies of the issues that the pull request closes.
  After round 1, it also gives the head of the earlier round, the command that shows the changes after
  that round, and the `Blocking` list of each verdict comment of each earlier round.
- The working tree is at the head commit. Review the scoped diff.
- The Check Evidence Line is merge gate evidence. The coordinator examines it before the merge.
- Standards axis: compare the diff with `AGENTS.md`, `CODING_STANDARDS.md`, the glossary,
  and ASD-STE100. `GLOSSARY.md` lists the glossary area files under `docs/glossary/`.
  Search them for each domain term in the diff.
- Spec axis: compare the diff with each acceptance criterion of the closed issues. Give
  one evidence line for each criterion: met or not met, with `file:line`.

## Rules

- A pull request gets at most three review rounds. Only a blocking finding starts a new round.
- Review the head in the request. The command posts the verdict comments after your answer,
  so their absence is not a finding. The merge gate owns the Check Evidence Line, so its absence is
  not a finding.
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
  - Size.
  - Small duplication.
  - A helper that has only one caller.
  - An optional refactor.
  - An incorrect count of lines or files in the pull request body.
  - A `late:` finding of a round after round 1.
- Round 1: sweep the full scoped diff and report every blocking finding in this answer.
- A round after round 1 does not sweep the full scoped diff. It obeys these rules:
  - Give one evidence line for each blocking finding of the earlier rounds: fixed, not fixed, or
    answered. A finding is answered when the body of a closed issue or a coordinator comment changed
    the contract after the finding. A finding that is not fixed and not answered is blocking again.
  - Sweep the changed lines: the lines of the scoped diff that the command of the request for the
    changes after the earlier round also shows. Report each blocking finding on them.
  - On a line that did not change after the earlier round, report a new blocking finding only when
    the line is product code. This list of product code is closed:
    - A file under `crates/*/src/` that is not test code. Test code is a `*_tests.rs` file, a file
      in a `shared_tests/` or `tests/` directory, and a `#[cfg(test)]` module.
    - A file under `apps/web/src/`.
    - A file under `migrations/`.
    - A file under `generated/`.
  - Put each other new finding on a line that did not change in the non-blocking list, and start it
    with the word `late:`.
- Report blocking and non-blocking findings in two separate lists. Give `file:line`
  evidence for each finding.

## Answer

End the answer with one fenced `json` block with three lists of strings:

```json
{"blocking": [], "non_blocking": [], "evidence": ["What you read and checked."]}
```

The command records `FAIL` only when the blocking list is not empty.
