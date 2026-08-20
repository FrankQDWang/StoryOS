import react from "@vitejs/plugin-react";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const webRoot = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(webRoot, "../..");

export default defineConfig({
  plugins: [react()],
  root: webRoot,
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
  },
  esbuild: {
    target: "es2022",
  },
  server: {
    fs: {
      allow: [repositoryRoot],
    },
    proxy: process.env.STORYOS_DEV_SERVER
      ? { "/api": process.env.STORYOS_DEV_SERVER }
      : undefined,
  },
});
