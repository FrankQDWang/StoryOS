import { expect } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  SESSION,
  closeTrackedDatabases,
  type createBrowserScenario,
  deleteJournal,
  emptyActivityStream,
  jsonResponse,
} from "./scenario.ts";

/** The Project of the rows that open an empty Project. */
export const LISTED_PROJECT = "018f0000-0000-7001-8000-000000000012";
export const VOLUME = "018f0000-0000-7001-8000-000000000815";

type Scenario = ReturnType<typeof createBrowserScenario>;
type Volume = { volume_id: string; title: string; order: string; chapters: Chapter[] };
type Chapter = { chapter_id: string; title: string; order: string };
/** Gives the response of one request, or `undefined` when the route does not know the request. */
type Route = (method: string, path: string) => Response | undefined | Promise<Response | undefined>;

export function listedEmptyProject(revision = "1") {
  return {
    schema_id: "storyos.query.project-list.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000013",
    owner_user_id: OWNER,
    projects: [{
      project_scope: { owner_user_id: OWNER, project_id: LISTED_PROJECT },
      title: "Listed Novel",
      revision,
      open: { kind: "empty" },
    }],
  };
}

function emptyProject() {
  return {
    schema_id: "storyos.query.project.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000014",
    project_scope: { owner_user_id: OWNER, project_id: LISTED_PROJECT },
    project: { project_id: LISTED_PROJECT, title: "Listed Novel", open: { kind: "empty" } },
  };
}

function tree(volumes: Volume[]) {
  return {
    schema_id: "storyos.query.manuscript-tree.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000015",
    project_scope: { owner_user_id: OWNER, project_id: LISTED_PROJECT },
    tree_revision: volumes.length === 0 ? "1" : "2",
    snapshot: {
      snapshot_id: "018f0000-0000-7001-8000-000000000032",
      project_scope: { owner_user_id: OWNER, project_id: LISTED_PROJECT },
    },
    volumes,
  };
}

/** A Challenge as the Server issues it: it expires 5 minutes after its issue. */
export function challenge() {
  return {
    nonce: "a".repeat(64),
    expires_at: new Date(Date.now() + 5 * 60_000).toISOString(),
    limit_profile_revision: "storyos.foundation.absolute.v1",
  };
}

export function historicalProblem(message: string): Response {
  return jsonResponse({
    schema_id: "storyos.problem.v1",
    code: "historical_acknowledgement_unavailable",
    message,
  }, 409);
}

/** A fetch that answers through `route`, records each request as `<method> <path>`, and refuses each other request. */
export function recordingFetch(route: Route): { fetchImpl: typeof fetch; methods: string[] } {
  const methods: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? (input instanceof Request ? input.method : "GET");
    methods.push(`${method} ${path}`);
    const response = await route(method, path);
    if (response === undefined) throw new Error(`unexpected request: ${method} ${path}`);
    return response;
  };
  return { fetchImpl, methods };
}

/** Answers the reads of the empty listed Project with the tree `volumes`, and gives each other request to `command`. */
export function emptyProjectRoute(volumes: Volume[], command: Route): Route {
  return (method, path) => {
    if (method === "GET" && path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (method === "GET" && path === "/api/v1/projects") return jsonResponse(listedEmptyProject());
    if (method === "GET" && path === `/api/v1/projects/${LISTED_PROJECT}`) return jsonResponse(emptyProject());
    if (method === "GET" && path === `/api/v1/projects/${LISTED_PROJECT}/manuscript/tree`) {
      return jsonResponse(tree(volumes));
    }
    if (path === `/api/v1/projects/${LISTED_PROJECT}/anti-forgery-challenges` && method === "POST") {
      return jsonResponse(challenge());
    }
    return command(method, path);
  };
}

/**
 * Answers the reads of the Current Chapter of `scenario` in one Volume with `chapters`, and gives each other request
 * to `command`. `sessionRead` adds fields to the read of the Editor Session.
 */
export function openedChapterRoute(scenario: Scenario, treeRevision: string, chapters: Chapter[], command: Route,
  sessionRead: Record<string, string> = {}): Route {
  return (method, path) => {
    if (method === "GET" && path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (method === "GET" && path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013",
        owner_user_id: OWNER,
        projects: [{
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
          title: "Project A",
          revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER },
        }],
      });
    }
    if (method === "GET" && path === `/api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (method === "GET" && path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) {
      return jsonResponse(scenario.chapter);
    }
    if (method === "GET" && path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({
        schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014",
        project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        tree_revision: treeRevision,
        snapshot: {
          snapshot_id: "018f0000-0000-7001-8000-000000000032",
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters }],
      });
    }
    if (path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges` && method === "POST") {
      return jsonResponse(challenge());
    }
    if (path === `/api/v1/projects/${PROJECT}/editor-sessions` && method === "POST") {
      return jsonResponse(scenario.session);
    }
    if (path === `/api/v1/projects/${PROJECT}/editor-sessions/${SESSION}` && method === "GET") {
      return jsonResponse({
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v2",
        ...sessionRead,
      });
    }
    if (path === `/api/v1/projects/${PROJECT}/activity` && method === "GET") return emptyActivityStream();
    return command(method, path);
  };
}

/** Mounts the Web client at `pathname` in a new `#app` root. */
export async function mountApp(fetchImpl: typeof fetch, pathname: string, indexedDBImpl?: IDBFactory) {
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({
    documentImpl: document,
    locationImpl: { origin: location.origin, pathname },
    fetchImpl,
    ...(indexedDBImpl === undefined ? {} : { indexedDBImpl }),
    cryptoImpl: crypto,
  });
  mountStage1View(loaded.root, loaded);
  return loaded;
}

/** Closes the Journal databases of the test, clears the page, and removes the Editor Session key and the Journal. */
export async function closeOpenedChapter(scenario: Scenario, openDatabases: Set<IDBDatabase>): Promise<void> {
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
  await deleteJournal(scenario.journalName);
}

/** Expects that the client sent the command `entry` one time and asked for one Command Challenge. */
export function expectOneCommandAndChallenge(methods: readonly string[], entry: string): void {
  expect(methods.filter((sent) => sent === entry)).toEqual([entry]);
  expect(methods.filter((sent) => sent.includes("anti-forgery-challenges"))).toHaveLength(1);
}
