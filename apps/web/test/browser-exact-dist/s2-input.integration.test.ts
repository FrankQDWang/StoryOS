import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyImeComposition,
  applyTrustedInput,
  updateClientSessionCookie,
  updateClipboardPermission,
} from "../support/browser-command-client.ts";
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
      reject(new Error("the exact-dist input page did not load"));
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
  await updateClipboardPermission({ action: "clear" });
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

async function waitSaved(root: Element, previousRevisionId?: string): Promise<void> {
  await expect.poll(() => {
    const node = root.querySelector("[data-save-state]");
    const revision = node?.getAttribute("data-authoritative-revision-id") ?? "";
    const failure = node?.getAttribute("data-editor-failure") ?? "";
    const save = node?.getAttribute("data-save-state");
    const unsettled = node?.getAttribute("data-unsettled-intent-count");
    if (save === "saved"
      && unsettled === "0"
      && failure === ""
      && (previousRevisionId === undefined || revision !== previousRevisionId)) {
      return { ok: true as const, failure: "" };
    }
    return { ok: false as const, save, unsettled, failure, revision };
  }, { timeout: 10_000 }).toEqual({ ok: true, failure: "" });
}

function selectAll(editor: HTMLElement, realm: Window & typeof globalThis): void {
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.selectNodeContents(editor);
  selection.removeAllRanges();
  selection.addRange(range);
}

async function openChapterEditor(): Promise<{
  frame: HTMLIFrameElement;
  projectId: string;
  chapterId: string;
}> {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Chinese and English input";
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
  title.value = "Input Novel";
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
    const projectId = root?.querySelector("form[data-rename]")?.getAttribute("data-rename");
    const chapterId = root?.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    )?.getAttribute("data-chapter-id");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null
      && typeof projectId === "string"
      && UUID.test(projectId)
      && typeof chapterId === "string"
      && UUID.test(chapterId);
  }, { timeout: 10_000 }).toBe(true);
  const root = appRoot(frame);
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

it("settles IME, clipboard, drop, and contiguous Block replacement without reusing identity", async () => {
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
  await waitSaved(root, before.revision_id);
  const helloRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  selectAll(editor, childWindow);
  expect(root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
    .toBe("0");
  expect(manuscriptBody(editor)).toBe("Hello");

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "enter" });
  await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(2);
  await waitSaved(root, helloRevisionId);
  const splitRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  await applyTrustedInput({ operation: "insert_text", text: "World" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello\nWorld");
  await waitSaved(root, splitRevisionId);
  const split = await readRevision(frame, projectId, chapterId);
  const rightId = split.blocks[1]?.manuscript_block_id;
  expect(split.blocks[0]?.manuscript_block_id).toBe(leftId);
  expect(rightId).toMatch(UUID);

  await updateClipboardPermission({ action: "grant" });
  await childWindow.navigator.clipboard.writeText("Alpha\nBeta");
  selectAll(editor, childWindow);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Alpha\nBeta");
  await waitSaved(root, split.revision_id);
  const pasted = await readRevision(frame, projectId, chapterId);
  expect(pasted.blocks.map((block) => block.text)).toEqual(["Alpha", "Beta"]);
  expect(pasted.blocks[0]?.manuscript_block_id).toBe(leftId);
  expect(pasted.blocks[1]?.manuscript_block_id).toMatch(UUID);
  expect(pasted.blocks[1]?.manuscript_block_id).not.toBe(rightId);

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyImeComposition({
    text: "取消",
    replacementStart: 4,
    replacementEnd: 4,
    selectionStart: 2,
    selectionEnd: 2,
  });
  await applyImeComposition({
    text: "",
    replacementStart: 4,
    replacementEnd: 4,
    selectionStart: 0,
    selectionEnd: 0,
  });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Alpha\nBeta");
  expect(root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
    .toBe("0");

  await applyImeComposition({
    text: "中文",
    replacementStart: 4,
    replacementEnd: 4,
    selectionStart: 2,
    selectionEnd: 2,
  });
  await applyTrustedInput({ operation: "insert_text", text: "中文" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Alpha\nBeta中文");
  await waitSaved(root, pasted.revision_id);
  const afterFirstIme = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  await applyImeComposition({
    text: "再",
    replacementStart: 2,
    replacementEnd: 2,
    selectionStart: 1,
    selectionEnd: 1,
  });
  await applyTrustedInput({ operation: "insert_text", text: "再" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Alpha\nBeta中文再");
  await waitSaved(root, afterFirstIme);
  const afterImeRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  const transfer = new childWindow.DataTransfer();
  transfer.setData("text/plain", "Drop");
  editor.dispatchEvent(new childWindow.DragEvent("drop", {
    bubbles: true,
    cancelable: true,
    dataTransfer: transfer,
  }));
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toMatch(/Drop/);
  await waitSaved(root, afterImeRevisionId);

  await destroyApplicationFrame(frame);
  const reopened = document.createElement("iframe");
  applicationFrame = reopened;
  reopened.title = "StoryOS exact-dist input reload";
  const reopenedLoaded = nextFrameLoad(reopened);
  reopened.src = `/projects/${projectId}`;
  document.body.append(reopened);
  await reopenedLoaded;
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks[0]?.manuscript_block_id).toBe(leftId);
  expect(reopenedRevision.blocks.some((block) => block.manuscript_block_id === rightId))
    .toBe(false);
  expect(reopenedRevision.body).toContain("Alpha");
  expect(reopenedRevision.body).toContain("中文");
  expect(reopenedRevision.body).toContain("再");
  expect(reopenedRevision.body).toContain("Drop");
});
