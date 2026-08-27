import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";

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
  const volumeItem = root?.querySelector('nav[aria-label="稿件目录"] > ul > li');
  return [...(volumeItem?.querySelectorAll(":scope > ul > li") ?? [])].map((item) =>
    item.textContent?.trim() ?? "",
  );
}

it("the author creates three named Chapters and keeps the first current Chapter", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
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
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.querySelector("textarea")?.value === ""
      && root?.querySelector("h2")?.textContent === "Chapter A"
      && chapterTitles(root).join("\n") === "Chapter A";
  }).toBe(true);

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
        && root?.querySelector("h2")?.textContent === "Chapter A"
        && root?.querySelector("textarea") !== null
        && chapterTitles(root).join("\n") === expected;
    }).toBe(true);
  }

  const root = frame.contentDocument?.querySelector("#app");
  const chapterItems = [...(root?.querySelectorAll('nav[aria-label="稿件目录"] > ul > li > ul > li') ?? [])];
  expect(chapterItems).toHaveLength(3);
  expect(root?.querySelector("h2")?.textContent).toBe("Chapter A");
  expect(root?.querySelector("textarea")?.value).toBe("");
  expect(root?.textContent).not.toContain("模型");
  expect(root?.textContent).not.toContain("Agent");
});
