import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import {
  openControlledProject,
  PROJECT_OPEN_DIAGNOSTIC_CAUSE,
} from "../../src/boot.ts";
import type {
  GetChapterResponse,
  GetProjectResponse,
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
  const chapter = fixture<GetChapterResponse>("get-chapter.json");
  const requests: Array<{ path: string; options: RequestInit | undefined }> = [];
  const responses: unknown[] = [profile, project, chapter];
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
    `/api/v1/projects/${project.project.project_id}/chapters/${
      project.project.open.kind === "current_chapter" ? project.project.open.current_chapter_id : "missing"
    }`,
  ]);
  const projectRequest = requests.at(1);
  const chapterRequest = requests.at(2);
  assert.ok(projectRequest);
  assert.ok(chapterRequest);
  assert.equal(projectRequest.options?.credentials, "same-origin");
  assert.equal(chapterRequest.options?.credentials, "same-origin");
  assert.equal(new Headers(projectRequest.options?.headers).get("x-storyos-client-session"), null);
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
    const responses = [profile, project, chapter];
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
