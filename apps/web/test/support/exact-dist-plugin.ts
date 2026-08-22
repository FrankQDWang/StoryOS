import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";

import type { Plugin } from "vite";

const PROJECT_ROUTE = /^\/projects\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\/?$/u;
const ASSET_ROUTE = /^\/assets\/([A-Za-z0-9][A-Za-z0-9._-]*)$/u;

const CONTENT_TYPES: Readonly<Record<string, string>> = {
  ".css": "text/css; charset=utf-8",
  ".gif": "image/gif",
  ".html": "text/html; charset=utf-8",
  ".jpeg": "image/jpeg",
  ".jpg": "image/jpeg",
  ".js": "text/javascript; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".webp": "image/webp",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
};

function errorCode(error: unknown): unknown {
  return typeof error === "object" && error !== null ? Reflect.get(error, "code") : undefined;
}

export function exactDistPlugin(distRoot: string): Plugin {
  return {
    name: "storyos-exact-dist",
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        if (request.method !== "GET" && request.method !== "HEAD") {
          next();
          return;
        }
        const pathname = new URL(request.url ?? "/", "http://storyos.invalid").pathname;
        let filePath: string | undefined;
        if (pathname === "/" || PROJECT_ROUTE.test(pathname)) {
          filePath = join(distRoot, "index.html");
        } else {
          const asset = ASSET_ROUTE.exec(pathname)?.[1];
          if (asset !== undefined) {
            filePath = join(distRoot, "assets", asset);
          }
        }
        if (filePath === undefined) {
          next();
          return;
        }
        let bytes: Uint8Array;
        try {
          bytes = await readFile(filePath);
        } catch (error: unknown) {
          if (errorCode(error) === "ENOENT") {
            next();
            return;
          }
          next(error);
          return;
        }
        response.statusCode = 200;
        response.setHeader("cache-control", "no-store");
        response.setHeader("content-length", String(bytes.byteLength));
        response.setHeader(
          "content-type",
          CONTENT_TYPES[extname(filePath)] ?? "application/octet-stream",
        );
        response.setHeader("x-content-type-options", "nosniff");
        response.end(request.method === "HEAD" ? undefined : bytes);
      });
    },
  };
}
