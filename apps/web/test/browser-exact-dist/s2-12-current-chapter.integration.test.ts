import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";
import {
  appRoot,
  applicationWindow,
  closeApplicationFrame,
  nextFrameLoad,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  typeIntoCurrent,
  waitLastChapter,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function chapterButton(root: Element, title: string): HTMLButtonElement | undefined {
  return [...root.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].find((button) => button.textContent === title);
}

it("writes in each selected Chapter, undoes a chapter switch, and reopens the Current Chapter", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist current Chapter");
  await submitProjectForm(frame, "Current Chapter Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }

  const root = appRoot(frame);
  expect(root.querySelector("h2")?.textContent).toBe("Chapter A");
  await typeIntoCurrent(frame, "Alpha prose");
  const projectId = root.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  const selectEditable = async (name: string) => {
    await selectTreeChapter(appRoot(frame), name === "Chapter A" ? chapterAId : chapterBId);
    await expect.poll(() => {
      const nextRoot = appRoot(frame);
      const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
      return nextRoot.querySelector("h2")?.textContent === name
        && editor !== null
        && editor !== undefined
        && manuscriptIsEditable(editor);
    }, { timeout: 15_000 }).toBe(true);
  };
  await selectEditable("Chapter B");
  expect(manuscriptBody(manuscriptEditor(appRoot(frame), applicationWindow(frame)))).toBe("");
  await typeIntoCurrent(frame, "Beta prose");

  await selectEditable("Chapter A");
  expect(manuscriptBody(manuscriptEditor(appRoot(frame), applicationWindow(frame)))).toBe("Alpha prose");
  await typeIntoCurrent(frame, ", again", "Alpha prose, again");

  // An Author Undo of a chapter switch returns the editor to the earlier Chapter.
  await selectEditable("Chapter B");
  manuscriptEditor(appRoot(frame), applicationWindow(frame)).focus();
  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return {
      title: nextRoot.querySelector("h2")?.textContent,
      body: editor === null ? undefined : manuscriptBody(editor),
      editable: editor !== null && manuscriptIsEditable(editor),
    };
  }, { timeout: 15_000 }).toEqual({ title: "Chapter A", body: "Alpha prose, again", editable: true });

  const childWindow = applicationWindow(frame);
  const authoritativeA = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId: chapterAId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  const authoritativeB = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId: chapterBId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  expect(authoritativeA.chapter.current_revision.body).toBe("Alpha prose, again");
  expect(authoritativeB.chapter.current_revision.body).toBe("Beta prose");

  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reopened;
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");
  const reopenedRoot = appRoot(frame);
  expect(reopenedRoot.querySelector("h2")?.textContent).toBe("Chapter A");
  const reopenedEditor = manuscriptEditor(reopenedRoot, applicationWindow(frame));
  await expect.poll(() => manuscriptBody(reopenedEditor)).toBe("Alpha prose, again");
  expect(manuscriptIsEditable(reopenedEditor)).toBe(true);
});
