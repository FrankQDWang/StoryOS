import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist update-volume page did not load"));
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

async function loadApplication(title: string): Promise<HTMLIFrameElement> {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = title;
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  return frame;
}

function volumeTitles(root: Element | null | undefined): string[] {
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-volume-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

async function createNamedVolume(root: Document, title: string): Promise<void> {
  const volumeTitle = root.querySelector<HTMLInputElement>(
    '#app form[data-create-volume] input[name="volume-title"]',
  );
  const volumeForm = volumeTitle?.form;
  if (volumeTitle === null || volumeTitle === undefined
    || volumeForm === null || volumeForm === undefined) {
    throw new Error("the Create Volume form is missing");
  }
  volumeTitle.value = title;
  volumeForm.requestSubmit();
}

afterEach(async () => {
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("the author renames and reorders Volumes from the canonical tree and they survive reopen", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const created = await loadApplication("StoryOS exact-dist create for volume update");
  await expect.poll(() =>
    created.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = created.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Empty Novel";
  form.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  const createdRoot = created.contentDocument;
  if (createdRoot === null || createdRoot === undefined) {
    throw new Error("the empty Project document is missing");
  }
  await createNamedVolume(createdRoot, "Volume A");
  await expect.poll(() => volumeTitles(createdRoot.querySelector("#app"))).toEqual(["Volume A"]);
  await createNamedVolume(createdRoot, "Volume B");
  await expect.poll(() => volumeTitles(createdRoot.querySelector("#app"))).toEqual([
    "Volume A",
    "Volume B",
  ]);

  const renameInput = createdRoot.querySelector<HTMLInputElement>(
    '#app form[data-rename-volume] input[name="volume-title"]',
  );
  const renameForm = renameInput?.form;
  if (renameInput === null || renameInput === undefined
    || renameForm === null || renameForm === undefined) {
    throw new Error("the Volume rename form is missing");
  }
  renameInput.value = "Volume C";
  renameForm.requestSubmit();
  await expect.poll(() => volumeTitles(createdRoot.querySelector("#app"))).toEqual([
    "Volume C",
    "Volume B",
  ]);
  await destroyApplicationFrame(created);

  const reopened = await loadApplication("StoryOS exact-dist renamed Volume library");
  await expect.poll(() =>
    [...(reopened.contentDocument?.querySelectorAll('#app button[data-open="empty"]') ?? [])]
      .some((button) => button.textContent === "Empty Novel")
  ).toBe(true);
  const libraryRoot = reopened.contentDocument?.querySelector("#app");
  if (libraryRoot === null || libraryRoot === undefined) {
    throw new Error("the protected-ready library is missing");
  }
  const openButton = [...libraryRoot.querySelectorAll<HTMLButtonElement>('button[data-open="empty"]')]
    .find((button) => button.textContent === "Empty Novel");
  openButton?.click();
  await expect.poll(() => libraryRoot.getAttribute("data-boot-state")).toBe("empty-project-ready");
  await expect.poll(() => volumeTitles(libraryRoot)).toEqual(["Volume C", "Volume B"]);
  expect(libraryRoot.querySelector("textarea")).toBeNull();
  expect(libraryRoot.textContent).not.toContain("模型");
  expect(libraryRoot.textContent).not.toContain("Agent");

  const moveDown = libraryRoot.querySelector<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] li[data-volume-order="1"] button[data-volume-move="down"]',
  );
  if (moveDown === null) {
    throw new Error("the canonical first Volume has no move-down control");
  }
  moveDown.click();
  await expect.poll(() => volumeTitles(libraryRoot)).toEqual(["Volume B", "Volume C"]);
  await destroyApplicationFrame(reopened);

  const ordered = await loadApplication("StoryOS exact-dist reordered Volume library");
  await expect.poll(() =>
    [...(ordered.contentDocument?.querySelectorAll('#app button[data-open="empty"]') ?? [])]
      .some((button) => button.textContent === "Empty Novel")
  ).toBe(true);
  const orderedRoot = ordered.contentDocument?.querySelector("#app");
  if (orderedRoot === null || orderedRoot === undefined) {
    throw new Error("the reopened library is missing");
  }
  const reopenButton = [...orderedRoot.querySelectorAll<HTMLButtonElement>('button[data-open="empty"]')]
    .find((button) => button.textContent === "Empty Novel");
  reopenButton?.click();
  await expect.poll(() => orderedRoot.getAttribute("data-boot-state")).toBe("empty-project-ready");
  await expect.poll(() => volumeTitles(orderedRoot)).toEqual(["Volume B", "Volume C"]);
  const first = orderedRoot.querySelector('nav[aria-label="稿件目录"] li[data-volume-order="1"]');
  const second = orderedRoot.querySelector('nav[aria-label="稿件目录"] li[data-volume-order="2"]');
  expect(first?.querySelector("[data-volume-title]")?.textContent?.trim()).toBe("Volume B");
  expect(second?.querySelector("[data-volume-title]")?.textContent?.trim()).toBe("Volume C");
});
