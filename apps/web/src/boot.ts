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
import { startEditorWorkspace } from "./editor-session.ts";

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

/** The Project of a command response: the Server settled a command of this session for this Project. */
export type SettledProject = Pick<GetProjectResponse, "project_scope" | "project">;

export async function openControlledProject(options: ProjectOpenOptions): Promise<ControlledProjectState> {
  const { baseUrl, projectId, fetchImpl = globalThis.fetch, signal } = options;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  const [boot, listed] = await Promise.all([
    bootProtectedWebClient(queryOptions),
    readSessionProject(queryOptions, projectId).then(
      (value) => ({ kind: "read" as const, value }),
      (error: unknown) => ({ kind: "failed" as const, error }),
    ),
  ]);
  if (boot.kind !== "protected-ready") return boot;
  if (listed.kind === "failed") return projectUnavailable(listed.error);
  if (listed.value === undefined) return projectUnavailable();
  return openProjectWorkspace(options, boot.profile, listed.value.project, listed.value.revision);
}

/**
 * Opens the Project again after a settled command of this page. The page holds the protocol profile and the
 * revision of the list, so only the Project (when the command response does not carry it), the Chapter, and the
 * Editor Session are read.
 */
export async function reopenControlledProject(options: Omit<ProjectOpenOptions, "projectId"> & {
  opened: Pick<ProjectReadyState, "profile" | "project" | "revision">;
  settled?: SettledProject | undefined;
}): Promise<ControlledProjectState> {
  const { baseUrl, opened, settled, fetchImpl = globalThis.fetch, signal } = options;
  const scope = opened.project.project_scope;
  const projectId = scope.project_id;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  try {
    const projectValue: unknown = settled ?? await getProject({ ...queryOptions, projectId });
    const projectView = projectValue as {
      project_scope?: { owner_user_id?: unknown; project_id?: unknown };
      project?: { project_id?: unknown };
    };
    if (projectView?.project_scope?.owner_user_id !== scope.owner_user_id
      || projectView?.project_scope?.project_id !== projectId
      || projectView?.project?.project_id !== projectId) throw new Error("Project Scope mismatch");
    const project: GetProjectResponse = settled === undefined ? projectValue as GetProjectResponse
      : { ...opened.project, project_scope: settled.project_scope, project: settled.project };
    return await openProjectWorkspace({ ...options, projectId }, opened.profile, project, opened.revision);
  } catch (error) {
    return projectUnavailable(error);
  }
}

/** Reads the Chapter and the Editor Session of a Project at the same time, or the tree of an empty Project. */
async function openProjectWorkspace(
  options: ProjectOpenOptions,
  profile: Release1ProtocolProfile,
  project: GetProjectResponse,
  revision: string,
): Promise<ControlledProjectState> {
  const { baseUrl, projectId, fetchImpl = globalThis.fetch, signal,
    indexedDBImpl = globalThis.indexedDB, cryptoImpl = globalThis.crypto } = options;
  const queryOptions = { baseUrl, fetchImpl, signal } as StoryOSQueryOptions;
  try {
    const ownerUserId = project.project_scope.owner_user_id;
    if (project.project.open.kind === "empty") {
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
    const chapterId = project.project.open.current_chapter_id;
    const opening = startEditorWorkspace({ baseUrl, project, profile, fetchImpl, indexedDBImpl, cryptoImpl });
    let chapter: GetChapterResponse;
    try {
      chapter = await readCurrentChapter(queryOptions, projectId, chapterId, ownerUserId);
    } catch (error) {
      await opening.abandon();
      throw error;
    }
    const { editor, tree } = await opening.finish(chapter);
    return { kind: "project-ready", profile, project, revision, chapter, editor, tree };
  } catch (error) {
    return projectUnavailable(error);
  }
}

async function readCurrentChapter(
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
  const [boot, listed] = await Promise.all([
    bootProtectedWebClient(queryOptions),
    readSessionProject(queryOptions, projectId).then(
      (value) => ({ kind: "read" as const, value }),
      (error: unknown) => ({ kind: "failed" as const, error }),
    ),
  ]);
  if (boot.kind !== "protected-ready") return boot;
  if (listed.kind === "failed") return projectUnavailable(listed.error);
  return listed.value === undefined
    ? projectUnavailable()
    : { kind: "project-settings-ready", profile: boot.profile, project: listed.value.project };
}

/**
 * The Project and its revision in the list when its read has the Scope of the path and the Project list of the
 * session has it. The two reads are in flight at the same time.
 */
async function readSessionProject(
  queryOptions: StoryOSQueryOptions,
  projectId: string,
): Promise<{ project: GetProjectResponse; revision: string } | undefined> {
  const [projectValue, listed] = await Promise.all([
    getProject({ ...queryOptions, projectId }) as Promise<unknown>,
    listProjects(queryOptions),
  ]);
  const projectView = projectValue as {
    project_scope?: { owner_user_id?: unknown; project_id?: unknown };
    project?: { project_id?: unknown; open?: { kind?: unknown; current_chapter_id?: unknown } };
  };
  const scope = projectView?.project_scope;
  if (!validUuid(scope?.owner_user_id)
    || scope?.project_id !== projectId
    || projectView?.project?.project_id !== projectId) throw new Error("Project Scope mismatch");
  const item = listed.projects.find((entry) => entry.project_scope.project_id === projectId);
  return item === undefined ? undefined : { project: projectValue as GetProjectResponse, revision: item.revision };
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
