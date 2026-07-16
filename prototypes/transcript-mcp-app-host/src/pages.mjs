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
