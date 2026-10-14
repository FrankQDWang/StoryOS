import { beginInlineChapterCreation, beginInlineVolumeCreation, selectTreeChapter } from "../support/inline-chapter-creation.ts";
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

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist current-chapter page did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

async function destroyApplicationFrame(frame: HTMLIFrameElement): Promise<void> {
  if (!frame.isConnected) return;
  const unloaded = nextFrameLoad(frame);
  frame.src = "about:blank";
  await unloaded;
  frame.remove();
}

afterEach(async () => {
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function applicationWindow(frame: HTMLIFrameElement): Window & typeof globalThis {
  const result = frame.contentWindow;
  if (result === null) throw new Error("the production page realm is unavailable");
  return result as Window & typeof globalThis;
}

function appRoot(frame: HTMLIFrameElement): Element {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the production page root is missing");
  }
  return root;
}

function chapterButton(root: Element, title: string): HTMLButtonElement | undefined {
  return [...root.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].find((button) => button.textContent === title);
}

async function typeIntoCurrent(frame: HTMLIFrameElement, text: string, body = text): Promise<void> {
  const root = appRoot(frame);
  const editor = manuscriptEditor(root, applicationWindow(frame));
  const previousRevision = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id");
  if (previousRevision === null || previousRevision === undefined) {
    throw new Error("the authoritative revision is missing");
  }
  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text });
  // A new authoritative revision proves that the input reached the saved manuscript.
  await expect.poll(() => {
    const save = root.querySelector("[data-save-state]");
    return {
      body: manuscriptBody(editor),
      saveState: save?.getAttribute("data-save-state"),
      unsettledIntentCount: save?.getAttribute("data-unsettled-intent-count"),
      revisionChanged: save?.getAttribute("data-authoritative-revision-id") !== previousRevision,
    };
  }, { timeout: 10_000 }).toEqual({
    body,
    saveState: "saved",
    unsettledIntentCount: "0",
    revisionChanged: true,
  });
}

it("writes in each selected Chapter, undoes a chapter switch, and reopens the Current Chapter", async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist current Chapter";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Current Chapter Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  await beginInlineVolumeCreation(frame.contentDocument);
  const volumeTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-volume] input[name="volume-title"]',
  );
  const volumeForm = volumeTitle?.form;
  if (volumeTitle === null || volumeTitle === undefined
    || volumeForm === null || volumeForm === undefined) {
    throw new Error("the Create Volume form is missing");
  }
  volumeTitle.value = "Volume A";
  volumeForm.requestSubmit();
  await beginInlineChapterCreation(frame.contentDocument);
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app form[data-create-chapter]') !== null
  ).toBe(true);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await beginInlineChapterCreation(frame.contentDocument);
    const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
      '#app form[data-create-chapter] input[name="chapter-title"]',
    );
    const chapterForm = chapterTitle?.form;
    if (chapterTitle === null || chapterTitle === undefined
      || chapterForm === null || chapterForm === undefined) {
      throw new Error("the Create Chapter form is missing");
    }
    chapterTitle.value = name;
    chapterForm.requestSubmit();
    await expect.poll(() => {
      const root = frame.contentDocument?.querySelector("#app");
      const titles = [...(root?.querySelectorAll(
        'nav[aria-label="稿件目录"] button[data-chapter-id]',
      ) ?? [])].map((button) => button.textContent);
      return root?.getAttribute("data-boot-state") === "project-ready"
        && titles[titles.length - 1] === name;
    }).toBe(true);
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
