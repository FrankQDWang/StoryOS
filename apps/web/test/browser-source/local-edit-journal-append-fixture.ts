import { openEditorWorkspace } from "../../src/editor-session.ts";
import type { ReplaceSelectionEdit } from "../../src/editor-types.ts";
import {
  OWNER,
  PROJECT,
  SESSION,
  closeTrackedDatabases,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requireEditorReady,
  trackDatabase,
} from "./scenario.ts";

export const FIRST_APPEND_EDIT: ReplaceSelectionEdit = {
  from: 4,
  to: 4,
  text: "!",
  resultingBody: "Base!",
  inputOrigin: "typing",
  undoGroupId: "018f0000-0000-7001-8000-000000000040",
  createdAt: "2026-08-15T08:00:00.000Z",
};

export const SECOND_APPEND_EDIT: ReplaceSelectionEdit = {
  from: 5,
  to: 5,
  text: "?",
  resultingBody: "Base!?",
  inputOrigin: "typing",
  undoGroupId: "018f0000-0000-7001-8000-000000000040",
  createdAt: "2026-08-15T08:00:00.001Z",
};

export function withDigest(
  cryptoImpl: Crypto,
  digest: SubtleCrypto["digest"],
): Crypto {
  const subtle = new Proxy(cryptoImpl.subtle, {
    get(target, property) {
      if (property === "digest") return digest;
      const value: unknown = Reflect.get(target, property, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  return new Proxy(cryptoImpl, {
    get(target, property) {
      if (property === "subtle") return subtle;
      const value: unknown = Reflect.get(target, property, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
}

export function createPausedDigestCrypto(cryptoImpl: Crypto) {
  let releaseDigest!: () => void;
  let reachedDigest!: () => void;
  const release = new Promise<void>((resolve) => { releaseDigest = resolve; });
  const reached = new Promise<void>((resolve) => { reachedDigest = resolve; });
  let shouldPause = true;
  const digest: SubtleCrypto["digest"] = async (algorithm, data) => {
    if (shouldPause) {
      shouldPause = false;
      reachedDigest();
      await release;
    }
    return cryptoImpl.subtle.digest(algorithm, data);
  };
  return { cryptoImpl: withDigest(cryptoImpl, digest), reached, release: releaseDigest };
}

export function withDigestBudget(cryptoImpl: Crypto, remaining: number): Crypto {
  const digest: SubtleCrypto["digest"] = async (algorithm, data) => {
    if (remaining === 0) {
      throw new Error("unexpected Journal digest after validated append");
    }
    remaining -= 1;
    return cryptoImpl.subtle.digest(algorithm, data);
  };
  return withDigest(cryptoImpl, digest);
}

export async function openJournalAppendTestWorkspace() {
  const scenario = createBrowserScenario();
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
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
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({
    baseUrl: location.origin,
    project: scenario.project,
    chapter: scenario.chapter,
    profile: scenario.profile,
    fetchImpl,
    indexedDBImpl: indexedDB,
    cryptoImpl: crypto,
  });
  requireEditorReady(workspace);
  trackDatabase(workspace.database, openDatabases);
  return {
    workspace,
    async close() {
      closeTrackedDatabases(openDatabases);
      sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
      await deleteJournal(scenario.journalName);
    },
  };
}
