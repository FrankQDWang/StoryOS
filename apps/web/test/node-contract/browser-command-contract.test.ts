import { expect, it } from "vitest";

import { parseProductionHostRequest } from "../support/browser-command-contract.ts";

it("refuses navigation and code inputs at the production command boundary", () => {
  for (const input of [null, {}, { scenario: "navigate" },
    { scenario: "open_edit_reload_two_pages", url: "https://example.invalid/" },
    { scenario: "open_edit_reload_two_pages", source: "document.body.remove()" }]) {
    expect(() => parseProductionHostRequest(input)).toThrow(TypeError);
  }
});
