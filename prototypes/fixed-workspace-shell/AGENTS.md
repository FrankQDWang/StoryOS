# Prototype Instructions

Run the local server yourself and open the preview in the browser available to this environment. Do not give the user server-start instructions when you can run it.

Before making substantial visual changes, use the Product Design plugin's `get-context` skill when the visual source is unclear or no longer matches the current goal. When the user gives durable prototype-specific design feedback, preferences, or decisions, record them in `AGENTS.md`.

When implementing from a selected generated mock, treat that image as the source of truth for layout, component anatomy, density, spacing, color, typography, visible content, and hierarchy.

## StoryOS issue 55 direction

- The confirmed visual source is the current `../tiptap-proposal-lab` implementation and its latest browser evidence, not the earlier `../../docs/design/storyos-three-column-writing-workspace.png` mock by itself.
- Preserve the confirmed 4173 workspace: fixed volume/chapter tree, center editor, 44 px collapsed writing-assistant rail, compact fixed bars, account/settings footer, and content-sized composer.
- Do not add fixed Run controls to the editor, writing-assistant header, or Transcript row. Match the author-selected Codex composer behavior: an empty composer shows a square pause button during a Run, while typing restores the send arrow for additional guidance.
- Keep Proposal interaction in the editor, MCP Apps inside the transcript, Eval on a separate page, and optional Project Instruction in project settings.
- Do not expose ContextManifest, Pin, role tables, internal state grids, debug controls, login, billing, teams, deployment choices, or story outlines in the author-facing shell.
