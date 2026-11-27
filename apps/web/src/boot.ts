import {
  getChapter,
  getManuscriptTree,
  getProject,
  getProtocolProfile,
  listProjects,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type {
  GetChapterResponse,
  GetManuscriptTreeResponse,
  GetProjectResponse,
  Release1ProtocolProfile,
  StoryOSQueryOptions,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ControlledProjectState,
  ProjectBlockedState,
  ProjectReadyState,
  ProtocolBlockedState,
  ProtocolBootState,
  StateDifference,
} from "./editor-types.ts";
import { startEditorWorkspace, type EditorWorkspaceOpening } from "./editor-session.ts";
import { setOwnedCurrentChapter } from "./set-current-chapter.ts";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";

const { release_identity: expectedIdentity, required_capabilities: expectedCapabilities, ...expectedProtocol } = RELEASE_1_PROTOCOL_PROFILE;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const blockedMessages: Record<ProtocolBlockedState["code"], string> = {
  protocol_identity_missing: "服务器缺少 Release 1 兼容身份，受保护状态不会开放。",
  protocol_upgrade_required: "Web 客户端与服务器的 Release 1 契约不一致，请更新后重试。",
  protocol_capabilities_incompatible: "服务器能力与此 Web 客户端不兼容，受保护状态不会开放。",
  protocol_unavailable: "无法读取服务器协议身份，StoryOS 已停止进入受保护状态。",
};
const blocked = (
  code: ProtocolBlockedState["code"],
  details: StateDifference[] = [],
): ProtocolBlockedState => ({
  kind: "protocol-blocked", code, heading: "StoryOS 无法安全启动", message: blockedMessages[code], details,
});

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function objectField(value: object, field: string): unknown {
  return Reflect.get(value, field);
}

function closedMismatches(
  actual: Record<string, unknown>,
  expected: object,
  prefix: string,
): StateDifference[] {
  return [...new Set([...Object.keys(expected), ...Object.keys(actual)])].flatMap((field) => {
    const expectedValue = objectField(expected, field);
    if (Object.hasOwn(expected, field) && actual[field] === expectedValue) return [];
    return [{
      path: `${prefix}.${field}`,
      expected: Object.hasOwn(expected, field) ? expectedValue : "absent",
      received: actual[field],
    }];
  });
}
const validUuid = (value: unknown): value is string => typeof value === "string" && UUID.test(value);

export const PROJECT_OPEN_DIAGNOSTIC_CAUSE = Symbol("storyos.project-open-diagnostic-cause");

export function validateProtocolProfile(profile: unknown): ProtocolBootState {
  const received = isRecord(profile) ? profile : {};
  if (!isRecord(received.release_identity)) {
    return blocked("protocol_identity_missing", [
      { path: "profile.release_identity", expected: "object", received: received.release_identity },
    ]);
  }

  const { release_identity: identity, required_capabilities: capabilities, ...protocol } = received;
  const differences = [
    ...closedMismatches(protocol, expectedProtocol, "profile"),
    ...closedMismatches(identity, expectedIdentity, "profile.release_identity"),
  ];
  if (differences.length) return blocked("protocol_upgrade_required", differences);

  const actualCapabilities = Array.isArray(capabilities) ? capabilities : [];
  const missingCapabilities = expectedCapabilities.filter((item) =>
    !actualCapabilities.includes(item));
  const unexpectedCapabilities = actualCapabilities.filter((item) =>
    !expectedCapabilities.includes(item));
  if (missingCapabilities.length || unexpectedCapabilities.length || actualCapabilities.length !== expectedCapabilities.length) {
    return blocked("protocol_capabilities_incompatible", [{
      path: "profile.required_capabilities",
      expected: expectedCapabilities, received: capabilities,
      missing: missingCapabilities, unexpected: unexpectedCapabilities,
    }]);
  }
  return { kind: "protected-ready", profile: profile as Release1ProtocolProfile };
}

export async function bootProtectedWebClient({
  baseUrl, fetchImpl = globalThis.fetch, signal,
}: {
  baseUrl: string;
  fetchImpl?: typeof fetch;
  signal?: AbortSignal;
}): Promise<ProtocolBootState> {
  try {
    const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
    const profile: unknown = await getProtocolProfile(queryOptions);
    return validateProtocolProfile(profile);
  } catch (error) {
    return blocked("protocol_unavailable", [{
      path: "request.getProtocolProfile", error: error instanceof Error ? error.message : String(error),
    }]);
  }
}

type ProjectOpenOptions = {
  baseUrl: string;
  projectId: string;
  fetchImpl?: typeof fetch;
  signal?: AbortSignal;
  indexedDBImpl?: IDBFactory;
  cryptoImpl?: Crypto;
};

type ReadResult<Value> = { kind: "read"; value: Value } | { kind: "failed"; error: unknown };
const settle = <Value,>(promise: Promise<Value>): Promise<ReadResult<Value>> => promise.then(
  (value) => ({ kind: "read" as const, value }),
  (error: unknown) => ({ kind: "failed" as const, error }),
);

export async function openControlledProject(options: ProjectOpenOptions): Promise<ControlledProjectState> {
  const { baseUrl, projectId, fetchImpl = globalThis.fetch, signal } = options;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  // The protocol profile, the Project, and the list of Projects are independent reads.
  const booted = bootProtectedWebClient(queryOptions);
  const listed = settle(readSessionProject(queryOptions, projectId, "together"));
  const boot = await booted;
  if (boot.kind !== "protected-ready") return boot;
  const read = await listed;
  if (read.kind === "failed") return projectUnavailable(read.error);
  if (read.value === undefined) return projectUnavailable();
  return await openProjectWorkspace(options, boot.profile, read.value.project, read.value.revision);
}

/** The Chapter read and the Editor Session read that a reopen starts with the Project read (#1898). */
type EarlyChapterOpen = {
  chapterId: string;
  chapter: Promise<ReadResult<GetChapterResponse>>;
  opening: EditorWorkspaceOpening;
};

/**
 * Opens the Project again after a settled command of this page. The page keeps the protocol profile and the editor of
 * its last open. The reopen sends four reads at the same time: the Project, the list of Projects, the Chapter
 * `chapterId` (the Current Chapter that the command gave, else the Current Chapter of the page), and the Editor
 * Session. The Project read stays the authority: when it names another Chapter, or an empty Project, the open discards
 * the early Chapter read and the early Editor Session read and reads as the first open does. The open of the
 * workspace ends after the Chapter read of the Chapter of the Project read, and only that open writes to the Journal.
 * `shown` is the text of an editor whose base is earlier than the Server. The open puts it on the current Chapter
 * (ADR 0107, section 2). `chapter` is a read of the Chapter `chapterId` that the reopen uses instead of a new read.
 */
export async function reopenControlledProject(options: Omit<ProjectOpenOptions, "projectId"> & {
  opened: Pick<ProjectReadyState, "profile" | "project">;
  chapterId?: string | undefined;
  shown?: readonly ManuscriptParagraph[] | undefined;
  chapter?: GetChapterResponse | undefined;
}): Promise<ControlledProjectState> {
  const { baseUrl, opened, fetchImpl = globalThis.fetch, signal, indexedDBImpl = globalThis.indexedDB,
    cryptoImpl = globalThis.crypto } = options;
  const scope = opened.project.project_scope;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  const open = opened.project.project.open;
  const chapterId = options.chapterId ?? (open.kind === "current_chapter" ? open.current_chapter_id : undefined);
  // The early open requests no Challenge: when the open discards it, its replacement requests one, and one open
  // requests at most one Challenge (#1899).
  const early = chapterId === undefined ? undefined : { chapterId,
    chapter: settle(options.chapter?.chapter.chapter_id === chapterId ? Promise.resolve(options.chapter)
      : readCurrentChapter(queryOptions, scope.project_id, chapterId, scope.owner_user_id)),
    opening: startEditorWorkspace({ baseUrl, project: opened.project, profile: opened.profile, fetchImpl,
      indexedDBImpl, cryptoImpl, earlyChallenge: false }) };
  try {
    const read = await readSessionProject(queryOptions, scope.project_id, "together");
    if (read === undefined) {
      void early?.opening.abandon();
      return projectUnavailable();
    }
    if (read.project.project_scope.owner_user_id !== scope.owner_user_id) throw new Error("Project Scope mismatch");
    // A tab at the front keeps its Chapter: the reopen with its Chapter read sets that Chapter current again.
    return await openProjectWorkspace({ ...options, projectId: scope.project_id }, opened.profile, read.project,
      read.revision, early, options.shown, options.chapter !== undefined);
  } catch (error) {
    void early?.opening.abandon();
    return projectUnavailable(error);
  }
}

/**
 * Reads the Chapter and the Editor Session of a Project at the same time, or the tree of an empty Project. It uses the
 * early reads of a reopen only when they are of the Current Chapter of `project`.
 */
async function openProjectWorkspace(
  options: ProjectOpenOptions,
  profile: Release1ProtocolProfile,
  project: GetProjectResponse,
  revision: string,
  early?: EarlyChapterOpen,
  shown?: readonly ManuscriptParagraph[],
  keepChapter = false,
): Promise<ControlledProjectState> {
  const { baseUrl, projectId, fetchImpl = globalThis.fetch, signal,
    indexedDBImpl = globalThis.indexedDB, cryptoImpl = globalThis.crypto } = options;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  try {
    const ownerUserId = project.project_scope.owner_user_id;
    const open = project.project.open;
    const used = open.kind === "current_chapter" && early?.chapterId === open.current_chapter_id ? early : undefined;
    // A discarded open ends before its replacement starts: its Chapter read and its Session read end first.
    const discarded = used === undefined && early !== undefined
      ? (await Promise.all([early.chapter, early.opening.abandon()]))[0] : undefined;
    if (open.kind === "empty") {
      const treeValue: unknown = await getManuscriptTree({ ...queryOptions, projectId });
      const treeView = treeValue as {
        project_scope?: { owner_user_id?: unknown; project_id?: unknown };
        snapshot?: { snapshot_id?: unknown; project_scope?: { owner_user_id?: unknown; project_id?: unknown } };
        tree_revision?: unknown;
        volumes?: unknown;
      };
      const treeOwnerUserId = treeView?.project_scope?.owner_user_id;
      if (!validUuid(treeOwnerUserId)
        || treeOwnerUserId !== ownerUserId
        || treeView?.project_scope?.project_id !== projectId
        || !validUuid(treeView?.snapshot?.snapshot_id)
        || treeView?.snapshot?.project_scope?.owner_user_id !== treeOwnerUserId
        || treeView?.snapshot?.project_scope?.project_id !== projectId
        || typeof treeView?.tree_revision !== "string"
        || treeView.tree_revision.length === 0
        || !Array.isArray(treeView?.volumes)) throw new Error("Manuscript tree Scope mismatch");
      const tree = treeValue as GetManuscriptTreeResponse;
      return { kind: "empty-project-ready", profile, project, revision, tree };
    }
    const opening = used?.opening ?? startEditorWorkspace({ baseUrl, project, profile, fetchImpl, indexedDBImpl,
      cryptoImpl });
    const readCurrent = () => settle(readCurrentChapter(queryOptions, projectId, open.current_chapter_id, ownerUserId));
    // A tab that keeps its Chapter reads the Current Chapter only when it does not set its own Chapter current again.
    let reading = used?.chapter ?? (keepChapter ? undefined : readCurrent());
    // Another tab set another Chapter current while this tab had text to send on its Chapter, or while this tab was
    // not at the front. The later action of this tab wins: it sets its Chapter current again and catches up there
    // (ADR 0107, section 2).
    const other = used === undefined ? await opening.otherChapter(open.current_chapter_id, shown, keepChapter)
      : undefined;
    const back = other === undefined ? undefined : await setChapterBack(options, project, other,
      discarded?.kind === "read" ? discarded.value : undefined).catch(() => undefined);
    if (back !== undefined) {
      ({ project, revision } = back.read);
      reading = Promise.resolve({ kind: "read", value: back.chapter });
    }
    const chapter = await (reading ?? readCurrent());
    if (chapter.kind === "failed") {
      void opening.abandon();
      throw chapter.error;
    }
    const opened = await opening.finish(chapter.value, project, shown);
    return { kind: "project-ready", profile, project, revision, chapter: opened.chapter, editor: opened.editor };
  } catch (error) {
    return projectUnavailable(error);
  }
}

/**
 * Sets the Chapter of the stored Editor Session current again, with that Session. `read` is a read of that Chapter,
 * when the reopen has one. It gives the Project and the Chapter after the command, or `undefined` when the command
 * had no effect.
 */
async function setChapterBack(options: ProjectOpenOptions, project: GetProjectResponse,
  other: { chapterId: string; editorSessionId: string }, read: GetChapterResponse | undefined) {
  const { baseUrl, projectId, fetchImpl = globalThis.fetch, signal, cryptoImpl = globalThis.crypto } = options;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  const open = project.project.open;
  if (open.kind !== "current_chapter") return undefined;
  const chapter = read?.chapter.chapter_id === other.chapterId ? read
    : await readCurrentChapter(queryOptions, projectId, other.chapterId, project.project_scope.owner_user_id);
  const switched = await setOwnedCurrentChapter({ baseUrl, fetchImpl, cryptoImpl, projectId,
    chapterId: other.chapterId, expectedCurrentChapterId: open.current_chapter_id,
    expectedTargetRevisionId: chapter.chapter.current_revision.revision_id, editorSessionId: other.editorSessionId });
  if (switched.effect.kind !== "authoritative_applied") return undefined;
  const after = await readSessionProject(queryOptions, projectId, "together");
  const reopened = after?.project.project.open;
  return reopened?.kind === "current_chapter" && reopened.current_chapter_id === other.chapterId
    ? { read: after!, chapter } : undefined;
}

/** Reads the Chapter `chapterId` of the Project, and refuses a read of another Project Scope or Chapter. */
export async function readCurrentChapter(
  queryOptions: StoryOSQueryOptions,
  projectId: string,
  chapterId: string,
  ownerUserId: string,
): Promise<GetChapterResponse> {
  const chapterValue: unknown = await getChapter({ ...queryOptions, projectId, chapterId });
  const chapterView = chapterValue as {
    project_scope?: { owner_user_id?: unknown; project_id?: unknown };
    chapter?: { chapter_id?: unknown };
  };
  const chapterOwnerUserId = chapterView?.project_scope?.owner_user_id;
  if (!validUuid(chapterOwnerUserId)
    || chapterOwnerUserId !== ownerUserId
    || chapterView?.project_scope?.project_id !== projectId
    || chapterView?.chapter?.chapter_id !== chapterId) throw new Error("Chapter Scope mismatch");
  return chapterValue as GetChapterResponse;
}

/** The settings page of a Project in the Scope of the session (ADR 0094 section 9). It opens no Editor Session. */
export async function openProjectSettings(options: {
  baseUrl: string;
  projectId: string;
  fetchImpl?: typeof fetch;
  signal?: AbortSignal;
}): Promise<ControlledProjectState> {
  const { baseUrl, projectId, fetchImpl = globalThis.fetch, signal } = options;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  const boot = await bootProtectedWebClient(queryOptions);
  if (boot.kind !== "protected-ready") return boot;
  try {
    const listed = await readSessionProject(queryOptions, projectId, "in series");
    return listed === undefined
      ? projectUnavailable()
      : { kind: "project-settings-ready", profile: boot.profile, project: listed.project };
  } catch (error) {
    return projectUnavailable(error);
  }
}

/** The Project and its revision in the list when its read has the Scope of the path and the Project list of the session has it. */
async function readSessionProject(
  queryOptions: StoryOSQueryOptions,
  projectId: string,
  order: "together" | "in series",
): Promise<{ project: GetProjectResponse; revision: string } | undefined> {
  const projectRead = (getProject({ ...queryOptions, projectId }) as Promise<unknown>).then((projectValue) => {
    const projectView = projectValue as {
      project_scope?: { owner_user_id?: unknown; project_id?: unknown };
      project?: { project_id?: unknown; open?: { kind?: unknown; current_chapter_id?: unknown } };
    };
    const scope = projectView?.project_scope;
    if (!validUuid(scope?.owner_user_id)
      || scope?.project_id !== projectId
      || projectView?.project?.project_id !== projectId) throw new Error("Project Scope mismatch");
    return projectValue as GetProjectResponse;
  });
  const [project, listed] = order === "together"
    ? await Promise.all([projectRead, listProjects(queryOptions)])
    : [await projectRead, await listProjects(queryOptions)];
  const item = listed.projects.find((entry) => entry.project_scope.project_id === projectId);
  return item === undefined ? undefined : { project, revision: item.revision };
}

function projectUnavailable(error?: unknown): ProjectBlockedState {
  const blockedState: ProjectBlockedState = {
    kind: "project-blocked",
    code: "project_unavailable",
    heading: "StoryOS 无法打开项目",
    message: "无法读取这个受控项目或其当前章节。",
  };
  if (error !== undefined) {
    Object.defineProperty(blockedState, PROJECT_OPEN_DIAGNOSTIC_CAUSE, {
      value: new Error("Controlled Project open failed.", { cause: error }),
    });
  }
  return blockedState;
}
