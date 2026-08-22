import { openControlledProject } from "./boot.ts";
import type {
  ControlledProjectState,
  EditorReadyState,
  ProjectBlockedState,
} from "./editor-types.ts";
import { attachManualInput } from "./manual-input.ts";

const PROJECT_PATH = /^\/projects\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\/?$/i;

export interface LoadedStoryOSWebState {
  documentImpl: Document;
  root: HTMLElement;
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}

function render(
  documentImpl: Document,
  root: HTMLElement,
  state: ControlledProjectState,
  { baseUrl, fetchImpl, cryptoImpl }: Pick<
    LoadedStoryOSWebState,
    "baseUrl" | "fetchImpl" | "cryptoImpl"
  >,
): void {
  const element = <K extends keyof HTMLElementTagNameMap>(
    tag: K,
    text: string,
  ): HTMLElementTagNameMap[K] => {
    const node = documentImpl.createElement(tag);
    node.textContent = text;
    return node;
  };

  root.dataset.bootState = state.kind;
  if (state.kind === "project-ready") {
    const project = element("section", "");
    const editor = documentImpl.createElement("textarea");
    const saveState = element("small", "");
    const initialBody = state.editor.kind === "editor-ready"
      ? state.editor.pending.body : state.chapter.chapter.current_revision.body;
    editor.value = initialBody;
    editor.textContent = initialBody;
    editor.readOnly = state.editor.kind !== "editor-ready";
    saveState.textContent = state.editor.kind === "editor-ready"
      ? state.editor.pending.save_state : "needs_attention";
    saveState.dataset.saveState = saveState.textContent;
    if (state.editor.kind === "editor-ready" && typeof editor.addEventListener === "function") {
      attachManualInput({
        editor, workspace: state.editor, baseUrl, fetchImpl, cryptoImpl,
        onProjection(projection) {
          (state.editor as EditorReadyState).pending = projection;
          saveState.textContent = projection.save_state;
          saveState.dataset.saveState = projection.save_state;
        },
        onFailure() {
          editor.readOnly = true;
          saveState.textContent = "needs_attention";
          saveState.dataset.saveState = "needs_attention";
        },
      });
    }
    project.append(
      element("h1", state.project.project.title),
      element("h2", state.chapter.chapter.title),
      editor,
      saveState,
      element("small", `权威修订 ${state.chapter.chapter.current_revision.revision_id}`),
    );
    root.replaceChildren(project);
  } else {
    const panel = documentImpl.createElement("section");
    panel.setAttribute("role", "alert");
    panel.append(
      element("h1", state.heading),
      element("p", state.message),
      element("pre", JSON.stringify({ code: state.code, details: state.details }, null, 2)),
    );
    root.replaceChildren(panel);
  }
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
  return { documentImpl, root, state, baseUrl, fetchImpl, cryptoImpl };
}

export async function runStoryOSWeb(
  options: Parameters<typeof loadStoryOSWebState>[0] = {},
): Promise<ControlledProjectState> {
  const loaded = await loadStoryOSWebState(options);
  render(loaded.documentImpl, loaded.root, loaded.state, loaded);
  return loaded.state;
}
