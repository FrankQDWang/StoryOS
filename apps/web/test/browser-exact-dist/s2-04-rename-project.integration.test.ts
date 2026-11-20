import { beginTreeAction } from "../support/inline-chapter-creation.ts";
import { afterEach, expect, it } from "vitest";

import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import {
  closeApplicationFrame,
  destroyApplicationFrame,
  openApplicationFrame,
  submitProjectForm,
} from "../support/exact-dist-frame.ts";

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("the author renames one exact Project and the library plus opened title converge", async () => {
  const created = await openApplicationFrame("StoryOS exact-dist create for rename");
  await submitProjectForm(created, "Rename Source");
  await beginTreeAction(created.contentDocument, "[data-project-id]", "[data-begin-rename-project]");
  created.contentDocument?.querySelector<HTMLButtonElement>("[data-begin-rename-project]")?.click();
  await expect.poll(() => created.contentDocument?.querySelector('input[name="rename-title"]')?.tagName).toBe("INPUT");
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

  const frame = await openApplicationFrame("StoryOS exact-dist renamed library");
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
  expect(root.querySelector(".editor-panel textarea")).toBeNull();
  expect(root.textContent).toContain("空工作区");
  const tree = root.querySelector('nav[aria-label="稿件目录"]');
  expect(tree).not.toBeNull();
  expect(tree?.querySelectorAll("li")).toHaveLength(0);
});
