import { afterEach, expect, it } from "vitest";

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist production page did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

async function waitForProtectedReady(frame: HTMLIFrameElement): Promise<Element> {
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    return root?.getAttribute("data-boot-state") === "protected-ready"
      && root.querySelector("h1") !== null
      ? root
      : null;
  }).not.toBeNull();
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the Vite production page did not reach the protected-ready state");
  }
  return root;
}

afterEach(() => {
  document.body.replaceChildren();
});

it("loads the exact Vite production page in Google Chrome and shows the Stage 1 surface", async () => {
  const rootResponse = await fetch("/");
  const rootBytes = new Uint8Array(await rootResponse.arrayBuffer());
  expect(rootResponse.status).toBe(200);
  expect(String(rootBytes.byteLength)).toBe(rootResponse.headers.get("content-length"));
  expect(new TextDecoder().decode(rootBytes)).not.toContain("/@vite/client");

  const frame = document.createElement("iframe");
  frame.title = "StoryOS production application";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;

  const root = await waitForProtectedReady(frame);
  expect({
    alert: root.querySelector('[role="alert"]') !== null,
    bootState: root.getAttribute("data-boot-state"),
    heading: root.querySelector("h1")?.textContent ?? null,
    message: root.querySelector("p")?.textContent ?? null,
    textarea: root.querySelector("textarea") !== null,
    userAgent: frame.contentWindow?.navigator.userAgent.includes("Chrome/") ?? false,
  }).toEqual({
    alert: false,
    bootState: "protected-ready",
    heading: "StoryOS",
    message: "本地写作已就绪。",
    textarea: false,
    userAgent: true,
  });
  expect(root.textContent).not.toContain("模型");
  expect(root.textContent).not.toContain("Agent");
  expect(root.textContent).not.toContain("Provider");
});
