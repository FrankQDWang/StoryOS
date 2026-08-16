import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";
import test from "node:test";

import { captureBrowserDom } from "./author-edit-batch-browser-process.mjs";

class FakeChild extends EventEmitter {
  constructor({ killError, killResult = true } = {}) {
    super();
    this.stdout = new PassThrough();
    this.stderr = new PassThrough();
    this.killError = killError;
    this.killResult = killResult;
    this.killSignals = [];
  }

  kill(signal) {
    this.killSignals.push(signal);
    if (this.killError) throw this.killError;
    return this.killResult;
  }
}

const capture = (child, overrides = {}) => captureBrowserDom({
  chrome: "/synthetic/chrome",
  args: ["synthetic.html"],
  outputIsComplete: output => output.includes("</pre>"),
  executionTimeoutMs: 20,
  shutdownTimeoutMs: 20,
  spawnProcess: () => child,
  ...overrides,
});

test("complete output waits for browser close and settles once", async () => {
  const child = new FakeChild();
  const resultPromise = capture(child);
  child.stdout.write('<pre id="result">evidence</pre>');
  assert.deepEqual(child.killSignals, ["SIGKILL"]);
  child.emit("close");
  child.emit("error", new Error("late duplicate event"));
  assert.equal((await resultPromise).stdout, '<pre id="result">evidence</pre>');
  assert.deepEqual(child.killSignals, ["SIGKILL"]);
});

test("execution timeout has a second bounded shutdown deadline", async () => {
  const child = new FakeChild();
  await assert.rejects(capture(child, { executionTimeoutMs: 1, shutdownTimeoutMs: 1 }),
    /did not close within 1 ms/);
  assert.deepEqual(child.killSignals, ["SIGKILL"]);
});

test("spawn and kill errors reject", async () => {
  await assert.rejects(capture(new FakeChild(), {
    spawnProcess: () => { throw new Error("spawn threw"); },
  }), /spawn threw/);

  const spawnChild = new FakeChild();
  const spawnPromise = capture(spawnChild);
  spawnChild.emit("error", new Error("spawn failed"));
  await assert.rejects(spawnPromise, /spawn failed/);

  const killChild = new FakeChild({ killError: new Error("kill failed") });
  const killPromise = capture(killChild);
  killChild.stdout.write("</pre>");
  await assert.rejects(killPromise, /kill failed/);

  const refusedKillChild = new FakeChild({ killResult: false });
  const refusedKillPromise = capture(refusedKillChild);
  refusedKillChild.stdout.write("</pre>");
  await assert.rejects(refusedKillPromise, /could not stop Chrome/);
});

test("close before complete output rejects", async () => {
  const child = new FakeChild();
  const resultPromise = capture(child);
  child.emit("close");
  await assert.rejects(resultPromise, /closed before complete output/);
});
