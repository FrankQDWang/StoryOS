import { expect } from "vitest";

export async function beginInlineChapterCreation(root: ParentNode | null | undefined, volumeId?: string) {
  if (root?.querySelector("form[data-create-chapter]")) return;
  const selector = volumeId === undefined ? "[data-add-chapter]" : `[data-create-chapter-menu="${volumeId}"]`;
  await expect.poll(() => root?.querySelector<HTMLButtonElement>(selector)?.disabled).toBe(false);
  root?.querySelector<HTMLButtonElement>(selector)?.click();
  if (volumeId !== undefined) {
    await expect.poll(() => root?.querySelector('[data-chapter-placement="append"]')?.tagName).toBe("BUTTON");
    root?.querySelector<HTMLButtonElement>('[data-chapter-placement="append"]')?.click();
  }
  await expect.poll(() => root?.querySelector("form[data-create-chapter] input")?.tagName).toBe("INPUT");
}
