import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyTrustedInput,
  updateClientSessionCookie,
  updateClipboardPermission,
} from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  closeApplicationFrame,
  destroyApplicationFrame,
  openApplicationFrame,
  openChapterEditor,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  await updateClipboardPermission({ action: "clear" });
  document.body.replaceChildren();
});

async function readRevision(frame: HTMLIFrameElement, projectId: string, chapterId: string) {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  return chapter.chapter.current_revision;
}

function focusBlock(
  editor: HTMLElement,
  realm: Window & typeof globalThis,
  index: number,
): void {
  const block = editor.querySelectorAll("p, h1")[index];
  if (block === undefined) throw new Error("the Block is missing");
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.setStart(block, 0);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

it("moves and retypes Blocks with stable identity and refuses copy as a move", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor(
    "StoryOS exact-dist move and retype",
    "Move Retype Novel",
  );
  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const before = await readRevision(frame, projectId, chapterId);
  const leftId = before.blocks[0]?.manuscript_block_id;
  if (before.blocks.length !== 1 || leftId === undefined) {
    throw new Error("the starting Block is missing");
  }

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Hello" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello");
  await waitSaved(root, before.revision_id);
  const helloRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  await applyTrustedInput({ operation: "enter" });
  await expect.poll(() => editor.querySelectorAll("p, h1").length, { timeout: 10_000 }).toBe(2);
  await waitSaved(root, helloRevisionId);
  const splitRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  await applyTrustedInput({ operation: "insert_text", text: "World" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello\nWorld");
  await waitSaved(root, splitRevisionId);
  const split = await readRevision(frame, projectId, chapterId);
  const rightId = split.blocks[1]?.manuscript_block_id;
  expect(split.blocks.map((block) => block.manuscript_block_id)).toEqual([leftId, rightId]);
  expect(rightId).toMatch(UUID);
  expect(rightId).not.toBe(leftId);

  focusBlock(editor, childWindow, 1);
  await applyTrustedInput({ operation: "move_block_up" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("World\nHello");
  await waitSaved(root, split.revision_id);
  const moved = await readRevision(frame, projectId, chapterId);
  expect(moved.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "paragraph", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "Hello" },
  ]);

  await updateClipboardPermission({ action: "grant" });
  await childWindow.navigator.clipboard.writeText("Copied\nExtra");
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 })
    .toBe("World\nHelloCopied\nExtra");
  await waitSaved(root, moved.revision_id);
  const pasted = await readRevision(frame, projectId, chapterId);
  const copiedId = pasted.blocks[2]?.manuscript_block_id;
  expect(pasted.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "paragraph", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "HelloCopied" },
    { manuscript_block_id: copiedId, block_kind: "paragraph", text: "Extra" },
  ]);
  expect(copiedId).toMatch(UUID);
  expect(copiedId).not.toBe(leftId);
  expect(copiedId).not.toBe(rightId);

  focusBlock(editor, childWindow, 0);
  await applyTrustedInput({ operation: "retype_block" });
  await expect.poll(() => editor.getAttribute("data-manuscript-block-kinds"), { timeout: 10_000 })
    .toBe("heading paragraph paragraph");
  await waitSaved(root, pasted.revision_id);
  const retyped = await readRevision(frame, projectId, chapterId);
  expect(retyped.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "heading", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "HelloCopied" },
    { manuscript_block_id: copiedId, block_kind: "paragraph", text: "Extra" },
  ]);

  await destroyApplicationFrame(frame);
  const reopened = await openApplicationFrame(
    "StoryOS exact-dist move and retype reload",
    `/projects/${projectId}`,
  );
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedEditor = manuscriptEditor(appRoot(reopened), applicationWindow(reopened));
  await expect.poll(() => reopenedEditor.getAttribute("data-manuscript-block-kinds"))
    .toBe("heading paragraph paragraph");
  expect(manuscriptBody(reopenedEditor)).toBe("World\nHelloCopied\nExtra");
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks).toEqual([
    { manuscript_block_id: rightId, block_kind: "heading", text: "World" },
    { manuscript_block_id: leftId, block_kind: "paragraph", text: "HelloCopied" },
    { manuscript_block_id: copiedId, block_kind: "paragraph", text: "Extra" },
  ]);
});
