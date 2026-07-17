# Design QA — StoryOS Tiptap Proposal Lab

- Source visual truth: `/Users/frankqdwang/MLE/StoryOS/docs/design/storyos-three-column-writing-workspace.png`
- Bottom-control component reference: `/var/folders/ns/k10qv8w14s3c6kfkgp_xk3z00000gn/T/codex-clipboard-9b374641-729f-464b-aec5-91dd1c65b747.png`
- Fixed-boundary and transcript reference: `/var/folders/ns/k10qv8w14s3c6kfkgp_xk3z00000gn/T/codex-clipboard-208a99d8-15ba-45ef-b3b3-2f63d1bccc7d.png`
- Latest left fixed-chrome and rejected-Proposal reference: `/var/folders/ns/k10qv8w14s3c6kfkgp_xk3z00000gn/T/codex-clipboard-291e9b34-c2e3-4426-9b27-ce75ff765695.png`
- Latest interaction override: `/Users/frankqdwang/MLE/StoryOS/prototypes/tiptap-proposal-lab/AGENTS.md` specifies a 44 px collapsed rail and one 36 px workspace-level toggle whose screen position is identical in expanded and collapsed states.
- Implementation screenshots: `/Users/frankqdwang/MLE/StoryOS/prototypes/tiptap-proposal-lab/artifacts/prototype-default.png` and `/Users/frankqdwang/MLE/StoryOS/prototypes/tiptap-proposal-lab/artifacts/prototype-fixed-chrome-reopen.png`
- Latest color-calibrated implementation screenshot: `/Users/frankqdwang/MLE/StoryOS/prototypes/tiptap-proposal-lab/artifacts/prototype-color-calibration.png` at `1329 × 768`, captured in Chrome after a full reload.
- Viewport: `1487 × 1058`
- State: default desktop workspace; volume two expanded; chapter twelve active; editable Proposal ready and valid; 写作助手 panel expanded. The collapsed state is captured separately in `artifacts/prototype-collapsed.png`.

## Full-view comparison evidence

`artifacts/design-qa-comparison.png` places the approved source on the left and the browser-rendered implementation on the right at the same viewport and state.

The final comparison preserves the source's three major regions, volume-to-chapter hierarchy, manuscript measure, neutral warm palette, low-chrome density, Proposal placement, conversation structure, and compact bottom composer. The user-directed `写作助手` naming and spatial role distinction intentionally replace the source's explicit `你` / `Agent` labels. The refreshed implementation also restores the source's softer surfaces, quieter selection treatment, and more literary prose rhythm. No actionable P0, P1, or P2 differences remain.

`artifacts/design-qa-color-calibration-comparison.png` adds the latest Chrome capture beside the approved source after removing the browser chrome and normalizing both images into equal comparison frames. The current browser viewport is shorter than the approved source, so this evidence validates palette balance and regional hierarchy rather than false position-by-position precision.

## Focused comparison evidence

- `artifacts/design-qa-proposal-comparison.png` compares the manuscript and editable Proposal regions. Heading hierarchy, paragraph measure, line wrapping, pale Proposal background, soft vertical marker, reading order, and accept/reject placement align with the source.
- `artifacts/design-qa-agent-comparison.png` compares the conversation, run summary, and composer. Panel proportions, softened card treatment, spacing, and composer anchoring align with the source; the author card is right-aligned and the writing-assistant response is left-aligned without role headings.
- `artifacts/design-qa-codex-reference-comparison.png` places the supplied Codex screenshot beside the implementation. It verifies the requested bottom-control anatomy: compact account/settings footer, low composer dock, soft fade and shadow, quiet add action, and restrained circular send control.
- `artifacts/design-qa-boundary-reference-comparison.png` places the supplied fixed-boundary screenshot beside the implementation. It verifies the compact 40 px writing-assistant header, subtle fixed-bar dividers, outward fades, and the unobstructed transcript-to-composer transition.
- `artifacts/prototype-transcript-boundary.png` records an intentionally overflowing conversation scrolled to its end. The final writing-assistant message clears the visible composer fade rather than being cut by the message viewport.
- `artifacts/prototype-composer-two-lines.png` and `artifacts/prototype-composer-ten-lines.png` verify the composer's minimum and maximum content-sized states without changing the fixed bottom anchor or control alignment.
- `artifacts/prototype-collapsed.png` verifies the requested collapsed state: the 44 px rail contains no visible text and the split-panel expand button sits in its upper-right corner.
- `artifacts/design-qa-fixed-chrome-left-rail-comparison.png` normalizes the supplied Chrome reference and current Chrome capture to the same app-surface height. It verifies the matching 40 px directory/footer bars, centered footer controls, and quiet fixed-to-scroll boundaries. The source and implementation were captured at different browser viewport sizes, so this focused comparison is used for component rhythm rather than false pixel-level full-page precision.
- `artifacts/design-qa-fixed-chrome-comparison.png` records the complete supplied and current Chrome frames together for overall composition evidence; the existing `artifacts/design-qa-comparison.png` remains the same-viewport full-workspace fidelity check.
- `artifacts/design-qa-color-calibration-focused.png` compares the Proposal and writing-assistant regions before and after the latest token calibration. It confirms that the Proposal, author message, and run summary retain distinct warm-gray surfaces without the previous repeated yellow cast.

## Required fidelity surfaces

- Fonts and typography: Noto Serif SC and Noto Sans SC provide a close, legally reusable approximation of the raster source. Hierarchy, weight, line height, wrapping, and Chinese text density are aligned. The live browser font renders marginally lighter than the generated source; this is P3 polish, not a structural mismatch.
- Spacing and layout rhythm: the final grid uses the source proportions at 1487 × 1058. The manuscript starts at the same horizontal measure, the Proposal follows the affected paragraph, and the empty/two-line composer measures 99.1 px high inside a 113.1 px dock. It grows upward one line at a time to 265.5 px at ten lines while its bottom remains 14 px from the viewport edge. The directory header and account footer now share the same 40 px fixed-bar height and 8 px boundary fade, removing the footer's previous visual imbalance. The latest author direction intentionally places the persistent toggle 4 px from the viewport's right edge so it remains centered in the narrower collapsed rail.
- Colors and visual tokens: the canvas and tree panel retain their already aligned neutral off-whites. The assistant panel now uses `#f9f8f7`; Proposal, author message, and run summary use separate sampled surfaces (`#f6f4f3`, `#f5f3f2`, and `#f6f5f4`); and the active chapter uses `#f1f0ee`. This removes the uniform yellow cast created by one shared card token while preserving the approved asset's intentional warmth. The composer alone adds the explicitly requested subtle background fade and low shadow; the rest of the workspace remains flat and low-chrome.
- Image quality and asset fidelity: the target contains no raster illustrations, logos, or custom imagery that require separate assets. Phosphor supplies the visible UI icons; no handcrafted SVG, CSS icon, emoji, or placeholder asset is used.
- Copy and content: the chapter tree, manuscript, author request, writing-assistant response, run summary, and accept/reject actions reproduce the approved target's content and intent. `写作助手` replaces `Agent`, and explicit role headings are omitted by author direction. The implementation intentionally shows Proposal revision `r3` instead of the mock's elapsed duration because it exposes real prototype state.

## Browser interaction evidence

### Proposal contract completion pass — 2026-07-16

- The product compatibility scope is desktop Google Chrome with Chinese and
  English author input.
- Real Google Chrome entered `FULL` admission and classified an insertion exactly
  at the inline Proposal start as
  authoritative and a strict-interior insertion as Proposal-owned.
- Real Google Chrome classified the exact Proposal end as authoritative. Exclusive decoration
  mapping kept adjacent insertions outside the Proposal range.
- Real Google Chrome cross-owner keyboard replacement, backward delete, and
  forward delete each left the editor byte-for-byte unchanged and produced an
  inspectable `Refused Edit Draft` containing the attempted result.
- Real Google Chrome exercised runtime-capability mismatch and invariant violation.
  Each entered Safe Mode and refused direct Proposal editing. Compatibility-
  evidence mismatch remains covered by the deterministic contract tests.
- The real Tiptap Block ID matrix passed split, exact undo/redo restoration, join,
  atomic move, StoryOS copy, and one-to-one retype. The first copy probe exposed
  that Tiptap `UniqueID` retains duplicate IDs for arbitrary programmatic copies;
  the StoryOS command boundary now clears copied identity before insertion.
- Real Google Chrome unified-undo evidence showed direct Acceptance followed by
  `Mod-z` preserving the original Receipt while reopening the Proposal, then
  `Mod-Shift-z` performing a new Acceptance with a new Receipt.
- On 2026-07-17 the author manually verified real Chinese Pinyin input in the
  contract probe on supported desktop Google Chrome. The real OS IME evidence
  gate is therefore passing for this prototype; English direct input had
  already passed separately.
- On 2026-07-17 real Chrome `Mod-v` over the mixed `｜提` selection emitted
  `native_paste`, kept the document byte-for-byte unchanged, and preserved the
  attempted `原生粘贴` result as a `Refused Edit Draft`. Real `Mod-x` emitted
  `native_cut`, kept the document unchanged, preserved its Draft, and copied the
  actual selected text `｜提` to the clipboard.
- On 2026-07-17 the author manually dragged the mixed `｜提` selection across the
  ownership boundary in real desktop Chrome. The browser emitted
  `native_dragstart` and `native_drop`; the contract emitted
  `refused_edit_draft`, kept the authoritative document byte-for-byte unchanged,
  and preserved the attempted moved result as a `Refused Edit Draft`.
- No Chrome console errors were reported during the contract matrix.
- All native Chrome input evidence required by this browser/editor ticket is
  passing. Crash-window reconciliation remains separately deferred until the
  Core and storage boundaries exist.

- Volume/chapter navigation: at the short `1487 × 720` check, selecting chapter fourteen scrolled the independent tree to its maximum while the bottom utility bar remained fixed; chapter twelve restores its document and Proposal.
- Writing-assistant panel: collapse and expand both work; the collapsed rail has no visible text and measures 44 px. The writing-assistant header is 40 px high. Its title and the 36 × 36 px toggle share `centerY = 20`; the toggle keeps the same `x = 1447`, `y = 2`, and `right = 4` geometry in expanded and collapsed states.
- Color-calibration regression: after a full reload at `1329 × 768`, the writing-assistant panel still collapsed and expanded from the same fixed upper-right toggle without layout movement. The CSS-only token change introduced no interaction or content-state regression.
- Fixed boundaries: the writing-assistant header retains its 1 px divider and 18 px fade. The left directory header and account footer each use a 1 px low-contrast divider plus a tighter 8 px directional fade; the chapter scroller reserves exactly 8 px at its bottom, with no unrelated spacer. The composer owns a 24 px fade that overlaps the message viewport instead of occupying a separate grid track, with `42 px` transcript bottom padding keeping the final message clear.
- Transcript boundary regression: with `scrollHeight = 1209 px` in the `905 px` message viewport, the conversation reaches `scrollTop = 304 px`. The final message ends at `y = 903.0`, retaining `17.9 px` of visible clearance above the fade's opaque edge at `y = 920.9`; no line is clipped or hidden.
- Composer autosizing: empty, one-line, and two-line values all measure 55.6 px in the textarea; three lines measure 76.4 px; ten lines measure 222.0 px. Twelve lines remain capped at 222.0 px while `scrollHeight` increases to 264 px. Deleting back to one line and sending both restore the 55.6 px minimum. Automatic Chinese wrapping also expands the field without manual newlines.
- Conversation roles: the initial author message is right-aligned, the writing-assistant message is left-aligned, and no `.message-label` elements remain.
- Proposal editing: author input at the Proposal end created revision `r4`, showed pending validation, then returned to valid.
- Acceptance: accepting removed Proposal controls and exposed a transcript action; undoing acceptance reopened the same content through a new revision.
- Rejection: rejecting captured only the candidate blocks and adjacent placement anchors, then removed only those blocks. After a full browser reload, the transcript action remained available; reopening inserted a new `r4` Proposal at the validated anchors and preserved the authoritative chapter content.
- Streaming: simulated Agent batches were applied off-history; author input paused the stream, projected `ready_partial`, fenced the admitted sequence, and prevented later chunks.
- Partial completion: explicit completion moved `ready_partial` to `ready`, kept accept disabled during validation, then enabled it after validation.
- Mixed ownership: selecting the full editor and attempting replacement left the document byte-for-byte unchanged and surfaced the whole-transaction refusal notice.
- Conflict: simulated target conflict disabled acceptance and projected the need to replan.
- Recovery: reload restored the document, author-edited partial text, Proposal axes, revision `r10`, and transcript from the prototype scratch snapshot.
- Console errors checked: none.

## Comparison history

### Iteration 1

- Earlier findings: center manuscript measure was too narrow; the right Agent panel began about 22 px too far right; Proposal wrapping and height drifted from the source.
- Fixes: widened the manuscript measure, matched the Agent column width, and aligned asymmetric manuscript padding to the reference.
- Post-fix evidence: `artifacts/design-qa-comparison.png` shows matching column boundaries and text measure.

### Iteration 2

- Earlier findings: the Proposal began about 18 px too high and the Agent response/run group began about 25 px too high.
- Fixes: increased authoritative-paragraph spacing and the post-author-message gap without changing Proposal paragraph density.
- Post-fix evidence: both focused comparison images show aligned Proposal and transcript landmarks.

### Iteration 3

- Earlier findings: the chapter tree could overflow behind a fixed inspection button; the collapsed conversation rail retained vertical `Agent` text and centered its expand control; message roles relied on explicit labels.
- Fixes: made the chapter navigation the only scrollable left-column track, moved the inspection control into the fixed bottom utility bar, renamed the panel `写作助手`, adopted the split-panel icon, removed visible role headings, aligned assistant/author messages left/right, and placed the collapsed expand control at the rail's upper-right.
- Post-fix evidence: `artifacts/prototype-default.png`, `artifacts/prototype-collapsed.png`, and the refreshed comparison images show the corrected states at the approved `1487 × 1058` viewport. Browser geometry checks also verified the short-viewport tree scroll and fixed footer.

### Iteration 4

- Earlier finding: the right rail measured 52 px, and separate expanded/collapsed toggle elements used different padding rules, causing the icon to move when the panel changed state.
- Fixes: reduced the collapsed rail to 44 px, replaced the two state-specific controls with one 36 px workspace-level toggle, and derived its top/right position from the shared rail, toggle, and header dimensions.
- Post-fix evidence: `artifacts/prototype-default.png` and `artifacts/prototype-collapsed.png` show the revised states. Browser geometry checks report the same `36 × 36`, `x = 1447`, `y = 21`, and `right = 4` button rectangle before and after the transition; the collapsed panel width is exactly 44 px.

### Iteration 5

- Earlier finding: despite structural alignment, the implementation felt more componentized than the approved asset because Proposal and card fills were darker, dividers and selection states were stronger, the chapter tree was denser, the prose measure and wrapping drifted, and the composer was about 25 px too tall.
- Fixes: replaced the yellow-green panel palette with sampled neutral off-whites, softened dividers and active states, restored 40 px tree rows, aligned the manuscript and Proposal measure, tuned literary type rhythm, removed the left footer separator, softened transcript surfaces, and reduced the composer to the source-aligned 104.5 px height.
- Post-fix evidence: the refreshed full-view and focused comparison images show the quieter visual character and matching component rhythm. Browser geometry places the Proposal at `top = 471.6` and `bottom = 950.6` against the raster target's approximately `469–948`, while the composer sits at `y = 918.5` with a `104.5` px height.

### Iteration 6

- Earlier finding: the left footer still behaved like a utility strip, with a search action, settings beside it, a 68 px track, and unused vertical space. The conversation composer also retained a 35 px bottom gap, a taller text area, and oversized padded icon buttons.
- Fixes: replaced search with an account placeholder, moved settings to the opposite edge, reduced the footer to 52 px, introduced a dedicated faded composer dock, reduced the composer to 85.5 px, and normalized both control hit areas to 28 × 28 px with zero internal padding. The add action is visually borderless like the Codex reference; send retains a restrained circular fill.
- Post-fix evidence: `artifacts/prototype-default.png` and `artifacts/design-qa-codex-reference-comparison.png` show the corrected bottom anatomy. Browser geometry reports a 52 px left footer, no search control, a 123.5 px dock ending exactly at the viewport bottom, an 85.5 px composer, and two aligned 28 × 28 px control hit areas with `padding = 0`.

### Iteration 7

- Earlier finding: the compact composer still used a fixed 42 px textarea. Long input scrolled immediately instead of allowing the author to see a useful amount of the prompt, and deleting text could not change the component height because there was no content-sized layout behavior.
- Fixes: replaced the fixed height with native `field-sizing: content`, set an exact two-line minimum and ten-line maximum using `lh`, kept overflow internal only after the maximum, and made the message track explicitly `minmax(0, 1fr)` so the bottom-anchored composer owns only the space its content requires. No refs, effects, hidden mirrors, `scrollHeight` mutation, or compatibility branches were added.
- Post-fix evidence: `artifacts/prototype-composer-two-lines.png` and `artifacts/prototype-composer-ten-lines.png` show the two boundary states. Browser geometry verifies natural growth from 55.6 px to 222.0 px, a hard ten-line cap, internal scrolling at twelve lines, automatic-wrap growth, deletion shrinkage, and immediate return to the two-line minimum after send.

### Iteration 8

- Earlier finding: the writing-assistant title occupied the full 78 px workspace header track, leaving more vertical whitespace than the author wanted. The persistent toggle was correctly stable but followed that taller track and therefore sat lower than the desired compact title line.
- Fixes: introduced a separate 58 px writing-assistant header token while preserving the left project header at 78 px. The title row and the single workspace toggle now derive their vertical alignment from the same assistant-specific token.
- Post-fix evidence: `artifacts/prototype-default.png` shows the tighter expanded header, and `artifacts/prototype-collapsed.png` verifies the matching collapsed position. Browser geometry reports `header height = 58`, `title centerY = 29`, and `toggle centerY = 29`; the toggle remains `36 × 36`, `x = 1447`, `y = 11`, and `right = 4` in both states.

### Iteration 9

- Earlier findings: the 58 px writing-assistant title row still felt too tall; the fixed header and left footer lacked the quiet boundary treatment visible in the supplied reference; and the composer's fade lived in its own grid-row padding, so the message viewport ended at a hard clipping edge before the visual fade began.
- Fixes: reduced the shared writing-assistant header token to 40 px, added one reusable low-contrast divider/fade treatment to both fixed bars, and moved the composer fade into a pseudo-element that extends over the transcript boundary. Added matching scroll-safe bottom padding to the message list and chapter tree rather than introducing JavaScript measurement, masking, or compatibility branches.
- Post-fix evidence: `artifacts/design-qa-boundary-reference-comparison.png` and the refreshed default/collapsed screenshots show the compact, separated fixed bars and uninterrupted transcript edge. Browser geometry reports `header height = 40`, `title centerY ≈ 20`, and `toggle = 36 × 36` at `x = 1447`, `y = 2`, `right = 4` in both panel states. The empty composer remains at `y = 944.9` with bottom `1044`, while the 24 px fade now spans the message edge without consuming dock padding. The two-to-ten-line regression remains `55.6 px → 222.0 px`, caps at twelve lines with internal scrolling, and shrinks back to `55.6 px` after deletion.

### Iteration 10

- Earlier findings: the directory header still used a 58 px track, the account footer used 52 px plus 36 px of combined fade/spacer reservation, and the mismatch made the area above `作者` look deeper than the area below it. The rejected-Proposal action also depended on an in-memory full-document ref, so it survived neither reload nor HMR and could silently do nothing.
- Fixes: unified the two left fixed bars at 40 px, gave each a 1 px low-contrast divider and an 8 px outward fade, and reduced the chapter scroller's reserved bottom space to the fade depth only. Replaced the full-document ref with a versioned rejected-revision record containing only candidate blocks and adjacent placement anchors. Reopening validates those anchors, inserts only the candidate as a new revision, and projects a durable conflict state instead of guessing or returning silently.
- Post-fix evidence: `artifacts/design-qa-fixed-chrome-left-rail-comparison.png` shows the balanced 40 px bars and compact directory boundary. `artifacts/prototype-fixed-chrome-reopen.png` records the reopened `r4` state. Chrome interaction checks verified reject → reload → reopen end to end, and the four proposal-revision integration tests cover preserved authority edits, moved end anchors, and missing anchors.

### Iteration 11

- Earlier finding: the implementation was in the approved warm-neutral palette family, but the Proposal, author message, and run summary all reused `#f5f4f2`. Repeating that one surface over large areas made the browser rendering feel more uniformly yellow than the approved asset. The assistant panel and active chapter were also one to two RGB steps warmer than their sampled source regions.
- Fixes: preserved the aligned canvas and tree panel; split the shared surface into semantic Proposal, author-message, and run-summary tokens; calibrated those tokens to the sampled source values; aligned the assistant panel and active chapter; and updated the transparent fade endpoints to derive from the new assistant-panel color. No layout, typography, sizing, or interaction rules changed.
- Post-fix evidence: `artifacts/prototype-color-calibration.png`, `artifacts/design-qa-color-calibration-comparison.png`, and `artifacts/design-qa-color-calibration-focused.png` show the restored subtle surface hierarchy and reduced yellow cast. A full reload plus collapse/expand interaction check passed, and the focused comparison contains no actionable P0, P1, or P2 color mismatch.

## Findings

No actionable P0, P1, or P2 findings remain.

## Follow-up polish

- P3: the browser's Noto Serif SC appears slightly lighter than the generated mock's literary serif.
- P3: the Phosphor split-panel icon is the closest library match to the macOS-style reference glyph.

## Implementation checklist

- [x] Approved three-column source recreated at the target viewport.
- [x] Core visible interactions are functional.
- [x] Proposal behavior scenarios are inspectable without adding debug chrome to the default workspace.
- [x] Chapter navigation scrolls independently without covering the fixed bottom utilities.
- [x] Expanded and collapsed writing-assistant states match the latest user direction.
- [x] Browser screenshot, focused comparisons, interaction evidence, and console check are recorded.

final result: passed
