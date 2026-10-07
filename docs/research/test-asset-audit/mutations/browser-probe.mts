import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { startStoryOSServer, stopStoryOSServer } from '../../../../apps/web/test/support/node-integration.ts';

const repositoryRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const started = await startStoryOSServer({ repositoryRoot,
  serverBinary: `${repositoryRoot}/target/release-package/storyos-server`,
  sessions: { 'session-a': '018f0000-0000-7001-8000-000000000001' } });
try {
  const child = spawn('pnpm', ['--dir', 'apps/web', 'exec', 'vitest', 'run', '--project',
    'browser-exact-dist', 'test/browser-exact-dist/s2-move-retype.integration.test.ts',
    '-t', '^moves and retypes Blocks with stable identity and refuses copy as a move$'],
    { cwd: repositoryRoot, stdio: 'inherit', env: { ...process.env, STORYOS_DEV_SERVER: started.baseUrl } });
  process.exitCode = await new Promise<number>((resolve, reject) => {
    child.once('error', reject); child.once('exit', (code) => resolve(code ?? 1));
  });
} finally { await stopStoryOSServer(started.server); }
