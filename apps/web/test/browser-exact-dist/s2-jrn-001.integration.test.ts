import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, settleWorkerOnce, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  chapterButton,
  closeApplicationFrame,
  makeCurrent,
  openApplicationFrame,
  openFirstChapterForm,
  reloadProject,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitLastChapter,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BODY_A = "Hello 中文";
const BODY_B = "Beta prose";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

async function typeExpected(
  frame: HTMLIFrameElement,
  inserted: string,
  expected: string,
): Promise<void> {
  const root = appRoot(frame);
  const editor = manuscriptEditor(root, applicationWindow(frame));
  const before = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text: inserted });
  // The end of a Chapter switch can replace the editor after the key, so each step reads the editor that is on the
  // screen (#1960).
  await expect.poll(() => manuscriptBody(manuscriptEditor(root, applicationWindow(frame))), { timeout: 10_000 })
    .toBe(expected);
  await waitSaved(root, before);
}

it("runs the AI-disabled production journey without losing Chapter work", {
  timeout: 90_000,
}, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist Stage 2 journey");
  await submitProjectForm(frame, "Stage 2 Journey");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }

  const root = appRoot(frame);
  expect(root.querySelector('nav[aria-label="稿件目录"]')).not.toBeNull();
  expect(root.querySelector(MANUSCRIPT_EDITOR_SELECTOR)).not.toBeNull();
  expect(root.querySelector("[data-writing-assistant]")).not.toBeNull();
  // The panel shows the availability after the answer of the assistance read (#1876).
  await expect.poll(() => root.querySelector("[data-writing-assistant]")
    ?.getAttribute("data-assistant-availability"), { timeout: 10_000 }).toBe("unavailable");
  expect(root.textContent).not.toContain("模型");
  expect(root.textContent).not.toContain("Agent");
  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }

  await typeExpected(frame, BODY_A, BODY_A);
  await makeCurrent(frame, chapterBId, "Chapter B");
  await typeExpected(frame, BODY_B, BODY_B);
  await reloadProject(frame, projectId);
  expect(manuscriptBody(manuscriptEditor(appRoot(frame), applicationWindow(frame)))).toBe(BODY_B);

  const searchInput = appRoot(frame).querySelector<HTMLInputElement>(
    'input[name="manuscript-search-query"]',
  );
  const searchRadio = appRoot(frame).querySelector<HTMLInputElement>(
    'input[name="manuscript-search-selection"][value="manuscript"]',
  );
  const searchForm = appRoot(frame).querySelector<HTMLFormElement>("form[data-manuscript-search-form]");
  if (searchInput === null || searchRadio === null || searchForm === null) {
    throw new Error("the manuscript search form is missing");
  }
  searchRadio.click();
  searchInput.value = "Hello";
  searchForm.requestSubmit();
  await expect.poll(() => {
    const node = appRoot(frame).querySelector("[data-search-outcome='ready']");
    return node?.getAttribute("data-search-query") === "Hello"
      ? node.querySelectorAll("[data-search-match]").length
      : 0;
  }, { timeout: 10_000 }).toBeGreaterThan(0);
  await expect.poll(() =>
    appRoot(frame).querySelector("[data-statistics-outcome='ready']")?.getAttribute("data-statistics-lag")
  ).toBe("0");
  expect(appRoot(frame).querySelector("[data-statistics-outcome='ready']")
    ?.getAttribute("data-statistics-snapshot-id")).toMatch(UUID);

  const exportButton = [...appRoot(frame).querySelectorAll<HTMLButtonElement>(
    "[data-readable-export] button",
  )].find((candidate) => candidate.textContent === "导出可读稿件");
  if (exportButton === undefined) throw new Error("the readable export request button is missing");
  exportButton.click();
  await expect.poll(() =>
    appRoot(frame).querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"),
  ).toBe("in_progress");
  expect(appRoot(frame).querySelector("[data-export-id]")?.getAttribute("data-export-id")).toBeTruthy();
  await settleWorkerOnce();
  await expect.poll(() =>
    appRoot(frame).querySelector("[data-readable-export]")?.getAttribute("data-export-outcome"),
    { timeout: 15_000 },
  ).toBe("ready");
  expect(appRoot(frame).querySelector("[data-readable-export-bytes]")?.textContent)
    .toContain(BODY_A);

  await reloadProject(frame, projectId);
  const reopenedRoot = appRoot(frame);
  expect(reopenedRoot.textContent).not.toContain("模型");
  expect(reopenedRoot.textContent).not.toContain("Agent");
  expect(manuscriptBody(manuscriptEditor(reopenedRoot, applicationWindow(frame)))).toBe(BODY_B);
  // An editor with no Proposal candidate shows no mark of a candidate (S10-REQ-004).
  expect(manuscriptEditor(reopenedRoot, applicationWindow(frame))
    .querySelectorAll(".proposal-change-mark, .proposal-change-tick, .proposal-change-pair")).toHaveLength(0);
  const childWindow = applicationWindow(frame);
  const [authoritativeA, authoritativeB] = await Promise.all([
    getChapter({
      baseUrl: childWindow.location.origin,
      projectId,
      chapterId: chapterAId,
      fetchImpl: childWindow.fetch.bind(childWindow),
    }),
    getChapter({
      baseUrl: childWindow.location.origin,
      projectId,
      chapterId: chapterBId,
      fetchImpl: childWindow.fetch.bind(childWindow),
    }),
  ]);
  expect(authoritativeA.chapter.current_revision.body).toBe(BODY_A);
  expect(authoritativeB.chapter.current_revision.body).toBe(BODY_B);
});
