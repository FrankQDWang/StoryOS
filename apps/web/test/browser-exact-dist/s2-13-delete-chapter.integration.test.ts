import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { manuscriptBody, MANUSCRIPT_EDITOR_SELECTOR }
  from "../support/manuscript-surface.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist delete-chapter page did not load"));
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
  return [...(root?.querySelectorAll('nav[aria-label="稿件目录"] [data-chapter-title]') ?? [])]
    .map((node) => node.textContent?.trim() ?? "");
}

async function createThreeChapters(frame: HTMLIFrameElement): Promise<void> {
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Delete Novel";
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
        && chapterTitles(root).join("\n") === expected;
    }).toBe(true);
  }
}

it("the author confirms Chapter removal, keeps the next current Chapter, then opens empty", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Delete Chapter";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await createThreeChapters(frame);
  const realm = frame.contentWindow;
  if (realm === null) throw new Error("the exact-dist application realm is unavailable");
  realm.confirm = () => true;

  const firstDelete = frame.contentDocument?.querySelector<HTMLButtonElement>(
    'button[data-delete-chapter]',
  );
  if (firstDelete === null || firstDelete === undefined) {
    throw new Error("the Delete Chapter control is missing");
  }
  firstDelete.click();
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter B\nChapter C"
      && root?.querySelector("h2")?.textContent === "Chapter B";
  }).toBe(true);

  const secondDelete = frame.contentDocument?.querySelector<HTMLButtonElement>(
    'button[data-delete-chapter]',
  );
  if (secondDelete === null || secondDelete === undefined) {
    throw new Error("the later Delete Chapter control is missing");
  }
  secondDelete.click();
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && chapterTitles(root).join("\n") === "Chapter C"
      && root?.querySelector("h2")?.textContent === "Chapter C";
  }).toBe(true);

  const lastDelete = frame.contentDocument?.querySelector<HTMLButtonElement>(
    'button[data-delete-chapter]',
  );
  if (lastDelete === null || lastDelete === undefined) {
    throw new Error("the last Delete Chapter control is missing");
  }
  lastDelete.click();
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "empty-project-ready"
      && chapterTitles(root).join("\n") === "";
  }).toBe(true);
  const editor = frame.contentDocument?.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  expect(editor === null || editor === undefined ? undefined : manuscriptBody(editor)).toBe(undefined);
});
