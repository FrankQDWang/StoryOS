export const RESOURCE_URI = "ui://storyos/library-summary";
export const RESOURCE_MIME = "text/html;profile=mcp-app";

export function resourceHtml(variant = "v1") {
  const label = variant === "v2" ? "资料源已更新" : "资料库";
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    :root {
      color-scheme: light;
      font: 13px/1.6 "PingFang SC", "Noto Sans SC", system-ui, sans-serif;
      color: #34322e;
      background: #f1efec;
    }
    * { box-sizing: border-box; }
    body { margin: 0; padding: 18px 20px; background: #f1efec; }
    header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
    h2 { margin: 0; font-size: 15px; font-weight: 600; }
    .tag { color: #77736b; font-size: 11px; }
    .summary { margin: 8px 0 13px; color: #67645d; }
    ul { display: grid; gap: 7px; margin: 0; padding: 0; list-style: none; }
    li { display: flex; align-items: center; justify-content: space-between; }
    li span:last-child { color: #77736b; font-size: 11px; }
    button { width: 100%; margin-top: 15px; padding: 9px 12px; border: 0; border-radius: 10px; background: #34312c; color: #fff; cursor: pointer; }
    button:hover { background: #24221e; }
    button:focus-visible { outline: 2px solid #77736c; outline-offset: 2px; }
  </style>
</head>
<body>
  <header>
    <h2>可用资料</h2>
    <span class="tag" id="status">${label}</span>
  </header>
  <p class="summary">StoryOS 找到了 3 个可以用于当前工作的资料源。</p>
  <ul id="sources" aria-label="可用资料源">
    <li><span>当前正文</span><span>已读取</span></li>
    <li><span>人物卡</span><span>已读取</span></li>
    <li><span>研究资料</span><span>已读取</span></li>
  </ul>
  <button id="edit">基于这些资料提出正文修改</button>
  <script>
    const status = document.querySelector("#status");
    let completeInput = null;
    let completeResult = null;

    function send(payload) {
      parent.postMessage({ type: "mcp-jsonrpc", payload }, "*");
    }

    window.addEventListener("message", (event) => {
      const envelope = event.data;
      if (!envelope || envelope.type !== "mcp-jsonrpc") return;
      const rpc = envelope.payload;
      if (!rpc || rpc.jsonrpc !== "2.0") return;

      if (rpc.method === "ui/initialize") {
        send({
          jsonrpc: "2.0",
          id: rpc.id,
          result: {
            protocolVersion: "2026-01-26",
            capabilities: { tools: { call: true } }
          }
        });
      } else if (rpc.method === "ui/notifications/initialized") {
        status.textContent = "已连接";
      } else if (rpc.method === "ui/notifications/tool-input") {
        completeInput = rpc.params;
      } else if (rpc.method === "ui/notifications/tool-result") {
        completeResult = rpc.params;
        status.textContent = completeResult?.structuredContent?.count === 3 ? "3 个来源" : "已恢复";
      }
    });

    document.querySelector("#edit").addEventListener("click", () => {
      send({
        jsonrpc: "2.0",
        id: "app-edit-browser-" + crypto.randomUUID(),
        method: "tools/call",
        params: {
          name: "story.request_edit",
          arguments: { replacement: "远处的灯火在盐沼尽头燃起，像有人终于举起了回应。" }
        }
      });
      status.textContent = "等待确认";
    });

    parent.postMessage({ type: "app-resource-ready" }, "*");
  </script>
</body>
</html>`;
}
