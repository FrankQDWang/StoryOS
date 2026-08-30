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
      reject(new Error("the exact-dist save-truth page did not load"));
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

function saveNode(root: Element): Element | null {
  return root.querySelector("[data-save-state]");
}

async function waitSaved(root: Element): Promise<void> {
  await expect.poll(() => {
    const node = saveNode(root);
    return node?.getAttribute("data-save-state") === "saved"
      && node.getAttribute("data-unsettled-intent-count") === "0"
      ? "saved"
      : node?.getAttribute("data-save-state") ?? null;
  }, { timeout: 10_000 }).toBe("saved");
}

it("shows pending, saving, and saved without calling local input saved, across Chapters", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist save truth";
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
  title.value = "Save Truth Novel";
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
  for (const name of ["Chapter A", "Chapter B"] as const) {
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
  expect(saveNode(root)?.getAttribute("data-save-state")).toBe("clean");
  expect(saveNode(root)?.textContent).not.toContain("已保存");
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
  expect(seenWhileLocal.some((state) => state === "clean" || state === "saving")).toBe(true);
  await waitSaved(root);
  expect(saveNode(root)?.textContent).toContain("已保存");

  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  root.querySelector<HTMLButtonElement>(
    `[data-make-current-chapter="${chapterBId}"]`,
  )?.click();
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
  expect(saveNode(switchedRoot)?.getAttribute("data-save-state")).toBe("clean");

  switchedEditor.focus();
  focusManuscriptEnd(switchedEditor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text: "Beta prose" });
  await expect.poll(() => manuscriptBody(switchedEditor), { timeout: 10_000 }).toBe("Beta prose");
  await waitSaved(switchedRoot);

  chapterButton(switchedRoot, "Chapter A")?.click();
  await expect.poll(() => switchedRoot.querySelector("h2")?.textContent).toBe("Chapter A");
  const inspectEditor = switchedRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(inspectEditor === null || inspectEditor === undefined
    ? undefined : manuscriptBody(inspectEditor)).toBe("Alpha prose");
  expect(inspectEditor === null || inspectEditor === undefined
    ? undefined : manuscriptIsEditable(inspectEditor)).toBe(false);
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
  expect(reopenedRoot.querySelector("h2")?.textContent).toBe("Chapter B");
  expect(manuscriptBody(manuscriptEditor(reopenedRoot, applicationWindow(frame))))
    .toBe("Beta prose");
  await waitSaved(reopenedRoot);
});
