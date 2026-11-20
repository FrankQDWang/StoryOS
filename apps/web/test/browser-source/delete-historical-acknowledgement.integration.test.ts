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

it("explains a historical Delete Volume acknowledgement and does not retry it", async () => {
  const volumes = [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [] }];
  const { fetchImpl, methods } = recordingFetch(emptyProjectRoute(volumes, (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/volumes/${VOLUME}` && method === "DELETE") {
      return historicalProblem("The original Delete Volume acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`);
    await beginTreeAction(loaded.root, `li[data-volume-id="${VOLUME}"]`, "[data-delete-volume]");
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>(`[data-delete-volume="${VOLUME}"]`) !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>(`[data-delete-volume="${VOLUME}"]`)?.click();
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-volume="${VOLUME}"]`) !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-volume="${VOLUME}"]`)?.click();
    await expect.poll(() =>
      loaded.root.querySelector('[role="alert"]')?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Listed Novel");
    expectOneCommandAndChallenge(methods, `DELETE /api/v1/projects/${PROJECT}/volumes/${VOLUME}`);
  } finally {
    document.body.replaceChildren();
  }
});

it("explains a historical Delete Chapter acknowledgement and does not retry it", async () => {
  const volumes = [{
    volume_id: VOLUME,
    title: "Volume A",
    order: "1",
    chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }],
  }];
  const { fetchImpl, methods } = recordingFetch(emptyProjectRoute(volumes, (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}` && method === "DELETE") {
      return historicalProblem("The original Delete Chapter acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`);
    await beginTreeAction(loaded.root, `li[data-chapter-id="${CHAPTER}"]`, "[data-delete-chapter]");
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>(`[data-delete-chapter="${CHAPTER}"]`) !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>(`[data-delete-chapter="${CHAPTER}"]`)?.click();
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-chapter="${CHAPTER}"]`) !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>(`[data-confirm-delete-chapter="${CHAPTER}"]`)?.click();
    await expect.poll(() =>
      loaded.root.querySelector('[role="alert"]')?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Listed Novel");
    expect(loaded.root.textContent).toContain("Chapter A");
    expectOneCommandAndChallenge(methods, `DELETE /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`);
  } finally {
    document.body.replaceChildren();
  }
});
