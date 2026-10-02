import { page } from "vitest/browser";
import { beginInlineChapterCreation, beginInlineVolumeCreation } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { manuscriptBody, MANUSCRIPT_EDITOR_SELECTOR }
  from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist create-chapter page did not load"));
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

it("the author creates Chapters at inline menu positions and keeps the first current Chapter", async () => {
  await page.viewport(1487, 1058);
  const frame = document.createElement("iframe");
  frame.style.cssText = "width:1487px;height:1058px;border:0";
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Create Chapter";
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
  title.value = "Empty Novel";
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
  await expect.poll(() => frame.contentDocument?.querySelectorAll("li[data-volume-id]").length).toBe(1);
  await beginInlineVolumeCreation(frame.contentDocument);
  const nextVolumeTitle = frame.contentDocument!.querySelector<HTMLInputElement>('form[data-create-volume] input[name="volume-title"]')!;
  nextVolumeTitle.value = "Volume B"; nextVolumeTitle.form!.requestSubmit();
  await expect.poll(() => frame.contentDocument?.querySelectorAll("li[data-volume-id]").length).toBe(2);
  await beginInlineChapterCreation(frame.contentDocument,
    frame.contentDocument!.querySelector("li[data-volume-id]")!.getAttribute("data-volume-id")!);
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
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return editor !== null && editor !== undefined
      && manuscriptBody(editor) === ""
      && root?.querySelector("h2")?.textContent === "Chapter A"
      && chapterTitles(root).join("\n") === "Chapter A";
  }).toBe(true);

  const documentRoot = frame.contentDocument!;
  const open = async (index: number, pointer = false) => {
    const row = documentRoot.querySelectorAll<HTMLElement>("li[data-chapter-id]")[index]!;
    const trigger = row.querySelector<HTMLButtonElement>("[data-chapter-menu]")!;
    trigger.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    if (pointer) row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, clientX: 180, clientY: 240 }));
    else trigger.click();
    await expect.poll(() => documentRoot.querySelector(".chapter-creation-menu")?.tagName).toBe("DIV");
    const menu = documentRoot.querySelector<HTMLElement>(".chapter-creation-menu")!;
    const rect = trigger.getBoundingClientRect();
    await expect.poll(() => [menu.getBoundingClientRect().left, menu.getBoundingClientRect().top])
      .toEqual(pointer ? [180, 240] : [rect.left, rect.bottom + 4]);
    expect(documentRoot.defaultView!.getComputedStyle(menu).fontSize).toBe("12px");
    return [...menu.querySelectorAll<HTMLButtonElement>("button[data-chapter-placement]")].map((button) => button.dataset.chapterPlacement);
  };
  expect(await open(0, true)).toEqual(["after"]);
  await page.screenshot({ element: frame, path: "../../../../target/issue-254/chapter-pointer-menu.png" });
  documentRoot.querySelector<HTMLButtonElement>('[data-chapter-placement="after"]')!.click();
  const create = async (name: string, titles: string[]) => {
    await expect.poll(() => documentRoot.querySelector("form[data-create-chapter] input")?.tagName).toBe("INPUT");
    const input = documentRoot.querySelector<HTMLInputElement>('form[data-create-chapter] input')!;
    input.value = name;
    input.form!.requestSubmit();
    await expect.poll(() => chapterTitles(documentRoot.querySelector("#app"))).toEqual(titles);
    await expect.poll(() => documentRoot.querySelector("form[data-create-chapter]")).toBeNull();
  };
  await create("Chapter B", ["Chapter A", "Chapter B"]);
  expect(await open(1)).toEqual(["before"]);
  await page.screenshot({ element: frame, path: "../../../../target/issue-254/chapter-overflow-menu.png" });
  documentRoot.querySelector<HTMLButtonElement>('[data-chapter-placement="before"]')!.click();
  await expect.poll(() => documentRoot.querySelector(".inline-chapter-creation")?.tagName).toBe("LI");
  const inline = documentRoot.querySelector(".inline-chapter-creation")!;
  expect(inline.previousElementSibling?.querySelector("[data-chapter-title]")?.textContent).toBe("Chapter A");
  expect(inline.nextElementSibling?.querySelector("[data-chapter-title]")?.textContent).toBe("Chapter B");
  await page.screenshot({ element: frame, path: "../../../../target/issue-254/chapter-inline-title.png" });
  await create("Chapter C", ["Chapter A", "Chapter C", "Chapter B"]);
  expect(await open(0)).toEqual(["after"]);
  expect(await open(1)).toEqual(["before", "after"]);
  documentRoot.querySelector<HTMLButtonElement>('[data-chapter-placement="after"]')!.click();
  await expect.poll(() => documentRoot.querySelector("form[data-create-chapter] input")?.tagName).toBe("INPUT");
  documentRoot.querySelector<HTMLInputElement>('form[data-create-chapter] input')!
    .dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "Escape" }));
  await expect.poll(() => documentRoot.querySelector("form[data-create-chapter]")).toBeNull();
  expect(chapterTitles(documentRoot.querySelector("#app"))).toEqual(["Chapter A", "Chapter C", "Chapter B"]);

  await beginInlineChapterCreation(documentRoot);
  expect(documentRoot.querySelector("form[data-create-chapter]")?.closest("li[data-volume-id]")?.querySelector("[data-volume-title]")?.textContent).toBe("Volume B");
  await create("Chapter D", ["Chapter A", "Chapter C", "Chapter B", "Chapter D"]);
  const volumeA = documentRoot.querySelector("li[data-volume-id]")!;
  await beginInlineChapterCreation(documentRoot, volumeA.getAttribute("data-volume-id")!);
  await create("Chapter E", ["Chapter A", "Chapter C", "Chapter B", "Chapter E", "Chapter D"]);

  const root = frame.contentDocument?.querySelector("#app");
  const chapterItems = [...(root?.querySelectorAll('nav[aria-label="稿件目录"] > ul > li > ul > li') ?? [])];
  expect(chapterItems).toHaveLength(5);
  expect(chapterItems.map((item) => item.getAttribute("data-chapter-order"))).toEqual(["1", "2", "3", "4", "1"]);
  expect(root?.querySelector("h2")?.textContent).toBe("Chapter A");
  const editor = root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(editor === null || editor === undefined ? undefined : manuscriptBody(editor)).toBe("");
  expect(root?.textContent).not.toContain("模型");
  expect(root?.textContent).not.toContain("Agent");
  const reloaded = nextFrameLoad(frame);
  frame.src = `/projects/${root!.querySelector("[data-project-id]")!.getAttribute("data-project-id")}`;
  await reloaded;
  await expect.poll(() => chapterTitles(frame.contentDocument?.querySelector("#app")))
    .toEqual(["Chapter A", "Chapter C", "Chapter B", "Chapter E", "Chapter D"]);
  await expect.poll(() => frame.contentDocument?.querySelector("#app h2")?.textContent).toBe("Chapter A");
  expect(manuscriptBody(frame.contentDocument!.querySelector(MANUSCRIPT_EDITOR_SELECTOR)!)).toBe("");
  await page.screenshot({ element: frame, path: "../../../../target/issue-254/chapter-relative-reloaded.png" });
});
