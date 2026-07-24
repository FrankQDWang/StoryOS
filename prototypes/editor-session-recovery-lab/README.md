# Editor Session Recovery Lab

> Disposable prototype evidence for StoryOS Issue #69. This directory is not
> product code, is not a second editor/session contract, and must never enter a
> StoryOS runtime dependency or release artifact.

This lab exercises the current StoryOS editor-session contracts through a real
Tiptap editor, a real browser IndexedDB Local Edit Journal, and a separately
restartable fake Core. It exists to compare command-boundary policies and expose
ordering, crash, reload, writer-fencing, and resynchronization behavior.

The normative sources remain:

- `docs/foundation/author-command-admission.md`
- `docs/foundation/manuscript-revision-proposal-state-machine.md`
- `docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md`
- `docs/foundation/versioned-command-query-artifact-event-protocol.md`
- `docs/foundation/deterministic-verification-and-failure-recovery-gates.md`

## Run

Requirements: Node.js 22 or newer and Google Chrome.

```sh
npm install
npm run evidence
```

`npm run evidence` starts the restartable fake Core and Vite app, launches
Google Chrome, runs the deterministic scenario matrix, and writes bounded
primary-source outputs beneath `artifacts/latest/`:

- `trace.jsonl` — ordered browser, journal, Core, Receipt, Activity, projection,
  recovery, and GC observations;
- `scenario-results.json` — acceptance results and fault-window outcomes;
- `measurements.csv` — latency, command-count, payload, and journal-growth data;
- `environment.json` — exact tool, browser, schema, and source versions.

For an interactive inspection session:

```sh
npm run lab
```

The lab prints both URLs. Open the editor URL in Chrome. The fake Core persists
only beneath the lab's ignored `.lab-state/` directory, so killing and
restarting it does not collapse browser and Core durability into one process.

## Evidence limits

- Automated composition coverage verifies boundary logic only. It is never
  labeled as real operating-system Chinese IME evidence.
- The real-IME checkpoint must use macOS Chinese Pinyin in Google Chrome and
  records the exact committed UTF-8 text, journal entry, Core Receipt, Activity
  position, converged projection, and undo boundary.
- Clipboard and drag/drop checkpoints use native Chrome input paths.
- The fake Core is a contract probe, not a production Core implementation.
- Lab source stays on its throwaway evidence branch. Only bounded exported
  measurements, conclusions, and contract handoffs may be proposed to `main`.
