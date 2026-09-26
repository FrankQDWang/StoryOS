import { Editor } from "@tiptap/core";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { Fragment, Slice } from "@tiptap/pm/model";
import { afterEach, expect, it } from "vitest";

import {
  type CapturedManuscriptEdit,
  manuscriptBlocksJson,
  manuscriptJson,
  paragraphUtf16,
} from "../../src/manuscript-doc.ts";
import {
  capturedManuscriptEditFromTransaction,
  isStoryosHydrateTransaction,
  storyosManuscriptExtensions,
  storyosEditorProps,
} from "../../src/manuscript-tiptap-adapter.ts";

const BLOCK_ID = "11111111-1111-4111-8111-111111111111";
const RIGHT_ID = "22222222-2222-4222-8222-222222222222";

let editor: Editor | undefined;

afterEach(() => {
  editor?.destroy();
  editor = undefined;
  document.body.replaceChildren();
});

it("refuses an unsupported Block document and keeps the previous text", () => {
  const host = document.createElement("div");
  document.body.append(host);
  editor = new Editor({
    element: host,
    extensions: storyosManuscriptExtensions(BLOCK_ID),
    content: manuscriptJson(BLOCK_ID, "Hello"),
    injectCSS: false,
  });
  const before = editor.state.doc;
  editor.view.dispatch(editor.state.tr.setNodeMarkup(0, undefined, { id: "other-block" }));
  expect(editor.state.doc.eq(before)).toBe(true);
  expect(paragraphUtf16(editor.state.doc)).toBe("Hello");
  expect(editor.state.doc.firstChild?.attrs.id).toBe(BLOCK_ID);

  editor.commands.splitBlock();
  expect(editor.state.doc.childCount).toBe(1);
  expect(paragraphUtf16(editor.state.doc)).toBe("Hello");
});

function pressKey(editor: Editor, key: string, shiftKey = false): boolean {
  const event = new KeyboardEvent("keydown", { key, shiftKey, bubbles: true, cancelable: true });
  return editor.view.someProp("handleKeyDown", (handler) => handler(editor.view, event)) === true;
}

it("splits on Enter and keeps the starting fragment identity", () => {
  const host = document.createElement("div");
  document.body.append(host);
  editor = new Editor({
    element: host,
    extensions: storyosManuscriptExtensions(BLOCK_ID),
    content: manuscriptJson(BLOCK_ID, "HelloWorld"),
    injectCSS: false,
  });
  editor.commands.setTextSelection(6);
  expect(pressKey(editor, "Enter")).toBe(true);
  expect(editor.state.doc.childCount).toBe(2);
  expect(editor.state.doc.firstChild?.attrs.id).toBe(BLOCK_ID);
  expect(editor.state.doc.firstChild?.textContent).toBe("Hello");
  expect(editor.state.doc.lastChild?.textContent).toBe("World");
  const rightId = editor.state.doc.lastChild?.attrs.id;
  expect(typeof rightId).toBe("string");
  expect(rightId).not.toBe(BLOCK_ID);
  expect(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(rightId))
    .toBe(true);
});

it("inserts a line break in the same paragraph on Shift-Enter", () => {
  const host = document.createElement("div");
  document.body.append(host);
  editor = new Editor({
    element: host,
    extensions: storyosManuscriptExtensions(BLOCK_ID),
    content: manuscriptJson(BLOCK_ID, "HelloWorld"),
    injectCSS: false,
  });
  editor.commands.setTextSelection(6);
  expect(pressKey(editor, "Enter", true)).toBe(true);
  expect(editor.state.doc.childCount).toBe(1);
  expect(editor.state.doc.firstChild?.attrs.id).toBe(BLOCK_ID);
  expect(editor.state.doc.firstChild?.textContent).toBe("Hello\nWorld");
});

it("deletes a nonempty selection then splits on Enter", () => {
  const host = document.createElement("div");
  document.body.append(host);
  editor = new Editor({
    element: host,
    extensions: storyosManuscriptExtensions(BLOCK_ID),
    content: manuscriptJson(BLOCK_ID, "HelloWorld"),
    injectCSS: false,
  });
  editor.commands.setTextSelection({ from: 6, to: 11 });
  expect(pressKey(editor, "Enter")).toBe(true);
  expect(editor.state.doc.childCount).toBe(2);
  expect(editor.state.doc.firstChild?.attrs.id).toBe(BLOCK_ID);
  expect(editor.state.doc.firstChild?.textContent).toBe("Hello");
  expect(editor.state.doc.lastChild?.textContent).toBe("");
});

it("joins adjacent paragraphs on Backspace at the start of the following paragraph", () => {
  const host = document.createElement("div");
  document.body.append(host);
  editor = new Editor({
    element: host,
    extensions: storyosManuscriptExtensions(BLOCK_ID),
    content: manuscriptBlocksJson([
      { manuscript_block_id: BLOCK_ID, text: "Hello" },
      { manuscript_block_id: RIGHT_ID, text: "World" },
    ]),
    injectCSS: false,
  });
  editor.commands.setTextSelection(8);
  expect(pressKey(editor, "Backspace")).toBe(true);
  expect(editor.state.doc.childCount).toBe(1);
  expect(editor.state.doc.firstChild?.attrs.id).toBe(BLOCK_ID);
  expect(editor.state.doc.firstChild?.textContent).toBe("HelloWorld");
});

it("carries the validated replacement on an accepted document transaction", () => {
  const host = document.createElement("div");
  document.body.append(host);
  let carried: CapturedManuscriptEdit | undefined;
  editor = new Editor({
    element: host,
    extensions: storyosManuscriptExtensions(BLOCK_ID),
    content: manuscriptJson(BLOCK_ID, "Hello"),
    injectCSS: false,
    onTransaction({ transaction }) {
      if (isStoryosHydrateTransaction(transaction) || !transaction.docChanged) return;
      carried = capturedManuscriptEditFromTransaction(transaction);
    },
  });
  editor.commands.setTextSelection(6);
  editor.view.dispatch(editor.state.tr.insertText("!"));
  expect(paragraphUtf16(editor.state.doc)).toBe("Hello!");
  expect(carried?.kind).toBe("replace_block_selection");
  expect(carried?.resultingBody).toBe("Hello!");
  if (carried?.kind === "replace_block_selection") {
    expect(carried.manuscript_block_id).toBe(BLOCK_ID);
    expect(carried.from).toBe(5);
    expect(carried.to).toBe(5);
    expect(carried.text).toBe("!");
  }
});

it("captures a complete backward mixed selection without changing either durable projection", async () => {
  const host = document.createElement("div");
  document.body.append(host);
  let captured: unknown;
  editor = new Editor({ element: host, injectCSS: false,
    extensions: storyosManuscriptExtensions(BLOCK_ID, undefined, () => true),
    editorProps: storyosEditorProps(BLOCK_ID),
    content: { type: "doc", content: [
      { type: "heading", attrs: { id: BLOCK_ID, level: 1 }, content: [{ type: "text", text: "Hello" }] },
      { type: "blockProposal", attrs: { proposalId: BLOCK_ID, operationId: RIGHT_ID,
        revisionId: RIGHT_ID, blockId: BLOCK_ID, eligible: true, expectedHeads: [RIGHT_ID] },
        content: [{ type: "text", text: "Candidate" }] },
      { type: "heading", attrs: { id: RIGHT_ID, level: 1 }, content: [{ type: "text", text: "World" }] },
    ] },
    onTransaction({ transaction }) { captured = transaction.getMeta("storyos.structuredEdit"); },
  });
  editor.commands.setTextSelection({ from: 21, to: 2 });
  const before = editor.state.doc;
  const transaction = editor.state.tr.insertText("New");
  editor.view.dispatch(transaction);
  expect(captured).toEqual({ kind: "structured_selection", expectedProposalHeads: [RIGHT_ID],
    authorEditUnit: { normalized_primitives: [{ kind: "replace_structured_selection",
      replacement: [{ block_kind: "heading", text: "New" }] }],
    selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 2, to: 1,
      ordered_selection: { anchor: { source_index: 2, source_offset: 2 },
        head: { source_index: 0, source_offset: 1 }, sources: [
          { owner: { kind: "manuscript", manuscript_block_id: BLOCK_ID },
            coordinate_profile: "prosemirror-token-utf16.v1", from: 1, to: 5,
            block_kind: "heading", source_text: "Hello" },
          { owner: { kind: "proposal", proposal_id: BLOCK_ID, operation_id: RIGHT_ID,
            revision_id: RIGHT_ID, manuscript_block_id: BLOCK_ID },
            coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 9,
            block_kind: "heading", source_text: "Candidate" },
          { owner: { kind: "manuscript", manuscript_block_id: RIGHT_ID },
            coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 2,
            block_kind: "heading", source_text: "World" },
        ] } } } });
  expect(editor.state.doc.eq(before)).toBe(true);
  const expected = captured;
  captured = undefined;
  editor.view.focus();
  await applyTrustedInput({ operation: "insert_text", text: "New" });
  expect(captured).toEqual(expected);
  expect(editor.state.doc.eq(before)).toBe(true);
});

it("retains paragraph boundaries from a real multi-line mixed paste", () => {
  const host = document.createElement("div");
  document.body.append(host);
  let captured: unknown;
  editor = new Editor({ element: host, injectCSS: false,
    extensions: storyosManuscriptExtensions(BLOCK_ID, undefined, () => true),
    editorProps: storyosEditorProps(BLOCK_ID),
    content: { type: "doc", content: [
      { type: "paragraph", attrs: { id: BLOCK_ID }, content: [{ type: "text", text: "Hello" }] },
      { type: "blockProposal", attrs: { proposalId: BLOCK_ID, operationId: RIGHT_ID,
        revisionId: RIGHT_ID, blockId: BLOCK_ID, eligible: true, expectedHeads: [RIGHT_ID] },
        content: [{ type: "text", text: "Candidate" }] },
    ] }, onTransaction({ transaction }) { captured = transaction.getMeta("storyos.structuredEdit"); },
  });
  editor.commands.setTextSelection({ from: 2, to: 10 });
  const before = editor.state.doc;
  const clipboardData = new DataTransfer();
  clipboardData.setData("text/plain", "First\r\nSecond");
  editor.view.dom.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, cancelable: true, clipboardData }));
  expect(captured).toMatchObject({ authorEditUnit: { normalized_primitives: [
    { kind: "replace_structured_selection", replacement: [
      { block_kind: "paragraph", text: "First" }, { block_kind: "paragraph", text: "Second" },
    ] },
  ] } });
  expect(editor.state.doc.eq(before)).toBe(true);
  const structured = new Slice(Fragment.fromArray([
    editor.schema.nodes.heading!.create({ level: 1 }, editor.schema.text("Heading")),
    editor.schema.nodes.paragraph!.create(null, editor.schema.text("Tail")),
  ]), 1, 1);
  editor.view.dispatch(editor.state.tr.replaceSelection(structured));
  expect(captured).toMatchObject({ authorEditUnit: { normalized_primitives: [
    { kind: "replace_structured_selection", replacement: [
      { block_kind: "paragraph", text: "Heading" }, { block_kind: "paragraph", text: "Tail" },
    ] },
  ] } });
  expect(editor.state.doc.eq(before)).toBe(true);
  editor.view.dispatch(editor.state.tr.deleteSelection().setMeta("storyos.origin", "cut"));
  expect(captured).toMatchObject({ authorEditUnit: { normalized_primitives: [
    { kind: "replace_structured_selection", replacement: [{ block_kind: "paragraph", text: "" }] },
  ] } });
  expect(editor.state.doc.eq(before)).toBe(true);
  expect(pressKey(editor, "Enter")).toBe(true);
  expect(captured).toMatchObject({ authorEditUnit: { normalized_primitives: [
    { kind: "replace_structured_selection", replacement: [
      { block_kind: "paragraph", text: "" }, { block_kind: "paragraph", text: "" },
    ] },
  ] } });
  expect(editor.state.doc.eq(before)).toBe(true);
});
