// One author-facing Transcript slice around the existing host/recovery harness.
// The shell follows the approved StoryOS workspace; the Transcript hierarchy
// follows the low-chrome conversational pattern used by Codex.
export function transcriptPage({ sandboxOrigin }) {
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>第十二章 · StoryOS</title>
  <style>
    :root {
      color: #26241f;
      background: #fbfaf8;
      font-family: "PingFang SC", "Noto Sans SC", system-ui, sans-serif;
      font-synthesis: none;
      --paper: #fbfaf8;
      --side: #f8f7f4;
      --agent: #faf9f7;
      --soft: #f1efec;
      --proposal: #f3f1ee;
      --line: rgba(62, 59, 53, .11);
      --ink: #292722;
      --muted: #77736b;
      --faint: #aaa69e;
    }
    * { box-sizing: border-box; }
    html, body { width: 100%; height: 100%; }
    body { margin: 0; overflow: hidden; background: var(--paper); }
    button, textarea { font: inherit; }
    button { color: inherit; }
    button:focus-visible, textarea:focus-visible, [tabindex]:focus-visible {
      outline: 2px solid #77736c;
      outline-offset: 3px;
    }
    .workspace {
      width: 100vw;
      height: 100dvh;
      display: grid;
      grid-template-columns: 222px minmax(500px, 1fr) clamp(404px, 33vw, 456px);
      background: var(--paper);
    }
    .manuscript-tree {
      min-width: 0;
      overflow: hidden;
      border-right: 1px solid var(--line);
      background: var(--side);
      color: #56524a;
    }
    .project-name {
      height: 58px;
      display: flex;
      align-items: center;
      padding: 0 22px;
      color: #403d37;
      font: 600 14px/1.2 "Noto Serif SC", "Songti SC", serif;
    }
    .tree-heading {
      margin: 29px 22px 17px;
      color: #817d75;
      font-size: 12px;
    }
    .tree-list { margin: 0; padding: 0 10px; list-style: none; }
    .tree-list li { padding: 7px 12px 7px 24px; font: 13px/1.45 "Noto Serif SC", "Songti SC", serif; }
    .tree-list .volume { padding-left: 12px; color: #3f3c36; font-weight: 600; }
    .tree-list .active {
      border-radius: 7px;
      background: #eceae6;
      box-shadow: inset 2px 0 #4d4942;
      color: #26241f;
      font-weight: 600;
    }
    .editor {
      min-width: 0;
      overflow-y: auto;
      padding: 76px clamp(48px, 7vw, 104px) 96px;
      scrollbar-width: thin;
    }
    .editor article { width: min(690px, 100%); margin: 0 auto; }
    .editor h1 {
      margin: 0 0 42px;
      color: #282620;
      font: 600 31px/1.35 "Noto Serif SC", "Songti SC", serif;
      letter-spacing: .04em;
    }
    .editor p {
      margin: 0 0 23px;
      color: #4b4841;
      font: 16px/2.08 "Noto Serif SC", "Songti SC", serif;
    }
    .editor-proposal {
      margin: 26px 0 30px;
      padding: 18px 21px;
      background: var(--proposal);
      box-shadow: inset 2px 0 #706b63;
    }
    .editor-proposal[hidden] { display: none; }
    .editor-proposal small {
      display: block;
      margin-bottom: 9px;
      color: #858078;
      font-size: 11px;
    }
    .editor-proposal p { margin: 0; color: #37342e; }
    .agent-panel {
      min-width: 0;
      height: 100dvh;
      display: grid;
      grid-template-rows: 58px minmax(0, 1fr) auto;
      overflow: hidden;
      border-left: 1px solid var(--line);
      background: var(--agent);
    }
    .agent-header { display: flex; align-items: center; padding: 0 30px; }
    .agent-header strong { font: 600 17px/1 "Noto Serif SC", "Songti SC", serif; }
    .transcript {
      min-height: 0;
      overflow-y: auto;
      padding: 20px 30px 26px;
      scrollbar-width: thin;
    }
    .author-message {
      width: min(100%, 330px);
      margin: 0 0 38px auto;
      padding: 14px 16px;
      border-radius: 16px;
      background: var(--soft);
      color: #3d3a34;
    }
    .author-message p,
    .agent-turn > p {
      margin: 0;
      font: 14px/1.8 "Noto Serif SC", "Songti SC", serif;
    }
    .agent-turn { color: #3b3832; }
    .activity {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: 16px;
      margin: 18px 0 13px;
      color: #858078;
      font-size: 12px;
    }
    .activity span:last-child { color: #7d7870; }
    .app-shell {
      margin: 0 0 25px;
      overflow: hidden;
      border-radius: 16px;
      background: var(--soft);
    }
    .app-shell iframe {
      display: block;
      width: 100%;
      height: 224px;
      border: 0;
      background: var(--soft);
    }
    .fallback { padding: 19px 20px 20px; }
    .fallback h2 { margin: 0 0 9px; font-size: 15px; font-weight: 600; }
    .fallback p { margin: 0 0 7px; color: #5f5b54; font-size: 13px; line-height: 1.7; }
    .fallback small { color: #8f8a82; font-size: 11px; }
    .approval { margin: 0 0 30px; }
    .approval-kicker { display: block; margin-bottom: 8px; color: #8a857d; font-size: 11px; }
    .approval strong { display: block; margin-bottom: 7px; font-size: 15px; font-weight: 600; }
    .approval p { margin: 0; color: #68645d; font-size: 12px; line-height: 1.75; }
    .approval-actions { display: flex; align-items: center; gap: 8px; margin-top: 15px; }
    .approval-actions button {
      min-height: 34px;
      padding: 7px 12px;
      border: 0;
      border-radius: 10px;
      background: transparent;
      cursor: pointer;
      font-size: 12px;
    }
    .approval-actions button:hover { background: #efede9; }
    .approval-actions button.primary { background: #34312c; color: #fff; }
    .approval-actions button.primary:hover { background: #24221e; }
    .boundary-note {
      margin: 0 0 30px;
      color: #67635c;
      font-size: 13px;
      line-height: 1.75;
    }
    .boundary-note strong { display: block; margin-bottom: 3px; color: #35322d; font-weight: 600; }
    .text-action {
      margin: 9px 0 0;
      padding: 0;
      border: 0;
      background: transparent;
      color: #4e4a43;
      cursor: pointer;
      font-size: 12px;
      text-decoration: underline;
      text-underline-offset: 3px;
    }
    .composer-dock { padding: 0 20px 16px; background: var(--agent); }
    .composer {
      overflow: hidden;
      border: 1px solid rgba(71, 67, 60, .17);
      border-radius: 15px;
      background: rgba(255, 255, 255, .72);
      box-shadow: 0 8px 24px rgba(44, 41, 36, .06);
    }
    .composer textarea {
      width: 100%;
      min-height: 54px;
      resize: none;
      border: 0;
      background: transparent;
      padding: 12px 14px 6px;
      outline: none;
      color: #77736c;
      font-size: 13px;
      line-height: 1.6;
    }
    .composer textarea::placeholder { color: #817d75; }
    .composer-footer { display: flex; justify-content: space-between; padding: 1px 10px 9px; }
    .composer-footer button {
      padding: 2px 4px;
      border: 0;
      background: transparent;
      color: #817d75;
      font-size: 11px;
    }
    @media (max-width: 1050px) {
      .workspace { grid-template-columns: minmax(500px, 1fr) minmax(390px, 430px); }
      .manuscript-tree { display: none; }
    }
    @media (max-width: 780px) {
      .workspace { grid-template-columns: 1fr; }
      .editor { display: none; }
      .agent-panel { border-left: 0; }
      .transcript { padding-right: 24px; padding-left: 24px; }
      .text-action { display: none; }
    }
  </style>
</head>
<body>
  <main class="workspace">
    <nav class="manuscript-tree" aria-label="稿件目录">
      <div class="project-name">雾尽时与月同归</div>
      <div class="tree-heading">目录</div>
      <ul class="tree-list">
        <li class="volume">卷二　风起</li>
        <li>第九章　迷雾</li>
        <li>第十章　亡局</li>
        <li>第十一章　暗涌</li>
        <li class="active" aria-current="page">第十二章　雨夜</li>
        <li>第十三章　归期</li>
        <li>第十四章　破晓</li>
      </ul>
    </nav>

    <section class="editor" aria-label="正文编辑器">
      <article>
        <h1>第十二章　雨夜</h1>
        <p>雨下得很大，城外的灯火被水汽揉成一片模糊的光。</p>
        <p>苏砚站在廊下，望着巷口渐深的夜色，迟迟没有迈步。</p>
        <p>旧仓在城西，早年是官府的粮仓，如今半废，杂草丛生。</p>
        <section class="editor-proposal" id="editor-proposal" tabindex="-1" hidden>
          <small>待审阅提案 · 尚未写入正文</small>
          <p id="editor-proposal-text"></p>
        </section>
        <p>风从城墙的裂缝里钻出来，带着潮湿的土腥味。</p>
      </article>
    </section>

    <aside class="agent-panel" aria-label="Agent 对话">
      <header class="agent-header"><strong>Agent</strong></header>
      <div class="transcript" id="message-list" aria-live="polite">
        <article class="author-message">
          <p>看看项目里有哪些资料可以用于这一章。</p>
        </article>
        <section class="agent-turn">
          <p>我找到了当前正文、人物卡和研究资料，可以直接用于这一章。</p>
          <div class="activity" aria-label="工具活动">
            <span>读取项目资料</span>
            <span>3 个来源</span>
          </div>
          <div class="app-shell" id="app-slot" aria-label="资料库 App"></div>
          <div id="host-flow"></div>
        </section>
      </div>
      <div class="composer-dock">
        <div class="composer">
          <textarea aria-label="给 Agent 的消息" placeholder="继续对话…" disabled></textarea>
          <div class="composer-footer">
            <button type="button" disabled>添加上下文</button>
            <button type="button" disabled>发送</button>
          </div>
        </div>
      </div>
    </aside>
  </main>
  <script>
    const SANDBOX_ORIGIN = ${JSON.stringify(sandboxOrigin)};
    const appSlot = document.querySelector("#app-slot");
    const hostFlow = document.querySelector("#host-flow");
    const messageList = document.querySelector("#message-list");
    const editorProposal = document.querySelector("#editor-proposal");
    const editorProposalText = document.querySelector("#editor-proposal-text");
    let snapshot = null;
    let sandboxFrame = null;
    let rejectedMessages = 0;

    async function refresh() {
      snapshot = await fetch("/api/snapshot").then((response) => response.json());
      renderApp(snapshot.durable.appViews[0] ?? null);
      renderHostFlow(snapshot.durable);
      renderEditorState(snapshot.durable);
    }

    function renderApp(view) {
      appSlot.replaceChildren();
      sandboxFrame = null;
      if (!view) {
        const fallback = document.createElement("div");
        fallback.className = "fallback";
        fallback.textContent = "资料正在准备中…";
        appSlot.append(fallback);
        return;
      }
      const projected = snapshot.projection.views.find((candidate) => candidate.id === view.id);
      if (projected.renderMode === "static_fallback") {
        const fallback = document.createElement("div");
        fallback.className = "fallback";
        const title = document.createElement("h2");
        title.textContent = "可用资料";
        const text = document.createElement("p");
        text.textContent = view.staticFallback.text;
        const note = document.createElement("small");
        note.textContent = "互动视图暂不可用，已显示 StoryOS 保存的结果。";
        fallback.append(title, text, note);
        appSlot.append(fallback);
        return;
      }
      sandboxFrame = document.createElement("iframe");
      sandboxFrame.title = "资料库";
      sandboxFrame.setAttribute("sandbox", "allow-scripts allow-same-origin");
      sandboxFrame.src = SANDBOX_ORIGIN + "/proxy.html";
      appSlot.append(sandboxFrame);
    }

    function renderHostFlow(state) {
      hostFlow.replaceChildren();
      const unresolved = state.waits.find((wait) => wait.status === "unresolved");
      if (unresolved) {
        const approval = document.createElement("section");
        approval.className = "approval";
        approval.innerHTML = "<span class='approval-kicker'>StoryOS 需要确认</span><strong>允许资料库创建正文提案？</strong><p>允许后只会把提案放进编辑器，正文不会自动改变。</p><div class='approval-actions'><button type='button' data-action='reject'>拒绝</button><button type='button' class='primary' data-action='approve'>允许</button></div>";
        approval.dataset.waitId = unresolved.id;
        hostFlow.append(approval);
        return;
      }

      const latestProposal = state.proposals.at(-1);
      const rejectedResolution = state.waitResolutions.find((resolution) => resolution.decision === "rejected_by_author");
      if (latestProposal?.status === "awaiting_author_acceptance") {
        const note = document.createElement("div");
        note.className = "boundary-note";
        note.innerHTML = "<strong>提案已放进编辑器</strong>它仍未写入正文，需要你在正文中继续审阅。<br><button class='text-action' type='button' data-action='view-proposal'>查看提案</button>";
        hostFlow.append(note);
      } else if (rejectedResolution) {
        const note = document.createElement("div");
        note.className = "boundary-note";
        note.innerHTML = "<strong>已取消</strong>没有生成提案，正文没有变化。";
        hostFlow.append(note);
      }
    }

    function renderEditorState(state) {
      const proposal = state.proposals.find((candidate) => candidate.status === "awaiting_author_acceptance");
      editorProposal.hidden = !proposal;
      editorProposalText.textContent = proposal?.requestedEdit ?? "";
    }

    hostFlow.addEventListener("click", async (event) => {
      const button = event.target.closest("button[data-action]");
      if (!button) return;
      if (button.dataset.action === "view-proposal") {
        editorProposal.scrollIntoView({ behavior: "smooth", block: "center" });
        editorProposal.focus({ preventScroll: true });
        return;
      }
      const approval = button.closest(".approval");
      const path = button.dataset.action === "approve" ? "/api/approve" : "/api/reject";
      button.disabled = true;
      await fetch(path, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ waitId: approval.dataset.waitId })
      });
      await refresh();
      messageList.scrollTop = messageList.scrollHeight;
    });

    function sendToSandbox(message) {
      sandboxFrame.contentWindow.postMessage(message, SANDBOX_ORIGIN);
    }

    window.addEventListener("message", async (event) => {
      const validSource = sandboxFrame && event.source === sandboxFrame.contentWindow;
      const validOrigin = event.origin === SANDBOX_ORIGIN;
      if (!validSource || !validOrigin) {
        rejectedMessages += 1;
        await fetch("/api/audit", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ requestId: "client-source-denial-" + rejectedMessages, reason: "source_or_origin_mismatch" })
        });
        return;
      }

      const message = event.data;
      if (message?.type === "sandbox-proxy-ready") {
        const view = snapshot.durable.appViews[0];
        sendToSandbox({ type: "sandbox-resource", resourceHtml: view.resource.html, resourceDigest: view.resource.digest });
        return;
      }
      if (message?.type === "sandbox-resource-ready") {
        sendToSandbox({ type: "mcp-jsonrpc", payload: {
          jsonrpc: "2.0",
          id: "host-init-1",
          method: "ui/initialize",
          params: {
            protocolVersion: "2026-01-26",
            hostCapabilities: { tools: { call: true } },
            hostContext: snapshot.durable.appViews[0].hostContext
          }
        }});
        return;
      }
      if (message?.type !== "mcp-jsonrpc") return;
      const rpc = message.payload;
      if (!rpc || rpc.jsonrpc !== "2.0" || (rpc.id === undefined && !rpc.method)) {
        await fetch("/api/audit", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ requestId: rpc?.id ?? "malformed-browser-unknown", reason: "malformed_jsonrpc" })
        });
        return;
      }
      if (rpc.id === "host-init-1" && rpc.result) {
        sendToSandbox({ type: "mcp-jsonrpc", payload: { jsonrpc: "2.0", method: "ui/notifications/initialized", params: {} }});
        sendToSandbox({ type: "mcp-jsonrpc", payload: { jsonrpc: "2.0", method: "ui/notifications/tool-input", params: { revision: snapshot.durable.appViews[0].inputRevision } }});
        sendToSandbox({ type: "mcp-jsonrpc", payload: {
          jsonrpc: "2.0",
          method: "ui/notifications/tool-result",
          params: {
            revision: snapshot.durable.appViews[0].resultRevision,
            structuredContent: snapshot.durable.appViews[0].structuredResult
          }
        }});
        return;
      }
      if (rpc.method) {
        const response = await fetch("/api/bridge", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            appInstanceId: snapshot.durable.appViews[0].appInstanceId,
            viewId: snapshot.durable.appViews[0].id,
            rpc
          })
        }).then((result) => result.json());
        sendToSandbox({ type: "mcp-jsonrpc", payload: response });
        await refresh();
        messageList.scrollTop = messageList.scrollHeight;
      }
    });

    refresh();
  </script>
</body>
</html>`;
}
