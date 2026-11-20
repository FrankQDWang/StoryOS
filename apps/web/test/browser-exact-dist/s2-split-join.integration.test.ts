import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
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

function focusParagraphStart(
  editor: HTMLElement,
  realm: Window & typeof globalThis,
  index: number,
): void {
  const paragraph = editor.querySelectorAll("p")[index];
  if (paragraph === undefined) throw new Error("the paragraph is missing");
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.setStart(paragraph, 0);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

it("splits and joins adjacent Blocks and reopens the same identities", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor(
    "StoryOS exact-dist split and join",
    "Split Join Novel",
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
  await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(2);
  await waitSaved(root, helloRevisionId);
  const splitRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  await applyTrustedInput({ operation: "insert_text", text: "World" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello\nWorld");
  await waitSaved(root, splitRevisionId);

  const split = await readRevision(frame, projectId, chapterId);
  expect(split.blocks.map((block) => block.text)).toEqual(["Hello", "World"]);
  expect(split.blocks[0]?.manuscript_block_id).toBe(leftId);
  const rightId = split.blocks[1]?.manuscript_block_id;
  expect(rightId).toMatch(UUID);
  expect(rightId).not.toBe(leftId);

  await destroyApplicationFrame(frame);
  const reopened = await openApplicationFrame(
    "StoryOS exact-dist split and join reload",
    `/projects/${projectId}`,
  );
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks.map((block) => ({
    manuscript_block_id: block.manuscript_block_id,
    text: block.text,
  }))).toEqual([
    { manuscript_block_id: leftId, text: "Hello" },
    { manuscript_block_id: rightId, text: "World" },
  ]);

  const reopenedRoot = appRoot(reopened);
  const reopenedEditor = manuscriptEditor(reopenedRoot, applicationWindow(reopened));
  const reopenedRevisionId = reopenedRoot.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  focusParagraphStart(reopenedEditor, applicationWindow(reopened), 1);
  await applyTrustedInput({ operation: "backspace" });
  await expect.poll(() => reopenedEditor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(1);
  await expect.poll(() => manuscriptBody(reopenedEditor), { timeout: 10_000 }).toBe("HelloWorld");
  await waitSaved(reopenedRoot, reopenedRevisionId);

  const joined = await readRevision(reopened, projectId, chapterId);
  expect(joined.blocks).toEqual([{
    manuscript_block_id: leftId,
    block_kind: "paragraph",
    text: "HelloWorld",
  }]);

  await destroyApplicationFrame(reopened);
  const finalFrame = await openApplicationFrame(
    "StoryOS exact-dist split and join final reload",
    `/projects/${projectId}`,
  );
  await expect.poll(() => {
    const nextRoot = finalFrame.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const finalRevision = await readRevision(finalFrame, projectId, chapterId);
  expect(finalRevision.blocks).toEqual([{
    manuscript_block_id: leftId,
    block_kind: "paragraph",
    text: "HelloWorld",
  }]);
});
