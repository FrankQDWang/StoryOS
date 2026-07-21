# Design QA

## Visual truth

- Confirmed source: `../tiptap-proposal-lab/artifacts/prototype-default.png` and the current `../tiptap-proposal-lab/src/App.jsx` / `styles.css` implementation.
- Issue #55 implementation: the live root page; `artifacts/eval-page.png` records the separate Eval boundary.
- Matched comparison state: `1487 × 1058`, chapter 12 selected, Proposal pending, Agent idle, Transcript expanded.
- The baseline stays in its original prototype; no duplicate or reference asset is loaded by this prototype runtime.

Before the author interaction correction, the full-size confirmed baseline and implementation were inspected together in one comparison input. A separate crop was not necessary: at original resolution the tree typography, manuscript measure, Proposal controls, Transcript header, run summary, App boundary, and composer were all readable. The final correction removes prototype-only controls and reuses the already-compared composer action slot; it does not change the shell geometry.

## Findings and iteration

1. The fixed geometry remains faithful to the confirmed 4173 shell: the left tree, manuscript column, right transcript boundary, compact fixed bars, footer controls, and collapsed 44 px rail keep the same hierarchy and density.
2. The new MCP App increases transcript density by design, but stays inside the right column and identifies itself as a read-only view. Its action hands work to the Agent rather than changing prose.
3. The editor remains the only visible home for Proposal editing and accept/reject. Eval is a separate page; optional Project Instruction is under project settings.
4. The author rejected all additional Run-control placements and selected the established Codex composer behavior. The A/B/C switcher, editor work bar, header control, and Transcript stop button were removed.
5. During an active Run, the existing send-arrow location shows a square pause button when the composer is empty. Typing guidance restores the arrow; sending it continues the Run with the new author message. Clearing the composer restores the square. Pausing records that unfinished content did not enter prose.
6. Browser QA previously found a P1 interaction defect where stop controls passed the click event into the Transcript. The obsolete controls were removed; the remaining composer pause action invokes the callback without an event payload.
7. No remaining P0, P1, or P2 visual or interaction defects were found. No intentional baseline mismatch needs a waiver.

## Interaction evidence

- Changed chapters and returned to chapter 12.
- Accepted and rejected the Proposal; both remove the pending Proposal controls while the editor remains authoritative.
- Sent a normal Agent message and started/paused a Run from the composer action location.
- Started a Run from the transcript MCP App; closed it to its static fallback and reopened it.
- Collapsed the transcript to a measured 44 px rail and reopened it.
- Opened project settings, entered an optional Project Instruction, and confirmed the empty/default path remains usable.
- Opened the separate Eval Studio page and returned to writing.
- Confirmed the author-selected composer state model in source: empty + running → square pause; non-empty + running → send arrow; paused → ordinary disabled send arrow until text is entered.
- At `1160 × 800`, the layout measured `220px 600px 340px` with no body overflow.
- A fresh final load at `1487 × 1058` produced no browser warnings or errors.

## Verification

```text
vite v6.4.2
4570 modules transformed
built in 3.03s
```

The restarted Vite server served the final source with the exact `running && empty` pause-button branch and no obsolete variant, header, workspace-bar, or Transcript stop-control code.

Final result: passed
