import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

import { createInitialState, reduce } from "./model.mjs";

export async function loadState(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    if (error.code === "ENOENT") {
      return createInitialState();
    }
    throw error;
  }
}

export async function saveState(path, state) {
  await mkdir(dirname(path), { recursive: true });
  const temporaryPath = `${path}.next`;
  await writeFile(temporaryPath, `${JSON.stringify(state, null, 2)}\n`, "utf8");
  await rename(temporaryPath, path);
}

export async function dispatch(path, action) {
  const before = await loadState(path);
  const after = reduce(before, action);
  await saveState(path, after);
  return after;
}

export async function resetScratch(path) {
  await rm(path, { force: true, recursive: true });
}
