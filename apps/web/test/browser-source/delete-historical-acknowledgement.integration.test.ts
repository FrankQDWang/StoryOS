import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { jsonResponse } from "./scenario.ts";

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000012";
const VOLUME = "018f0000-0000-7001-8000-000000000815";
const CHAPTER = "018f0000-0000-7001-8000-000000000816";

function listedEmptyProject() {
  return {
    schema_id: "storyos.query.project-list.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000013",
    owner_user_id: OWNER,
    projects: [{
      project_scope: { owner_user_id: OWNER, project_id: PROJECT },
      title: "Listed Novel",
      lifecycle: { kind: "active" },
      revision: "1",
      open: { kind: "empty" },
    }],
  };
}

function emptyProject() {
  return {
    schema_id: "storyos.query.project.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000014",
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    project: { project_id: PROJECT, title: "Listed Novel", open: { kind: "empty" } },
  };
}

function tree(volumes: Array<{
  volume_id: string;
  title: string;
  order: string;
  chapters: Array<{ chapter_id: string; title: string; order: string }>;
}>) {
  return {
    schema_id: "storyos.query.manuscript-tree.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000015",
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    tree_revision: "2",
    snapshot: {
      snapshot_id: "018f0000-0000-7001-8000-000000000032",
      project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    },
    volumes,
  };
}

function challenge() {
  return {
    nonce: "a".repeat(64),
    expires_at: "2026-09-12T00:00:00.000Z",
    limit_profile_revision: "storyos.foundation.absolute.v1",
  };
}

function historicalProblem(message: string) {
  return jsonResponse({
    schema_id: "storyos.problem.v1",
    code: "historical_acknowledgement_unavailable",
    message,
  }, 409);
}

it("explains a historical Delete Volume acknowledgement and does not retry it", async () => {
  const methods: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    methods.push(`${method} ${path}`);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") return jsonResponse(listedEmptyProject());
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(emptyProject());
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse(tree([{
        volume_id: VOLUME,
        title: "Volume A",
        order: "1",
        chapters: [],
      }]));
    }
    if (path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges`) return jsonResponse(challenge());
    if (path === `/api/v1/projects/${PROJECT}/volumes/${VOLUME}` && method === "DELETE") {
      return historicalProblem("The original Delete Volume acknowledgement cannot be recovered.");
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  document.body.innerHTML = '<main id="app"></main>';
  try {
    const loaded = await loadStoryOSWebState({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl,
      cryptoImpl: crypto,
    });
    mountStage1View(loaded.root, loaded);
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
    expect(methods.filter((entry) =>
      entry === `DELETE /api/v1/projects/${PROJECT}/volumes/${VOLUME}`
    )).toEqual([
      `DELETE /api/v1/projects/${PROJECT}/volumes/${VOLUME}`,
    ]);
    expect(methods.filter((entry) => entry.includes("anti-forgery-challenges"))).toHaveLength(1);
  } finally {
    document.body.replaceChildren();
  }
});

it("explains a historical Delete Chapter acknowledgement and does not retry it", async () => {
  const methods: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    methods.push(`${method} ${path}`);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") return jsonResponse(listedEmptyProject());
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(emptyProject());
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse(tree([{
        volume_id: VOLUME,
        title: "Volume A",
        order: "1",
        chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }],
      }]));
    }
    if (path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges`) return jsonResponse(challenge());
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}` && method === "DELETE") {
      return historicalProblem("The original Delete Chapter acknowledgement cannot be recovered.");
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  document.body.innerHTML = '<main id="app"></main>';
  try {
    const loaded = await loadStoryOSWebState({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl,
      cryptoImpl: crypto,
    });
    mountStage1View(loaded.root, loaded);
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
    expect(methods.filter((entry) =>
      entry === `DELETE /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`
    )).toEqual([
      `DELETE /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`,
    ]);
    expect(methods.filter((entry) => entry.includes("anti-forgery-challenges"))).toHaveLength(1);
  } finally {
    document.body.replaceChildren();
  }
});
