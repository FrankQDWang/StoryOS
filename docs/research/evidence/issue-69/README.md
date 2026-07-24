# Issue #69 bounded evidence data

Source run: `issue69-2026-07-24T07-24-28-542Z`

Primary-source harness:
[`codex/issue-69-editor-session-harness@d994d25`](https://github.com/FrankQDWang/StoryOS/commit/d994d25d95ea635ebd33ae871c86704eda4c5377)

Unchanged executable harness source:
[`2c67c9abe126a17c6c02fb66ab63b843ecfb4a63`](https://github.com/FrankQDWang/StoryOS/commit/2c67c9abe126a17c6c02fb66ab63b843ecfb4a63)

The three automated files below are byte-for-byte copies from the frozen
harness run. The representative trace is a bounded 450-line extract; the full
2,100-line trace remains on the harness branch.

Trace Stream identity is at least
`{run_id, scenario_id, editor_session_id}`. `seq` and `at_ms` are
recorder-local ordering and elapsed-time fields inside one stream. A multi-tab
export deterministically concatenates multiple streams; file position does not
prove cross-browser chronological or Project authority order. The frozen
`secondary-tab-takeover-stale-writer` scenario intentionally contains
`writer-a` `seq` 1–12 followed by `writer-b` `seq` 1–15. Cross-session
authority order must be read from Core-owned Author Action, Activity, Head, and
writer-generation fields.

```text
abcbe8061ca56fcb5b5473eea060d428521c9527e6b2e10ebf39ae8760ef181c  automated-scenario-results.json
b44d25ab75ef952d9e9d6da5956b9e67d9a00e555773bbf4e26d9be5545d262d  automated-measurements.csv
8de5fe872d85de302497394417cedf197c4d063617bb0bb54caa7753e8b0f6ca  automated-environment.json
2065ee5da6208b74a193597c3f6bfea2b470291ef1a94d0f395ef293449280d0  representative-trace.jsonl
8ebc029c44562f86999e40640a84493ce0671aeabe9be020ccbfc1f98153dc04  manual-real-ime-checkpoint.json
7797c7f748b7a47f0cfd5763cbe1549b4b2954757910945c91d54b655777b0f3  manual-real-ime-trace.jsonl
```

The full harness trace SHA-256 is
`6d3d342c6f9b8792e937d3af102442bafd11ad9341fd241f4cf29d46b599e20f`.

The automated composition probe remains explicitly non-IME evidence. The
separate manual files capture the repository owner's real macOS Simplified
Chinese Pinyin checkpoint: three candidate commits, exact 18-byte final text,
three ordered Author Actions, one native undo action, and exact reload.

The harness branch additionally retains the before-undo, after-undo, and
after-reload screenshots. They are mechanism observations, not UI references.
