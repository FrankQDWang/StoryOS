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
  const responses: unknown[] = [profile, project, listed, chapter];
  const fetchImpl: typeof fetch = async (url, options) => {
    requests.push({ path: new URL(requestUrl(url)).pathname, options });
    return new Response(JSON.stringify(responses.shift()), {
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
    `/api/v1/projects/${project.project.project_id}`,
    "/api/v1/projects",
    `/api/v1/projects/${project.project.project_id}/chapters/${
      project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "missing"
    }`,
  ]);
  const projectRequest = requests.at(1);
  const listRequest = requests.at(2);
  const chapterRequest = requests.at(3);
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

test("a reopen fails closed when the Project of the command or of the read has another Scope", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const foreign = { ...project, project_scope: { ...project.project_scope,
    owner_user_id: "018f0000-0000-7001-8000-000000000101" } };
  const opened = { profile, project, title: project.project.title, revision: "1" };
  const blocked = {
    kind: "project-blocked",
    code: "project_unavailable",
    heading: "StoryOS 无法打开项目",
    message: "无法读取这个受控项目或其当前章节。",
  };
  const requests: string[] = [];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(requestUrl(url)).pathname}`;
    requests.push(name);
    if (name === `GET /api/v1/projects/${project.project.project_id}`) {
      return new Response(JSON.stringify(foreign), { status: 200, headers: { "content-type": "application/json" } });
    }
    unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };

  const settled = await reopenControlledProject({ baseUrl: "http://storyos.test", opened, settled: foreign, fetchImpl });
  const read = await reopenControlledProject({ baseUrl: "http://storyos.test", opened, fetchImpl });

  assert.deepEqual({ settled, read, requests, unexpected }, {
    settled: blocked, read: blocked, requests: [`GET /api/v1/projects/${project.project.project_id}`], unexpected: [],
  });
});

test("a reopen reads the revision from the list when another tab renamed the Project", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const listed = fixture<ListProjectsResponse>("list-projects.json");
  const chapter = fixture<GetChapterResponse>("get-chapter.json");
  const projectId = project.project.project_id;
  const chapterId = project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "";
  listed.projects = listed.projects.map((entry) => entry.project_scope.project_id === projectId
    ? { ...entry, title: "Renamed", revision: "7" } : entry);
  const routes = new Map<string, unknown>([
    ["GET /api/v1/projects", listed],
    [`GET /api/v1/projects/${projectId}/chapters/${chapterId}`, chapter],
  ]);
  const requests: string[] = [];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(requestUrl(url)).pathname}`;
    requests.push(name);
    if (!routes.has(name)) {
      unexpected.push(name);
      throw new Error(`unexpected request: ${name}`);
    }
    return new Response(JSON.stringify(routes.get(name)), { status: 200, headers: { "content-type": "application/json" } });
  };
  const settled = { project_scope: project.project_scope, project: { ...project.project, title: "Renamed" } };
  const opened = { profile, project, title: project.project.title, revision: "1" };

  const renamed = await reopenControlledProject({ baseUrl: "http://storyos.test", opened, settled, fetchImpl });
  const same = await reopenControlledProject({ baseUrl: "http://storyos.test",
    opened: { ...opened, title: "Renamed" }, settled, fetchImpl });

  assert.deepEqual({
    revisions: [renamed, same].map((state) => state.kind === "project-ready" ? state.revision : state.kind),
    requests: requests.sort(), unexpected,
  }, {
    revisions: ["7", "1"],
    requests: ["GET /api/v1/projects", `GET /api/v1/projects/${projectId}/chapters/${chapterId}`,
      `GET /api/v1/projects/${projectId}/chapters/${chapterId}`].sort(),
    unexpected: [],
  });
});

test("a reopen leaves the revision unknown when the list read fails after another tab renamed the Project", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const chapter = fixture<GetChapterResponse>("get-chapter.json");
  const projectId = project.project.project_id;
  const chapterId = project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "";
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(requestUrl(url)).pathname}`;
    if (name === "GET /api/v1/projects") {
      return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "service_unavailable",
        message: "Unavailable." }), { status: 503, headers: { "content-type": "application/json" } });
    }
    if (name === `GET /api/v1/projects/${projectId}/chapters/${chapterId}`) {
      return new Response(JSON.stringify(chapter), { status: 200, headers: { "content-type": "application/json" } });
    }
    unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  const settled = { project_scope: project.project_scope, project: { ...project.project, title: "Renamed" } };

  const state = await reopenControlledProject({ baseUrl: "http://storyos.test",
    opened: { profile, project, title: project.project.title, revision: "1" }, settled, fetchImpl });

  // The view reads the list again, and the rename waits for the revision.
  assert.deepEqual({ kind: state.kind, revision: state.kind === "project-ready" ? state.revision : "none", unexpected },
    { kind: "project-ready", revision: undefined, unexpected: [] });
});

test("a reopen reads the Project when the Chapter of the Command-response Project cannot be read", async () => {
  const profile = fixture<Release1ProtocolProfile>("get-protocol-profile.json");
  const project = fixture<GetProjectResponse>("get-project.json");
  const chapter = fixture<GetChapterResponse>("get-chapter.json");
  const projectId = project.project.project_id;
  const chapterId = project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "";
  // The stored response of an exact retry names a Chapter that a later command removed.
  const removed = "018f0000-0000-7001-8000-0000000009f1";
  const requests: string[] = [];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (url, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(requestUrl(url)).pathname}`;
    requests.push(name);
    const json = (body: unknown, status = 200) => new Response(JSON.stringify(body),
      { status, headers: { "content-type": "application/json" } });
    if (name === `GET /api/v1/projects/${projectId}/chapters/${removed}`) {
      return json({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === `GET /api/v1/projects/${projectId}`) return json(project);
    if (name === `GET /api/v1/projects/${projectId}/chapters/${chapterId}`) return json(chapter);
    unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  const settled = { project_scope: project.project_scope,
    project: { ...project.project, open: { kind: "current_chapter" as const, current_chapter_id: removed } } };

  const state = await reopenControlledProject({ baseUrl: "http://storyos.test",
    opened: { profile, project, title: project.project.title, revision: "1" }, settled, fetchImpl });

  assert.deepEqual({ kind: state.kind, chapter: state.kind === "project-ready" ? state.chapter.chapter.chapter_id : "",
    requests, unexpected }, { kind: "project-ready", chapter: chapterId, requests: [
    `GET /api/v1/projects/${projectId}/chapters/${removed}`, `GET /api/v1/projects/${projectId}`,
    `GET /api/v1/projects/${projectId}/chapters/${chapterId}`], unexpected: [] });
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
