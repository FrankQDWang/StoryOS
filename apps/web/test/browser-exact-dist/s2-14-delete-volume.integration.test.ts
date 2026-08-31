import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist delete-volume page did not load"));
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

function volumeTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-volume-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

function volumeRow(frame: HTMLIFrameElement, title: string): Element | undefined {
  return [...(frame.contentDocument?.querySelectorAll("li[data-volume-id]") ?? [])]
    .find((item) => item.querySelector("[data-volume-title]")?.textContent?.trim() === title);
}

async function confirmDeleteVolume(frame: HTMLIFrameElement, title: string): Promise<void> {
  const row = volumeRow(frame, title);
  const start = row?.querySelector<HTMLButtonElement>("button[data-delete-volume]");
  if (start === undefined || start === null) {
    throw new Error(`the Delete Volume control is missing for ${title}`);
  }
  start.click();
  await expect.poll(() =>
    volumeRow(frame, title)?.querySelector("button[data-confirm-delete-volume]")?.tagName
  ).toBe("BUTTON");
  const confirm = volumeRow(frame, title)
    ?.querySelector<HTMLButtonElement>("button[data-confirm-delete-volume]");
  if (confirm === undefined || confirm === null) {
    throw new Error(`the Delete Volume confirmation is missing for ${title}`);
  }
  confirm.click();
}

it("the author cannot remove a nonempty Volume, then removes an empty Volume", { timeout: 90_000 }, async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Delete Volume";
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
  title.value = "Delete Volume Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");

  for (const [name, expected] of [
    ["Volume A", "Volume A"],
    ["Volume B", "Volume A\nVolume B"],
  ] as const) {
    await expect.poll(() =>
      frame.contentDocument?.querySelector('#app form[data-create-volume] input[name="volume-title"]')
        ?.tagName
    ).toBe("INPUT");
    const volumeTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
      '#app form[data-create-volume] input[name="volume-title"]',
    );
    const volumeForm = volumeTitle?.form;
    if (volumeTitle === null || volumeTitle === undefined
      || volumeForm === null || volumeForm === undefined) {
      throw new Error("the Create Volume form is missing");
    }
    volumeTitle.value = name;
    volumeForm.requestSubmit();
    await expect.poll(() => {
      const root = frame.contentDocument?.querySelector("#app");
      return volumeTitles(root).join("\n") === expected;
    }).toBe(true);
  }

  const chapterTitle = volumeRow(frame, "Volume B")
    ?.querySelector<HTMLInputElement>('form[data-create-chapter] input[name="chapter-title"]');
  const chapterForm = chapterTitle?.form;
  if (chapterTitle === undefined || chapterTitle === null
    || chapterForm === null || chapterForm === undefined) {
    throw new Error("the Create Chapter form for Volume B is missing");
  }
  chapterTitle.value = "Chapter B";
  chapterForm.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");

  await expect.poll(() =>
    volumeRow(frame, "Volume B")?.querySelector("button[data-delete-volume]")?.tagName
  ).toBe("BUTTON");
  await confirmDeleteVolume(frame, "Volume B");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.querySelector("[role=alert]")?.textContent === "无法删除卷。"
      && volumeTitles(root).join("\n") === "Volume A\nVolume B";
  }, { timeout: 15_000 }).toBe(true);

  await expect.poll(() =>
    volumeRow(frame, "Volume A")?.querySelector("button[data-delete-volume]")?.tagName
  ).toBe("BUTTON");
  await confirmDeleteVolume(frame, "Volume A");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const alertNode = root?.querySelector("[role=alert]");
    const alert = alertNode?.textContent;
    if (alert !== undefined && alert !== null && alert.length > 0 && alert !== "无法删除卷。") {
      throw new Error(alert);
    }
    return volumeTitles(root).join("\n") === "Volume B"
      && (root?.querySelector("[role=alert]")?.textContent ?? "") !== "无法删除卷。";
  }, { timeout: 15_000 }).toBe(true);
});
