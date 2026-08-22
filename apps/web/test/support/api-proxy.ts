import type { ProxyOptions } from "vite";

export function storyOSApiProxy(
  targetValue: string | undefined,
): Record<string, ProxyOptions> | undefined {
  if (targetValue === undefined) {
    return undefined;
  }
  const target = new URL(targetValue);
  if (target.protocol !== "http:" && target.protocol !== "https:") {
    throw new Error("STORYOS_DEV_SERVER must use HTTP or HTTPS");
  }
  const origin = target.origin;
  return {
    "/api": {
      changeOrigin: true,
      configure(proxy) {
        proxy.on("proxyReq", (proxyRequest) => {
          proxyRequest.setHeader("host", target.host);
          proxyRequest.setHeader("origin", origin);
        });
      },
      target: origin,
    },
  };
}
