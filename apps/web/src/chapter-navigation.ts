import {
  getChapter,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetChapterResponse,
  ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { PendingEditProjection } from "./editor-types.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export type JournalSwitchGate =
  | { kind: "ready" }
  | {
    kind: "refused";
    reason: "incomplete_semantic_intent" | "journal_unavailable";
  };

export type OpenSelectedChapter =
  | { kind: "opened"; chapter: GetChapterResponse }
  | { kind: "missing" }
  | { kind: "snapshot_expired" }
  | { kind: "unavailable" };

export interface SelectedChapterSurface {
  title: string;
  body: string;
  save_state: PendingEditProjection["save_state"];
  pending: PendingEditProjection | null;
  editable: boolean;
  authoritative_revision_id: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function problemCode(error: unknown): string | undefined {
  if (!(error instanceof StoryOSProtocolError) || typeof error.responseBody !== "string") {
    return undefined;
  }
  try {
    const body: unknown = JSON.parse(error.responseBody);
    if (!isRecord(body)) return undefined;
    const code = body.code;
    return typeof code === "string" ? code : undefined;
  } catch {
    return undefined;
  }
}

function validUuid(value: unknown): value is string {
  return typeof value === "string" && UUID.test(value);
}

export async function completeJournalOrRefuse(options: {
  incompleteSemanticIntent: boolean;
  whenIdle: () => Promise<void>;
}): Promise<JournalSwitchGate> {
  if (options.incompleteSemanticIntent) {
    return { kind: "refused", reason: "incomplete_semantic_intent" };
  }
  try {
    await options.whenIdle();
  } catch {
    return { kind: "refused", reason: "journal_unavailable" };
  }
  return { kind: "ready" };
}

export async function openSelectedChapter(options: {
  baseUrl: string;
  projectId: string;
  chapterId: string;
  expectedScope: ProjectScope;
  fetchImpl: typeof fetch;
}): Promise<OpenSelectedChapter> {
  try {
    const chapterValue: unknown = await getChapter({
      baseUrl: options.baseUrl,
      projectId: options.projectId,
      chapterId: options.chapterId,
      fetchImpl: options.fetchImpl,
    });
    const chapterView = chapterValue as {
      project_scope?: { owner_user_id?: unknown; project_id?: unknown };
      chapter?: { chapter_id?: unknown };
    };
    const ownerUserId = chapterView?.project_scope?.owner_user_id;
    if (!validUuid(ownerUserId)
      || ownerUserId !== options.expectedScope.owner_user_id
      || chapterView?.project_scope?.project_id !== options.expectedScope.project_id
      || chapterView?.chapter?.chapter_id !== options.chapterId) {
      return { kind: "unavailable" };
    }
    return { kind: "opened", chapter: chapterValue as GetChapterResponse };
  } catch (error) {
    const code = problemCode(error);
    if (error instanceof StoryOSProtocolError && error.status === 404
      && code === "resource_unavailable") {
      return { kind: "missing" };
    }
    if (error instanceof StoryOSProtocolError && error.status === 409
      && code === "snapshot_expired") {
      return { kind: "snapshot_expired" };
    }
    return { kind: "unavailable" };
  }
}

export function selectedChapterSurface(options: {
  selectedChapterId: string;
  currentChapterId: string;
  currentPending: PendingEditProjection | null;
  opened: GetChapterResponse;
}): SelectedChapterSurface {
  const { opened } = options;
  if (options.selectedChapterId === options.currentChapterId && options.currentPending) {
    return {
      title: opened.chapter.title,
      body: options.currentPending.body,
      save_state: options.currentPending.save_state,
      pending: options.currentPending,
      editable: true,
      authoritative_revision_id: options.currentPending.authoritative_revision_id,
    };
  }
  const saveState = options.selectedChapterId === options.currentChapterId
    ? "needs_attention"
    : "clean";
  return {
    title: opened.chapter.title,
    body: opened.chapter.current_revision.body,
    save_state: saveState,
    pending: {
      body: opened.chapter.current_revision.body,
      blocks: opened.chapter.current_revision.blocks,
      save_state: saveState,
      unsettled_intent_count: 0,
      authoritative_revision_id: opened.chapter.current_revision.revision_id,
    },
    editable: false,
    authoritative_revision_id: opened.chapter.current_revision.revision_id,
  };
}

export function chapterSwitchRecoveryMessage(
  reason: Exclude<JournalSwitchGate, { kind: "ready" }>["reason"]
    | Exclude<OpenSelectedChapter, { kind: "opened" }>["kind"],
): string {
  if (reason === "incomplete_semantic_intent") return "无法切换章节：请先完成当前输入。";
  if (reason === "journal_unavailable") {
    return "无法切换章节：本地编辑需要恢复。";
  }
  if (reason === "snapshot_expired") return "无法打开这一章：快照已过期。";
  return "无法打开这一章。";
}
