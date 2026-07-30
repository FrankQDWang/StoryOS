import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const workload = JSON.parse(await readFile(join(here, "workload.json"), "utf8"));
const outputPath = join(here, "..", "browser-measurements.jsonl");
const chromePath =
  process.env.STORYOS_BENCH_CHROME ??
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const pageSource = String.raw`<!doctype html>
<meta charset="utf-8">
<title>StoryOS Issue 76 disposable browser instrument</title>
<div id="editor" contenteditable="true" spellcheck="false"></div>
<script>
const encoder = new TextEncoder();
const editor = document.querySelector("#editor");
const state = { db: null, sequence: 0, records: [], pending: [], eventEntries: [] };

new PerformanceObserver((list) => {
  for (const entry of list.getEntries()) {
    state.eventEntries.push({
      name: entry.name,
      duration_ms: entry.duration,
      interaction_id: entry.interactionId || 0,
      processing_start_ms: entry.processingStart,
      processing_end_ms: entry.processingEnd
    });
  }
}).observe({ type: "event", buffered: true, durationThreshold: 16 });

function raf2() {
  return new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now())))
  );
}

function openDb() {
  if (state.db) return Promise.resolve(state.db);
  return new Promise((resolve, reject) => {
    const request = indexedDB.open("storyos-issue76-browser-v1", 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      db.createObjectStore("journal", { keyPath: "sequence" });
      db.createObjectStore("chapters", { keyPath: "id" });
    };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      state.db = request.result;
      resolve(state.db);
    };
  });
}

async function clearStores() {
  const db = await openDb();
  await new Promise((resolve, reject) => {
    const tx = db.transaction(["journal", "chapters"], "readwrite", { durability: "strict" });
    tx.objectStore("journal").clear();
    tx.objectStore("chapters").clear();
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
  state.sequence = 0;
  state.records.length = 0;
  state.eventEntries.length = 0;
}

function persist(store, value) {
  return openDb().then((db) => new Promise((resolve, reject) => {
    const started = performance.now();
    const tx = db.transaction(store, "readwrite", { durability: "strict" });
    tx.objectStore(store).put(value);
    tx.oncomplete = () => resolve(performance.now() - started);
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  }));
}

editor.addEventListener("input", (event) => {
  const inputStarted = performance.now();
  const beforeBytes = encoder.encode(editor.textContent || "").length;
  const sequence = ++state.sequence;
  const record = {
    sequence,
    kind: event.inputType || "insertText",
    data: event.data || "",
    document_utf8_bytes: beforeBytes,
    created_at_ms: inputStarted
  };
  const logicalBytes = encoder.encode(JSON.stringify(record)).length;
  const completion = Promise.all([
    persist("journal", record),
    raf2().then((painted) => painted - inputStarted)
  ]).then(([journalMs, inputToPaintMs]) => {
    const measured = {
      sequence,
      input_type: record.kind,
      event_is_trusted: event.isTrusted,
      document_utf8_bytes: beforeBytes,
      journal_logical_bytes: logicalBytes,
      strict_journal_ms: journalMs,
      input_to_double_raf_ms: inputToPaintMs
    };
    state.records.push(measured);
    return measured;
  });
  state.pending.push(completion);
});

window.issue76 = {
  async reset(textBytes = 0) {
    await clearStores();
    editor.textContent = "x".repeat(textBytes);
    editor.focus();
    await raf2();
  },
  async waitForRecord(sequence) {
    while (state.records.length < sequence) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    return state.records[sequence - 1];
  },
  async syntheticComposition(text) {
    editor.focus();
    editor.dispatchEvent(new CompositionEvent("compositionstart", { data: "", bubbles: true }));
    editor.dispatchEvent(new CompositionEvent("compositionupdate", { data: text, bubbles: true }));
    document.execCommand("insertText", false, text);
    editor.dispatchEvent(new CompositionEvent("compositionend", { data: text, bubbles: true }));
    return this.waitForRecord(state.records.length + 1);
  },
  async seedNovel(chapterCount, chapterBytes) {
    await clearStores();
    const db = await openDb();
    const tx = db.transaction("chapters", "readwrite", { durability: "strict" });
    const store = tx.objectStore("chapters");
    for (let i = 0; i < chapterCount; i++) {
      store.put({ id: i, text: String(i % 10).repeat(chapterBytes) });
    }
    await new Promise((resolve, reject) => {
      tx.oncomplete = resolve;
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  },
  async openChapter(id) {
    const started = performance.now();
    const db = await openDb();
    const row = await new Promise((resolve, reject) => {
      const tx = db.transaction("chapters", "readonly");
      const request = tx.objectStore("chapters").get(id);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    editor.textContent = row.text;
    const painted = await raf2();
    return {
      chapter_id: id,
      chapter_utf8_bytes: encoder.encode(row.text).length,
      load_to_double_raf_ms: painted - started
    };
  },
  async journalGrowth(intentCount, patchBytes, checkpointEvery) {
    await clearStores();
    const usageBefore = await navigator.storage.estimate();
    let logicalBytes = 0;
    let naiveFullCopyBytes = 0;
    let text = "";
    let actualPatchBytes = 0;
    for (let i = 1; i <= intentCount; i++) {
      const patch = "字".repeat(Math.ceil(patchBytes / 3)).slice(0, patchBytes);
      actualPatchBytes = encoder.encode(patch).length;
      text += patch;
      const record = {
        sequence: i,
        kind: "patch",
        patch,
        checkpoint: i % checkpointEvery === 0 ? text : null
      };
      logicalBytes += encoder.encode(JSON.stringify(record)).length;
      naiveFullCopyBytes += encoder.encode(text).length * 2;
      await persist("journal", record);
    }
    const usageAfter = await navigator.storage.estimate();
    return {
      intent_count: intentCount,
      patch_utf8_bytes_requested: patchBytes,
      patch_utf8_bytes_actual: actualPatchBytes,
      checkpoint_every: checkpointEvery,
      logical_serialized_bytes: logicalBytes,
      naive_before_after_full_copy_bytes: naiveFullCopyBytes,
      storage_estimate_before: usageBefore,
      storage_estimate_after: usageAfter
    };
  },
  async networkPair(ackDelay, eventDelay) {
    const started = performance.now();
    const ack = fetch("/ack?delay=" + ackDelay).then((r) => r.json()).then(() => performance.now());
    const event = fetch("/event?delay=" + eventDelay).then((r) => r.json()).then(() => performance.now());
    const [ackAt, eventAt] = await Promise.all([ack, event]);
    return {
      ack_observed_ms: ackAt - started,
      event_observed_ms: eventAt - started,
      convergence_ms: Math.max(ackAt, eventAt) - started,
      observed_order: ackAt < eventAt ? "ack_first" : "event_first"
    };
  },
  async offlineJournal() {
    const started = performance.now();
    let networkError = null;
    try {
      await fetch("http://127.0.0.1:1/unreachable", { signal: AbortSignal.timeout(100) });
    } catch (error) {
      networkError = error.name;
    }
    const journalMs = await persist("journal", {
      sequence: ++state.sequence,
      kind: "offline_patch",
      patch: "offline-author-text"
    });
    return {
      network_error: networkError,
      strict_journal_ms: journalMs,
      total_ms: performance.now() - started
    };
  },
  metrics() {
    return {
      event_timing_entries: state.eventEntries,
      js_heap: performance.memory ? {
        used_js_heap_size: performance.memory.usedJSHeapSize,
        total_js_heap_size: performance.memory.totalJSHeapSize,
        js_heap_size_limit: performance.memory.jsHeapSizeLimit
      } : null
    };
  }
};
</script>`;

function delayedJson(response, delay, kind) {
  setTimeout(() => {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ kind }));
  }, Number(delay));
}

const server = createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  if (url.pathname === "/") {
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(pageSource);
    return;
  }
  if (url.pathname === "/ack" || url.pathname === "/event") {
    delayedJson(response, url.searchParams.get("delay") ?? 0, url.pathname.slice(1));
    return;
  }
  response.writeHead(404).end();
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const address = server.address();
const browser = await chromium.launch({
  executablePath: chromePath,
  headless: true,
  args: ["--disable-background-timer-throttling", "--disable-renderer-backgrounding"]
});
const context = await browser.newContext();
const page = await context.newPage();
await page.goto(`http://127.0.0.1:${address.port}/`);

const results = [];
const emit = (kind, fields) => results.push({ schema: "storyos.issue76.browser.v1", kind, ...fields });
const browserVersion = await browser.version();

for (const chapterBytes of workload.browser.chapter_utf8_bytes) {
  await page.evaluate((bytes) => window.issue76.reset(bytes), chapterBytes);
  const total = workload.browser.warmup_samples + workload.browser.measured_samples;
  for (let index = 0; index < total; index++) {
    const expected = index + 1;
    await page.keyboard.insertText(index % 2 === 0 ? "a" : "中");
    const sample = await page.evaluate((sequence) => window.issue76.waitForRecord(sequence), expected);
    if (index >= workload.browser.warmup_samples) {
      emit("input_and_journal", {
        language_probe: index % 2 === 0 ? "english_trusted_insert" : "unicode_trusted_insert_not_os_ime",
        chapter_profile_utf8_bytes: chapterBytes,
        warmup_discarded: false,
        sample: index - workload.browser.warmup_samples,
        ...sample
      });
    }
  }
}

await page.evaluate(() => window.issue76.reset(50000));
for (let index = 0; index < workload.browser.measured_samples; index++) {
  const sample = await page.evaluate(() => window.issue76.syntheticComposition("中文"));
  emit("synthetic_composition", {
    language_probe: "synthetic_chinese_composition_not_os_ime",
    sample: index,
    ...sample
  });
}

await page.evaluate(
  ({ chapters, bytes }) => window.issue76.seedNovel(chapters, bytes),
  { chapters: workload.browser.novel_chapters, bytes: workload.browser.novel_chapter_utf8_bytes }
);
for (let index = 0; index < workload.browser.chapter_switch_samples; index++) {
  const sample = await page.evaluate((id) => window.issue76.openChapter(id), index % workload.browser.novel_chapters);
  emit("chapter_switch", { sample: index, ...sample });
}
for (let index = 0; index < workload.browser.cold_open_samples; index++) {
  await page.reload();
  const sample = await page.evaluate((id) => window.issue76.openChapter(id), (index * 17) % workload.browser.novel_chapters);
  emit("cold_open", { sample: index, ...sample });
}

const growth = await page.evaluate(
  ({ intents, bytes, every }) => window.issue76.journalGrowth(intents, bytes, every),
  {
    intents: workload.browser.journal_intents,
    bytes: workload.browser.journal_patch_utf8_bytes,
    every: workload.browser.journal_checkpoint_every
  }
);
emit("journal_growth", growth);

for (const profile of workload.browser.network_profiles) {
  for (let sample = 0; sample < workload.browser.measured_samples; sample++) {
    const measured = await page.evaluate(
      ({ ack, event }) => window.issue76.networkPair(ack, event),
      { ack: profile.ack_delay_ms, event: profile.event_delay_ms }
    );
    emit("network_convergence", { profile: profile.name, sample, ...profile, ...measured });
  }
}
for (let sample = 0; sample < workload.browser.measured_samples; sample++) {
  emit("offline_journal", { sample, ...(await page.evaluate(() => window.issue76.offlineJournal())) });
}

emit("browser_environment", {
  browser_version: browserVersion,
  user_agent: await page.evaluate(() => navigator.userAgent),
  hardware_concurrency: await page.evaluate(() => navigator.hardwareConcurrency),
  device_memory_gib: await page.evaluate(() => navigator.deviceMemory ?? null),
  ...(await page.evaluate(() => window.issue76.metrics()))
});

await writeFile(outputPath, results.map((row) => JSON.stringify(row)).join("\n") + "\n");
await browser.close();
await new Promise((resolve) => server.close(resolve));
console.log(JSON.stringify({ output: outputPath, rows: results.length, browser_version: browserVersion }));
