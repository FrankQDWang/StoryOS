# Design QA — Transcript-embedded MCP App host

Result: **passed**

## Visual truth

- Approved StoryOS workspace: `/Users/frankqdwang/.codex/state/plugins/product-design/assets/storyos-approved-three-column-writing-workspace.png`
- Official Codex task hierarchy reference: `/Users/frankqdwang/.codex/visualizations/2026/07/16/019f68a6-7a38-7cc2-a5b6-ef927c09c6bc/storyos-mcp-app-redesign/source-codex-task.png`
- StoryOS design note: `docs/design/storyos-three-column-writing-workspace.md`

The StoryOS source fixes the three-column writing workspace, warm paper palette,
serif manuscript, and proposal-in-editor boundary. The Codex reference fixes the
Transcript hierarchy: the user message is the only bubble, assistant prose is
unboxed, work activity is lightweight, and only the operable artifact receives a
surface.

## Viewports and states

| Viewport | State | Evidence | Result |
| --- | --- | --- | --- |
| 1280x720 | ready App | `storyos-mcp-app-redesign/01-start.png` | passed |
| 1280x720 | Host Approval | `storyos-mcp-app-redesign/02-approval.png` | passed |
| 1280x720 | Proposal in editor | `storyos-mcp-app-redesign/03-proposal.png` | passed |
| 390x844 | compact static fallback | `storyos-mcp-app-redesign/04-mobile-fallback.png` | passed |
| 1280x720 | static fallback | `storyos-mcp-app-redesign/05-static-fallback.png` | passed |

The compact capture predates the final small-text contrast adjustment. The final
responsive source was rechecked after that adjustment: below 780px the manuscript
tree and editor are hidden, the Agent panel occupies the viewport, and the
desktop-only “查看提案” jump is hidden rather than pointing to an invisible editor.

## Same-input comparisons

- Workspace + official Transcript hierarchy + implementation:
  `storyos-mcp-app-redesign/comparison-start.png`
- Approved proposal boundary + implementation proposal boundary:
  `storyos-mcp-app-redesign/comparison-proposal.png`

The full 1280px comparisons keep the important typography, column proportions,
surface hierarchy, and proposal placement readable, so no additional crop was
needed.

## Fidelity review

- Typography: serif manuscript and headings; restrained sans-serif system text;
  passed.
- Spacing and layout: fixed manuscript tree, readable editor measure, wider Agent
  panel for the embedded App, and composer anchored below the conversation; passed.
- Colors: warm off-white shell, charcoal primary action, muted gray supporting
  text, and no decorative accent color; passed.
- Imagery and assets: this workflow has no product imagery or icon asset slot;
  no placeholder, emoji, CSS drawing, or fabricated asset was introduced; passed.
- Copy and hierarchy: one user request, one assistant response, one activity line,
  one App surface, one Host Approval, and one editor Proposal; passed.

## Interaction review

- App action creates a Host-owned Approval instead of changing StoryOS state.
- Rejection leaves the manuscript unchanged.
- A rejected action can be requested again without being swallowed by a stale
  idempotency key.
- Approval creates a visible Proposal inside the editor; it does not write the
  manuscript.
- Resource drift produces a readable saved-result fallback with no active App
  action.

## Comparison history

1. The rejected version placed user, assistant, activity, App, Approval, and
   boundary notice into competing bordered surfaces; the Approval could also be
   obscured by the composer.
2. The redesign restored the approved StoryOS three-column shell, removed nested
   borders, made activity a single line, and reserved the only content surface for
   the App.
3. The Proposal now appears in the editor, while the Transcript only reports that
   handoff. Latest desktop, fallback, and compact evidence show no clipping or
   competing action surfaces.
