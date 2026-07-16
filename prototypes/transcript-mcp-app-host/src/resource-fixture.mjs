export const RESOURCE_URI = "ui://storyos/library-summary";
export const RESOURCE_MIME = "text/html;profile=mcp-app";

export function resourceHtml(variant = "v1") {
  const label = variant === "v2" ? "Drifted server resource v2" : "Pinned resource v1";
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    :root { color-scheme: light; font: 14px/1.45 system-ui, sans-serif; }
    body { margin: 0; padding: 16px; background: #fafafa; color: #242424; }
    h2 { font-size: 16px; margin: 0 0 6px; }
    p { margin: 6px 0; }
    button { margin: 8px 6px 0 0; padding: 6px 10px; }
    pre { white-space: pre-wrap; padding: 8px; background: #f0f0f0; }
    .tag { color: #666; font-size: 12px; }
  </style>
</head>
<body>
  <div class="tag">${label}</div>
  <h2>Story library</h2>
  <p id="status">waiting for host initialization</p>
  <pre id="payload">no durable result delivered</pre>
  <button id="edit">Request story edit</button>
  <button id="unauthorized">Request shell access</button>
  <button id="malformed">Send malformed RPC</button>
  <script>
    const status = document.querySelector("#status");
    const payloadView = document.querySelector("#payload");
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
        status.textContent = "initialized on a fresh bridge";
      } else if (rpc.method === "ui/notifications/tool-input") {
        completeInput = rpc.params;
      } else if (rpc.method === "ui/notifications/tool-result") {
        completeResult = rpc.params;
        payloadView.textContent = JSON.stringify({ completeInput, completeResult }, null, 2);
      }
    });

    document.querySelector("#edit").addEventListener("click", () => {
      send({
        jsonrpc: "2.0",
        id: "app-edit-browser-1",
        method: "tools/call",
        params: {
          name: "story.request_edit",
          arguments: { replacement: "The lighthouse burned beyond the salt marsh." }
        }
      });
    });

    document.querySelector("#unauthorized").addEventListener("click", () => {
      send({
        jsonrpc: "2.0",
        id: "app-shell-browser-1",
        method: "tools/call",
        params: { name: "shell.exec", arguments: { command: "whoami" } }
      });
    });

    document.querySelector("#malformed").addEventListener("click", () => {
      send({ id: "malformed-browser-1", method: "tools/call" });
    });

    parent.postMessage({ type: "app-resource-ready" }, "*");
  </script>
</body>
</html>`;
}
