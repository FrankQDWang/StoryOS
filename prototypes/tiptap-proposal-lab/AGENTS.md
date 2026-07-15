# Prototype Instructions

Run the local server yourself and open the preview in the browser available to this environment. Do not give the user server-start instructions when you can run it.

Before making substantial visual changes, use the Product Design plugin's `get-context` skill when the visual source is unclear or no longer matches the current goal. When the user gives durable prototype-specific design feedback, preferences, or decisions, record them in `AGENTS.md`.

When implementing from a selected generated mock, treat that image as the source of truth for layout, component anatomy, density, spacing, color, typography, visible content, and hierarchy.

## StoryOS approved direction

- Treat `../../docs/design/storyos-three-column-writing-workspace.png` and its companion Markdown file as the fixed visual and interaction reference.
- Preserve a three-column desktop workspace: volume-to-chapter tree on the left, Tiptap manuscript editor in the center, and a collapsible normal Agent conversation panel on the right.
- Render replacement or continuation Proposal prose directly after the affected paragraph and keep it editable in place.
- Expose only accept and reject in the manuscript review surface. Do not add a visible diff view or a `查看差异` action.
- Keep technical scenario controls out of the default author-facing surface; any prototype-only inspection harness must stay behind the settings control.
- Keep the volume-to-chapter tree independently scrollable while its project header, directory header, and bottom utility bar remain fixed. The prototype inspection control belongs in that bottom utility bar and must never overlay chapter rows.
- Label the right conversation panel `写作助手`. When collapsed, render only the sidebar-toggle icon in the rail's upper-right corner, using the rounded split-panel icon style; do not keep a vertical text label in the rail.
- Keep the collapsed writing-assistant rail at 44 px. Render one 36 px workspace-level toggle centered within that rail's width and the compact 40 px writing-assistant header height, so it stays horizontally aligned with the title and keeps the same screen position in expanded and collapsed states.
- Preserve the approved asset's quiet, literary, low-chrome character in the implementation: neutral off-white surfaces, soft dividers and selection states, a pale Proposal treatment, spacious prose rhythm, and a compact composer. Avoid stronger card fills, borders, or dashboard-like component emphasis.
- Keep the large warm-gray surfaces subtly distinct instead of collapsing them into one shared fill: the Proposal, author message, and run summary each need their own calibrated surface token so repeated cards do not create a uniform yellow cast.
- Keep the left footer compact and Codex-like: show an account identity placeholder on the left and settings on the right, with no search action. Keep the writing-assistant composer compact at the bottom, use a soft surrounding fade, and render the add/send controls as small, symmetric circles.
- Use the same 40 px fixed-bar height for the left directory header and account footer. Separate each from the chapter scroller with a low-contrast 1 px divider and an 8 px outward fade; the scroller reserves only that fade depth, with no extra spacer.
- Separate the fixed writing-assistant header and left footer from their scrolling content with the same subtle divider and an outward soft fade. Let the composer fade overlap the transcript boundary instead of consuming its own grid space, and reserve matching scroll padding so content never clips beneath any fade.
- Let the writing-assistant textarea size itself from its content: two visible lines at rest, natural growth and shrinkage through ten lines, then internal scrolling. Keep the composer bottom-anchored and do not add imperative `scrollHeight` sizing or a compatibility fallback unless browser support requirements change explicitly.
- Do not render explicit `你` or `Agent` labels above conversation messages. Distinguish roles spatially: writing-assistant messages align left and author messages align right.
- Do not draw a focus outline around the full Tiptap manuscript root when the author clicks or edits prose; the caret and selection communicate editor focus, while buttons and text inputs retain visible keyboard focus treatment.
- Persist a rejected Proposal as its candidate blocks plus adjacent placement anchors, never as a restorable full-chapter snapshot. Reopening creates a new Proposal revision at those anchors while preserving later authoritative edits; missing or moved anchors become an explicit conflict instead of a guessed insertion or silent no-op.
