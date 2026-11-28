import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  chapterButton,
  closeApplicationFrame,
  destroyApplicationFrame,
  nextFrameLoad,
  openApplicationFrame,
  openChapterEditor,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitLastChapter,
  waitSaved,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function saveNode(root: Element): Element | null {
  return root.querySelector("[data-save-state]");
}

async function waitSaveState(root: Element): Promise<void> {
  await expect.poll(() => {
    const node = saveNode(root);
    return node?.getAttribute("data-save-state") === "saved"
      && node.getAttribute("data-unsettled-intent-count") === "0"
      ? "saved"
      : node?.getAttribute("data-save-state") ?? null;
  }, { timeout: 10_000 }).toBe("saved");
}

it("shows pending, saving, and saved without calling local input saved, across Chapters", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist save truth");
  await submitProjectForm(frame, "Save Truth Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }

  const root = appRoot(frame);
  expect(root.querySelector("h2")?.textContent).toBe("Chapter A");
  expect(saveNode(root)?.getAttribute("data-save-state")).toBe("saved");
  expect(saveNode(root)?.textContent).toContain("已保存");
  expect(saveNode(root)?.textContent).not.toContain("需要处理");

  const editor = manuscriptEditor(root, applicationWindow(frame));
  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  const seenWhileLocal: string[] = [];
  await applyTrustedInput({ operation: "insert_text", text: "Alpha prose" });
  await expect.poll(() => {
    const save = saveNode(root)?.getAttribute("data-save-state");
    if (manuscriptBody(editor) === "Alpha prose" && save !== null && save !== undefined) {
      seenWhileLocal.push(save);
    }
    return manuscriptBody(editor);
  }, { timeout: 10_000 }).toBe("Alpha prose");
  expect(seenWhileLocal.includes("saved")).toBe(false);
  expect(seenWhileLocal.some((state) => state === "pending" || state === "saving")).toBe(true);
  await waitSaveState(root);
  expect(saveNode(root)?.textContent).toContain("已保存");

  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  await selectTreeChapter(root, chapterBId);
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const nextEditor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter B"
      && nextEditor !== null
      && nextEditor !== undefined
      && manuscriptIsEditable(nextEditor);
  }, { timeout: 15_000 }).toBe(true);
  const switchedRoot = appRoot(frame);
  const switchedEditor = manuscriptEditor(switchedRoot, applicationWindow(frame));
  expect(manuscriptBody(switchedEditor)).toBe("");
  expect(saveNode(switchedRoot)?.getAttribute("data-save-state")).toBe("saved");

  switchedEditor.focus();
  focusManuscriptEnd(switchedEditor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text: "Beta prose" });
  await expect.poll(() => manuscriptBody(switchedEditor), { timeout: 10_000 }).toBe("Beta prose");
  await waitSaveState(switchedRoot);

  await selectTreeChapter(switchedRoot, chapterAId);
  await expect.poll(() => {
    const editor = switchedRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return {
      title: switchedRoot.querySelector("h2")?.textContent,
      body: editor === null ? undefined : manuscriptBody(editor),
      editable: editor !== null && manuscriptIsEditable(editor),
    };
  }, { timeout: 15_000 }).toEqual({ title: "Chapter A", body: "Alpha prose", editable: true });
  expect(saveNode(switchedRoot)?.getAttribute("data-save-state")).toBe("saved");
  expect(saveNode(switchedRoot)?.textContent).toContain("已保存");

  const childWindow = applicationWindow(frame);
  const authoritativeA = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId: chapterAId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  expect(authoritativeA.chapter.current_revision.body).toBe("Alpha prose");

  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reopened;
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");
  const reopenedRoot = appRoot(frame);
  expect(reopenedRoot.querySelector("h2")?.textContent).toBe("Chapter A");
  expect(manuscriptBody(manuscriptEditor(reopenedRoot, applicationWindow(frame))))
    .toBe("Alpha prose");
  await waitSaveState(reopenedRoot);
});

// ADR 0106 section 3: each save takes the base of the Editor Session from its response, and the next save starts on
// that base with no read of the Editor Session.
it("keeps each input of three saves in a row after a new load, with no read of the Editor Session", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor("StoryOS exact-dist saves in a row",
    "Saves In A Row Novel");
  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const sessionReads = () => childWindow.performance.getEntriesByType("resource")
    .filter((entry) => entry.name.includes(`/api/v1/projects/${projectId}/editor-sessions/`)).length;
  childWindow.performance.setResourceTimingBufferSize(10_000);
  const readsBefore = sessionReads();
  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  let revision = saveNode(root)?.getAttribute("data-authoritative-revision-id") ?? undefined;
  for (const [text, body] of [["One", "One"], [" two", "One two"], [" three", "One two three"]] as const) {
    await applyTrustedInput({ operation: "insert_text", text });
    await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(body);
    revision = await waitSaved(root, revision);
  }
  expect(sessionReads()).toBe(readsBefore);

  await destroyApplicationFrame(frame);
  const reopened = await openApplicationFrame("StoryOS exact-dist saves in a row reload", `/projects/${projectId}`);
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedWindow = applicationWindow(reopened);
  await expect.poll(() => manuscriptBody(manuscriptEditor(appRoot(reopened), reopenedWindow))).toBe("One two three");
  const stored = await getChapter({ baseUrl: reopenedWindow.location.origin, projectId, chapterId,
    fetchImpl: reopenedWindow.fetch.bind(reopenedWindow) });
  expect({ body: stored.chapter.current_revision.body, revision: stored.chapter.current_revision.revision_id })
    .toEqual({ body: "One two three", revision });
});
