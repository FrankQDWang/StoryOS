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
