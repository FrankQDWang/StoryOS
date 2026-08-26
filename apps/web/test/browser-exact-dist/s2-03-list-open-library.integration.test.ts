import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist library page did not load"));
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

it("the current User library lists owned Projects and opens an empty Project from getProject", async () => {
  await updateClientSessionCookie({ action: "set", value: "session-a" });
  const created = document.createElement("iframe");
  applicationFrame = created;
  created.title = "StoryOS exact-dist create for library";
  const createdLoaded = nextFrameLoad(created);
  created.src = "/";
  document.body.append(created);
  await createdLoaded;
  await expect.poll(() =>
    created.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = created.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Library Empty";
  form.requestSubmit();
  await expect.poll(() =>
    created.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  await destroyApplicationFrame(created);

  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist User library";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await expect.poll(() =>
    [...(frame.contentDocument?.querySelectorAll("#app button[data-open=\"empty\"]") ?? [])]
      .some((button) => button.textContent === "Library Empty")
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
  const emptyButton = [...root.querySelectorAll<HTMLButtonElement>('button[data-open="empty"]')]
    .find((button) => button.textContent === "Library Empty");
  emptyButton?.click();
  await expect.poll(() => root.getAttribute("data-boot-state")).toBe("empty-project-ready");
  expect(root.querySelector("h1")?.textContent).toBe("Library Empty");
  expect(root.querySelector("textarea")).toBeNull();
  expect(root.textContent).toContain("空工作区");
  const tree = root.querySelector('nav[aria-label="稿件目录"]');
  expect(tree).not.toBeNull();
  expect(tree?.querySelectorAll("li")).toHaveLength(0);
});
