import { afterEach, expect, it } from "vitest";

import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist delete-chapter page did not load"));
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

function chapterTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-chapter-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

async function waitSaved(frame: HTMLIFrameElement): Promise<void> {
  await expect.poll(() => {
    const node = frame.contentDocument?.querySelector("[data-save-state]");
    return node?.getAttribute("data-save-state") === "saved"
      && node.getAttribute("data-unsettled-intent-count") === "0";
  }, { timeout: 10_000 }).toBe(true);
}

async function typeIntoCurrent(frame: HTMLIFrameElement, text: string): Promise<void> {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the production page root is missing");
  }
  const realm = frame.contentWindow;
  if (realm === null) throw new Error("the exact-dist application realm is unavailable");
  const editor = manuscriptEditor(root, realm as Window & typeof globalThis);
  editor.focus();
  focusManuscriptEnd(editor, realm as Window & typeof globalThis);
  await applyTrustedInput({ operation: "insert_text", text });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(text);
  await waitSaved(frame);
}

function currentChapterRow(frame: HTMLIFrameElement): Element | undefined {
  const heading = frame.contentDocument?.querySelector("h2")?.textContent;
  return [...(frame.contentDocument?.querySelectorAll("li[data-chapter-id]") ?? [])]
    .find((item) => item.querySelector("[data-chapter-title]")?.textContent?.trim() === heading);
}

async function confirmCurrentDelete(frame: HTMLIFrameElement): Promise<void> {
  const row = currentChapterRow(frame);
  const start = row?.querySelector<HTMLButtonElement>("button[data-delete-chapter]");
  if (start === undefined || start === null) {
    throw new Error("the Delete Chapter control is missing");
  }
  start.click();
  await expect.poll(() =>
    currentChapterRow(frame)?.querySelector("button[data-confirm-delete-chapter]")?.tagName
  ).toBe("BUTTON");
  const confirm = currentChapterRow(frame)
    ?.querySelector<HTMLButtonElement>("button[data-confirm-delete-chapter]");
  if (confirm === undefined || confirm === null) {
    throw new Error("the Delete Chapter confirmation is missing");
  }
  confirm.click();
}

async function createThreeChapters(frame: HTMLIFrameElement): Promise<void> {
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Delete Novel";
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
    frame.contentDocument?.querySelector('#app form[data-create-chapter] input[name="chapter-title"]')
      ?.tagName
  ).toBe("INPUT");

  const firstChapter = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const firstForm = firstChapter?.form;
  if (firstChapter === null || firstChapter === undefined
    || firstForm === null || firstForm === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  firstChapter.value = "Chapter A";
  firstForm.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");

  for (const [name, expected] of [
    ["Chapter B", "Chapter A\nChapter B"],
    ["Chapter C", "Chapter A\nChapter B\nChapter C"],
  ] as const) {
    const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
      '#app form[data-create-chapter] input[name="chapter-title"]',
    );
    const chapterForm = chapterTitle?.form;
    if (chapterTitle === null || chapterTitle === undefined
      || chapterForm === null || chapterForm === undefined) {
      throw new Error("the later Create Chapter form is missing");
    }
    chapterTitle.value = name;
    chapterForm.requestSubmit();
    await expect.poll(() => {
      const root = frame.contentDocument?.querySelector("#app");
      return root?.getAttribute("data-boot-state") === "project-ready"
        && chapterTitles(root).join("\n") === expected;
    }).toBe(true);
  }
}

it("the author confirms Chapter removal, keeps the next current Chapter, then opens empty", { timeout: 90_000 }, async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Delete Chapter";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await createThreeChapters(frame);
  await typeIntoCurrent(frame, "Alpha");
  await confirmCurrentDelete(frame);
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const alert = root?.querySelector("[role=alert]")?.textContent;
    if (alert !== undefined && alert !== null && alert.length > 0) {
      throw new Error(alert);
    }
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter B\nChapter C"
      && root?.querySelector("h2")?.textContent === "Chapter B";
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, "Beta");
  await confirmCurrentDelete(frame);
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter C"
      && root?.querySelector("h2")?.textContent === "Chapter C";
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, "Gamma");
  await confirmCurrentDelete(frame);
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "empty-project-ready"
      && chapterTitles(root).join("\n") === "";
  }, { timeout: 15_000 }).toBe(true);
  const editor = frame.contentDocument?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(editor === null || editor === undefined ? undefined : manuscriptBody(editor)).toBe(undefined);
});
