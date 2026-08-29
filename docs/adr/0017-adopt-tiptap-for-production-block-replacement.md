---
status: accepted
---

# Adopt Tiptap and ProseMirror for One Production Block Replacement

This decision records the production Protected Web Client editor adoption required by `S2-REQ-009`. It supersedes the TipTap exclusion in ADR 0014. ADR 0014 remains the historical record of the first pnpm, Vite, Node, and React toolchain slice. ADR 0016 remains the production host and Trusted Types record.

## Decision

The production manuscript surface uses Tiptap 3.27.3 and the ProseMirror packages re-exported by `@tiptap/pm` 3.27.3. StoryOS owns the adapter. Tiptap is never Authoritative State. One visible paragraph Manuscript Block is the supported write shape for this slice. Author input becomes one complete `ReplaceBlockSelection` intent, then settles through the existing Local Edit Journal, Admission, Core, and PostgreSQL path.

The exact direct production package graph is:

- `@tiptap/core` 3.27.3
- `@tiptap/react` 3.27.3
- `@tiptap/pm` 3.27.3
- `@tiptap/extension-document` 3.27.3
- `@tiptap/extension-paragraph` 3.27.3
- `@tiptap/extension-text` 3.27.3
- `@tiptap/extension-unique-id` 3.27.3

`@tiptap/pm` 3.27.3 re-exports ProseMirror, including `prosemirror-model` 1.25.11, `prosemirror-state` 1.4.4, `prosemirror-view` 1.42.3, and `prosemirror-transform` 1.12.0. UniqueID depends on `uuid` 14.0.2. All of these packages use the MIT license. StoryOS does not import StarterKit, History, marks, or list extensions in this slice.

UniqueID declares the paragraph `id` attribute. Core owns the Manuscript Block identity. The adapter writes that identity into the document and sets UniqueID `updateDocument` to `false`, so UniqueID does not mint a new id.

The adapter captures only a complete supported replacement. Hydration, Snapshot installation, decoration, and other non-edit transactions create no author intent. An unsupported transaction is refused. The editor document stays at the previous supported state, and recovery material is not partially edited.

The production page retires the textarea write path. Paste and cut use `text/plain` only, so the existing Trusted Types policy without a default policy remains in force. Enter inserts U+000A in the same paragraph Block. Full IME product behavior, split, join, move, retype, and Undo remain later tickets.

This decision does not change `web_client_contract_revision`, IndexedDB meaning, or the Apply Author Edit command.

## Considered options

- Keeping the production textarea was rejected because `S2-EVD-009` says textarea-only evidence cannot satisfy `S2-REQ-009`.
- Promoting `prototypes/tiptap-proposal-lab` was rejected because that tree is Prototype Evidence, not the Protected Web Client.
- Adopting StarterKit was rejected because this slice supports one paragraph Block replacement, not headings, lists, marks, or editor History.
- Allowing UniqueID to generate ids was rejected because Manuscript Block identity is Core-owned and must survive settlement and reload.

## Consequences

- Production exact-dist and production-host journeys bind to `[data-manuscript-editor]`, not `textarea`.
- Later IME, structure, and Undo tickets extend this adapter. They must not introduce a second write path.
- A Tiptap or ProseMirror major upgrade is a new adoption review. It must re-pin this graph and re-run the production replacement journey.
