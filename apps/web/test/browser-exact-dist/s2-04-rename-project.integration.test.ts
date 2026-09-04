import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist rename page did not load"));
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

afterEach(async () => {
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("the author renames one exact Project and the library plus opened title converge", async () => {
  const created = await loadApplication("StoryOS exact-dist create for rename");
  await expect.poll(() =>
    created.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = created.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Rename Source";
  form.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  await expect.poll(() => {
    const submit = created.contentDocument?.querySelector<HTMLButtonElement>(
      '#app form[data-rename] button[type="submit"]',
    );
    return submit !== null && submit !== undefined && !submit.disabled;
  }).toBe(true);
  const renameInput = created.contentDocument?.querySelector<HTMLInputElement>(
    '#app input[name="rename-title"]',
  );
  const renameForm = renameInput?.form;
  if (renameInput === null || renameInput === undefined || renameForm === null || renameForm === undefined) {
    throw new Error("the rename form is missing");
  }
  renameInput.value = "Renamed Novel";
  renameForm.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app h1")?.textContent
  ).toBe("Renamed Novel");
  await destroyApplicationFrame(created);

  const frame = await loadApplication("StoryOS exact-dist renamed library");
  await expect.poll(() =>
    [...(frame.contentDocument?.querySelectorAll("#app button[data-open=\"empty\"]") ?? [])]
      .some((button) => button.textContent === "Renamed Novel")
  ).toBe(true);
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the protected-ready library is missing");
  }
  expect(root.getAttribute("data-boot-state")).toBe("protected-ready");
  expect(root.textContent).not.toContain("Project B secret");
  expect(root.textContent).not.toContain("secret");
  expect(root.textContent).not.toContain("模型");
  expect(root.textContent).not.toContain("Agent");
  const renamedButton = [...root.querySelectorAll<HTMLButtonElement>('button[data-open="empty"]')]
    .find((button) => button.textContent === "Renamed Novel");
  renamedButton?.click();
  await expect.poll(() => root.getAttribute("data-boot-state")).toBe("empty-project-ready");
  expect(root.querySelector("h1")?.textContent).toBe("Renamed Novel");
  expect(root.querySelector("textarea")).toBeNull();
  expect(root.textContent).toContain("空工作区");
});
