import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  applicationWindow,
  chapterButton,
  closeApplicationFrame,
  nextFrameLoad,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  waitLastChapter,
} from "../support/exact-dist-frame.ts";
import { manuscriptBody, manuscriptIsEditable, MANUSCRIPT_EDITOR_SELECTOR }
  from "../support/manuscript-surface.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

async function createThreeChapterProject(frame: HTMLIFrameElement): Promise<void> {
  await submitProjectForm(frame, "Navigate Reopen Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B", "Chapter C"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }
}

it("the author opens each Chapter from the tree and reopens the current Chapter", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist Chapter navigation");
  await createThreeChapterProject(frame);

  const root = frame.contentDocument?.querySelector("#app");
  expect(root?.querySelector("h2")?.textContent).toBe("Chapter A");
  const chapterIds = new Map(["Chapter A", "Chapter B", "Chapter C"].map((title) =>
    [title, chapterButton(root, title)?.getAttribute("data-chapter-id") ?? ""]));
  const select = async (title: string) => {
    await selectTreeChapter(root, chapterIds.get(title) ?? "");
    await expect.poll(() => {
      const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
      return {
        title: root?.querySelector("h2")?.textContent,
        body: editor === null || editor === undefined ? undefined : manuscriptBody(editor),
        editable: editor !== null && editor !== undefined && manuscriptIsEditable(editor),
      };
    }, { timeout: 15_000 }).toEqual({ title, body: "", editable: true });
    expect(root?.querySelector('[role="alert"]')).toBeNull();
  };
  await select("Chapter B");
  await select("Chapter C");
  await select("Chapter A");

  const projectId = root?.querySelector("[data-project-id]")?.getAttribute("data-project-id");
  const chapterBId = chapterIds.get("Chapter B");
  const childWindow = applicationWindow(frame);
  if (root === null || root === undefined
    || projectId === null || projectId === undefined
    || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  childWindow.localStorage.setItem("current_chapter", chapterBId);
  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}?chapter=${chapterBId}#chapter=${chapterBId}`;
  await reopened;
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state") === "project-ready"
  ).toBe(true);
  await expect.poll(() =>
    frame.contentDocument?.querySelector("h2")?.textContent
  ).toBe("Chapter A");
  const reopenedRoot = frame.contentDocument?.querySelector("#app");
  const reopenedEditor = reopenedRoot?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(reopenedEditor === null || reopenedEditor === undefined
    ? undefined : manuscriptBody(reopenedEditor)).toBe("");
  expect(reopenedRoot?.textContent).not.toContain("模型");
  expect(reopenedRoot?.textContent).not.toContain("Agent");
});
