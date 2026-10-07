import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { beginInlineChapterCreation, beginInlineVolumeCreation } from "../support/inline-chapter-creation.ts";
import { jsonResponse } from "./scenario.ts";

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000012";
const VOLUME = "018f0000-0000-7001-8000-000000000815";
const CHAPTER = "018f0000-0000-7001-8000-000000000816";

type Created = { path: string; idempotencyKey: string | null; expectedTreeRevision: unknown };

function manuscriptTree(withVolume: boolean) {
  return {
    schema_id: "storyos.query.manuscript-tree.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000015",
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    tree_revision: withVolume ? "2" : "1",
    snapshot: {
      snapshot_id: "018f0000-0000-7001-8000-000000000032",
      project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    },
    volumes: withVolume ? [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [] }] : [],
  };
}

async function createThenBlur(options: {
  createPath: string;
  appliedEffect: Record<string, unknown>;
  withVolume: boolean;
  begin: (root: HTMLElement) => Promise<void>;
  inputSelector: string;
  title: string;
}): Promise<Created[]> {
  const created: Created[] = [];
  let digests = 0;
  let input: HTMLInputElement | undefined;
  let blurAfterRefresh = false;
  const subtle = new Proxy(crypto.subtle, {
    get: (target, key) => {
      if (key === "digest") {
        return (algorithm: AlgorithmIdentifier, data: BufferSource) => {
          digests += 1;
          return target.digest(algorithm, data);
        };
      }
      const value: unknown = Reflect.get(target, key);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  const cryptoImpl = new Proxy(crypto, {
    get: (target, key) => {
      if (key === "subtle") return subtle;
      const value: unknown = Reflect.get(target, key);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  const fetchImpl: typeof fetch = async (request, init) => {
    const path = new URL(request instanceof Request ? request.url : request).pathname;
    const method = init?.method ?? "GET";
    if (blurAfterRefresh && method === "GET") {
      blurAfterRefresh = false;
      // The created form stays on screen until React commits; an author can blur it in that interval.
      void (async () => {
        for (let tick = 0; tick < 50 && input?.isConnected === true; tick += 1) {
          input.focus();
          input.blur();
          await Promise.resolve();
        }
      })();
    }
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
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
      });
    }
    if (path === `/api/v1/projects/${PROJECT}`) {
      return jsonResponse({
        schema_id: "storyos.query.project.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014",
        project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        project: { project_id: PROJECT, title: "Listed Novel", open: { kind: "empty" } },
      });
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse(manuscriptTree(options.withVolume || created.length > 0));
    }
    if (path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges`) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path === options.createPath && method === "POST") {
      const body = JSON.parse(String(init?.body)) as Record<string, { expected_tree_revision?: unknown }>;
      created.push({
        path,
        idempotencyKey: new Headers(init?.headers).get("idempotency-key"),
        expectedTreeRevision: Object.values(body).find((value) => typeof value === "object")
          ?.expected_tree_revision,
      });
      blurAfterRefresh = created.length === 1;
      return jsonResponse({
        effect: created.length === 1 ? options.appliedEffect : { kind: "conflicted", reason: "stale_tree_revision" },
      });
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  document.body.innerHTML = '<main id="app"></main>';
  try {
    const loaded = await loadStoryOSWebState({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl,
      cryptoImpl,
    });
    mountStage1View(loaded.root, loaded);
    await options.begin(loaded.root);
    input = loaded.root.querySelector<HTMLInputElement>(options.inputSelector) ?? undefined;
    if (input?.form === null || input?.form === undefined) throw new Error("the creation form is missing");
    input.focus();
    input.value = options.title;
    input.form.requestSubmit();
    await expect.poll(() => loaded.root.querySelector(options.inputSelector)).toBeNull();
    await expect.poll(() => created.length).toBe(digests);
    return created;
  } finally {
    document.body.replaceChildren();
  }
}

it("sends one Create Volume when the title input blurs after the Volume is created", async () => {
  const created = await createThenBlur({
    createPath: `/api/v1/projects/${PROJECT}/volumes`,
    appliedEffect: { kind: "authoritative_applied", volume_id: VOLUME, tree_revision: "2", order: "1" },
    withVolume: false,
    begin: beginInlineVolumeCreation,
    inputSelector: 'form[data-create-volume] input[name="volume-title"]',
    title: "Volume A",
  });
  expect(created).toEqual([{
    path: `/api/v1/projects/${PROJECT}/volumes`,
    idempotencyKey: created[0]?.idempotencyKey,
    expectedTreeRevision: "1",
  }]);
});

it("sends one Create Chapter when the title input blurs after the Chapter is created", async () => {
  const created = await createThenBlur({
    createPath: `/api/v1/projects/${PROJECT}/volumes/${VOLUME}/chapters`,
    appliedEffect: { kind: "authoritative_applied", chapter_id: CHAPTER, tree_revision: "3" },
    withVolume: true,
    begin: (root) => beginInlineChapterCreation(root, VOLUME),
    inputSelector: `form[data-create-chapter="${VOLUME}"] input[name="chapter-title"]`,
    title: "Chapter A",
  });
  expect(created).toEqual([{
    path: `/api/v1/projects/${PROJECT}/volumes/${VOLUME}/chapters`,
    idempotencyKey: created[0]?.idempotencyKey,
    expectedTreeRevision: "2",
  }]);
});
