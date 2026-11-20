import { beginInlineChapterCreation, beginInlineVolumeCreation } from "../support/inline-chapter-creation.ts";
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

it("explains a historical Create Volume acknowledgement and does not retry it", async () => {
  const { fetchImpl, methods } = recordingFetch(emptyProjectRoute([], (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/volumes` && method === "POST") {
      return historicalProblem("The original Create Volume acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`);
    await beginInlineVolumeCreation(loaded.root);
    await expect.poll(() =>
      loaded.root.querySelector<HTMLInputElement>('form[data-create-volume] input[name="volume-title"]')
        !== null
    ).toBe(true);
    const input = loaded.root.querySelector<HTMLInputElement>('form[data-create-volume] input[name="volume-title"]');
    const form = input?.form;
    if (input === null || input === undefined || form === null || form === undefined) {
      throw new Error("the Create Volume form is missing");
    }
    input.value = "Volume A";
    form.requestSubmit();
    await expect.poll(() =>
      loaded.root.querySelector("[data-create-volume-error]")?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Listed Novel");
    expectOneCommandAndChallenge(methods, `POST /api/v1/projects/${PROJECT}/volumes`);
  } finally {
    document.body.replaceChildren();
  }
});

it("keeps the pending inline title when another creation is requested and explains the historical acknowledgement", async () => {
  let release!: () => void, reached!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const submitted = new Promise<void>((resolve) => { reached = resolve; });
  const volumes = [
    { volume_id: VOLUME, title: "Volume A", order: "1", chapters: [] },
    { volume_id: "018f0000-0000-7001-8000-000000002540", title: "Volume B", order: "2", chapters: [] },
  ];
  const { fetchImpl, methods } = recordingFetch(emptyProjectRoute(volumes, async (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/volumes/${VOLUME}/chapters` && method === "POST") {
      reached(); await held;
      return historicalProblem("The original Create Chapter acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`);
    await beginInlineChapterCreation(loaded.root, VOLUME);
    await expect.poll(() =>
      loaded.root.querySelector<HTMLInputElement>(`form[data-create-chapter="${VOLUME}"] input[name="chapter-title"]`)
        !== null
    ).toBe(true);
    const input = loaded.root.querySelector<HTMLInputElement>(
      `form[data-create-chapter="${VOLUME}"] input[name="chapter-title"]`,
    );
    const form = input?.form;
    if (input === null || input === undefined || form === null || form === undefined) {
      throw new Error("the Create Chapter form is missing");
    }
    input.value = "Chapter A";
    form.requestSubmit();
    await submitted;
    loaded.root.querySelector<HTMLButtonElement>("[data-add-chapter]")!.click();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    expect([input.isConnected, input.value, input.readOnly, input.form?.getAttribute("data-create-chapter")])
      .toEqual([true, "Chapter A", true, VOLUME]);
    release();
    await expect.poll(() =>
      loaded.root.querySelector("[data-create-chapter-error]")?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Listed Novel");
    expect(loaded.root.textContent).toContain("Volume A");
    expectOneCommandAndChallenge(methods, `POST /api/v1/projects/${PROJECT}/volumes/${VOLUME}/chapters`);
  } finally {
    release();
    document.body.replaceChildren();
  }
});
