# Fixed Workspace Shell Prototype

> Disposable Wayfinder prototype for **Prototype the Fixed Workspace Shell and Dynamic Surface Boundary**. This is not the production frontend or a runtime architecture.

## Question

Does the confirmed writing workspace remain coherent when editor Proposal, ordinary Agent conversation, transcript MCP App, optional Project Instruction, and separate Eval ownership are shown together—and which Host controls must remain fixed?

## Conclusion

Yes. The confirmed three-column workspace remains coherent without a new Run bar or control mode. The minimum fixed Host shell is project/tree navigation, project settings, the collapsible writing-assistant boundary, and the ordinary composer.

Run interruption follows the established Codex composer behavior selected by the author: while a Run is active and the composer is empty, the send arrow becomes a square pause button in the same location. Typing during the Run changes it back to the send arrow so the author can add guidance. Run state remains part of the Transcript; it is not duplicated in the editor or writing-assistant header.

## Run

```bash
npm run dev
```

Open the printed local URL.

## Review path

1. Select another chapter and return to `第十二章 雨夜`.
2. Edit Proposal prose, then try accept or reject.
3. Send a normal Agent message. While it runs, confirm that the empty composer shows the square pause button; type guidance to restore the send arrow; clear the text and pause the Run.
4. In the transcript MCP App, use `交给写作助手`, close it to its static fallback, and reopen it.
5. Use the project menu to open optional Project Instruction and the separate Eval page.
6. Collapse and reopen the writing-assistant column.

The prototype holds all state in memory. It intentionally does not implement Rust, PostgreSQL, accounts, persistence, auth, model calls, Tool/MCP execution, or the already-validated Tiptap and App-host contracts.
