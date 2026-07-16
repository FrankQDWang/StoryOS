export function hostPage({ hostOrigin, sandboxOrigin }) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>StoryOS transcript MCP App host prototype</title>
  <style>
    :root { font: 14px/1.45 system-ui, sans-serif; color: #202020; background: #f6f6f4; }
    body { margin: 0; padding: 24px; }
    main { max-width: 960px; margin: auto; display: grid; gap: 16px; }
    section { background: white; border: 1px solid #ddd; padding: 16px; }
    h1, h2 { margin: 0 0 10px; }
    h1 { font-size: 20px; } h2 { font-size: 15px; }
    button { margin: 0 8px 8px 0; padding: 7px 10px; }
    iframe { width: 100%; height: 280px; border: 1px solid #ccc; background: #fff; }
    pre { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
    .muted { color: #666; }
    .fallback { padding: 14px; background: #f1f1ef; border-left: 3px solid #777; }
    #protocol { max-height: 180px; overflow: auto; }
  </style>
</head>
<body>
  <main>
    <section>
      <h1>Transcript host recovery probe</h1>
      <p class="muted">Host ${hostOrigin} · Sandbox ${sandboxOrigin}</p>
      <button id="refresh">Reload durable projection</button>
      <button id="inject">Send sibling-frame injection</button>
      <strong id="summary">loading…</strong>
    </section>
    <section>
      <h2>Author-visible transcript item</h2>
      <div id="view"></div>
    </section>
    <section>
      <h2>Durable state</h2>
      <pre id="state"></pre>
    </section>
    <section>
      <h2>Bridge log</h2>
      <pre id="protocol"></pre>
    </section>
  </main>
  <script>
    const HOST_ORIGIN = ${JSON.stringify(hostOrigin)};
    const SANDBOX_ORIGIN = ${JSON.stringify(sandboxOrigin)};
    const viewNode = document.querySelector("#view");
    const stateNode = document.querySelector("#state");
    const protocolNode = document.querySelector("#protocol");
    const summaryNode = document.querySelector("#summary");
    let snapshot = null;
    let sandboxFrame = null;
    let currentFrameKey = null;
    let rejectedMessages = 0;
    const protocol = [];

    function log(message) {
      protocol.push(message);
      protocolNode.textContent = protocol.slice(-20).join("\\n");
    }

    async function refresh() {
      snapshot = await fetch("/api/snapshot").then((response) => response.json());
      const state = snapshot.durable;
      const view = state.appViews[0] ?? null;
      const passes = state.evidence.filter((item) => item.passed).length;
      summaryNode.textContent = "host generation " + state.hostGeneration
        + " · MCP invocations " + (state.toolCalls[0]?.invocationCount ?? 0)
        + " · evidence " + passes + "/10";
      stateNode.textContent = JSON.stringify({
        authoritativeRevision: state.project.authoritativeRevision,
        waits: state.waits,
        proposals: state.proposals,
        bridgeRequests: state.bridgeRequests,
        transcript: snapshot.projection
      }, null, 2);
      renderView(view);
    }

    function renderView(view) {
      if (!view) {
        viewNode.textContent = "Press b in the terminal to create the originating ToolCall and View.";
        return;
      }
      const projected = snapshot.projection.views.find((candidate) => candidate.id === view.id);
      if (projected.renderMode === "static_fallback") {
        currentFrameKey = null;
        sandboxFrame?.remove();
        sandboxFrame = null;
        viewNode.innerHTML = "";
        const fallback = document.createElement("div");
        fallback.className = "fallback";
        fallback.textContent = view.staticFallback.title + ": " + view.staticFallback.text
          + " (" + view.staticFallback.provenance + ")";
        viewNode.append(fallback);
        log("rendered StoryOS static fallback");
        return;
      }
      const key = view.revision + ":" + view.resource.digest + ":" + snapshot.durable.hostGeneration;
      if (currentFrameKey === key && sandboxFrame?.isConnected) return;
      currentFrameKey = key;
      viewNode.innerHTML = "";
      sandboxFrame = document.createElement("iframe");
      sandboxFrame.title = "Sandboxed MCP App";
      sandboxFrame.sandbox = "allow-scripts allow-same-origin";
      sandboxFrame.src = SANDBOX_ORIGIN + "/proxy.html";
      viewNode.append(sandboxFrame);
      log("created fresh cross-origin Sandbox proxy");
    }

    function sendToSandbox(message) {
      sandboxFrame.contentWindow.postMessage(message, SANDBOX_ORIGIN);
    }

    window.addEventListener("message", async (event) => {
      const validSource = sandboxFrame && event.source === sandboxFrame.contentWindow;
      const validOrigin = event.origin === SANDBOX_ORIGIN;
      if (!validSource || !validOrigin) {
        rejectedMessages += 1;
        log("rejected window message: source/origin mismatch #" + rejectedMessages);
        await fetch("/api/audit", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            requestId: "client-source-denial-" + rejectedMessages,
            reason: "source_or_origin_mismatch"
          })
        });
        return;
      }

      const message = event.data;
      if (message?.type === "sandbox-proxy-ready") {
        const view = snapshot.durable.appViews[0];
        sendToSandbox({
          type: "sandbox-resource",
          resourceHtml: view.resource.html,
          resourceDigest: view.resource.digest
        });
        log("delivered exact stored resource snapshot");
        return;
      }
      if (message?.type === "sandbox-resource-ready") {
        sendToSandbox({
          type: "mcp-jsonrpc",
          payload: {
            jsonrpc: "2.0",
            id: "host-init-1",
            method: "ui/initialize",
            params: {
              protocolVersion: "2026-01-26",
              hostCapabilities: { tools: { call: true } },
              hostContext: snapshot.durable.appViews[0].hostContext
            }
          }
        });
        log("sent ui/initialize after resource ready");
        return;
      }
      if (message?.type !== "mcp-jsonrpc") return;
      const rpc = message.payload;
      if (!rpc || rpc.jsonrpc !== "2.0" || (rpc.id === undefined && !rpc.method)) {
        log("rejected malformed JSON-RPC before host dispatch");
        await fetch("/api/audit", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ requestId: rpc?.id ?? "malformed-browser-unknown", reason: "malformed_jsonrpc" })
        });
        return;
      }
      if (rpc.id === "host-init-1" && rpc.result) {
        sendToSandbox({ type: "mcp-jsonrpc", payload: {
          jsonrpc: "2.0", method: "ui/notifications/initialized", params: {}
        }});
        sendToSandbox({ type: "mcp-jsonrpc", payload: {
          jsonrpc: "2.0", method: "ui/notifications/tool-input",
          params: { revision: snapshot.durable.appViews[0].inputRevision }
        }});
        sendToSandbox({ type: "mcp-jsonrpc", payload: {
          jsonrpc: "2.0", method: "ui/notifications/tool-result",
          params: {
            revision: snapshot.durable.appViews[0].resultRevision,
            structuredContent: snapshot.durable.appViews[0].structuredResult
          }
        }});
        log("initialized; replayed complete input then durable result");
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
        log("host mediated App request " + rpc.id + ": " + (response.error ? "denied" : "accepted"));
        await refresh();
      }
    });

    document.querySelector("#refresh").addEventListener("click", refresh);
    document.querySelector("#inject").addEventListener("click", () => {
      const attacker = document.createElement("iframe");
      attacker.hidden = true;
      attacker.sandbox = "allow-scripts";
      const script = "<scr" + "ipt>parent.postMessage({type:'mcp-jsonrpc',payload:{jsonrpc:'2.0',id:'sibling-1',method:'tools/call',params:{name:'story.request_edit'}}},'*')</scr" + "ipt>";
      attacker.src = "data:text/html," + encodeURIComponent(script);
      document.body.append(attacker);
      setTimeout(() => attacker.remove(), 500);
    });

    refresh();
  </script>
</body>
</html>`;
}

export function proxyPage({ hostOrigin }) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>html,body,iframe{box-sizing:border-box;width:100%;height:100%;margin:0;border:0;background:#fff}</style>
</head>
<body>
  <script>
    const HOST_ORIGIN = ${JSON.stringify(hostOrigin)};
    let inner = null;

    window.addEventListener("message", (event) => {
      if (event.source === parent) {
        if (event.origin !== HOST_ORIGIN) return;
        const message = event.data;
        if (message?.type === "sandbox-resource") {
          inner?.remove();
          inner = document.createElement("iframe");
          inner.sandbox = "allow-scripts";
          inner.srcdoc = message.resourceHtml;
          document.body.append(inner);
          return;
        }
        if (message?.type === "mcp-jsonrpc" && inner) {
          inner.contentWindow.postMessage(message, "*");
        }
        return;
      }

      if (!inner || event.source !== inner.contentWindow) return;
      if (event.data?.type === "app-resource-ready") {
        parent.postMessage({ type: "sandbox-resource-ready" }, HOST_ORIGIN);
      } else if (event.data?.type === "mcp-jsonrpc") {
        parent.postMessage(event.data, HOST_ORIGIN);
      }
    });

    parent.postMessage({ type: "sandbox-proxy-ready" }, HOST_ORIGIN);
  </script>
</body>
</html>`;
}
