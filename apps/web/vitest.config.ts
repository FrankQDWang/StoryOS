import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig, defineProject } from "vitest/config";

import { storyOSApiProxy } from "./test/support/api-proxy";
import { storyOSBrowserCommands } from "./test/support/browser-commands";
import { exactDistPlugin } from "./test/support/exact-dist-plugin";
import { ScriptOrderSequencer } from "./test/support/script-order-sequencer";

const webRoot = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(webRoot, "../..");
const proxy = storyOSApiProxy(process.env.STORYOS_DEV_SERVER);
const physicalDrill = process.env.STORYOS_PHYSICAL_DRILL === "1";
const serialTest = { fileParallelism: false, isolate: true, retry: 0 } as const;
// scripts/check-lanes.py gives each browser lane its own port. Two Vite servers that try the same free
// port at the same time can both listen on it, one on ::1 and one on 127.0.0.1, and then a browser
// connects to the server of the other lane.
const browserApiPort = process.env.STORYOS_BROWSER_API_PORT;
function browserTest() {
  return {
    browser: {
      ...(browserApiPort === undefined
        ? {}
        : { api: { host: "127.0.0.1", port: Number(browserApiPort), strictPort: true } }),
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

export default defineConfig({
  test: {
    sequence: { sequencer: ScriptOrderSequencer },
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
        plugins: [react()],
        root: webRoot,
        server: {
          fs: { allow: [repositoryRoot] },
          ...(proxy === undefined ? {} : { proxy }),
        },
        test: {
          ...browserTest(),
          include: ["test/browser-source/**/*.test.ts"],
          name: "browser-source",
          setupFiles: ["./test/support/browser-page-reset.ts"],
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
          include: physicalDrill
            ? ["test/browser-exact-dist/s2-physical-drill.integration.test.ts"]
            : ["test/browser-exact-dist/**/*.test.ts"],
          ...(physicalDrill
            ? {}
            : { exclude: ["test/browser-exact-dist/s2-physical-drill.integration.test.ts"] }),
          name: "browser-exact-dist",
          testTimeout: 120_000,
        },
      }),
    ],
  },
});
