import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const distDir = join(repositoryRoot, "apps/web/dist");

async function run(command: string, args: readonly string[]) {
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

test("make check packages the production graph before the Web tests that need it", async () => {
  const { code, stdout, stderr } = await run("make", ["-n", "check"]);
  assert.equal(code, 0, stderr);
  // The tier runs its steps in one shell command, so the order is the order in the text.
  const packageIndex = stdout.indexOf("python3 scripts/package-release.py");
  const contractIndex = stdout.indexOf("--project node-contract test/node-contract/");
  const phaseIndex = stdout.indexOf("python3 scripts/check-phases.py");
  assert.ok(packageIndex >= 0, `make check must build the paired release package\n${stdout}`);
  assert.ok(contractIndex >= 0, `make check must run the package contract files\n${stdout}`);
  assert.ok(phaseIndex >= 0, `make check must run the shared phases\n${stdout}`);
  assert.ok(packageIndex < contractIndex && packageIndex < phaseIndex, "vite build must fail closed before Web tests");
});

test("the Vite production build emits hashed Protected Web Client assets", async () => {
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
