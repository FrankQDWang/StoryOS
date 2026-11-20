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
} from "../support/exact-dist-frame.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

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
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }).toBe(true);

  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  expect(root.querySelector(".editor-panel textarea")).toBeNull();
  const editor = manuscriptEditor(root, childWindow);
  expect(manuscriptBody(editor)).toBe("");
  expect(editor.getAttribute("contenteditable")).toBe("true");
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count")
  ).toBe("0");
  await expect.poll(() => {
    const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
    const chapterId = root.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    )?.getAttribute("data-chapter-id");
    return typeof projectId === "string"
      && UUID.test(projectId)
      && typeof chapterId === "string"
      && UUID.test(chapterId);
  }, { timeout: 10_000 }).toBe(true);

  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterId = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
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
