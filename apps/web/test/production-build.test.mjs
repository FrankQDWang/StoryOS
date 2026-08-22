import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const distDir = join(repositoryRoot, "apps/web/dist");

async function run(command, args) {
  const child = spawn(command, args, {
    cwd: repositoryRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const [code] = await once(child, "close");
  return { code, stdout, stderr };
}

test("make web builds the Vite production graph before Web tests", async () => {
  const { code, stdout, stderr } = await run("make", ["-n", "web"]);
  assert.equal(code, 0, stderr);
  const commands = stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
  const viteIndex = commands.findIndex((line) => /vite build/.test(line));
  const testIndex = commands.findIndex((line) => line.includes("node --test"));
  assert.ok(viteIndex >= 0, `make web must invoke vite build\n${stdout}`);
  assert.ok(testIndex >= 0, `make web must keep node:test\n${stdout}`);
  assert.ok(viteIndex < testIndex, "vite build must fail closed before Web tests");
});

test("the Vite production build emits hashed Protected Web Client assets", async () => {
  const { code, stderr } = await run("pnpm", ["--dir", "apps/web", "run", "build"]);
  assert.equal(code, 0, stderr);
  const html = readFileSync(join(distDir, "index.html"), "utf8");
  const assets = readdirSync(join(distDir, "assets"));
  assert.match(html, /id="app"/);
  assert.doesNotMatch(html, /src="\.\/src\/main\.ts"/);
  assert.ok(
    assets.some((name) => /-[A-Za-z0-9_-]+\.js$/.test(name)),
    `production assets must be content-hashed, found ${assets.join(", ")}`,
  );
  assert.match(html, /assets\/[^"]+\.js/);
});
