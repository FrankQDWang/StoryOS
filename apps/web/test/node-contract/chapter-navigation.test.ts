import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import type { GetChapterResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  completeJournalOrRefuse,
  openSelectedChapter,
  selectedChapterSurface,
} from "../../src/chapter-navigation.ts";
import type { PendingEditProjection } from "../../src/editor-types.ts";

const fixture = <Value>(name: string): Value => JSON.parse(readFileSync(
  new URL(`../../../../generated/golden-wire/storyos-public-release-1/${name}`, import.meta.url),
  "utf8",
));

const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const REVISION_B = "018f0000-0000-7001-8000-000000000805";

function chapterB(source: GetChapterResponse): GetChapterResponse {
  return {
    ...source,
    correlation_id: "018f0000-0000-7001-8000-000000000806",
    chapter: {
      chapter_id: CHAPTER_B,
      title: "Chapter B",
      current_revision: { revision_id: REVISION_B, body: "" },
    },
  };
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

test("openSelectedChapter returns the requested Chapter Head", async () => {
  const source = fixture<GetChapterResponse>("get-chapter.json");
  const opened = chapterB(source);
  const requests: string[] = [];
  const result = await openSelectedChapter({
    baseUrl: "http://storyos.test",
    projectId: source.project_scope.project_id,
    chapterId: CHAPTER_B,
    expectedScope: source.project_scope,
    fetchImpl: async (url) => {
      requests.push(new URL(url instanceof Request ? url.url : url).pathname);
      return jsonResponse(opened);
    },
  });
  assert.deepEqual(result, { kind: "opened", chapter: opened });
  assert.deepEqual(requests, [
    `/api/v1/projects/${source.project_scope.project_id}/chapters/${CHAPTER_B}`,
  ]);
});

test("openSelectedChapter classifies missing, expired, and Scope-mismatched Chapters", async () => {
  const source = fixture<GetChapterResponse>("get-chapter.json");
  const foreign = chapterB(source);
  foreign.project_scope = {
    owner_user_id: "018f0000-0000-7001-8000-000000000101",
    project_id: source.project_scope.project_id,
  };
  const cases: Array<{
    name: string;
    response: Response | Error;
    expected: "missing" | "snapshot_expired" | "unavailable";
  }> = [
    {
      name: "missing",
      response: jsonResponse({
        schema_id: "storyos.problem.v1",
        code: "resource_unavailable",
        message: "The requested resource is unavailable.",
      }, 404),
      expected: "missing",
    },
    {
      name: "expired",
      response: jsonResponse({
        schema_id: "storyos.problem.v1",
        code: "snapshot_expired",
        message: "The Snapshot is no longer available.",
      }, 409),
      expected: "snapshot_expired",
    },
    {
      name: "wrong Scope",
      response: jsonResponse(foreign),
      expected: "unavailable",
    },
    {
      name: "wrong Chapter identity",
      response: jsonResponse(source),
      expected: "unavailable",
    },
  ];
  for (const { name, response, expected } of cases) {
    const result = await openSelectedChapter({
      baseUrl: "http://storyos.test",
      projectId: source.project_scope.project_id,
      chapterId: CHAPTER_B,
      expectedScope: source.project_scope,
      fetchImpl: async () => {
        if (response instanceof Error) throw response;
        return response;
      },
    });
    assert.deepEqual(result, { kind: expected }, name);
    assert.doesNotMatch(JSON.stringify(result), /Chapter B|雨落在窗沿|000000000101/);
  }
});

test("completeJournalOrRefuse waits for a durable Journal or refuses with a typed gate", async () => {
  const idleCalls: number[] = [];
  const whenIdle = async () => {
    idleCalls.push(idleCalls.length + 1);
  };
  assert.deepEqual(await completeJournalOrRefuse({
    incompleteSemanticIntent: true,
    whenIdle,
  }), { kind: "refused", reason: "incomplete_semantic_intent" });
  assert.deepEqual(idleCalls, []);
  assert.deepEqual(await completeJournalOrRefuse({
    incompleteSemanticIntent: false,
    whenIdle: async () => {
      throw new Error("Local Edit Journal is corrupt");
    },
  }), { kind: "refused", reason: "journal_unavailable" });
  assert.deepEqual(await completeJournalOrRefuse({
    incompleteSemanticIntent: false,
    whenIdle,
  }), { kind: "ready" });
  assert.deepEqual(idleCalls, [1]);
});

test("selectedChapterSurface keeps pending bytes on the current Chapter", () => {
  const source = fixture<GetChapterResponse>("get-chapter.json");
  const openedB = chapterB(source);
  const currentPending: PendingEditProjection = {
    body: "雨落在窗沿。Hello",
    save_state: "saving",
    unsettled_intent_count: 1,
    authoritative_revision_id: source.chapter.current_revision.revision_id,
  };
  assert.deepEqual(selectedChapterSurface({
    selectedChapterId: source.chapter.chapter_id,
    currentChapterId: source.chapter.chapter_id,
    currentPending,
    opened: source,
  }), {
    title: source.chapter.title,
    body: currentPending.body,
    save_state: "saving",
    pending: currentPending,
    editable: true,
    authoritative_revision_id: currentPending.authoritative_revision_id,
  });
  assert.deepEqual(selectedChapterSurface({
    selectedChapterId: CHAPTER_B,
    currentChapterId: source.chapter.chapter_id,
    currentPending,
    opened: openedB,
  }), {
    title: "Chapter B",
    body: "",
    save_state: "clean",
    pending: {
      body: "",
      save_state: "clean",
      unsettled_intent_count: 0,
      authoritative_revision_id: REVISION_B,
    },
    editable: false,
    authoritative_revision_id: REVISION_B,
  });
  assert.deepEqual(selectedChapterSurface({
    selectedChapterId: source.chapter.chapter_id,
    currentChapterId: source.chapter.chapter_id,
    currentPending: null,
    opened: source,
  }), {
    title: source.chapter.title,
    body: source.chapter.current_revision.body,
    save_state: "needs_attention",
    pending: {
      body: source.chapter.current_revision.body,
      save_state: "needs_attention",
      unsettled_intent_count: 0,
      authoritative_revision_id: source.chapter.current_revision.revision_id,
    },
    editable: false,
    authoritative_revision_id: source.chapter.current_revision.revision_id,
  });
});
