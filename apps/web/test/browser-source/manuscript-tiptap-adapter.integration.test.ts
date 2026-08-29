import { Editor } from "@tiptap/core";
import { afterEach, expect, it } from "vitest";

import { manuscriptJson, paragraphUtf16 } from "../../src/manuscript-doc.ts";
import { storyosManuscriptExtensions } from "../../src/manuscript-tiptap-adapter.ts";

const BLOCK_ID = "11111111-1111-4111-8111-111111111111";

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
