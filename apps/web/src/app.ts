import { openControlledProject } from "./boot.ts";
import type {
  ControlledProjectState,
  ProjectBlockedState,
} from "./editor-types.ts";

const PROJECT_PATH = /^\/projects\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\/?$/i;

export interface LoadedStoryOSWebState {
  root: HTMLElement;
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}

export async function loadStoryOSWebState({
  documentImpl = globalThis.document,
  locationImpl = globalThis.location,
  fetchImpl = globalThis.fetch,
  indexedDBImpl = globalThis.indexedDB,
  cryptoImpl = globalThis.crypto,
}: {
  documentImpl?: Document;
  locationImpl?: Pick<Location, "origin" | "pathname">;
  fetchImpl?: typeof fetch;
  indexedDBImpl?: IDBFactory;
  cryptoImpl?: Crypto;
} = {}): Promise<LoadedStoryOSWebState> {
  const root = documentImpl.querySelector<HTMLElement>("#app");
  if (!root) throw new TypeError("StoryOS Web cannot start because the required #app root is missing.");
  const baseUrl = documentImpl.documentElement.dataset.storyosServer ?? locationImpl.origin;
  const projectId = locationImpl.pathname.match(PROJECT_PATH)?.[1];
  const invalidUrlState: ProjectBlockedState = {
    kind: "project-blocked",
    code: "project_url_invalid",
    heading: "StoryOS 无法打开项目",
    message: "项目地址缺少有效的受控项目身份。",
  };
  const state: ControlledProjectState = projectId
    ? await openControlledProject({ baseUrl, projectId, fetchImpl, indexedDBImpl, cryptoImpl })
    : invalidUrlState;
  return { root, state, baseUrl, fetchImpl, cryptoImpl };
}
