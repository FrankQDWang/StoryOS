import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";
import type { Plugin } from "vite";
import { defineConfig, defineProject } from "vitest/config";

import { storyOSApiProxy } from "./test/support/api-proxy";
import { storyOSBrowserCommands } from "./test/support/browser-commands";
import { exactDistPlugin } from "./test/support/exact-dist-plugin";

const webRoot = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(webRoot, "../..");
const proxy = storyOSApiProxy(process.env.STORYOS_DEV_SERVER);
const serialTest = { fileParallelism: false, isolate: true, retry: 0 } as const;
function browserTest() {
  return {
    browser: {
      commands: storyOSBrowserCommands,
      enabled: true,
      headless: true,
      instances: [{ browser: "chromium" as const }],
      provider: playwright({
        launchOptions: { channel: "chrome" },
        persistentContext: false,
      }),
    },
    ...serialTest,
    testTimeout: 30_000,
  };
}

const sessionCookieProbe: Plugin = {
  name: "storyos-session-cookie-probe",
  configureServer(server) {
    server.middlewares.use("/__storyos_browser_foundation__/session", (request, response) => {
      const bound = (request.headers.cookie ?? "")
        .split(";")
        .map((part) => part.trim())
        .includes("storyos_session=session-a");
      response.statusCode = 200;
      response.setHeader("cache-control", "no-store");
      response.setHeader("content-type", "application/json; charset=utf-8");
      response.end(JSON.stringify({ bound }));
    });
  },
};

export default defineConfig({
  test: {
    projects: [
      defineProject({
        root: webRoot,
        test: { ...serialTest, include: ["test/node-contract/**/*.test.ts"], name: "node-contract" },
      }),
      defineProject({
        root: webRoot,
        test: {
          ...serialTest,
          include: ["test/node-postgresql/**/*.test.ts"],
          name: "node-postgresql",
          testTimeout: 120_000,
        },
      }),
      defineProject({
        root: webRoot,
        test: {
          ...serialTest,
          include: ["test/node-process-cut/**/*.test.ts"],
          name: "node-process-cut",
          testTimeout: 120_000,
        },
      }),
      defineProject({
        plugins: [react(), sessionCookieProbe],
        root: webRoot,
        server: {
          fs: { allow: [repositoryRoot] },
          ...(proxy === undefined ? {} : { proxy }),
        },
        test: {
          ...browserTest(),
          include: ["test/browser-source/**/*.test.ts"],
          name: "browser-source",
        },
      }),
      defineProject({
        plugins: [exactDistPlugin(resolve(webRoot, "dist"))],
        root: webRoot,
        server: {
          fs: { allow: [repositoryRoot] },
          ...(proxy === undefined ? {} : { proxy }),
        },
        test: {
          ...browserTest(),
          include: ["test/browser-exact-dist/**/*.test.ts"],
          name: "browser-exact-dist",
        },
      }),
    ],
  },
});
