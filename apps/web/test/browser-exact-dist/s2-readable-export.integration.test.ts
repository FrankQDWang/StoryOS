import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { settleWorkerOnce, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  manuscriptEditor,
  manuscriptIsEditable,
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
  typeIntoCurrent,
} from "../support/exact-dist-frame.ts";

const CHAPTER_A = "Hello world";
const CHAPTER_B = "雨落在窗沿。";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

async function typeIntoEditableChapter(frame: HTMLIFrameElement, text: string): Promise<void> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return editor !== null && editor !== undefined && manuscriptIsEditable(editor);
  }, { timeout: 10_000 }).toBe(true);
  const realm = applicationWindow(frame);
  realm.focus();
  manuscriptEditor(appRoot(frame), realm).click();
  await typeIntoCurrent(frame, text);
}

async function requestExport(root: Element): Promise<void> {
  const previousId =
    root.querySelector("[data-export-id]")?.getAttribute("data-export-id") ?? "";
  const button = [...root.querySelectorAll<HTMLButtonElement>(
    "[data-readable-export] button",
  )].find((candidate) => candidate.textContent === "导出可读稿件");
  if (button === undefined) throw new Error("the readable export request button is missing");
  button.click();
  await expect.poll(() =>
    root.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"),
  ).toBe("in_progress");
  expect(root.querySelector("[data-export-id]")?.getAttribute("data-export-id")).toBeTruthy();
  await settleWorkerOnce();
  await expect.poll(() => {
    const panel = root.querySelector("[data-readable-export]");
    const exportId = root.querySelector("[data-export-id]")?.getAttribute("data-export-id") ?? "";
    if (
      panel?.getAttribute("data-export-outcome") !== "ready"
      || exportId.length === 0
      || exportId === previousId
    ) {
      return "";
    }
    return exportId;
  }, { timeout: 15_000 }).not.toBe("");
}

it("exports a durable human-readable manuscript through the Worker", { timeout: 120_000 }, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist readable export");
  await submitProjectForm(frame, "Readable Export Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");
  await typeIntoEditableChapter(frame, CHAPTER_A);
  const root = appRoot(frame);
  await requestExport(root);
  expect(root.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"))
    .toBe("ready");
  expect(root.querySelector("[data-readable-export-bytes]")?.textContent)
    .toBe("# Volume A\n\n## Chapter A\n\nHello world\n");
  expect(root.querySelector("[data-readable-export-bytes]")?.textContent)
    .not.toContain("Chapter B");
  await submitChapterForm(frame, "Chapter B");
  await expect.poll(() =>
    [...appRoot(frame).querySelectorAll('nav[aria-label="稿件目录"] button[data-chapter-id]')]
      .some((button) => button.textContent === "Chapter B")
  ).toBe(true);
  const chapterBId = [...appRoot(frame).querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].find((button) => button.textContent === "Chapter B")?.getAttribute("data-chapter-id");
  if (chapterBId === null || chapterBId === undefined) throw new Error("Chapter B is missing");
  await selectTreeChapter(appRoot(frame), chapterBId);
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter B"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoEditableChapter(frame, CHAPTER_B);
  const afterRebuild = appRoot(frame);
  await requestExport(afterRebuild);
  expect(afterRebuild.querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"))
    .toBe("ready");
  expect(afterRebuild.querySelector("[data-readable-export-bytes]")?.textContent)
    .toBe(`# Volume A\n\n## Chapter A\n\n${CHAPTER_A}\n\n## Chapter B\n\n${CHAPTER_B}\n`);
  expect(
    [...afterRebuild.querySelectorAll<HTMLButtonElement>("[data-readable-export] button")]
      .some((candidate) => candidate.textContent === "下载"),
  ).toBe(true);
});
