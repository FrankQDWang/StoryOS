import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { runStoryOSWeb } from "../../src/app.ts";
import {
  OWNER,
  PROJECT,
  SESSION,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
} from "./scenario.ts";

it("opens the URL-selected Project and fails closed before invalid entry requests", async () => {
  const scenario = createBrowserScenario();
  const requests: Array<{ init: RequestInit | undefined; path: string }> = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push({ init, path });
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${scenario.chapter.chapter.chapter_id}`) {
      return jsonResponse(scenario.chapter);
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v1",
      });
    }
    throw new Error(`unexpected request: ${path}`);
  };

  await deleteJournal(scenario.journalName);
  try {
    document.documentElement.dataset.storyosProject =
      "018f0000-0000-7001-8000-000000000102";
    document.body.innerHTML = '<main id="app"></main>';
    const state = await runStoryOSWeb({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    expect(state.kind).toBe("project-ready");
    const root = document.querySelector("#app");
    if (!(root instanceof HTMLElement)) throw new Error("the Web root is unavailable");
    expect(root.dataset.bootState).toBe("project-ready");
    expect(root.firstElementChild?.tagName).toBe("SECTION");
    expect(root.textContent).toContain("Project A");
    expect(root.textContent).toContain("Chapter A");
    expect(root.textContent).toContain("Base");
    expect(root.textContent).toContain(scenario.chapter.chapter.current_revision.revision_id);
    expect(requests.slice(0, 3).map(({ path }) => path)).toEqual([
      "/api/v1/protocol",
      `/api/v1/projects/${PROJECT}`,
      `/api/v1/projects/${PROJECT}/chapters/${scenario.chapter.chapter.chapter_id}`,
    ]);
    expect(requests.slice(0, 3).every(({ init }) => init?.credentials === "same-origin"))
      .toBe(true);
    if (state.kind === "project-ready" && state.editor.kind === "editor-ready") {
      state.editor.database.close();
    }

    document.body.replaceChildren();
    let requestCount = 0;
    await expect(runStoryOSWeb({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl: async () => {
        requestCount += 1;
        throw new Error("must not request");
      },
    })).rejects.toThrow(/required #app root is missing/);
    expect(requestCount).toBe(0);

    for (const pathname of ["/", "/projects/not-a-uuid", `/projects/${PROJECT}/extra`]) {
      document.body.innerHTML = '<main id="app">stale Project page</main>';
      requestCount = 0;
      const blocked = await runStoryOSWeb({
        documentImpl: document,
        locationImpl: { origin: location.origin, pathname },
        fetchImpl: async () => {
          requestCount += 1;
          throw new Error("must not request");
        },
      });
      expect(blocked).toEqual({
        kind: "project-blocked",
        code: "project_url_invalid",
        heading: "StoryOS 无法打开项目",
        message: "项目地址缺少有效的受控项目身份。",
      });
      const blockedRoot = document.querySelector("#app");
      if (!(blockedRoot instanceof HTMLElement)) throw new Error("the blocked root is unavailable");
      expect(requestCount).toBe(0);
      expect(blockedRoot.dataset.bootState).toBe("project-blocked");
      expect(blockedRoot.textContent).toContain("StoryOS 无法打开项目");
      expect(blockedRoot.textContent).toContain("项目地址缺少有效的受控项目身份。");
      expect(blockedRoot.textContent).not.toContain("stale Project page");
    }
  } finally {
    delete document.documentElement.dataset.storyosProject;
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
