import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyTrustedInput,
  resetCommandChallengeRateWindows,
  updateClientSessionCookie,
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
  chapterButton,
  closeApplicationFrame,
  makeCurrent,
  nextFrameLoad,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitLastChapter,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const SETTLED_A = "Alpha 中文 EN";
const LONG_A = `Alpha${"a".repeat(2401)}`;

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
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
  await waitSaved(root, before, { timeout: 30_000 });
}

it("repeats Chapter switching, Undo, search, and reload without losing work", {
  timeout: 300_000,
}, async () => {
  const started = performance.now();
  const frame = await openApplicationFrame("StoryOS exact-dist long session");
  await submitProjectForm(frame, "Long Session Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }
  const coldOpenMs = Math.round(performance.now() - started);

  const root = appRoot(frame);
  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }

  const inputStarted = performance.now();
  await typeExpected(frame, "Alpha", "Alpha");
  await makeCurrent(frame, chapterBId, "Chapter B");
  await typeExpected(frame, "Beta", "Beta");
  await makeCurrent(frame, chapterAId, "Chapter A");
  expect(manuscriptBody(manuscriptEditor(appRoot(frame), applicationWindow(frame)))).toBe("Alpha");
  await typeExpected(frame, " 中文 EN", SETTLED_A);
  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await expect.poll(() => manuscriptBody(manuscriptEditor(appRoot(frame), applicationWindow(frame))))
    .toBe("Alpha");
  await waitSaved(appRoot(frame), undefined, { timeout: 30_000 });
  const longEditor = manuscriptEditor(appRoot(frame), applicationWindow(frame));
  longEditor.focus();
  focusManuscriptEnd(longEditor, applicationWindow(frame));
  for (let batch = 0; batch < 11; batch++) {
    if (batch % 5 === 0) {
      await resetCommandChallengeRateWindows();
      longEditor.focus();
      focusManuscriptEnd(longEditor, applicationWindow(frame));
    }
    const count = batch === 10 ? 1 : 240;
    for (let input = 0; input < count; input++) {
      await applyTrustedInput({ operation: "insert_text", text: "a" });
    }
    await waitSaved(appRoot(frame), undefined, { timeout: 30_000 });
  }
  expect(manuscriptBody(longEditor)).toBe(LONG_A);
  await makeCurrent(frame, chapterBId, "Chapter B");
  await typeExpected(frame, "!", "Beta!");
  await makeCurrent(frame, chapterAId, "Chapter A");
  const inputMs = Math.round(performance.now() - inputStarted);

  const searchStarted = performance.now();
  const searchInput = appRoot(frame).querySelector<HTMLInputElement>(
    'input[name="manuscript-search-query"]',
  );
  const searchRadio = appRoot(frame).querySelector<HTMLInputElement>(
    'input[name="manuscript-search-selection"][value="current_chapter"]',
  );
  const searchForm = appRoot(frame).querySelector<HTMLFormElement>("form[data-manuscript-search-form]");
  if (searchInput === null || searchRadio === null || searchForm === null) {
    throw new Error("the manuscript search form is missing");
  }
  searchRadio.click();
  searchInput.value = "Alpha";
  searchForm.requestSubmit();
  await expect.poll(() => {
    const node = appRoot(frame).querySelector("[data-search-outcome='ready']");
    return node?.getAttribute("data-search-query") === "Alpha"
      ? node.querySelectorAll("[data-search-match]").length
      : 0;
  }, { timeout: 10_000 }).toBeGreaterThan(0);
  const searchMs = Math.round(performance.now() - searchStarted);

  await expect.poll(() =>
    appRoot(frame).querySelector("[data-statistics-outcome='ready']")?.getAttribute("data-statistics-lag")
  ).toBe("0");
  const statistics = appRoot(frame).querySelector("[data-statistics-outcome='ready']");
  expect(statistics?.getAttribute("data-statistics-snapshot-id")).toMatch(UUID);
  expect(appRoot(frame).querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
    .toBe("0");

  const reloadStarted = performance.now();
  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reopened;
  await expect.poll(() => {
    const nextRoot = frame.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedRoot = appRoot(frame);
  expect(reopenedRoot.querySelector("h2")?.textContent).toBe("Chapter A");
  expect(manuscriptBody(manuscriptEditor(reopenedRoot, applicationWindow(frame)))).toBe(LONG_A);
  await typeExpected(frame, "+", `${LONG_A}+`);
  await waitSaved(reopenedRoot, undefined, { timeout: 30_000 });
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
  expect(authoritativeA.chapter.current_revision.body).toBe(`${LONG_A}+`);
  expect(authoritativeB.chapter.current_revision.body).toBe("Beta!");
  const reloadMs = Math.round(performance.now() - reloadStarted);
  console.info(JSON.stringify({
    adopted_recovery_bounds: { rpo_seconds: 900, rto_seconds: 7200 },
    evidence_class: "EV-INT",
    fixture: "FX-LONG-SESSION",
    invented_latency_target: false,
    measurements: {
      cold_open_ms: coldOpenMs,
      input_ms: inputMs,
      journal_unsettled: 0,
      reload_ms: reloadMs,
      search_ms: searchMs,
      snapshot_present: true,
    },
  }));
});
