# Frozen automated evidence run

- Run: `issue69-2026-07-24T07-11-56-086Z`
- Harness source: `4b952821a6f96203f608b3e8118f12d36d4efe62`
- Command: `npm run build && npm run evidence`
- Google Chrome: `150.0.7871.182`
- Result: `24/24` scenarios passed; zero unexpected browser errors

## SHA-256

```text
0a7a5419148d95882c51ee43a1ad6fb9f98700bae965cd8d25fa4ef63dad82e6  trace.jsonl
47d0e92b8e82e27fbfae6a64f43f52f151e2ecaea196398e2935b728c05cede9  scenario-results.json
e5795e6c41ddf0504db108d704709c422633c068920d44601ac7bb2d441209c1  measurements.csv
f5d7fe1b62b73d4d4ee6d8ef461b8a8705cd1f90e08c60e99854b88b01613964  environment.json
```

`synthetic-composition-boundary-only` is explicitly non-IME evidence. A
separate visible macOS Chinese Pinyin checkpoint is required before Issue #69
can settle.
