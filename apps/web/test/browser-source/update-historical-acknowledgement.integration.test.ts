import { beginTreeAction } from "../support/inline-chapter-creation.ts";
import { expect, it } from "vitest";

import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
import {
  LISTED_PROJECT as PROJECT,
  VOLUME,
  emptyProjectRoute,
  expectOneCommandAndChallenge,
  historicalProblem,
  mountApp,
  recordingFetch,
} from "./historical-acknowledgement-fixture.ts";

const CHAPTER = "018f0000-0000-7001-8000-000000000816";

it("explains a historical Update Volume acknowledgement and does not retry it", async () => {
  const volumes = [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [] }];
  const { fetchImpl, methods } = recordingFetch(emptyProjectRoute(volumes, (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/volumes/${VOLUME}` && method === "PATCH") {
      return historicalProblem("The original Update Volume acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`);
    await beginTreeAction(loaded.root, `li[data-volume-id="${VOLUME}"]`, "[data-begin-rename-volume]");
    loaded.root.querySelector<HTMLButtonElement>("[data-begin-rename-volume]")?.click();
    await expect.poll(() =>
      loaded.root.querySelector<HTMLInputElement>(`form[data-rename-volume="${VOLUME}"] input[name="volume-title"]`)
        !== null
    ).toBe(true);
    const input = loaded.root.querySelector<HTMLInputElement>(
      `form[data-rename-volume="${VOLUME}"] input[name="volume-title"]`,
    );
    const form = input?.form;
    if (input === null || input === undefined || form === null || form === undefined) {
      throw new Error("the Update Volume form is missing");
    }
    input.value = "Kept Volume Title";
    form.requestSubmit();
    await expect.poll(() =>
      loaded.root.querySelector("[data-rename-volume-error]")?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(input.value).toBe("Kept Volume Title");
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Listed Novel");
    expectOneCommandAndChallenge(methods, `PATCH /api/v1/projects/${PROJECT}/volumes/${VOLUME}`);
  } finally {
    document.body.replaceChildren();
  }
});

it("explains a historical Update Chapter acknowledgement and does not retry it", async () => {
  const volumes = [{
    volume_id: VOLUME,
    title: "Volume A",
    order: "1",
    chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }],
  }];
  const { fetchImpl, methods } = recordingFetch(emptyProjectRoute(volumes, (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}` && method === "PATCH") {
      return historicalProblem("The original Update Chapter acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`);
    await beginTreeAction(loaded.root, `li[data-chapter-id="${CHAPTER}"]`, "[data-begin-rename-chapter]");
    loaded.root.querySelector<HTMLButtonElement>("[data-begin-rename-chapter]")?.click();
    await expect.poll(() =>
      loaded.root.querySelector<HTMLInputElement>(`form[data-rename-chapter="${CHAPTER}"] input[name="chapter-title"]`)
        !== null
    ).toBe(true);
    const input = loaded.root.querySelector<HTMLInputElement>(
      `form[data-rename-chapter="${CHAPTER}"] input[name="chapter-title"]`,
    );
    const form = input?.form;
    if (input === null || input === undefined || form === null || form === undefined) {
      throw new Error("the Update Chapter form is missing");
    }
    input.value = "Kept Chapter Title";
    form.requestSubmit();
    await expect.poll(() =>
      loaded.root.querySelector("[data-rename-chapter-error]")?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(input.value).toBe("Kept Chapter Title");
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Listed Novel");
    expect(loaded.root.textContent).toContain("Volume A");
    expectOneCommandAndChallenge(methods, `PATCH /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`);
  } finally {
    document.body.replaceChildren();
  }
});
