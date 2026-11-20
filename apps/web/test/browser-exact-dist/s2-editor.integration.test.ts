import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  closeApplicationFrame,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitOpenedChapter,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

async function readChapter(frame: HTMLIFrameElement, projectId: string, chapterId: string) {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  const revision = chapter.chapter.current_revision;
  const block = revision.blocks[0];
  if (revision.blocks.length !== 1 || block === undefined) {
    throw new Error("the Chapter Block is missing");
  }
  return {
    revisionId: revision.revision_id,
    body: revision.body,
    manuscriptBlockId: block.manuscript_block_id,
  };
}

it("hydrates production Tiptap for one paragraph Block without a textarea write path", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist Tiptap Block replacement");
  await submitProjectForm(frame, "Tiptap Novel");
  expect(appRoot(frame).querySelector(".editor-panel textarea")).toBeNull();
  expect(appRoot(frame).querySelector(MANUSCRIPT_EDITOR_SELECTOR)).toBeNull();

  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
  const { projectId, chapterId } = await waitOpenedChapter(frame);

  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  expect(root.querySelector(".editor-panel textarea")).toBeNull();
  const editor = manuscriptEditor(root, childWindow);
  expect(manuscriptBody(editor)).toBe("");
  expect(editor.getAttribute("contenteditable")).toBe("true");
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count")
  ).toBe("0");
  const before = await readChapter(frame, projectId, chapterId);
  expect(editor.getAttribute("data-manuscript-block-id")).toBe(before.manuscriptBlockId);
  expect(before.body).toBe("");
  expect(editor.querySelectorAll("p")).toHaveLength(1);
  const bold = new KeyboardEvent("keydown", { key: "b", ctrlKey: true, bubbles: true });
  editor.dispatchEvent(bold);
  expect(editor.querySelector("strong")).toBeNull();
  expect(editor.querySelector("b")).toBeNull();
  expect(manuscriptBody(editor)).toBe("");
  expect(root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
    .toBe("0");
});
