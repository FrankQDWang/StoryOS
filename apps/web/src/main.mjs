import { bootProtectedWebClient } from "./boot.mjs";

const root = document.querySelector("#app");
const baseUrl = document.documentElement.dataset.storyosServer ?? window.location.origin;
const state = await bootProtectedWebClient({ baseUrl });
const element = (tag, text) => { const node = document.createElement(tag); node.textContent = text; return node; };

root.dataset.bootState = state.kind;
if (state.kind === "protected-ready") {
  root.replaceChildren(element("p", "Release 1 协议已验证。StoryOS 可以进入受保护状态。"));
} else {
  const panel = document.createElement("section");
  panel.setAttribute("role", "alert");
  panel.append(
    element("h1", state.heading),
    element("p", state.message),
    element("pre", JSON.stringify({ code: state.code, details: state.details }, null, 2)),
  );
  root.replaceChildren(panel);
}
