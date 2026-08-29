import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist split-join page did not load"));
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

async function waitSaved(root: Element): Promise<void> {
  await expect.poll(() =>
    root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    { timeout: 10_000 },
  ).toBe("saved");
}

async function openChapterEditor(): Promise<{
  frame: HTMLIFrameElement;
  projectId: string;
  chapterId: string;
}> {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist split and join";
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
  title.value = "Split Join Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
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
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app form[data-create-chapter]') !== null
  ).toBe(true);
  const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const chapterForm = chapterTitle?.form;
  if (chapterTitle === null || chapterTitle === undefined
    || chapterForm === null || chapterForm === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  chapterTitle.value = "Chapter A";
  chapterForm.requestSubmit();
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }).toBe(true);
  const root = appRoot(frame);
  await expect.poll(() =>
    root.querySelector("form[data-rename]")?.getAttribute("data-rename") !== null
    && root.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    )?.getAttribute("data-chapter-id") !== null
  ).toBe(true);
  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterId = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  return { frame, projectId, chapterId };
}

async function readRevision(frame: HTMLIFrameElement, projectId: string, chapterId: string) {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  return chapter.chapter.current_revision;
}

function focusParagraphStart(
  editor: HTMLElement,
  realm: Window & typeof globalThis,
  index: number,
): void {
  const paragraph = editor.querySelectorAll("p")[index];
  if (paragraph === undefined) throw new Error("the paragraph is missing");
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.setStart(paragraph, 0);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

it("splits and joins adjacent Blocks and reopens the same identities", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor();
  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const before = await readRevision(frame, projectId, chapterId);
  const leftId = before.blocks[0]?.manuscript_block_id;
  if (before.blocks.length !== 1 || leftId === undefined) {
    throw new Error("the starting Block is missing");
  }

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Hello" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello");
  await waitSaved(root);

  await applyTrustedInput({ operation: "enter" });
  await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(2);
  await waitSaved(root);

  await applyTrustedInput({ operation: "insert_text", text: "World" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello\nWorld");
  await waitSaved(root);

  const split = await readRevision(frame, projectId, chapterId);
  expect(split.blocks.map((block) => block.text)).toEqual(["Hello", "World"]);
  expect(split.blocks[0]?.manuscript_block_id).toBe(leftId);
  const rightId = split.blocks[1]?.manuscript_block_id;
  expect(rightId).toMatch(UUID);
  expect(rightId).not.toBe(leftId);

  await destroyApplicationFrame(frame);
  const reopened = document.createElement("iframe");
  applicationFrame = reopened;
  reopened.title = "StoryOS exact-dist split and join reload";
  const reopenedLoaded = nextFrameLoad(reopened);
  reopened.src = "/";
  document.body.append(reopened);
  await reopenedLoaded;
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }).toBe(true);
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks.map((block) => ({
    manuscript_block_id: block.manuscript_block_id,
    text: block.text,
  }))).toEqual([
    { manuscript_block_id: leftId, text: "Hello" },
    { manuscript_block_id: rightId, text: "World" },
  ]);

  const reopenedRoot = appRoot(reopened);
  const reopenedEditor = manuscriptEditor(reopenedRoot, applicationWindow(reopened));
  focusParagraphStart(reopenedEditor, applicationWindow(reopened), 1);
  await applyTrustedInput({ operation: "backspace" });
  await expect.poll(() => reopenedEditor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(1);
  await expect.poll(() => manuscriptBody(reopenedEditor), { timeout: 10_000 }).toBe("HelloWorld");
  await waitSaved(reopenedRoot);

  const joined = await readRevision(reopened, projectId, chapterId);
  expect(joined.blocks).toEqual([{
    manuscript_block_id: leftId,
    block_kind: "paragraph",
    text: "HelloWorld",
  }]);

  await destroyApplicationFrame(reopened);
  const finalFrame = document.createElement("iframe");
  applicationFrame = finalFrame;
  finalFrame.title = "StoryOS exact-dist split and join final reload";
  const finalLoaded = nextFrameLoad(finalFrame);
  finalFrame.src = "/";
  document.body.append(finalFrame);
  await finalLoaded;
  await expect.poll(() => {
    const nextRoot = finalFrame.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }).toBe(true);
  const finalRevision = await readRevision(finalFrame, projectId, chapterId);
  expect(finalRevision.blocks).toEqual([{
    manuscript_block_id: leftId,
    block_kind: "paragraph",
    text: "HelloWorld",
  }]);
});
