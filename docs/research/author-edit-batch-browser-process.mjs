import { spawn } from "node:child_process";

export function captureBrowserDom({
  chrome,
  args,
  outputIsComplete,
  executionTimeoutMs,
  shutdownTimeoutMs,
  spawnProcess = spawn,
}) {
  return new Promise((resolve, reject) => {
    const child = spawnProcess(chrome, args);
    let stdout = "";
    let stderr = "";
    let settled = false;
    let failure;
    let executionTimer;
    let shutdownTimer;
    let stopRequested = false;

    const settle = error => {
      if (settled) return;
      settled = true;
      clearTimeout(executionTimer);
      clearTimeout(shutdownTimer);
      if (error) reject(error); else resolve({ stdout, stderr });
    };
    const stop = error => {
      if (settled) return;
      if (error) failure = error;
      if (stopRequested) return;
      stopRequested = true;
      clearTimeout(executionTimer);
      try {
        if (!child.kill("SIGKILL")) {
          settle(new Error("browser harness could not stop Chrome"));
          return;
        }
      } catch (killError) {
        settle(killError);
        return;
      }
      if (!settled) shutdownTimer = setTimeout(() => {
        const cause = failure ? `${failure.message}; ` : "";
        settle(new Error(`${cause}browser did not close within ${shutdownTimeoutMs} ms`));
      }, shutdownTimeoutMs);
    };

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", data => {
      stdout += data;
      if (outputIsComplete(stdout)) stop();
    });
    child.stderr.on("data", data => { stderr += data; });
    child.on("error", settle);
    child.on("close", () => {
      if (failure) settle(failure);
      else if (!outputIsComplete(stdout)) {
        settle(new Error(`browser closed before complete output: ${stderr}`));
      } else settle();
    });
    executionTimer = setTimeout(() => {
      stop(new Error(`browser harness timed out: ${stderr}`));
    }, executionTimeoutMs);
  });
}
