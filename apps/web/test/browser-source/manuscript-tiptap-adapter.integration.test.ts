import { Editor } from "@tiptap/core";
import { afterEach, expect, it } from "vitest";

import { manuscriptBlocksJson, manuscriptJson, paragraphUtf16 } from "../../src/manuscript-doc.ts";
import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";

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

function pressKey(editor: Editor, key: string): boolean {
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
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
