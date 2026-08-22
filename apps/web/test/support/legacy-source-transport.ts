import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";

import { transformWithEsbuild } from "vite";

interface LegacyBrowserModule {
  body: string;
  contentType: string;
}

const JAVASCRIPT_CONTENT_TYPE = "text/javascript; charset=utf-8";

export async function loadLegacyBrowserModule(
  repositoryRoot: string,
  pathname: string,
): Promise<LegacyBrowserModule> {
  const extension = extname(pathname);
  const sourcePath = join(repositoryRoot, pathname);
  const source = await readFile(sourcePath, "utf8");
  if (extension !== ".ts" && extension !== ".tsx") {
    return {
      body: source,
      contentType: extension === ".mjs" ? JAVASCRIPT_CONTENT_TYPE : "text/plain",
    };
  }
  const transformed = await transformWithEsbuild(source, sourcePath, {
    format: "esm",
    loader: extension === ".tsx" ? "tsx" : "ts",
    target: "es2022",
  });
  return { body: transformed.code, contentType: JAVASCRIPT_CONTENT_TYPE };
}
