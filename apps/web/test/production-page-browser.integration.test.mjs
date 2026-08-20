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
const distDir = join(repositoryRoot, "apps/web/dist");
const chromeExecutable = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean).find(existsSync);

function contentType(pathname) {
  switch (extname(pathname)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    default:
      return "application/octet-stream";
  }
}

function distPath(pathname) {
  const relative = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
  const resolved = join(distDir, relative);
  if (!resolved.startsWith(`${distDir}/`) && resolved !== join(distDir, "index.html")) {
    return null;
  }
  return resolved;
}

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
      reject(new Error(
        `Chrome exited before DevTools started (${code ?? signal ?? "unknown"}): ${stderr}`,
      ));
    });
  });
}

async function readProductionSurface(browserWebSocketUrl, pageUrl) {
  const { port } = new URL(browserWebSocketUrl);
  const target = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(pageUrl)}`, {
    method: "PUT",
    signal: AbortSignal.timeout(5_000),
  }).then((response) => response.json());
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await Promise.race([new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  }), new Promise((_, reject) => setTimeout(
    () => reject(new Error("Chrome DevTools WebSocket timed out")), 5_000,
  ))]);
  let commandId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const command = pending.get(message.id);
    if (!command) return;
    pending.delete(message.id);
    if (message.error) command.reject(new Error(message.error.message));
    else command.resolve(message.result);
  });
  const command = (method, params = {}) => new Promise((resolve, reject) => {
    commandId += 1;
    const id = commandId;
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`Chrome DevTools command timed out: ${method}`));
    }, 5_000);
    pending.set(id, {
      resolve(value) {
        clearTimeout(timeout);
        resolve(value);
      },
      reject(error) {
        clearTimeout(timeout);
        reject(error);
      },
    });
    socket.send(JSON.stringify({ id: commandId, method, params }));
  });
  await command("Runtime.enable");
  const deadline = Date.now() + 15_000;
  try {
    while (Date.now() < deadline) {
      const evaluation = await command("Runtime.evaluate", {
        expression: `({
          bootState: document.querySelector("#app")?.dataset.bootState ?? null,
          heading: document.querySelector("#app h1")?.textContent ?? null,
          message: document.querySelector("#app p")?.textContent ?? null,
          alert: document.querySelector('[role="alert"]') ? true : false,
          textarea: document.querySelector("textarea") ? true : false
        })`,
        returnByValue: true,
      });
      const surface = evaluation.result.value;
      if (surface?.bootState && (surface.heading || surface.textarea)) return surface;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("Vite production page did not boot the Stage 1 surface");
  } finally {
    socket.close();
  }
}

async function terminateBrowser(browser) {
  if (browser.exitCode !== null) return;
  const exited = once(browser, "exit");
  browser.kill("SIGTERM");
  await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
  if (browser.exitCode === null) {
    browser.kill("SIGKILL");
    await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
  }
}

test("a real Chrome session loads the Vite production page and shows the Stage 1 surface", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 30_000,
}, async () => {
  assert.equal(existsSync(join(distDir, "index.html")), true, "vite production dist is missing");
  const server = createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    const filePath = distPath(url.pathname);
    if (!filePath) {
      response.writeHead(404).end();
      return;
    }
    try {
      const body = await readFile(filePath);
      response.writeHead(200, { "content-type": contentType(filePath) });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-vite-page-"));
  const browser = spawn(chromeExecutable, [
    "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
    "--disable-dev-shm-usage", "--no-first-run", "--remote-debugging-port=0",
    "--remote-allow-origins=*", `--user-data-dir=${profileDirectory}`, "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  try {
    const browserWebSocketUrl = await devToolsAddress(browser);
    const surface = await readProductionSurface(
      browserWebSocketUrl,
      `http://127.0.0.1:${server.address().port}/`,
    );
    assert.deepEqual(surface, {
      bootState: "project-blocked",
      heading: "StoryOS 无法打开项目",
      message: "项目地址缺少有效的受控项目身份。",
      alert: true,
      textarea: false,
    });
  } finally {
    server.close();
    await terminateBrowser(browser);
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});
