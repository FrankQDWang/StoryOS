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
  closeApplicationFrame,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const ROUNDS = 12;

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("keeps the editor writable through frequent Author Undo between Author Edit batches", {
  timeout: 180_000,
}, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist frequent undo");
  const childWindow = frame.contentWindow as (Window & typeof globalThis) | null;
  if (childWindow === null) throw new Error("the production page realm is unavailable");
  childWindow.performance.setResourceTimingBufferSize(2_000);
  await submitProjectForm(frame, "Frequent Undo Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null
      && root.querySelector('nav[aria-label="稿件目录"] button[aria-current="true"]') !== null;
  }, { timeout: 10_000 }).toBe(true);
  const root = frame.contentDocument!.querySelector("#app")!;
  const projectId = root.querySelector("[data-project-id]")!.getAttribute("data-project-id")!;
  const chapterId = root.querySelector('nav[aria-label="稿件目录"] button[aria-current="true"]')!
    .getAttribute("data-chapter-id")!;
  const editor = manuscriptEditor(root, childWindow);
  editor.focus();
  focusManuscriptEnd(editor, childWindow);

  const started = childWindow.performance.now();
  let revision = root.querySelector("[data-save-state]")?.getAttribute("data-authoritative-revision-id") ?? "";
  let expected = "";
  for (let round = 0; round < ROUNDS; round += 1) {
    for (const [phrase, body] of [["我们", `${expected}我们`], ["写作", `${expected}我们写作`]] as const) {
      await applyTrustedInput({ operation: "insert_text", text: phrase });
      await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(body);
      revision = await waitSaved(root, revision);
    }
    expected += "我们";
    await applyTrustedInput({ operation: "undo_latest_author_action" });
    await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
    revision = await waitSaved(root, revision);
    focusManuscriptEnd(editor, childWindow);
  }
  const elapsedSeconds = (childWindow.performance.now() - started) / 1_000;

  const requests = childWindow.performance.getEntriesByType("resource")
    .filter((entry): entry is PerformanceResourceTiming => entry instanceof childWindow.PerformanceResourceTiming)
    .filter((entry) => entry.startTime >= started);
  const challenges = requests.filter((entry) => entry.name.endsWith("/anti-forgery-challenges"));
  const undos = requests.filter((entry) => entry.name.endsWith("/author-actions/undo"));
  const authorEdits = requests.filter((entry) => entry.name.endsWith("/manuscript/author-edits"));
  console.info(JSON.stringify({ issue: 888, elapsedSeconds, undoRequests: undos.length,
    undoRequestsPerMinute: undos.length / (elapsedSeconds / 60), authorEditRequests: authorEdits.length,
    challengeRequests: challenges.length,
    challengeStatuses: [...new Set(challenges.map((entry) => entry.responseStatus))] }));
  const chapter = await getChapter({ baseUrl: childWindow.location.origin, projectId, chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow) });

  expect({
    undoRequests: undos.length,
    authorEditRequests: authorEdits.length,
    challengeStatuses: [...new Set(challenges.map((entry) => entry.responseStatus))],
    editable: editor.getAttribute("contenteditable"),
    text: chapter.chapter.current_revision.blocks.map((block) => block.text),
  }).toEqual({
    undoRequests: ROUNDS,
    authorEditRequests: ROUNDS * 2,
    challengeStatuses: [200],
    editable: "true",
    text: ["我们".repeat(ROUNDS)],
  });
});
