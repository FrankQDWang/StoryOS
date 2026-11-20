import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  closeApplicationFrame,
  openApplicationFrame,
  openFirstChapterForm,
  reloadProject,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitSaved,
} from "../support/exact-dist-frame.ts";

const SETTLED = "Settled prose";
const FULL = "Settled prose more";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function saveNode(root: Element): Element | null {
  return root.querySelector("[data-save-state]");
}

it("recovers Local Edit Journal text after reload without a second Author Edit", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist Local Edit Journal recovery");
  await submitProjectForm(frame, "Journal Recovery Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  await submitChapterForm(frame, "Chapter A");
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");

  let root = appRoot(frame);
  const realm = applicationWindow(frame);
  let editor = manuscriptEditor(root, realm);
  editor.focus();
  focusManuscriptEnd(editor, realm);
  await applyTrustedInput({ operation: "insert_text", text: SETTLED });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(SETTLED);
  await waitSaved(root, undefined, { timeout: 15_000 });

  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterId = root.querySelector<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }

  root = await reloadProject(frame, projectId);
  editor = manuscriptEditor(root, applicationWindow(frame));
  expect(manuscriptBody(editor)).toBe(SETTLED);
  await waitSaved(root, undefined, { timeout: 15_000 });

  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text: " more" });
  await expect.poll(() => {
    const node = saveNode(root);
    if (manuscriptBody(editor) !== FULL) return null;
    const save = node?.getAttribute("data-save-state");
    const unsettled = node?.getAttribute("data-unsettled-intent-count");
    if (save === "saving" || (unsettled !== null && unsettled !== undefined && unsettled !== "0")) {
      return "pending";
    }
    if (save === "saved" && unsettled === "0") return "saved";
    return null;
  }, { timeout: 10_000 }).toMatch(/^(pending|saved)$/);

  root = await reloadProject(frame, projectId);
  editor = manuscriptEditor(root, applicationWindow(frame));
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(FULL);
  await waitSaved(root, undefined, { timeout: 15_000 });

  const afterReload = applicationWindow(frame);
  const firstRecovered = await getChapter({
    baseUrl: afterReload.location.origin,
    projectId,
    chapterId,
    fetchImpl: afterReload.fetch.bind(afterReload),
  });
  expect(firstRecovered.chapter.current_revision.body).toBe(FULL);

  root = await reloadProject(frame, projectId);
  editor = manuscriptEditor(root, applicationWindow(frame));
  expect(manuscriptBody(editor)).toBe(FULL);
  await waitSaved(root, undefined, { timeout: 15_000 });
  const afterRepeat = applicationWindow(frame);
  const repeated = await getChapter({
    baseUrl: afterRepeat.location.origin,
    projectId,
    chapterId,
    fetchImpl: afterRepeat.fetch.bind(afterRepeat),
  });
  expect(repeated.chapter.current_revision).toEqual(firstRecovered.chapter.current_revision);
});
