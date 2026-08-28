import { afterEach, expect, it } from "vitest";
import { page } from "vitest/browser";

import {
  applyImeComposition,
  applyTrustedInput,
  updateClientSessionCookie,
  updateClipboardPermission,
} from "../support/browser-command-client";
import { parseProductionHostRequest } from "../support/browser-command-contract";

afterEach(async () => {
  await updateClientSessionCookie({ action: "clear" });
  await updateClipboardPermission({ action: "clear" });
  document.body.replaceChildren();
});

it("uses typed input, clipboard, and Client Session commands in Google Chrome", async () => {
  document.body.innerHTML = `
    <label for="story-input">Story text</label>
    <input id="story-input" type="text">
  `;
  const input = page.getByRole("textbox", { name: "Story text" });
  await input.click();

  await expect(applyTrustedInput({ operation: "insert_text", text: "Story" }))
    .resolves.toEqual({ kind: "trusted_input_applied" });
  await expect.element(input).toHaveValue("Story");

  const inputElement = document.querySelector("#story-input");
  if (!(inputElement instanceof HTMLInputElement)) {
    throw new Error("the Story input is unavailable");
  }
  inputElement.focus();
  inputElement.setSelectionRange(5, 5);
  await expect(applyImeComposition({
    replacementEnd: 5,
    replacementStart: 5,
    selectionEnd: 2,
    selectionStart: 2,
    text: "中文",
  })).resolves.toEqual({ kind: "ime_composition_applied" });
  await expect.element(input).toHaveValue("Story中文");

  await expect(updateClipboardPermission({ action: "grant" })).resolves.toEqual({
    kind: "clipboard_permission_updated",
  });
  await navigator.clipboard.writeText("StoryOS Browser Mode");
  await expect(navigator.clipboard.readText()).resolves.toBe("StoryOS Browser Mode");

  await expect(updateClientSessionCookie({ action: "set", value: "session-a" }))
    .resolves.toEqual({ kind: "client_session_cookie_updated" });
  const sessionResponse = await fetch("/__storyos_browser_foundation__/session");
  const sessionProbe: unknown = await sessionResponse.json();
  expect(sessionProbe).toEqual({ bound: true });
  expect(navigator.userAgent).toContain("Chrome/");
});

it("refuses navigation and code inputs at the production command boundary", () => {
  for (const input of [null, {}, { scenario: "navigate" },
    { scenario: "open_edit_reload_takeover", url: "https://example.invalid/" },
    { scenario: "open_edit_reload_takeover", source: "document.body.remove()" }]) {
    expect(() => parseProductionHostRequest(input)).toThrow(TypeError);
  }
});
