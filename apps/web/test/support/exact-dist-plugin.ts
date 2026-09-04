import { readFile } from "node:fs/promises";
import http from "node:http";
import type { IncomingHttpHeaders, IncomingMessage, ServerResponse } from "node:http";
import https from "node:https";
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

function incomingCookieHeader(value: string | string[] | undefined): string | undefined {
  if (value === undefined) return undefined;
  const header = Array.isArray(value) ? value.join("; ") : value;
  return header.length === 0 ? undefined : header;
}

function setCookieValues(headers: IncomingHttpHeaders): string[] {
  const value = headers["set-cookie"];
  if (value === undefined) return [];
  return Array.isArray(value) ? value : [value];
}

// Vite serves exact-dist HTML. The printed origin is a different Host.
// Copy the packaged Server Set-Cookie onto this document GET so single-User
// journeys can present the issued handle without a Browser Command cookie.
// Forward the browser Cookie so a mismatched handle stays unchanged.
function copyServerSessionCookie(
  pathname: string,
  cookieHeader: string | undefined,
  response: ServerResponse,
): Promise<void> {
  const targetValue = process.env.STORYOS_DEV_SERVER;
  if (targetValue === undefined) return Promise.resolve();
  const target = new URL(targetValue);
  if (target.protocol !== "http:" && target.protocol !== "https:") {
    return Promise.reject(new Error("STORYOS_DEV_SERVER must use HTTP or HTTPS"));
  }
  const documentUrl = new URL(pathname, target);
  const transport = target.protocol === "https:" ? https.request : http.request;
  return new Promise((resolve, reject) => {
    const documentRequest = transport(documentUrl, {
      headers: cookieHeader === undefined ? {} : { cookie: cookieHeader },
      method: "GET",
    }, (documentResponse: IncomingMessage) => {
      documentResponse.resume();
      documentResponse.on("error", reject);
      documentResponse.on("end", () => {
        for (const cookie of setCookieValues(documentResponse.headers)) {
          if (cookie.startsWith("storyos_session=")) {
            response.appendHeader("set-cookie", cookie);
          }
        }
        resolve();
      });
    });
    documentRequest.on("error", reject);
    documentRequest.end();
  });
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
        const isDocument = pathname === "/" || PROJECT_ROUTE.test(pathname);
        let filePath: string | undefined;
        if (isDocument) {
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
        if (request.method === "GET" && isDocument) {
          try {
            await copyServerSessionCookie(
              pathname,
              incomingCookieHeader(request.headers.cookie),
              response,
            );
          } catch (error: unknown) {
            next(error);
            return;
          }
        }
        response.end(request.method === "HEAD" ? undefined : bytes);
      });
    },
  };
}
