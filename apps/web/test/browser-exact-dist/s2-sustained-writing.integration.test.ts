import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyImeComposition,
  applyTrustedInput,
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
  closeApplicationFrame,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const ROUNDS = 10;
const PHRASES = ["我们", "写作"] as const;

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("saves sustained Chinese composition, idle pauses, and Block boundaries without a quota reset", {
  timeout: 180_000,
}, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist sustained writing");
  const childWindow = applicationWindow(frame);
  childWindow.performance.setResourceTimingBufferSize(2_000);
  await submitProjectForm(frame, "Sustained Writing Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null
      && root.querySelector('nav[aria-label="稿件目录"] button[aria-current="true"]') !== null;
  }, { timeout: 10_000 }).toBe(true);
  const root = appRoot(frame);
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
    for (const phrase of PHRASES) {
      const offset = manuscriptBody(editor).length;
      await applyImeComposition({ text: phrase, replacementStart: offset, replacementEnd: offset,
        selectionStart: phrase.length, selectionEnd: phrase.length });
      await applyTrustedInput({ operation: "insert_text", text: phrase });
      expected += phrase;
      await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
      revision = await waitSaved(root, revision);
    }
    await applyTrustedInput({ operation: "insert_text", text: "。" });
    expected += "。";
    await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
    revision = await waitSaved(root, revision);
    await applyTrustedInput({ operation: "enter" });
    await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(2);
    revision = await waitSaved(root, revision);
    await applyTrustedInput({ operation: "backspace" });
    await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(1);
    revision = await waitSaved(root, revision);
  }
  const elapsedSeconds = (childWindow.performance.now() - started) / 1_000;

  const requests = childWindow.performance.getEntriesByType("resource")
    .filter((entry): entry is PerformanceResourceTiming => entry instanceof childWindow.PerformanceResourceTiming)
    .filter((entry) => entry.startTime >= started);
  const challenges = requests.filter((entry) => entry.name.endsWith("/anti-forgery-challenges"));
  const authorEdits = requests.filter((entry) => entry.name.endsWith("/manuscript/author-edits"));
  console.info(JSON.stringify({ issue: 880, elapsedSeconds, challengeRequests: challenges.length,
    authorEditRequests: authorEdits.length,
    challengeStatuses: [...new Set(challenges.map((entry) => entry.responseStatus))] }));
  const chapter = await getChapter({ baseUrl: childWindow.location.origin, projectId, chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow) });

  expect({
    authorEditRequests: authorEdits.length,
    challengeStatuses: [...new Set(challenges.map((entry) => entry.responseStatus))],
    text: chapter.chapter.current_revision.blocks.map((block) => block.text),
  }).toEqual({
    authorEditRequests: ROUNDS * (PHRASES.length + 3),
    challengeStatuses: [200],
    text: ["我们写作。".repeat(ROUNDS)],
  });
});
