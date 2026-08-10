import { openControlledProject } from "./boot.mjs";

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
    project.append(
      element("h1", state.project.project.title),
      element("h2", state.chapter.chapter.title),
      element("p", state.chapter.chapter.current_revision.body),
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
} = {}) {
  const root = documentImpl.querySelector("#app");
  if (!root) throw new TypeError("StoryOS Web cannot start because the required #app root is missing.");
  const baseUrl = documentImpl.documentElement.dataset.storyosServer ?? locationImpl.origin;
  const projectId = locationImpl.pathname.match(PROJECT_PATH)?.[1];
  const state = projectId
    ? await openControlledProject({ baseUrl, projectId, fetchImpl })
    : {
        kind: "project-blocked",
        code: "project_url_invalid",
        heading: "StoryOS 无法打开项目",
        message: "项目地址缺少有效的受控项目身份。",
      };
  render(documentImpl, root, state);
  return state;
}
