import { openControlledProject } from "./boot.mjs";
import { persistReplaceSelection } from "./editor-session.mjs";

const PROJECT_PATH = /^\/projects\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\/?$/i;

function render(documentImpl, root, state) {
  const element = (tag, text) => {
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
      let durableBody = initialBody;
      let pendingWrite = Promise.resolve();
      editor.addEventListener("input", () => {
        const nextBody = editor.value;
        pendingWrite = pendingWrite.then(async () => {
          let from = 0;
          while (from < durableBody.length && from < nextBody.length
            && durableBody[from] === nextBody[from]) from += 1;
          let oldEnd = durableBody.length;
          let newEnd = nextBody.length;
          while (oldEnd > from && newEnd > from
            && durableBody[oldEnd - 1] === nextBody[newEnd - 1]) {
            oldEnd -= 1; newEnd -= 1;
          }
          const pending = await persistReplaceSelection(state.editor, {
            from, to: oldEnd, text: nextBody.slice(from, newEnd), resultingBody: nextBody,
          });
          durableBody = pending.body;
          state.editor.pending = pending;
          saveState.textContent = pending.save_state;
          saveState.dataset.saveState = pending.save_state;
        }).catch(() => {
          editor.readOnly = true;
          saveState.textContent = "needs_attention";
          saveState.dataset.saveState = "needs_attention";
        });
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

export async function runStoryOSWeb({
  documentImpl = globalThis.document,
  locationImpl = globalThis.location,
  fetchImpl = globalThis.fetch,
  indexedDBImpl = globalThis.indexedDB,
  cryptoImpl = globalThis.crypto,
} = {}) {
  const root = documentImpl.querySelector("#app");
  if (!root) throw new TypeError("StoryOS Web cannot start because the required #app root is missing.");
  const baseUrl = documentImpl.documentElement.dataset.storyosServer ?? locationImpl.origin;
  const projectId = locationImpl.pathname.match(PROJECT_PATH)?.[1];
  const state = projectId
    ? await openControlledProject({ baseUrl, projectId, fetchImpl, indexedDBImpl, cryptoImpl })
    : {
        kind: "project-blocked",
        code: "project_url_invalid",
        heading: "StoryOS 无法打开项目",
        message: "项目地址缺少有效的受控项目身份。",
      };
  render(documentImpl, root, state);
  return state;
}
