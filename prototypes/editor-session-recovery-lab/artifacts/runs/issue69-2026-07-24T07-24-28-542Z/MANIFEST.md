# Frozen automated evidence run

- Run: `issue69-2026-07-24T07-24-28-542Z`
- Harness source: `2c67c9abe126a17c6c02fb66ab63b843ecfb4a63`
- Command: `npm run build && npm run evidence`
- Google Chrome: `150.0.7871.182`
- Result: `24/24` scenarios passed; zero unexpected browser errors
- Native input coverage added by this run: backward delete and forward delete

## SHA-256

```text
6d3d342c6f9b8792e937d3af102442bafd11ad9341fd241f4cf29d46b599e20f  trace.jsonl
abcbe8061ca56fcb5b5473eea060d428521c9527e6b2e10ebf39ae8760ef181c  scenario-results.json
b44d25ab75ef952d9e9d6da5956b9e67d9a00e555773bbf4e26d9be5545d262d  measurements.csv
8de5fe872d85de302497394417cedf197c4d063617bb0bb54caa7753e8b0f6ca  environment.json
```

`synthetic-composition-boundary-only` is explicitly non-IME evidence. A
separate visible macOS Chinese Pinyin checkpoint is required before Issue #69
can settle.
