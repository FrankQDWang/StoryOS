import { beginTreeAction } from "../support/inline-chapter-creation.ts";
import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
} from "../../src/rename-project.ts";
import {
  LISTED_PROJECT as PROJECT,
  challenge,
  historicalProblem,
  listedEmptyProject,
  mountApp,
  recordingFetch,
} from "./historical-acknowledgement-fixture.ts";
import { jsonResponse } from "./scenario.ts";

it("explains a historical Update Project acknowledgement and does not retry it", async () => {
  const { fetchImpl, methods } = recordingFetch((method, path) => {
    if (method === "GET" && path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (method === "GET" && path === "/api/v1/projects") return jsonResponse(listedEmptyProject("2"));
    if (method === "POST" && path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges`) {
      return jsonResponse(challenge());
    }
    if (path === `/api/v1/projects/${PROJECT}` && method === "PATCH") {
      return historicalProblem("The original Update Project acknowledgement cannot be recovered.");
    }
    return undefined;
  });

  try {
    const loaded = await mountApp(fetchImpl, "/");
    await beginTreeAction(loaded.root, "div[data-project-id]", "[data-begin-rename-project]");
    loaded.root.querySelector<HTMLButtonElement>("[data-begin-rename-project]")?.click();
    await expect.poll(() =>
      loaded.root.querySelector<HTMLInputElement>('form[data-rename] input[name="rename-title"]')
        ?.disabled
    ).toBe(false);
    const input = loaded.root.querySelector<HTMLInputElement>('form[data-rename] input[name="rename-title"]');
    const form = input?.form;
    if (input === null || input === undefined || form === null || form === undefined) {
      throw new Error("the rename form is missing");
    }
    input.value = "Kept Author Text";
    form.requestSubmit();
    await expect.poll(() =>
      loaded.root.querySelector("[data-rename-error]")?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(input.value).toBe("Kept Author Text");
    expect(loaded.root.querySelector("h1")?.textContent).toBe("StoryOS");
    expect(loaded.root.textContent).toContain("Listed Novel");
    expect(methods.filter((entry) => entry.startsWith("PATCH "))).toEqual([
      `PATCH /api/v1/projects/${PROJECT}`,
    ]);
    expect(methods.filter((entry) => entry.includes("anti-forgery-challenges"))).toHaveLength(1);
  } finally {
    document.body.replaceChildren();
  }
});
