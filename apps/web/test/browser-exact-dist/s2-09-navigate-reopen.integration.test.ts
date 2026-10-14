import { beginInlineChapterCreation, beginInlineVolumeCreation, selectTreeChapter } from "../support/inline-chapter-creation.ts";
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
  for (const name of ["Chapter A", "Chapter B", "Chapter C"] as const) {
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
  const childWindow = frame.contentWindow;
  if (root === null || root === undefined
    || childWindow === null
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
