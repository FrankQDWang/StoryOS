import path from "node:path";
import { labRoot, startLab } from "./lab-processes.mjs";

const lab = await startLab({
  coreStatePath: path.join(labRoot, ".lab-state", "interactive-core.json"),
});

process.stdout.write(
  `\nStoryOS Issue #69 lab is ready:\nEditor: ${lab.editorUrl}\nCore:   ${lab.coreUrl}\nPress Ctrl-C to stop.\n`,
);

let stopping = false;
async function stop() {
  if (stopping) return;
  stopping = true;
  await lab.stop();
  process.exit(0);
}

process.on("SIGINT", stop);
process.on("SIGTERM", stop);
await new Promise(() => {});
