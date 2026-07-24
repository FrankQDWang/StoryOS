# Real macOS Chinese IME checkpoint

- Checkpoint: `issue69-real-ime-iab-2026-07-24`
- Harness source: `2c67c9abe126a17c6c02fb66ab63b843ecfb4a63`
- Surface: Codex In-app Browser on macOS
- Human action: the repository owner used the macOS Simplified Chinese Pinyin
  candidate UI to commit `中文输入验证`, then confirmed completion
- Result: exact 18-byte UTF-8 text converged to Core authority
- Composition result: three user-completed candidate commits (`中文`, `输入`,
  `验证`), three durable commands, and Author Action order `1, 2, 3`
- Undo result: one native `Meta+Z` reversed the uninterrupted typing burst as
  Author Action `4`
- Reload result: the post-undo Head, empty authority, and Activity position `4`
  reloaded exactly with no pending submission

This is primary-source mechanism evidence. It is not a UI recommendation.

## SHA-256

```text
8ebc029c44562f86999e40640a84493ce0671aeabe9be020ccbfc1f98153dc04  checkpoint.json
7797c7f748b7a47f0cfd5763cbe1549b4b2954757910945c91d54b655777b0f3  trace.jsonl
fb403115ad2fec65a760f2edbf7ecc46c4c34eafc477dbbaede1a84f9739a7bb  before-undo.png
33614c3e19f951df6f28754594c06e8b5d3ad40c898b258703e4e072bb784c76  after-undo.png
520ae25e7eff671d99369a31a635be73721631e395ca3861eef2630868a193f9  after-reload.png
```

`trace.jsonl` contains the 52-record input, settlement, undo, GC, and reload
sequence. `checkpoint.json` binds the exact states and fake Core snapshots and
records the #46 and #70 handoffs exposed by the checkpoint.
