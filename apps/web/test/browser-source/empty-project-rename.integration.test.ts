import { expect, it } from "vitest";

import updateProjectFixture from "../../../../generated/golden-wire/storyos-public-release-1/update-project.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { beginInlineVolumeCreation, beginTreeAction } from "../support/inline-chapter-creation.ts";
import { jsonResponse } from "./scenario.ts";

// An Empty Project view takes the title and the revision of each new state (#1888).

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000012";
const VOLUME = "018f0000-0000-7001-8000-000000000815";
const SCOPE = { owner_user_id: OWNER, project_id: PROJECT };

it("shows the title of a rename in another tab after a Volume creation opens the Empty Project again", async () => {
  const state = { title: "Listed Novel", revision: 1, volume: false, renames: [] as string[] };
  const fetchImpl: typeof fetch = async (request, init) => {
    const path = new URL(request instanceof Request ? request.url : request).pathname;
    const method = init?.method ?? "GET";
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: SCOPE, title: state.title, revision: String(state.revision),
          open: { kind: "empty" } }] });
    }
    if (path === `/api/v1/projects/${PROJECT}` && method === "GET") {
      return jsonResponse({ schema_id: "storyos.query.project.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: SCOPE,
        project: { project_id: PROJECT, title: state.title, open: { kind: "empty" } } });
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000015", project_scope: SCOPE,
        tree_revision: state.volume ? "2" : "1",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: SCOPE },
        volumes: state.volume ? [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [] }] : [] });
    }
    if (path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges`) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path === `/api/v1/projects/${PROJECT}/volumes` && method === "POST") {
      state.volume = true;
      // Another tab renames the Project before the Volume creation opens the Project again.
      state.title = "Renamed in another tab";
      state.revision += 1;
      return jsonResponse({ effect: { kind: "authoritative_applied", volume_id: VOLUME, tree_revision: "2",
        order: "1" } });
    }
    if (path === `/api/v1/projects/${PROJECT}` && method === "PATCH") {
      const input = (JSON.parse(String(init?.body)) as {
        update_project_input: { title: string; expected_project_revision: string } }).update_project_input;
      state.renames.push(input.expected_project_revision);
      state.title = input.title;
      state.revision += 1;
      return jsonResponse({ ...updateProjectFixture, project_scope: SCOPE,
        project: { project_id: PROJECT, title: state.title, open: { kind: "empty" } },
        effect: { ...updateProjectFixture.effect, title: state.title, revision: String(state.revision) },
        receipt: { ...updateProjectFixture.receipt, project_scope: SCOPE } });
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  document.body.innerHTML = '<main id="app"></main>';
  try {
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` }, fetchImpl, cryptoImpl: crypto });
    mountStage1View(loaded.root, loaded);
    const heading = () => loaded.root.querySelector(".project-heading h1")?.textContent;
    await expect.poll(heading).toBe("Listed Novel");
    await beginInlineVolumeCreation(loaded.root);
    const volume = loaded.root.querySelector<HTMLInputElement>('form[data-create-volume] input[name="volume-title"]')!;
    volume.value = "Volume A";
    volume.form!.requestSubmit();
    // The Volume shows when the view takes the state of the new open.
    await expect.poll(() => loaded.root.querySelector(`li[data-volume-id="${VOLUME}"]`)).not.toBeNull();
    const shown = heading();

    await beginTreeAction(loaded.root, "div[data-project-id]", "[data-begin-rename-project]");
    loaded.root.querySelector<HTMLButtonElement>("[data-begin-rename-project]")!.click();
    await expect.poll(() => loaded.root.querySelector("form[data-rename] input")).not.toBeNull();
    const input = loaded.root.querySelector<HTMLInputElement>("form[data-rename] input")!;
    input.value = "Renamed here";
    input.form!.requestSubmit();
    await expect.poll(() => state.renames.length).toBe(1);

    expect({ shown, renames: state.renames }).toEqual({ shown: "Renamed in another tab", renames: ["2"] });
  } finally {
    document.body.replaceChildren();
  }
});
