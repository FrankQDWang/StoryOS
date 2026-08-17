import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const chromeExecutable = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean).find(existsSync);

const harness = `<!doctype html><html><body data-result="loading">
<textarea id="editor">Base</textarea><textarea id="synthetic">Base</textarea>
<textarea id="silent">Base</textarea><script type="module">
import { attachManualInput } from "/apps/web/src/manual-input.mjs";
const editor = document.querySelector("#editor");
const trace = { native: [], persisted: [], submissions: [], projections: [], failures: [] };
for (const type of ["beforeinput", "input", "compositionstart", "compositionupdate", "compositionend",
  "paste", "cut"]) {
  editor.addEventListener(type, (event) => trace.native.push({
    type: event.type, inputType: event.inputType ?? null, data: event.data ?? null,
    isComposing: event.isComposing ?? null, isTrusted: event.isTrusted,
  }));
}
let body = editor.value;
let pending = 0;
const workspace = { pending: { body, save_state: "clean", unsettled_intent_count: 0,
  authoritative_revision_id: "revision-0" } };
const persistIntent = async (_workspace, edit) => {
  trace.persisted.push(JSON.parse(JSON.stringify(edit)));
  body = edit.resultingBody;
  pending += 1;
  return { body, save_state: "saving", unsettled_intent_count: pending,
    authoritative_revision_id: "revision-0" };
};
const submitGroup = async () => {
  trace.submissions.push({ body, pending });
  pending = 0;
  return { body, save_state: "saved", unsettled_intent_count: 0,
    authoritative_revision_id: "revision-" + trace.submissions.length };
};
let manualNow = Date.parse("2026-08-15T12:00:00.000Z");
globalThis.storyosHarness = {
  editor,
  trace,
  advanceTime: (milliseconds) => { manualNow += milliseconds; },
  controller: attachManualInput({ editor, workspace, persistIntent, submitGroup,
    onProjection: (projection) => trace.projections.push(projection),
    onFailure: (error) => trace.failures.push(String(error?.message ?? error)),
    nowImpl: () => manualNow, setTimeoutImpl: () => 1, clearTimeoutImpl: () => {},
    isTrustedEvent: (event) => event.isTrusted || event.type.startsWith("composition")
      || event.inputType === "insertCompositionText"
      || (event.inputType === "insertText" && event.data === "中文") }),
};
for (const id of ["synthetic", "silent"]) {
  const attackEditor = document.querySelector("#" + id);
  const attackTrace = { persisted: [], failures: [] };
  attachManualInput({ editor: attackEditor, workspace,
    persistIntent: async (_workspace, edit) => { attackTrace.persisted.push(edit); },
    submitGroup, onFailure: (error) => attackTrace.failures.push(String(error?.message ?? error)) });
  globalThis.storyosHarness[id] = { editor: attackEditor, trace: attackTrace };
}
document.body.dataset.result = "ready";
</script></body></html>`;

function devToolsAddress(browser) {
  return new Promise((resolve, reject) => {
    let stderr = "";
    const timeout = setTimeout(() => reject(new Error(`Chrome DevTools did not start: ${stderr}`)), 10_000);
    browser.stderr.setEncoding("utf8");
    browser.stderr.on("data", (chunk) => {
      stderr += chunk;
      const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    });
    browser.once("exit", (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`Chrome exited before DevTools started (${code ?? signal}): ${stderr}`));
    });
  });
}

async function startBrowser() {
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-manual-input-"));
  const browser = spawn(chromeExecutable, [
    "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
    "--disable-dev-shm-usage", "--no-first-run", "--remote-debugging-port=0",
    "--remote-allow-origins=*", `--user-data-dir=${profileDirectory}`, "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  return { browser, profileDirectory, browserWebSocketUrl: await devToolsAddress(browser) };
}

async function connectPage(browserWebSocketUrl, url) {
  const { port } = new URL(browserWebSocketUrl);
  const target = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT", signal: AbortSignal.timeout(5_000),
  }).then((response) => response.json());
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await once(socket, "open");
  let nextId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    if (message.error) entry.reject(new Error(message.error.message));
    else entry.resolve(message.result);
  });
  const command = (method, params = {}) => new Promise((resolve, reject) => {
    nextId += 1;
    pending.set(nextId, { resolve, reject });
    socket.send(JSON.stringify({ id: nextId, method, params }));
  });
  await command("Runtime.enable");
  return { socket, command };
}

async function evaluate(command, expression) {
  const result = await command("Runtime.evaluate", {
    expression, awaitPromise: true, returnByValue: true,
  });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
  return result.result.value;
}

async function waitFor(command, expression) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    if (await evaluate(command, expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`Browser condition timed out: ${expression}`);
}

async function stopBrowser(browser, profileDirectory) {
  if (browser.exitCode === null) {
    const exited = once(browser, "exit");
    browser.kill("SIGTERM");
    await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
  }
  rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

test("trusted browser input and controlled Chrome IME preserve completed-intent boundaries", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 30_000,
}, async () => {
  const server = createServer(async (request, response) => {
    if (request.url === "/harness") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(harness);
      return;
    }
    const pathname = new URL(request.url, "http://storyos.test").pathname;
    const allowed = new Set([
      "/apps/web/src/manual-input.mjs",
      "/apps/web/src/editor-session.mjs",
      "/apps/web/src/local-edit-journal.mjs",
      "/apps/web/src/author-edit-submission.mjs",
      "/generated/typescript/storyos-public-release-1/client.mjs",
    ]);
    if (!allowed.has(pathname)) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, {
      "content-type": extname(pathname) === ".mjs" ? "text/javascript; charset=utf-8" : "text/plain",
    });
    response.end(await readFile(join(repositoryRoot, pathname)));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  let launched;
  let page;
  try {
    launched = await startBrowser();
    page = await connectPage(
      launched.browserWebSocketUrl, `http://127.0.0.1:${server.address().port}/harness`,
    );
    await waitFor(page.command, "document.body.dataset.result === 'ready'");
    await evaluate(page.command, `
      storyosHarness.synthetic.editor.value = "Injected";
      storyosHarness.synthetic.editor.dispatchEvent(new InputEvent("input", {
        inputType: "insertText", data: "Injected",
      }));
    `);
    assert.deepEqual(await evaluate(page.command, "storyosHarness.synthetic.trace"), {
      persisted: [], failures: ["Manual input lost its trusted input boundary"],
    });
    await evaluate(page.command, `
      storyosHarness.silent.editor.value = "Injected";
      storyosHarness.silent.editor.focus();
      storyosHarness.silent.editor.setSelectionRange(8, 8);
    `);
    await page.command("Input.insertText", { text: "!" });
    await waitFor(page.command, "storyosHarness.silent.trace.failures.length === 1");
    assert.deepEqual(await evaluate(page.command, "storyosHarness.silent.trace"), {
      persisted: [], failures: ["Manual input lost its trusted beforeinput boundary"],
    });
    await evaluate(page.command, "storyosHarness.editor.focus(); storyosHarness.editor.setSelectionRange(4, 4)");
    await page.command("Input.insertText", { text: "A" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 1");
    await evaluate(page.command, "storyosHarness.advanceTime(251)");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, 4); storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "X" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 2");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.submissions.length"), 1);
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(2, 2); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 3");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await page.command("Browser.grantPermissions", {
      origin: `http://127.0.0.1:${server.address().port}`,
      permissions: ["clipboardReadWrite", "clipboardSanitizedWrite"],
    });
    await evaluate(page.command, "navigator.clipboard.writeText('P')");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 1); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "v", code: "KeyV", modifiers: 4, commands: ["Paste"],
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "v", code: "KeyV", modifiers: 4,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 4");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 2); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "x", code: "KeyX", modifiers: 4, commands: ["Cut"],
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "x", code: "KeyX", modifiers: 4,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 5");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 1); storyosHarness.editor.focus()");
    await page.command("Input.imeSetComposition", {
      text: "取消", selectionStart: 2, selectionEnd: 2,
    });
    await waitFor(page.command,
      "storyosHarness.trace.native.some((event) => event.type === 'compositionupdate')");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 5);
    assert.equal(await evaluate(page.command, "storyosHarness.trace.submissions.length"), 5);
    await page.command("Input.imeSetComposition", {
      text: "", selectionStart: 0, selectionEnd: 0,
    });
    await waitFor(page.command,
      "storyosHarness.trace.native.some((event) => event.type === 'compositionend')");
    await evaluate(page.command, "storyosHarness.controller.whenIdle()");
    assert.equal(await evaluate(page.command, "storyosHarness.editor.value"), "X");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 5);
    assert.equal(await evaluate(page.command, "storyosHarness.trace.submissions.length"), 5);

    await page.command("Input.imeSetComposition", {
      text: "中文", selectionStart: 2, selectionEnd: 2,
    });
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 5);
    await page.command("Input.insertText", { text: "中文" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 6");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 6");
    await evaluate(page.command, `
      const commit = { bubbles: true, inputType: "insertText", data: "中文" };
      storyosHarness.editor.dispatchEvent(new InputEvent("beforeinput", commit));
      storyosHarness.editor.dispatchEvent(new InputEvent("input", commit));
    `);
    await evaluate(page.command, "storyosHarness.controller.whenIdle()");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 6);
    await page.command("Input.insertText", { text: "!" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 7");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 8");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await page.command("Input.imeSetComposition", {
      text: "draft", selectionStart: 5, selectionEnd: 5,
    });
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 8);
    await page.command("Input.insertText", { text: "word" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 9");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 9");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, 7); storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "aaaa" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 10");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 2); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 11");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, 3); storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "😀" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 12");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, 2); storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "😃" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 13");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    const trace = await evaluate(page.command, "storyosHarness.trace");
    assert.deepEqual(trace.persisted.map(({ undoGroupId, createdAt, ...edit }) => edit), [{
      from: 4, to: 4, text: "A", resultingBody: "BaseA", inputOrigin: "typing",
    }, {
      from: 0, to: 4, text: "X", resultingBody: "XA", inputOrigin: "selection_replacement",
    }, {
      from: 1, to: 2, text: "", resultingBody: "X", inputOrigin: "deletion",
    }, {
      from: 1, to: 1, text: "P", resultingBody: "XP", inputOrigin: "paste",
    }, {
      from: 1, to: 2, text: "", resultingBody: "X", inputOrigin: "cut",
    }, {
      from: 1, to: 1, text: "中文", resultingBody: "X中文",
      inputOrigin: "composition_confirmation",
    }, {
      from: 3, to: 3, text: "!", resultingBody: "X中文!", inputOrigin: "typing",
    }, {
      from: 3, to: 4, text: "", resultingBody: "X中文", inputOrigin: "deletion",
    }, {
      from: 3, to: 3, text: "word", resultingBody: "X中文word",
      inputOrigin: "composition_confirmation",
    }, {
      from: 0, to: 7, text: "aaaa", resultingBody: "aaaa",
      inputOrigin: "selection_replacement",
    }, {
      from: 1, to: 2, text: "", resultingBody: "aaa", inputOrigin: "deletion",
    }, {
      from: 0, to: 3, text: "😀", resultingBody: "😀",
      inputOrigin: "selection_replacement",
    }, {
      from: 0, to: 2, text: "😃", resultingBody: "😃",
      inputOrigin: "selection_replacement",
    }]);
    assert.equal(trace.persisted.every(({ undoGroupId }) => /^[0-9a-f-]{36}$/.test(undoGroupId)), true);
    assert.equal(trace.persisted.every(({ createdAt }, index, records) =>
      !Number.isNaN(Date.parse(createdAt))
        && (index === 0 || createdAt >= records[index - 1].createdAt)), true);
    assert.deepEqual(trace.submissions, [
      { body: "BaseA", pending: 1 },
      { body: "XA", pending: 1 },
      { body: "X", pending: 1 },
      { body: "XP", pending: 1 },
      { body: "X", pending: 1 },
      { body: "X中文", pending: 1 },
      { body: "X中文!", pending: 1 },
      { body: "X中文", pending: 1 },
      { body: "X中文word", pending: 1 },
      { body: "aaaa", pending: 1 },
      { body: "aaa", pending: 1 },
      { body: "😀", pending: 1 },
      { body: "😃", pending: 1 },
    ]);
    assert.equal(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "insertText"), true);
    assert.equal(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "insertFromPaste"), true);
    assert.equal(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "deleteByCut"), true);
    assert.equal(trace.native.filter((event) => event.type === "compositionend").length, 3);
    assert.deepEqual(trace.failures, []);
  } finally {
    page?.socket.close();
    server.close();
    if (launched) await stopBrowser(launched.browser, launched.profileDirectory);
  }
});
