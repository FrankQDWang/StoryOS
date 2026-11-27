import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import {
  openControlledProject,
  PROJECT_OPEN_DIAGNOSTIC_CAUSE,
  reopenControlledProject,
} from "../../src/boot.ts";
import type {
  GetChapterResponse,
  GetManuscriptTreeResponse,
  GetProjectResponse,
  ListProjectsResponse,
  Release1ProtocolProfile,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

const fixture = <Value>(name: string): Value => JSON.parse(readFileSync(
  new URL(`../../../../generated/golden-wire/storyos-public-release-1/${name}`, import.meta.url), "utf8",
));

function record(value: unknown): Record<PropertyKey, unknown> | undefined {
  return typeof value === "object" && value !== null
    ? value as Record<PropertyKey, unknown>
    : undefined;
}

function requestUrl(input: string | URL | Request): string | URL {
  return input instanceof Request ? input.url : input;
}

test("the protected Web client opens the authoritative current Chapter", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  const chapter = fixture<GetChapterResponse>("get-chapter.json");
  const requests: Array<{ path: string; options: RequestInit | undefined }> = [];
  const projectPath = `/api/v1/projects/${project.project.project_id}`;
  const chapterPath = `${projectPath}/chapters/${
    project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "missing"}`;
  // The tree read and the statistics read start with the Chapter read (#1937). The open does not use their answers.
  const responses = new Map<string, unknown>([["/api/v1/protocol", profile], [projectPath, project],
    ["/api/v1/projects", listed], [chapterPath, chapter],
    [`${projectPath}/manuscript/tree`, fixture<GetManuscriptTreeResponse>("get-manuscript-tree.json")],
    [`${projectPath}/manuscript/statistics`, fixture<unknown>("get-statistics.json")]]);
  const fetchImpl: typeof fetch = async (url, options) => {
    const path = new URL(requestUrl(url)).pathname;
    requests.push({ path, options });
    if (!responses.has(path)) throw new Error(`unexpected request: ${path}`);
    return new Response(JSON.stringify(responses.get(path)), {
      status: 200, headers: { "content-type": "application/json" },
    });
  };

  const state = await openControlledProject({
    baseUrl: "http://storyos.test",
    projectId: project.project.project_id,
    fetchImpl,
  });
  assert.equal(state.kind, "project-ready");
  assert.deepEqual({ profile: state.profile, project: state.project, chapter: state.chapter }, {
    profile, project, chapter,
  });
  assert.equal(state.editor.kind, "editor-read-only-recovery");
  assert.deepEqual(requests.map(({ path }) => path), [
    "/api/v1/protocol",
    projectPath,
    "/api/v1/projects",
    `${projectPath}/manuscript/tree`,
    `${projectPath}/manuscript/statistics`,
    chapterPath,
  ]);
  const projectRequest = requests.at(1);
  const listRequest = requests.at(2);
  const chapterRequest = requests.at(5);
  assert.ok(projectRequest);
  assert.ok(listRequest);
  assert.ok(chapterRequest);
  assert.equal(projectRequest.options?.credentials, "same-origin");
  assert.equal(listRequest.options?.credentials, "same-origin");
  assert.equal(chapterRequest.options?.credentials, "same-origin");
  assert.equal(new Headers(projectRequest.options?.headers).get("x-storyos-client-session"), null);
});

test("a Canonical Query tree with a foreign Project Scope fails closed", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  const tree = fixture<GetManuscriptTreeResponse>("get-manuscript-tree.json");
  project.project = {
    project_id: project.project.project_id,
    title: "Empty Novel",
    open: { kind: "empty" },
  };
  tree.project_scope = {
    owner_user_id: "018f0000-0000-7001-8000-000000000101",
    project_id: project.project.project_id,
  };
  tree.snapshot.project_scope = tree.project_scope;
  const requests: string[] = [];
  const responses: unknown[] = [profile, project, listed, tree];
  const fetchImpl: typeof fetch = async (url) => {
    requests.push(new URL(requestUrl(url)).pathname);
    return new Response(JSON.stringify(responses.shift()), {
      status: 200, headers: { "content-type": "application/json" },
    });
  };

  const state = await openControlledProject({
    baseUrl: "http://storyos.test",
    projectId: project.project.project_id,
    fetchImpl,
  });
  assert.deepEqual(state, {
    kind: "project-blocked",
    code: "project_unavailable",
    heading: "StoryOS 无法打开项目",
    message: "无法读取这个受控项目或其当前章节。",
  });
  assert.deepEqual(requests, [
    "/api/v1/protocol",
    `/api/v1/projects/${project.project.project_id}`,
    "/api/v1/projects",
    `/api/v1/projects/${project.project.project_id}/manuscript/tree`,
  ]);
});

test("a Project that the library does not list fails closed without opening a workspace", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  listed.projects = listed.projects.filter((entry) =>
    entry.project_scope.project_id !== project.project.project_id);
  const requests: string[] = [];
  const responses: unknown[] = [profile, project, listed];
  const fetchImpl: typeof fetch = async (url) => {
    requests.push(new URL(requestUrl(url)).pathname);
    return new Response(JSON.stringify(responses.shift()), {
      status: 200, headers: { "content-type": "application/json" },
    });
  };

  const state = await openControlledProject({
    baseUrl: "http://storyos.test",
    projectId: project.project.project_id,
    fetchImpl,
  });
  assert.deepEqual(state, {
    kind: "project-blocked",
    code: "project_unavailable",
    heading: "StoryOS 无法打开项目",
    message: "无法读取这个受控项目或其当前章节。",
  });
  assert.deepEqual(requests, [
    "/api/v1/protocol",
    `/api/v1/projects/${project.project.project_id}`,
    "/api/v1/projects",
  ]);
});

test("a Project missing from the library fails closed without opening a workspace", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  listed.projects = listed.projects.filter((entry) =>
    entry.project_scope.project_id !== project.project.project_id);
  const requests: string[] = [];
  const responses: unknown[] = [profile, project, listed];
  const fetchImpl: typeof fetch = async (url) => {
    requests.push(new URL(requestUrl(url)).pathname);
    return new Response(JSON.stringify(responses.shift()), {
      status: 200, headers: { "content-type": "application/json" },
    });
  };

  const state = await openControlledProject({
    baseUrl: "http://storyos.test",
    projectId: project.project.project_id,
    fetchImpl,
  });
  assert.deepEqual(state, {
    kind: "project-blocked",
    code: "project_unavailable",
    heading: "StoryOS 无法打开项目",
    message: "无法读取这个受控项目或其当前章节。",
  });
  assert.deepEqual(requests, [
    "/api/v1/protocol",
    `/api/v1/projects/${project.project.project_id}`,
    "/api/v1/projects",
  ]);
});

test("an unavailable Project fails closed without displaying response data", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const sourceError = new Error("foreign title must remain internal");
  const fetchImpl: typeof fetch = async (url) => new URL(requestUrl(url)).pathname === "/api/v1/protocol"
    ? new Response(JSON.stringify(profile), { status: 200 })
    : Promise.reject(sourceError);

  const state = await openControlledProject({
    baseUrl: "http://storyos.test",
    projectId: "018f0000-0000-7001-8000-000000000102",
    fetchImpl,
  });
  assert.deepEqual(state, {
    kind: "project-blocked",
    code: "project_unavailable",
    heading: "StoryOS 无法打开项目",
    message: "无法读取这个受控项目或其当前章节。",
  });
  assert.equal(record(Reflect.get(state, PROJECT_OPEN_DIAGNOSTIC_CAUSE))?.cause, sourceError);
  assert.doesNotMatch(JSON.stringify(state), /foreign title/);
});

test("missing, empty, invalid, or mismatched owner identities fail closed", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const projectFixture = fixture<GetProjectResponse>("get-project.json");
  const chapterFixture = fixture<GetChapterResponse>("get-chapter.json");
  const cases = [
    { name: "missing", projectOwner: undefined, chapterOwner: undefined },
    { name: "empty", projectOwner: "", chapterOwner: "" },
    { name: "invalid", projectOwner: "not-a-uuid", chapterOwner: "not-a-uuid" },
    {
      name: "mismatched",
      projectOwner: "018f0000-0000-7001-8000-000000000001",
      chapterOwner: "018f0000-0000-7001-8000-000000000101",
    },
  ];

  for (const { name, projectOwner, chapterOwner } of cases) {
    const project = structuredClone(projectFixture);
    const chapter = structuredClone(chapterFixture);
    if (projectOwner === undefined) Reflect.deleteProperty(project.project_scope, "owner_user_id");
    else project.project_scope.owner_user_id = projectOwner;
    if (chapterOwner === undefined) Reflect.deleteProperty(chapter.project_scope, "owner_user_id");
    else chapter.project_scope.owner_user_id = chapterOwner;
    const listed = fixture<ListProjectsResponse>("list-projects.json");
    const responses = [profile, project, listed, chapter];
    const fetchImpl = async () => new Response(JSON.stringify(responses.shift()), { status: 200 });

    assert.deepEqual(await openControlledProject({
      baseUrl: "http://storyos.test",
      projectId: project.project.project_id,
      fetchImpl,
    }), {
      kind: "project-blocked",
      code: "project_unavailable",
      heading: "StoryOS 无法打开项目",
      message: "无法读取这个受控项目或其当前章节。",
    }, name);
  }
});

/** A fake that serves only the given routes. It records each other request and throws. */
function exactFake(routes: Map<string, { body: unknown; status?: number }>) {
  const requests: string[] = [];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(requestUrl(url)).pathname}`;
    requests.push(name);
    const route = routes.get(name);
    if (route === undefined) {
      unexpected.push(name);
      throw new Error(`unexpected request: ${name}`);
    }
    return new Response(JSON.stringify(route.body),
      { status: route.status ?? 200, headers: { "content-type": "application/json" } });
  };
  return { requests, unexpected, fetchImpl };
}

const PROJECT_BLOCKED = {
  kind: "project-blocked",
  code: "project_unavailable",
  heading: "StoryOS 无法打开项目",
  message: "无法读取这个受控项目或其当前章节。",
};

test("a reopen fails closed when the Project read has another Scope", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  const projectId = project.project.project_id;
  const chapterId = project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "";
  const foreign = { ...project, project_scope: { ...project.project_scope,
    owner_user_id: "018f0000-0000-7001-8000-000000000101" } };
  // The reopen reads the Current Chapter of the page with the Project read (#1898).
  const fake = exactFake(new Map([
    [`GET /api/v1/projects/${projectId}`, { body: foreign }],
    ["GET /api/v1/projects", { body: listed }],
    [`GET /api/v1/projects/${projectId}/chapters/${chapterId}`, { body: fixture<GetChapterResponse>("get-chapter.json") }],
  ]));

  const state = await reopenControlledProject({ baseUrl: "http://storyos.test", opened: { profile, project },
    fetchImpl: fake.fetchImpl });

  assert.deepEqual({ state, requests: fake.requests.sort(), unexpected: fake.unexpected }, {
    state: PROJECT_BLOCKED,
    requests: ["GET /api/v1/projects", `GET /api/v1/projects/${projectId}`,
      `GET /api/v1/projects/${projectId}/chapters/${chapterId}`].sort(),
    unexpected: [],
  });
});

test("a reopen opens the Current Chapter of the Project read with the revision of the list", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  const chapter = fixture<GetChapterResponse>("get-chapter.json");
  const projectId = project.project.project_id;
  const chapterId = project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "";
  // Another tab renamed the Project, so the list has a new revision.
  listed.projects = listed.projects.map((entry) => entry.project_scope.project_id === projectId
    ? { ...entry, title: "Renamed", revision: "7" } : entry);
  const fake = exactFake(new Map([
    [`GET /api/v1/projects/${projectId}`, { body: { ...project, project: { ...project.project, title: "Renamed" } } }],
    ["GET /api/v1/projects", { body: listed }],
    [`GET /api/v1/projects/${projectId}/chapters/${chapterId}`, { body: chapter }],
  ]));

  const state = await reopenControlledProject({ baseUrl: "http://storyos.test", opened: { profile, project },
    fetchImpl: fake.fetchImpl });

  assert.deepEqual({
    kind: state.kind,
    title: state.kind === "project-ready" ? state.project.project.title : "",
    revision: state.kind === "project-ready" ? state.revision : "",
    requests: fake.requests.sort(),
    unexpected: fake.unexpected,
  }, {
    kind: "project-ready",
    title: "Renamed",
    revision: "7",
    // The Chapter read goes with the Project read and the list read (#1898).
    requests: ["GET /api/v1/projects", `GET /api/v1/projects/${projectId}`,
      `GET /api/v1/projects/${projectId}/chapters/${chapterId}`].sort(),
    unexpected: [],
  });
});

test("a reopen fails closed when the list read fails", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const projectId = project.project.project_id;
  const chapterId = project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "";
  const fake = exactFake(new Map([
    [`GET /api/v1/projects/${projectId}`, { body: project }],
    ["GET /api/v1/projects", { status: 503, body: { schema_id: "storyos.problem.v1", code: "project_store_unavailable",
      message: "The Project store is unavailable." } }],
    [`GET /api/v1/projects/${projectId}/chapters/${chapterId}`, { body: fixture<GetChapterResponse>("get-chapter.json") }],
  ]));

  const state = await reopenControlledProject({ baseUrl: "http://storyos.test", opened: { profile, project },
    fetchImpl: fake.fetchImpl });

  assert.deepEqual({ state, unexpected: fake.unexpected }, { state: PROJECT_BLOCKED, unexpected: [] });
});

test("an incompatible protocol profile blocks the open while the Project reads do not end", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const projectId = "018f0000-0000-7001-8000-000000000102";
  // The Project reads of the open do not end. The fake records each other request.
  const reads = ["GET /api/v1/projects", `GET /api/v1/projects/${projectId}`];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(requestUrl(url)).pathname}`;
    if (name === "GET /api/v1/protocol") {
      return new Response(JSON.stringify({ ...profile,
        release_identity: { ...profile.release_identity, public_protocol_release: "0" } }), { status: 200 });
    }
    if (reads.includes(name)) return new Promise<Response>(() => {});
    unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };

  const state = await openControlledProject({ baseUrl: "http://storyos.test", projectId, fetchImpl });

  assert.deepEqual({ kind: state.kind, unexpected }, { kind: "protocol-blocked", unexpected: [] });
});
