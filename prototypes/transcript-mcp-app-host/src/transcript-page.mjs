// One author-facing Transcript slice around the existing host/recovery harness.
// The visual direction is already fixed; this is not a multi-variant UI exploration.
export function transcriptPage({ sandboxOrigin }) {
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>写作助手 · StoryOS</title>
  <style>
    :root {
      color: #24231f;
      background: #fbfbfa;
      font-family: "Noto Sans SC", "PingFang SC", system-ui, sans-serif;
      font-synthesis: none;
      --canvas: #fbfbfa;
      --assistant-panel: #f9f8f7;
      --author-surface: #f5f3f2;
      --activity-surface: #f6f5f4;
      --line: #ece9e5;
      --ink: #292722;
      --muted: #7f7b73;
      --faint: #a5a199;
    }
    * { box-sizing: border-box; }
    html, body { width: 100%; height: 100%; }
    body { margin: 0; overflow: hidden; background: var(--canvas); }
    button, textarea { font: inherit; }
    button { color: inherit; }
    button:focus-visible, textarea:focus-visible { outline: 2px solid #78756e; outline-offset: 2px; }
    .workspace-edge { width: 100vw; height: 100dvh; display: grid; grid-template-columns: minmax(0, 1fr) clamp(390px, 31vw, 452px); }
    .manuscript-context { min-width: 0; padding: 78px clamp(44px, 8vw, 130px); overflow: hidden; color: #3d3a34; }
    .manuscript-context article { width: min(680px, 100%); margin: 0 auto; opacity: .72; }
    .manuscript-context h1 { margin: 0 0 42px; font: 600 30px/1.3 "Noto Serif SC", "Songti SC", serif; letter-spacing: .04em; }
    .manuscript-context p { margin: 0 0 25px; font: 16px/2.15 "Noto Serif SC", "Songti SC", serif; }
    .assistant-panel { min-width: 0; height: 100dvh; display: grid; grid-template-rows: 40px minmax(0, 1fr) auto; border-left: 1px solid var(--line); background: var(--assistant-panel); }
    .assistant-header { position: relative; z-index: 2; display: flex; align-items: center; padding: 0 28px; border-bottom: 1px solid rgba(74,71,66,.1); background: var(--assistant-panel); }
    .assistant-header::after { content: ""; position: absolute; top: 100%; right: 0; left: 0; height: 18px; background: linear-gradient(to bottom, var(--assistant-panel), rgba(249,248,247,0)); pointer-events: none; }
    .assistant-header strong { font: 600 17px/1 "Noto Serif SC", "Songti SC", serif; }
    .message-list { min-height: 0; overflow-y: auto; padding: 25px 27px 42px; scrollbar-width: thin; }
    .message { width: min(100%, 340px); margin-bottom: 28px; color: #3c3a35; }
    .message p { margin: 0; font: 14px/1.85 "Noto Serif SC", "Songti SC", serif; }
    .message time { display: block; margin-top: 8px; color: #97938b; font-size: 11px; }
    .message.author { margin-left: auto; padding: 16px 17px 10px; border-radius: 13px; background: var(--author-surface); }
    .message.author time { text-align: right; }
    .message.agent { margin-right: auto; }
    .activity-row { display: grid; grid-template-columns: 19px 1fr auto; gap: 10px; align-items: center; margin: 4px 0 14px; padding: 12px 13px; border-radius: 10px; background: var(--activity-surface); color: #4b4842; }
    .activity-icon { display: grid; place-items: center; width: 19px; height: 19px; color: #6f6b63; }
    .activity-row strong { display: block; font-size: 12px; font-weight: 500; }
    .activity-row span { color: #908c84; font-size: 11px; }
    .app-shell { margin: 0 0 26px; overflow: hidden; border: 1px solid #dfdcd7; border-radius: 13px; background: #f6f5f3; }
    .app-shell iframe { display: block; width: 100%; height: 266px; border: 0; background: #f6f5f3; }
    .fallback { padding: 17px; }
    .fallback h2 { margin: 0 0 8px; font-size: 14px; }
    .fallback p { margin: 0 0 8px; color: #65625b; font-size: 13px; line-height: 1.7; }
    .fallback small { color: #938f87; }
    .approval { margin: 0 0 28px; padding: 15px; border: 1px solid #dcd8d2; border-radius: 12px; background: rgba(255,255,255,.72); }
    .approval-kicker { display: block; margin-bottom: 7px; color: #77736b; font-size: 11px; }
    .approval strong { display: block; margin-bottom: 6px; font-size: 13px; font-weight: 600; }
    .approval p { margin: 0; color: #68645d; font-size: 12px; line-height: 1.7; }
    .approval-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 13px; }
    .approval-actions button { padding: 7px 11px; border: 1px solid #d6d2cc; border-radius: 8px; background: transparent; cursor: pointer; font-size: 12px; }
    .approval-actions button.primary { border-color: #3e3b36; background: #3e3b36; color: white; }
    .approval-actions button:hover { filter: brightness(.96); }
    .boundary-note { margin: -10px 0 28px; padding-left: 12px; border-left: 2px solid #c7c2ba; color: #69665f; font-size: 12px; line-height: 1.75; }
    .boundary-note strong { display: block; color: #45423c; font-weight: 500; }
    .composer-dock { position: relative; z-index: 1; padding: 0 20px 14px; background: var(--assistant-panel); }
    .composer-dock::before { content: ""; position: absolute; right: 0; bottom: 100%; left: 0; height: 24px; background: linear-gradient(to bottom, rgba(249,248,247,0), var(--assistant-panel)); pointer-events: none; }
    .composer { border: 1px solid #dfdcd7; border-radius: 14px; background: rgba(255,255,255,.72); box-shadow: 0 10px 30px rgba(54,51,46,.07), 0 1px 2px rgba(54,51,46,.04); overflow: hidden; }
    .composer textarea { width: 100%; min-height: 54px; resize: none; border: 0; background: transparent; padding: 11px 14px 5px; outline: none; color: #77736c; font-size: 13px; line-height: 1.6; }
    .composer textarea::placeholder { color: #aaa7a0; }
    .composer-footer { display: flex; justify-content: space-between; padding: 2px 8px 8px; }
    .circle { display: grid; place-items: center; width: 28px; height: 28px; padding: 0; border: 1px solid #b5b2ac; border-radius: 50%; background: transparent; color: #6e6b65; }
    .circle.send { border-color: #aaa7a0; background: #aaa7a0; color: white; opacity: .5; }
    @media (max-width: 760px) {
      .workspace-edge { grid-template-columns: 1fr; }
      .manuscript-context { display: none; }
      .assistant-panel { border-left: 0; }
    }
  </style>
</head>
<body>
  <main class="workspace-edge">
    <section class="manuscript-context" aria-label="正文上下文">
      <article>
        <h1>第十二章　雨夜</h1>
        <p>雨下得很大，城外的灯火被水汽揉成一片模糊的光。</p>
        <p>苏砚站在廊下，望着巷口渐深的夜色，迟迟没有迈步。</p>
        <p>旧仓在城西，早年是官府的粮仓，如今半废，杂草丛生。</p>
      </article>
    </section>
    <aside class="assistant-panel" aria-label="写作助手对话">
      <header class="assistant-header"><strong>写作助手</strong></header>
      <div class="message-list" id="message-list" aria-live="polite">
        <article class="message author">
          <p>看看项目里有哪些资料可以用于这一章。</p>
          <time>刚刚</time>
        </article>
        <article class="message agent">
          <p>我查看了当前项目，可以直接使用正文、人物卡和研究资料。</p>
        </article>
        <div class="activity-row" aria-label="工具活动">
          <span class="activity-icon">✦</span>
          <div><strong>读取项目资料</strong><span>3 个资料源</span></div>
          <span>完成</span>
        </div>
        <div class="app-shell" id="app-slot" aria-label="资料库 App"></div>
        <div id="host-flow"></div>
      </div>
      <div class="composer-dock">
        <div class="composer">
          <textarea aria-label="给写作助手的消息" placeholder="告诉写作助手你想做什么…" disabled></textarea>
          <div class="composer-footer">
            <button class="circle" type="button" aria-label="添加上下文" disabled>＋</button>
            <button class="circle send" type="button" aria-label="发送" disabled>↑</button>
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
    let snapshot = null;
    let sandboxFrame = null;
    let rejectedMessages = 0;

    async function refresh() {
      snapshot = await fetch("/api/snapshot").then((response) => response.json());
      renderApp(snapshot.durable.appViews[0] ?? null);
      renderHostFlow(snapshot.durable);
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
        const card = document.createElement("section");
        card.className = "approval";
        card.innerHTML = "<span class='approval-kicker'>需要你的批准</span><strong>生成一份正文修改提案</strong><p>资料库希望基于刚才读取的资料提出修改。批准只会创建可审阅提案，不会改写正文。</p><div class='approval-actions'><button type='button' data-action='reject'>拒绝</button><button type='button' class='primary' data-action='approve'>允许生成提案</button></div>";
        card.dataset.waitId = unresolved.id;
        hostFlow.append(card);
        return;
      }

      const latestProposal = state.proposals.at(-1);
      const rejectedResolution = state.waitResolutions.find((resolution) => resolution.decision === "rejected_by_author");
      if (latestProposal?.status === "awaiting_author_acceptance") {
        const note = document.createElement("div");
        note.className = "boundary-note";
        note.innerHTML = "<strong>提案已生成</strong>请在正文编辑器中查看、修改、接受或拒绝。Transcript 不会替你改写正文。";
        hostFlow.append(note);
      } else if (rejectedResolution) {
        const note = document.createElement("div");
        note.className = "boundary-note";
        note.innerHTML = "<strong>已取消</strong>没有生成提案，正文没有变化。";
        hostFlow.append(note);
      }
    }

    hostFlow.addEventListener("click", async (event) => {
      const button = event.target.closest("button[data-action]");
      if (!button) return;
      const card = button.closest(".approval");
      const path = button.dataset.action === "approve" ? "/api/approve" : "/api/reject";
      button.disabled = true;
      await fetch(path, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ waitId: card.dataset.waitId })
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
