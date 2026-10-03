import { beginInlineChapterCreation, beginInlineVolumeCreation } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  applyImeComposition,
  applyTrustedInput,
  updateClientSessionCookie,
} from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const ROUNDS = 10;
const PHRASES = ["我们", "写作"] as const;

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist sustained-writing page did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

afterEach(async () => {
  if (applicationFrame?.isConnected) {
    const unloaded = nextFrameLoad(applicationFrame);
    applicationFrame.src = "about:blank";
    await unloaded;
    applicationFrame.remove();
  }
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

function submitTitle(frame: HTMLIFrameElement, selector: string, value: string): void {
  const input = frame.contentDocument?.querySelector<HTMLInputElement>(selector);
  if (input === null || input === undefined || input.form === null) {
    throw new Error(`the form for ${selector} is missing`);
  }
  input.value = value;
  input.form.requestSubmit();
}

function saveFacts(root: Element) {
  const node = root.querySelector("[data-save-state]");
  return {
    save: node?.getAttribute("data-save-state"),
    unsettled: node?.getAttribute("data-unsettled-intent-count"),
    failure: node?.getAttribute("data-editor-failure") ?? "",
    revision: node?.getAttribute("data-authoritative-revision-id") ?? "",
  };
}

async function waitSaved(root: Element, previousRevisionId: string): Promise<string> {
  await expect.poll(() => {
    const facts = saveFacts(root);
    return facts.save === "saved" && facts.unsettled === "0" && facts.failure === ""
      && facts.revision !== previousRevisionId ? "saved" : facts;
  }, { timeout: 10_000 }).toBe("saved");
  return saveFacts(root).revision;
}

it("saves sustained Chinese composition with frequent Block boundaries without a quota reset", {
  timeout: 180_000,
}, async () => {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist sustained writing";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  const childWindow = frame.contentWindow as (Window & typeof globalThis) | null;
  if (childWindow === null) throw new Error("the production page realm is unavailable");
  childWindow.performance.setResourceTimingBufferSize(2_000);
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName).toBe("INPUT");
  submitTitle(frame, '#app input[name="title"]', "Sustained Writing Novel");
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state"))
    .toBe("empty-project-ready");
  await beginInlineVolumeCreation(frame.contentDocument);
  submitTitle(frame, '#app form[data-create-volume] input[name="volume-title"]', "Volume A");
  await beginInlineChapterCreation(frame.contentDocument);
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app form[data-create-chapter]") !== null).toBe(true);
  submitTitle(frame, '#app form[data-create-chapter] input[name="chapter-title"]', "Chapter A");
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null
      && root.querySelector('nav[aria-label="稿件目录"] button[aria-current="true"]') !== null;
  }, { timeout: 10_000 }).toBe(true);
  const root = frame.contentDocument!.querySelector("#app")!;
  const projectId = root.querySelector("[data-project-id]")!.getAttribute("data-project-id")!;
  const chapterId = root.querySelector('nav[aria-label="稿件目录"] button[aria-current="true"]')!
    .getAttribute("data-chapter-id")!;
  const editor = manuscriptEditor(root, childWindow);
  editor.focus();
  focusManuscriptEnd(editor, childWindow);

  const started = childWindow.performance.now();
  let revision = saveFacts(root).revision;
  let expected = "";
  for (let round = 0; round < ROUNDS; round += 1) {
    for (const phrase of PHRASES) {
      const offset = manuscriptBody(editor).length;
      await applyImeComposition({ text: phrase, replacementStart: offset, replacementEnd: offset,
        selectionStart: phrase.length, selectionEnd: phrase.length });
      await applyTrustedInput({ operation: "insert_text", text: phrase });
      expected += phrase;
      await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(expected);
      revision = await waitSaved(root, revision);
    }
    await applyTrustedInput({ operation: "enter" });
    await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(2);
    revision = await waitSaved(root, revision);
    await applyTrustedInput({ operation: "backspace" });
    await expect.poll(() => editor.querySelectorAll("p").length, { timeout: 10_000 }).toBe(1);
    revision = await waitSaved(root, revision);
  }
  const elapsedSeconds = (childWindow.performance.now() - started) / 1_000;

  const requests = childWindow.performance.getEntriesByType("resource")
    .filter((entry): entry is PerformanceResourceTiming => entry instanceof childWindow.PerformanceResourceTiming)
    .filter((entry) => entry.startTime >= started);
  const challenges = requests.filter((entry) => entry.name.endsWith("/anti-forgery-challenges"));
  const authorEdits = requests.filter((entry) => entry.name.endsWith("/manuscript/author-edits"));
  console.info(JSON.stringify({ issue: 880, elapsedSeconds, challengeRequests: challenges.length,
    authorEditRequests: authorEdits.length,
    challengeStatuses: [...new Set(challenges.map((entry) => entry.responseStatus))] }));
  const chapter = await getChapter({ baseUrl: childWindow.location.origin, projectId, chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow) });

  expect({
    authorEditRequests: authorEdits.length,
    challengeStatuses: [...new Set(challenges.map((entry) => entry.responseStatus))],
    text: chapter.chapter.current_revision.blocks.map((block) => block.text),
  }).toEqual({
    authorEditRequests: ROUNDS * (PHRASES.length + 2),
    challengeStatuses: [200],
    text: ["我们写作".repeat(ROUNDS)],
  });
});
