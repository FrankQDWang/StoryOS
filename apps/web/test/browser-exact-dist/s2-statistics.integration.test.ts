import { selectTreeChapter } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { manuscriptIsEditable, MANUSCRIPT_EDITOR_SELECTOR } from "../support/manuscript-surface.ts";
import {
  appRoot,
  chapterButton,
  closeApplicationFrame,
  openApplicationFrame,
  openFirstChapterForm,
  submitChapterForm,
  submitProjectForm,
  submitVolumeForm,
  typeIntoCurrent,
  waitLastChapter,
} from "../support/exact-dist-frame.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const CHAPTER_A = "Hello world";
const CHAPTER_B = "雨落在窗沿。";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

async function waitStatistics(
  frame: HTMLIFrameElement,
  expected: {
    chapterWords: string;
    chapterCharacters: string;
    manuscriptWords: string;
    manuscriptCharacters: string;
    chapterCount: string;
  },
): Promise<Element> {
  const root = appRoot(frame);
  await expect.poll(() => {
    const node = root.querySelector("[data-statistics-outcome='ready']");
    if (node === null) return undefined;
    return [
      node.getAttribute("data-statistics-chapter-words"),
      node.getAttribute("data-statistics-chapter-characters"),
      node.getAttribute("data-statistics-manuscript-words"),
      node.getAttribute("data-statistics-manuscript-characters"),
      node.getAttribute("data-statistics-chapter-count"),
    ].join(":");
  }, { timeout: 10_000 }).toBe([
    expected.chapterWords,
    expected.chapterCharacters,
    expected.manuscriptWords,
    expected.manuscriptCharacters,
    expected.chapterCount,
  ].join(":"));
  const outcome = root.querySelector("[data-statistics-outcome='ready']");
  if (outcome === null) throw new Error("the statistics outcome is missing");
  return outcome;
}

it("rebuilds Chapter and manuscript statistics after edit, switch, and deletion", { timeout: 120_000 }, async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist manuscript statistics");
  await submitProjectForm(frame, "Statistics Novel");
  await submitVolumeForm(frame, "Volume A");
  await openFirstChapterForm(frame);
  for (const name of ["Chapter A", "Chapter B"] as const) {
    await submitChapterForm(frame, name);
    await waitLastChapter(frame, name);
  }

  const root = appRoot(frame);
  const chapterAId = chapterButton(root, "Chapter A")?.getAttribute("data-chapter-id");
  const chapterBId = chapterButton(root, "Chapter B")?.getAttribute("data-chapter-id");
  if (chapterAId === null || chapterAId === undefined
    || chapterBId === null || chapterBId === undefined) {
    throw new Error("the Chapter identity is missing");
  }
  expect(chapterButton(root, "Chapter A")?.getAttribute("aria-current")).toBe("true");
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter A"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, CHAPTER_A);
  const afterA = await waitStatistics(frame, {
    chapterWords: "2",
    chapterCharacters: "11",
    manuscriptWords: "2",
    manuscriptCharacters: "11",
    chapterCount: "2",
  });
  expect(afterA.getAttribute("data-statistics-lag")).toBe("0");
  expect(afterA.getAttribute("data-statistics-snapshot-id")).toMatch(UUID);
  expect(afterA.getAttribute("data-statistics-watermark")).toMatch(/^[1-9][0-9]*$/);
  expect(afterA.getAttribute("data-statistics-counting-profile"))
    .toBe("storyos.statistics.unicode-16.0.0.v1");

  await selectTreeChapter(root, chapterBId);
  await expect.poll(() => {
    const nextRoot = appRoot(frame);
    const editor = nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
    return nextRoot.querySelector("h2")?.textContent === "Chapter B"
      && editor !== null
      && manuscriptIsEditable(editor);
  }, { timeout: 15_000 }).toBe(true);
  await typeIntoCurrent(frame, CHAPTER_B);
  await waitStatistics(frame, {
    chapterWords: "1",
    chapterCharacters: "6",
    manuscriptWords: "3",
    manuscriptCharacters: "17",
    chapterCount: "2",
  });

  const row = [...(frame.contentDocument?.querySelectorAll("li[data-chapter-id]") ?? [])]
    .find((item) => item.getAttribute("data-chapter-id") === chapterBId);
  row?.querySelector<HTMLButtonElement>("[data-chapter-menu]")?.click();
  await expect.poll(() => row?.querySelector("button[data-delete-chapter]")?.tagName).toBe("BUTTON");
  row?.querySelector<HTMLButtonElement>("button[data-delete-chapter]")?.click();
  await expect.poll(() =>
    row?.querySelector("button[data-confirm-delete-chapter]")?.tagName
  ).toBe("BUTTON");
  row?.querySelector<HTMLButtonElement>("button[data-confirm-delete-chapter]")?.click();
  await expect.poll(() => {
    const next = appRoot(frame);
    return next.querySelector(`[data-chapter-id="${chapterBId}"]`) === null
      && next.querySelector("h2")?.textContent === "Chapter A";
  }, { timeout: 15_000 }).toBe(true);

  await waitStatistics(frame, {
    chapterWords: "2",
    chapterCharacters: "11",
    manuscriptWords: "2",
    manuscriptCharacters: "11",
    chapterCount: "1",
  });
});
