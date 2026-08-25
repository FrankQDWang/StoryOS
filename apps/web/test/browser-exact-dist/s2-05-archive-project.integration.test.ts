import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist archive page did not load"));
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

it("the author archives one exact Project and the library fails closed on open and write", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const created = await loadApplication("StoryOS exact-dist create for archive");
  await expect.poll(() =>
    created.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = created.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Archive Source";
  form.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  await expect.poll(() => {
    const submit = created.contentDocument?.querySelector<HTMLButtonElement>(
      '#app form[data-archive] button[type="submit"]',
    );
    return submit !== null && submit !== undefined && !submit.disabled;
  }).toBe(true);
  const archiveForm = created.contentDocument?.querySelector<HTMLFormElement>("#app form[data-archive]");
  if (archiveForm === null || archiveForm === undefined) {
    throw new Error("the archive form is missing");
  }
  archiveForm.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("protected-ready");
  await destroyApplicationFrame(created);

  const frame = await loadApplication("StoryOS exact-dist archived library");
  await expect.poll(() =>
    [...(frame.contentDocument?.querySelectorAll('#app button[data-lifecycle="archived"]') ?? [])]
      .some((button) => button.textContent === "Archive Source")
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
  const archivedButton = [...root.querySelectorAll<HTMLButtonElement>('button[data-lifecycle="archived"]')]
    .find((button) => button.textContent === "Archive Source");
  if (archivedButton === undefined) {
    throw new Error("the archived library button is missing");
  }
  expect(archivedButton.disabled).toBe(true);
  archivedButton.click();
  expect(root.getAttribute("data-boot-state")).toBe("protected-ready");
  const archivedItem = archivedButton.closest("li");
  expect(archivedItem?.querySelector("form[data-rename]")).toBeNull();
  expect(archivedItem?.querySelector("form[data-archive]")).toBeNull();
  expect(root.querySelector("textarea")).toBeNull();
});
