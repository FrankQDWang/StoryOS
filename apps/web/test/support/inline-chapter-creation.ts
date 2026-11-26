import { expect } from "vitest";
import { MANUSCRIPT_EDITOR_SELECTOR } from "./manuscript-surface.ts";

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

/**
 * Selects a Chapter in the outline. A chapter switch loads the outline again, so the click waits for the Chapter.
 *
 * An Arriving Chapter can be editable before the switch ends (ADR 0105), so an editable editor is not the end of the
 * switch. After the click, the helper waits for the end of the arrival: the outline marks the Chapter, no Arriving
 * Chapter is on the screen, and an editor is on the screen. With `{ arrival: "in-progress" }`, the helper returns
 * after the click, so that a journey can examine the arrival.
 */
export async function selectTreeChapter(
  root: ParentNode | null | undefined,
  chapterId: string,
  { arrival = "ended" }: { arrival?: "ended" | "in-progress" } = {},
) {
  const selector = `nav[aria-label="稿件目录"] button[data-chapter-id="${chapterId}"]`;
  await expect.poll(() => root?.querySelector(selector)?.tagName).toBe("BUTTON");
  root?.querySelector<HTMLButtonElement>(selector)?.click();
  if (arrival === "in-progress") return;
  await expect.poll(() => ({
    marked: root?.querySelector(selector)?.getAttribute("aria-current"),
    arriving: root?.querySelector("[data-arrival-no-open-suggestion]")?.getAttribute("data-arrival-no-open-suggestion"),
    editor: root?.querySelector(MANUSCRIPT_EDITOR_SELECTOR) != null,
  }), { timeout: 15_000 }).toEqual({ marked: "true", arriving: "", editor: true });
}
