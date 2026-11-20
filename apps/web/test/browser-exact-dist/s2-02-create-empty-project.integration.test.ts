import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { closeApplicationFrame, openApplicationFrame } from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("the protected browser creates one empty workspace without a starter Chapter", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist empty Project");
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Empty Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  const emptyRoot = frame.contentDocument?.querySelector("#app");
  expect(emptyRoot?.querySelector("h1")?.textContent).toBe("Empty Novel");
  expect(emptyRoot?.querySelector(".editor-panel textarea")).toBeNull();
  expect(emptyRoot?.textContent).toContain("空工作区");
  const tree = emptyRoot?.querySelector('nav[aria-label="稿件目录"]');
  expect(tree).not.toBeNull();
  expect(tree?.querySelectorAll("li")).toHaveLength(0);
  expect(emptyRoot?.textContent).not.toContain("chapter");
  expect(emptyRoot?.textContent).not.toMatch(/""|''/);
  expect(emptyRoot?.textContent).not.toContain("模型");
  expect(emptyRoot?.textContent).not.toContain("Agent");
});
