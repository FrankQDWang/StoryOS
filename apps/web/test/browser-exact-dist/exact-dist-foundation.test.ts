import { afterEach, expect, it } from "vitest";

const PROJECT_ROUTE = "/projects/018f0000-0000-7001-8000-000000000001";

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist application frame did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

afterEach(() => {
  window.sessionStorage.removeItem("storyos-foundation");
  document.body.replaceChildren();
});

it("serves untransformed dist bytes in a reloadable same-origin child frame", async () => {
  const rootResponse = await fetch("/");
  const rootBytes = new Uint8Array(await rootResponse.arrayBuffer());
  expect(String(rootBytes.byteLength)).toBe(rootResponse.headers.get("content-length"));

  const indexResponse = await fetch(PROJECT_ROUTE);
  expect(indexResponse.status).toBe(200);
  const indexBytes = new Uint8Array(await indexResponse.arrayBuffer());
  expect(indexBytes).toEqual(rootBytes);
  expect(String(indexBytes.byteLength)).toBe(indexResponse.headers.get("content-length"));
  const indexHtml = new TextDecoder().decode(indexBytes);
  expect(indexHtml).not.toContain("/@vite/client");

  const assetPath = /src="(\/assets\/[A-Za-z0-9._-]+)"/u.exec(indexHtml)?.[1];
  if (assetPath === undefined) {
    throw new Error("the production index has no root asset");
  }
  const assetResponse = await fetch(assetPath);
  expect(assetResponse.status).toBe(200);
  const assetBytes = new Uint8Array(await assetResponse.arrayBuffer());
  expect(String(assetBytes.byteLength)).toBe(assetResponse.headers.get("content-length"));

  const frame = document.createElement("iframe");
  frame.title = "StoryOS exact-dist application";
  const firstLoad = nextFrameLoad(frame);
  frame.src = PROJECT_ROUTE;
  document.body.append(frame);
  await firstLoad;
  const firstRealm = frame.contentWindow;
  if (firstRealm === null) {
    throw new Error("the exact-dist application realm is unavailable");
  }
  expect(firstRealm.location.origin).toBe(window.location.origin);
  firstRealm.sessionStorage.setItem("storyos-foundation", "retained");
  Reflect.set(firstRealm, "__storyosFoundationRealm", "first");

  const secondLoad = nextFrameLoad(frame);
  firstRealm.location.reload();
  await secondLoad;
  const secondRealm = frame.contentWindow;
  if (secondRealm === null) {
    throw new Error("the reloaded exact-dist application realm is unavailable");
  }
  expect(Reflect.get(secondRealm, "__storyosFoundationRealm")).toBeUndefined();
  expect(secondRealm.sessionStorage.getItem("storyos-foundation")).toBe("retained");
});
