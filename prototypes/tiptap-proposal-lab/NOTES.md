# Tiptap Proposal Lab

> Disposable prototype. Delete or absorb it after the Wayfinder decision is recorded.

## Question

Can a rough Tiptap implementation demonstrate the settled editable-Proposal interaction while preserving the approved three-column StoryOS writing experience?

## Run

```bash
npm run dev
```

## What this pass demonstrates

- a fixed volume → chapter tree, manuscript editor, and collapsible Agent transcript;
- an ordinary Tiptap review projection whose Proposal paragraphs remain directly editable;
- accept, reject, reopen-after-reject, and safe undo-accept interactions;
- Proposal edits creating a new revision and resetting validation;
- Agent stream transactions excluded from ProseMirror history;
- synchronous author-priority stream pausing on `compositionstart`, `beforeinput`, paste, drop, and cut;
- whole refusal of mixed authoritative/Proposal transactions;
- local scratch persistence and reload of the document, Proposal axes, and transcript;
- a hidden prototype harness behind the lower-left settings control for stream, conflict, reload, and reset scenarios.

## Package baseline

- Tiptap React / StarterKit / UniqueID: `3.27.3`
- React: `19.2.0`
- Vite: `6.4.2`

## Still requires human/browser evidence

- real Chinese Pinyin and Japanese IME on Chrome, Safari, and Firefox on macOS;
- DOM recovery for every refused paste, drop, cut, delete, and composition shape;
- full cross-stack `Mod-z` / native `historyUndo` ordering;
- all stable block-ID split, join, copy, paste, drag, undo, and redo cases;
- same-block disjoint inline-Proposal policy and multi-operation block reshaping;
- crash-window reconciliation against a real durable Core boundary.
