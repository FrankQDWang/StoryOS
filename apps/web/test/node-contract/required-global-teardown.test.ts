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
const managedProbe = `
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1]) / 'scripts'))
from verification_tests import VerificationCommandTests, COMMAND
case = VerificationCommandTests()
case.setUp()
try:
    result = case.cli('run', '--', sys.executable, str(COMMAND), 'step',
                      'required-oracle', '--', *sys.argv[2:])
    report = case.report()
    print(json.dumps({'exit': result.returncode, 'status': report['status'],
                      'steps': [{'stage': step['stage'], 'status': step['status'],
                                 'exit': step['exit_code']} for step in report['steps']],
                      'oracle_error': 'STORYOS_REQUIRED_ORACLE_FAILED' in result.stderr}))
finally:
    case.doCleanups()
`;

test.each([
  { name: "required teardown fails", oracleFails: true, testFails: false, exit: 1 },
  { name: "required teardown passes", oracleFails: false, testFails: false, exit: 0 },
  { name: "ordinary test fails", oracleFails: false, testFails: true, exit: 1 },
])("managed verification reports $name", async ({ oracleFails, testFails, exit }) => {
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
    const result = await execute("python3", ["-c", managedProbe, repositoryRoot, process.execPath,
      vitest, "run", "--root", directory, "--config", join(directory, "vitest.config.mjs")],
    { cwd: repositoryRoot, timeout: 30_000 });
    const status = exit === 0 ? "passed" : "failed";
    assert.deepEqual(JSON.parse(result.stdout), {
      exit, status, steps: [{ stage: "required-oracle", status, exit }], oracle_error: oracleFails,
    });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}, 40_000);
