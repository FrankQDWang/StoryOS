import { afterEach, expect, it } from "vitest";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { applyTrustedInput, updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
} from "../support/manuscript-surface.ts";

const SETTLED = "Settled prose";
const FULL = "Settled prose more";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist Local Edit Journal recovery page did not load"));
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

function applicationWindow(frame: HTMLIFrameElement): Window & typeof globalThis {
  const result = frame.contentWindow;
  if (result === null) throw new Error("the production page realm is unavailable");
  return result as Window & typeof globalThis;
}

function appRoot(frame: HTMLIFrameElement): Element {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the production page root is missing");
  }
  return root;
}

function saveNode(root: Element): Element | null {
  return root.querySelector("[data-save-state]");
}

async function waitSaved(root: Element): Promise<void> {
  await expect.poll(() => {
    const node = saveNode(root);
    const failure = node?.getAttribute("data-editor-failure") ?? "";
    return node?.getAttribute("data-save-state") === "saved"
      && node.getAttribute("data-unsettled-intent-count") === "0"
      && failure === ""
      ? "saved"
      : {
        save: node?.getAttribute("data-save-state") ?? null,
        unsettled: node?.getAttribute("data-unsettled-intent-count") ?? null,
        failure,
      };
  }, { timeout: 15_000 }).toBe("saved");
}

async function reloadProject(
  frame: HTMLIFrameElement,
  projectId: string,
): Promise<Element> {
  const reopened = nextFrameLoad(frame);
  frame.src = `/projects/${projectId}`;
  await reopened;
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");
  return appRoot(frame);
}

it("recovers Local Edit Journal text after reload without a second Author Edit", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Local Edit Journal recovery";
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
  title.value = "Journal Recovery Novel";
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
    frame.contentDocument?.querySelector('#app form[data-create-chapter]') !== null
  ).toBe(true);
  const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const chapterForm = chapterTitle?.form;
  if (chapterTitle === null || chapterTitle === undefined
    || chapterForm === null || chapterForm === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  chapterTitle.value = "Chapter A";
  chapterForm.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("project-ready");

  let root = appRoot(frame);
  const realm = applicationWindow(frame);
  let editor = manuscriptEditor(root, realm);
  editor.focus();
  focusManuscriptEnd(editor, realm);
  await applyTrustedInput({ operation: "insert_text", text: SETTLED });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(SETTLED);
  await waitSaved(root);

  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterId = root.querySelector<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }

  root = await reloadProject(frame, projectId);
  editor = manuscriptEditor(root, applicationWindow(frame));
  expect(manuscriptBody(editor)).toBe(SETTLED);
  await waitSaved(root);

  editor.focus();
  focusManuscriptEnd(editor, applicationWindow(frame));
  await applyTrustedInput({ operation: "insert_text", text: " more" });
  await expect.poll(() => {
    const node = saveNode(root);
    if (manuscriptBody(editor) !== FULL) return null;
    const save = node?.getAttribute("data-save-state");
    const unsettled = node?.getAttribute("data-unsettled-intent-count");
    if (save === "saving" || (unsettled !== null && unsettled !== undefined && unsettled !== "0")) {
      return "pending";
    }
    if (save === "saved" && unsettled === "0") return "saved";
    return null;
  }, { timeout: 10_000 }).toMatch(/^(pending|saved)$/);

  root = await reloadProject(frame, projectId);
  editor = manuscriptEditor(root, applicationWindow(frame));
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe(FULL);
  await waitSaved(root);

  const afterReload = applicationWindow(frame);
  const firstRecovered = await getChapter({
    baseUrl: afterReload.location.origin,
    projectId,
    chapterId,
    fetchImpl: afterReload.fetch.bind(afterReload),
  });
  expect(firstRecovered.chapter.current_revision.body).toBe(FULL);

  root = await reloadProject(frame, projectId);
  editor = manuscriptEditor(root, applicationWindow(frame));
  expect(manuscriptBody(editor)).toBe(FULL);
  await waitSaved(root);
  const afterRepeat = applicationWindow(frame);
  const repeated = await getChapter({
    baseUrl: afterRepeat.location.origin,
    projectId,
    chapterId,
    fetchImpl: afterRepeat.fetch.bind(afterRepeat),
  });
  expect(repeated.chapter.current_revision).toEqual(firstRecovered.chapter.current_revision);
});
