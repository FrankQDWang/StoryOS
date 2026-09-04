import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { manuscriptBody, manuscriptIsEditable, MANUSCRIPT_EDITOR_SELECTOR }
  from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist chapter navigation page did not load"));
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

function chapterButton(root: Element | null | undefined, title: string): HTMLButtonElement | undefined {
  return [...(root?.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  ) ?? [])].find((button) => button.textContent === title);
}

async function createThreeChapterProject(frame: HTMLIFrameElement): Promise<void> {
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Navigate Reopen Novel";
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
  for (const name of ["Chapter A", "Chapter B", "Chapter C"] as const) {
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
}

it("the author opens each Chapter from the tree and reopens the current Chapter", async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Chapter navigation";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await createThreeChapterProject(frame);

  const root = frame.contentDocument?.querySelector("#app");
  expect(root?.querySelector("h2")?.textContent).toBe("Chapter A");
  chapterButton(root, "Chapter B")?.click();
  await expect.poll(() => root?.querySelector("h2")?.textContent, { timeout: 10_000 })
    .toBe("Chapter B");
  const chapterBEditor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(chapterBEditor === null || chapterBEditor === undefined
    ? undefined : manuscriptBody(chapterBEditor)).toBe("");
  expect(chapterBEditor === null || chapterBEditor === undefined
    ? undefined : manuscriptIsEditable(chapterBEditor)).toBe(false);
  expect(root?.querySelector('[role="alert"]')).toBeNull();

  chapterButton(root, "Chapter C")?.click();
  await expect.poll(() => root?.querySelector("h2")?.textContent).toBe("Chapter C");
  const chapterCEditor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(chapterCEditor === null || chapterCEditor === undefined
    ? undefined : manuscriptBody(chapterCEditor)).toBe("");

  chapterButton(root, "Chapter A")?.click();
  await expect.poll(() => root?.querySelector("h2")?.textContent).toBe("Chapter A");
  const chapterAEditor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(chapterAEditor === null || chapterAEditor === undefined
    ? undefined : manuscriptBody(chapterAEditor)).toBe("");

  const projectId = root?.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  const childWindow = frame.contentWindow;
  if (root === null || root === undefined
    || childWindow === null
    || projectId === null || projectId === undefined
    || chapterBId === null || chapterBId === undefined) {
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
