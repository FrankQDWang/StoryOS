import { expect } from "vitest";

export async function beginInlineChapterCreation(root: ParentNode | null | undefined, volumeId?: string) {
  if (root?.querySelector("form[data-create-chapter]")) return;
  await expect.poll(() => root?.querySelector("li[data-volume-id]")?.tagName).toBe("LI");
  const selector = volumeId === undefined ? "[data-add-chapter]" : `[data-create-chapter-menu="${volumeId}"]`;
  await expect.poll(() => root?.querySelector<HTMLButtonElement>(selector)?.disabled).toBe(false);
  root?.querySelector<HTMLButtonElement>(selector)?.click();
  {
    await expect.poll(() => root?.querySelector<HTMLButtonElement>('[data-chapter-placement="append"]')?.disabled).toBe(false);
    root?.querySelector<HTMLButtonElement>('[data-chapter-placement="append"]')?.click();
  }
  await expect.poll(() => root?.querySelector("form[data-create-chapter] input")?.tagName).toBe("INPUT");
}

export async function beginInlineVolumeCreation(root: ParentNode | null | undefined) {
  if (root?.querySelector("form[data-create-volume]")) return;
  await expect.poll(() => root?.querySelector("[data-add-chapter]")?.tagName).toBe("BUTTON");
  root?.querySelector<HTMLButtonElement>("[data-add-chapter]")?.click();
  await expect.poll(() => root?.querySelector("[data-create-volume-action]")?.tagName).toBe("BUTTON");
  root?.querySelector<HTMLButtonElement>("[data-create-volume-action]")?.click();
  await expect.poll(() => root?.querySelector("form[data-create-volume] input")?.tagName).toBe("INPUT");
}

export async function beginTreeAction(root: ParentNode | null | undefined, rowSelector: string, action: string) {
  await expect.poll(() => root?.querySelector(rowSelector)?.querySelector("[data-project-menu], [data-create-chapter-menu], [data-chapter-menu]")?.tagName).toBe("BUTTON");
  const row = root?.querySelector(rowSelector);
  row?.querySelector<HTMLButtonElement>("[data-project-menu], [data-create-chapter-menu], [data-chapter-menu]")?.click();
  await expect.poll(() => row?.querySelector<HTMLButtonElement>(action)?.disabled).toBe(false);
}
