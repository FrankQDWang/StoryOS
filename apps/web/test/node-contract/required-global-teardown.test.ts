// Verification: repository-inputs-only.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

const execute = promisify(execFile);
const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const teardown = fileURLToPath(new URL("../support/required-global-teardown.ts", import.meta.url));
const require = createRequire(import.meta.url);
const vitest = join(dirname(require.resolve("vitest/package.json")), "vitest.mjs");

test.each([
  { name: "required teardown fails", oracleFails: true, testFails: false, exit: 1 },
  { name: "required teardown passes", oracleFails: false, testFails: false, exit: 0 },
  { name: "ordinary test fails", oracleFails: false, testFails: true, exit: 1 },
])("vitest reports $name", async ({ oracleFails, testFails, exit }) => {
  const directory = await mkdtemp(join(tmpdir(), "storyos-required-teardown-"));
  try {
    await writeFile(join(directory, "vitest.config.mjs"),
      "export default { test: { globals: true, include: ['sample.test.js'], globalSetup: ['./setup.ts'] } };\n");
    await writeFile(join(directory, "sample.test.js"),
      `test('sample', () => expect(${testFails ? 0 : 1}).toBe(1));\n`);
    await writeFile(join(directory, "setup.ts"),
      `import { requiredGlobalTeardown } from ${JSON.stringify(teardown)};\n` +
      "export default function setup() { return requiredGlobalTeardown(async () => {\n" +
      (oracleFails ? "throw new Error('STORYOS_REQUIRED_ORACLE_FAILED');\n" : "") +
      "}); }\n");
    // The check tiers stop at the first command that exits with a status other than 0.
    const result = await execute(process.execPath, [vitest, "run", "--root", directory, "--config",
      join(directory, "vitest.config.mjs")], { cwd: repositoryRoot, timeout: 30_000 })
      .then(({ stderr }) => ({ code: 0, stderr }), (error: { code: number; stderr: string }) => error);
    assert.deepEqual({ exit: result.code, oracle_error: result.stderr.includes("STORYOS_REQUIRED_ORACLE_FAILED") },
      { exit, oracle_error: oracleFails });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}, 40_000);
