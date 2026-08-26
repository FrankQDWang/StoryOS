import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { jsonResponse } from "./scenario.ts";

const OWNER = "018f0000-0000-7001-8000-000000000001";
const EMPTY_PROJECT = "018f0000-0000-7001-8000-000000000012";

it("opens a listed empty Project from getProject, not from the list payload", async () => {
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push(path);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013",
        owner_user_id: OWNER,
        projects: [{
          project_scope: { owner_user_id: OWNER, project_id: EMPTY_PROJECT },
          title: "List Lie",
          lifecycle: { kind: "active" },
          revision: "1",
          open: { kind: "empty" },
        }],
      });
    }
    if (path === `/api/v1/projects/${EMPTY_PROJECT}`) {
      return jsonResponse({
        schema_id: "storyos.query.project.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014",
        project_scope: { owner_user_id: OWNER, project_id: EMPTY_PROJECT },
        project: { project_id: EMPTY_PROJECT, title: "Authoritative Empty", open: { kind: "empty" } },
      });
    }
    if (path === `/api/v1/projects/${EMPTY_PROJECT}/manuscript/tree`) {
      return jsonResponse({
        project_scope: { owner_user_id: OWNER, project_id: EMPTY_PROJECT },
        tree_revision: "1",
        snapshot: {
          snapshot_id: "018f0000-0000-7001-8000-000000000032",
          project_scope: { owner_user_id: OWNER, project_id: EMPTY_PROJECT },
        },
        volumes: [],
      });
    }
    throw new Error(`unexpected request: ${path}`);
  };

  document.body.innerHTML = '<main id="app"></main>';
  try {
    const loaded = await loadStoryOSWebState({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: "/" },
      fetchImpl,
      cryptoImpl: crypto,
    });
    mountStage1View(loaded.root, loaded);
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>(`button[data-project-id="${EMPTY_PROJECT}"]`)
        ?.textContent
    ).toBe("List Lie");
    loaded.root.querySelector<HTMLButtonElement>(`button[data-project-id="${EMPTY_PROJECT}"]`)
      ?.click();
    await expect.poll(() => loaded.root.dataset.bootState).toBe("empty-project-ready");
    expect(loaded.root.querySelector("h1")?.textContent).toBe("Authoritative Empty");
    expect(loaded.root.textContent).not.toContain("List Lie");
    expect(loaded.root.querySelector('nav[aria-label="稿件目录"]')?.querySelectorAll("li"))
      .toHaveLength(0);
    expect(requests.filter((path) => path === `/api/v1/projects/${EMPTY_PROJECT}`).length)
      .toBe(1);
    expect(requests).toContain(`/api/v1/projects/${EMPTY_PROJECT}/manuscript/tree`);
  } finally {
    document.body.replaceChildren();
  }
});
