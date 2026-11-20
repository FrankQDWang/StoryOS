import { beginInlineChapterCreation, beginInlineVolumeCreation } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  closeApplicationFrame,
  openApplicationFrame,
  submitProjectForm,
  submitVolumeForm,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
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
  await expect.poll(() => volumeRow(frame, title)?.querySelector("[data-create-chapter-menu]")?.tagName).toBe("BUTTON");
  const row = volumeRow(frame, title);
  row?.querySelector<HTMLButtonElement>("[data-create-chapter-menu], [data-chapter-menu]")?.click();
  await expect.poll(() => row?.querySelector("button[data-delete-volume], button[data-delete-chapter]")?.tagName).toBe("BUTTON");
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
  const frame = await openApplicationFrame("StoryOS exact-dist Delete Volume");

  await submitProjectForm(frame, "Delete Volume Novel");

  for (const [name, expected] of [
    ["Volume A", "Volume A"],
    ["Volume B", "Volume A\nVolume B"],
  ] as const) {
    await beginInlineVolumeCreation(frame.contentDocument);
    await expect.poll(() =>
      frame.contentDocument?.querySelector('#app form[data-create-volume] input[name="volume-title"]')
        ?.tagName
    ).toBe("INPUT");
    await submitVolumeForm(frame, name);
    await expect.poll(() => {
      const root = frame.contentDocument?.querySelector("#app");
      return volumeTitles(root).join("\n") === expected;
    }).toBe(true);
  }

  await beginInlineChapterCreation(frame.contentDocument, volumeRow(frame, "Volume B")?.getAttribute("data-volume-id") ?? undefined);
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

  await confirmDeleteVolume(frame, "Volume B");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.querySelector("[role=alert]")?.textContent === "无法删除卷。"
      && volumeTitles(root).join("\n") === "Volume A\nVolume B";
  }, { timeout: 15_000 }).toBe(true);

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
