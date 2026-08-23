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

async function waitForBlockedSurface(frame: HTMLIFrameElement): Promise<Element> {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    const root = frame.contentDocument?.querySelector("#app");
    if (root?.getAttribute("data-boot-state") === "project-blocked"
      && root.querySelector("h1") !== null) {
      return root;
    }
    await new Promise<void>((resolve) => {
      window.setTimeout(resolve, 25);
    });
  }
  throw new Error("the Vite production page did not boot the Stage 1 surface");
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

  const root = await waitForBlockedSurface(frame);
  expect({
    alert: root.querySelector('[role="alert"]') !== null,
    bootState: root.getAttribute("data-boot-state"),
    heading: root.querySelector("h1")?.textContent ?? null,
    message: root.querySelector("p")?.textContent ?? null,
    textarea: root.querySelector("textarea") !== null,
    userAgent: frame.contentWindow?.navigator.userAgent.includes("Chrome/") ?? false,
  }).toEqual({
    alert: true,
    bootState: "project-blocked",
    heading: "StoryOS 无法打开项目",
    message: "项目地址缺少有效的受控项目身份。",
    textarea: false,
    userAgent: true,
  });
});
