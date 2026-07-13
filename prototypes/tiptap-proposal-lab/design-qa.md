# Design QA — StoryOS Tiptap Proposal Lab

- Source visual truth: `/Users/frankqdwang/MLE/StoryOS/docs/design/storyos-three-column-writing-workspace.png`
- Implementation screenshot: `/Users/frankqdwang/MLE/StoryOS/prototypes/tiptap-proposal-lab/artifacts/prototype-default.png`
- Viewport: `1487 × 1058`
- State: default desktop workspace; volume two expanded; chapter twelve active; editable Proposal ready and valid; Agent panel expanded.

## Full-view comparison evidence

`artifacts/design-qa-comparison.png` places the approved source on the left and the browser-rendered implementation on the right at the same viewport and state.

The final comparison preserves the source's three major regions, volume-to-chapter hierarchy, manuscript measure, warm neutral palette, low-chrome density, Proposal placement, Agent transcript structure, and bottom composer. No actionable P0, P1, or P2 differences remain.

## Focused comparison evidence

- `artifacts/design-qa-proposal-comparison.png` compares the manuscript and editable Proposal regions. Heading hierarchy, paragraph measure, Proposal background, vertical marker, reading order, and accept/reject placement align with the source.
- `artifacts/design-qa-agent-comparison.png` compares the transcript, run summary, and composer. Panel proportions, message hierarchy, card treatment, spacing, and composer anchoring align with the source.

## Required fidelity surfaces

- Fonts and typography: Noto Serif SC and Noto Sans SC provide a close, legally reusable approximation of the raster source. Hierarchy, weight, line height, wrapping, and Chinese text density are aligned. The live browser font renders marginally lighter than the generated source; this is P3 polish, not a structural mismatch.
- Spacing and layout rhythm: the final grid uses the source proportions at 1487 × 1058. The manuscript starts at the same horizontal measure, the Proposal follows the affected paragraph, and the transcript/run/composer rhythm matches.
- Colors and visual tokens: warm off-white canvas, charcoal text, warm-gray Proposal surface, subtle separators, and neutral active states match the approved direction without gradients or dashboard chrome.
- Image quality and asset fidelity: the target contains no raster illustrations, logos, or custom imagery that require separate assets. Phosphor supplies the visible UI icons; no handcrafted SVG, CSS icon, emoji, or placeholder asset is used.
- Copy and content: the chapter tree, manuscript, author request, Agent response, run summary, and accept/reject actions reproduce the approved target's content and intent. The implementation intentionally shows Proposal revision `r3` instead of the mock's elapsed duration because it exposes real prototype state.

## Browser interaction evidence

- Volume/chapter navigation: chapter thirteen opens without Proposal controls; chapter twelve restores its document and Proposal.
- Agent panel: collapse and expand both work.
- Proposal editing: author input at the Proposal end created revision `r4`, showed pending validation, then returned to valid.
- Acceptance: accepting removed Proposal controls and exposed a transcript action; undoing acceptance reopened the same content through a new revision.
- Rejection: rejecting removed only candidate content; reopening restored it and revalidated the new revision.
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

## Findings

No actionable P0, P1, or P2 findings remain.

## Follow-up polish

- P3: the browser's Noto Serif SC appears slightly lighter than the generated mock's literary serif.
- P3: the source uses an abstract collapse glyph; the implementation uses the closest matching Phosphor chevron.

## Implementation checklist

- [x] Approved three-column source recreated at the target viewport.
- [x] Core visible interactions are functional.
- [x] Proposal behavior scenarios are inspectable without adding debug chrome to the default workspace.
- [x] Browser screenshot, focused comparisons, interaction evidence, and console check are recorded.

final result: passed
