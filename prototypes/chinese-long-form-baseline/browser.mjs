import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const require = createRequire(resolve('apps/web/package.json'));
const { chromium } = require('playwright');

export async function browserRun({ baseUrl, projectId, chapters, out, measure, budget, recordWire, sql, user, worker, editorSessionId }) {
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  const network = [];
  page.on('response', response => {
    if (!response.url().includes('/api/')) return;
    network.push((async () => {
      const body = await response.body();
      let data; try { data = JSON.parse(body); } catch {}
      const positions = data ? { chapter: data.chapter ? data.project_activity_position : undefined, snapshot: data.snapshot?.project_activity_position, session_base: data.base_snapshot?.project_activity_position, writer_generation: data.writer?.writer_generation } : undefined;
      recordWire({ method: response.request().method(), path: new URL(response.url()).pathname, status: response.status(), request_bytes: Buffer.byteLength(response.request().postData() ?? ''), response_bytes: body.length, positions });
    })().catch(() => {}));
  });
  await context.addInitScript(() => {
    window.baselineIdb = { calls: {}, returned_records: 0, writes: 0, enabled: true };
    for (const cls of [IDBObjectStore, IDBIndex]) for (const name of ['get', 'getAll', 'getAllKeys', 'openCursor', 'openKeyCursor', 'put', 'add', 'delete']) {
      const original = cls.prototype[name];
      if (!original) continue;
      cls.prototype[name] = function (...args) {
        const request = original.apply(this, args), active = window.baselineIdb.enabled;
        if (active) {
          const key = `${this.name}:${name}`;
          window.baselineIdb.calls[key] = (window.baselineIdb.calls[key] ?? 0) + 1;
          if (['put', 'add', 'delete'].includes(name)) window.baselineIdb.writes++;
          else request.addEventListener('success', () => {
            const value = request.result;
            window.baselineIdb.returned_records += Array.isArray(value) ? value.length : value === undefined || value === null ? 0 : 1;
          });
        }
        return request;
      };
    }
  });
  await context.addInitScript(({ projectId, user, editorSessionId }) => sessionStorage.setItem(`active_session:${user}:${projectId}`, editorSessionId), { projectId, user, editorSessionId });
  const saved = async () => {
    await page.waitForFunction(() => { const e = document.querySelector('[data-save-state]'); return e?.getAttribute('data-save-state') === 'needs_attention' || e?.getAttribute('data-save-state') === 'saved' && e.getAttribute('data-unsettled-intent-count') === '0' && !e.getAttribute('data-editor-failure'); });
    if (await page.locator('[data-save-state]').getAttribute('data-save-state') === 'needs_attention') throw new Error(`Editor needs attention: ${await page.locator('[data-save-state]').getAttribute('data-editor-failure')}`);
  };
  const text = () => page.locator('[data-manuscript-editor]').evaluate(e => [...e.children].map(n => n.textContent).join('\n'));
  async function facts() {
    await page.waitForLoadState('networkidle');
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await Promise.all(network.splice(0));
    return page.evaluate(async () => {
      const metrics = structuredClone(window.baselineIdb);
      window.baselineIdb.enabled = false;
      const stores = {};
      for (const { name } of await indexedDB.databases()) {
        const db = await new Promise(resolve => { const r = indexedDB.open(name); r.onsuccess = () => resolve(r.result); });
        for (const store of db.objectStoreNames) stores[`${name}:${store}`] = await new Promise(resolve => { const r = db.transaction(store).objectStore(store).count(); r.onsuccess = () => resolve(r.result); });
        db.close();
      }
      window.baselineIdb.enabled = true;
      return { ...metrics, stores, record_count: Object.values(stores).reduce((a, b) => a + b, 0), editor_failure: document.querySelector('[data-save-state]')?.getAttribute('data-editor-failure'), visible_statistics: document.querySelector('[data-manuscript-statistics]')?.textContent, active_sessions: Object.fromEntries(Object.entries(sessionStorage).filter(([key]) => key.startsWith('active_session:'))) };
    });
  }
  async function sample(name, action) {
    if (page.url() !== 'about:blank') {
      await facts();
      await page.evaluate(() => { window.baselineIdb.calls = {}; window.baselineIdb.returned_records = 0; window.baselineIdb.writes = 0; });
    }
    return measure(`web-${name}`, async () => {
      try { await action(); return { browser: await facts() }; }
      catch (error) { error.baselineBrowser = await facts(); writeFileSync(resolve(out, `web-${name}-failure.json`), JSON.stringify({ error: String(error), body: await page.locator('body').innerText(), editor: await page.locator('[data-manuscript-editor]').getAttribute('contenteditable'), browser: error.baselineBrowser })); throw error; }
    });
  }
  async function open() {
    await page.goto(baseUrl);
    await page.locator(`button[data-project-id="${projectId}"]`).click();
    await page.locator('[data-manuscript-editor]').waitFor();
    await page.locator('[data-statistics-outcome="ready"]').waitFor();
  }
  const editor = page.locator('[data-manuscript-editor]');
  async function append(value) {
    const previous = await page.locator('[data-save-state]').getAttribute('data-authoritative-revision-id');
    await editor.evaluate(e => { e.focus(); const r = document.createRange(); r.selectNodeContents(e); r.collapse(false); const s = getSelection(); s.removeAllRanges(); s.addRange(r); });
    await page.keyboard.insertText(value);
    await page.waitForFunction(previous => document.querySelector('[data-save-state]')?.getAttribute('data-authoritative-revision-id') !== previous, previous);
    await saved();
  }
  try {
    await sample('open-project', open);
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await saved();
    await facts();
    await sample('input-save', () => append('测'));
    await sample('undo', async () => {
      const previous = await text();
      await editor.click(); await page.keyboard.press('Meta+z');
      await page.waitForFunction(previous => [...document.querySelector('[data-manuscript-editor]').children].map(n => n.textContent).join('\n') !== previous, previous);
      await saved();
    });
    await sample('switch-chapter', async () => {
      await page.locator(`[data-chapter-menu="${chapters[2]}"]`).click();
      await page.locator(`[data-make-current-chapter="${chapters[2]}"]`).click();
      await saved();
      await page.locator(`button[data-chapter-id="${chapters[2]}"][aria-current="true"]`).waitFor();
    });
    await sample('search', async () => {
      await page.locator('[name="manuscript-search-query"]').fill('不存在的紫色星河');
      await page.locator('[name="manuscript-search-selection"][value="manuscript"]').check();
      await page.locator('[data-manuscript-search-form] button[type="submit"]').click();
      await page.locator('[data-search-outcome="ready"]').waitFor();
    });
    await sample('session-recovery', async () => { await open(); await saved(); });
    await page.screenshot({ path: resolve(out, 'web.png'), fullPage: false });
    const probe = '甲𠀀𠮷e\u0301👩‍👩‍👧‍👦😀🇨🇳✈️，。！？ＡＢ１２　末';
    let expected = (await text()) + probe;
    await sample('unicode-insert', () => append(probe));
    const observations = [];
    async function capture(label) {
      const dom = await text();
      const server = await page.evaluate(async ({ projectId, chapterId }) => (await (await fetch(`/api/v1/projects/${projectId}/chapters/${chapterId}`)).json()).chapter.current_revision.body, { projectId, chapterId: chapters[2] });
      const rows = JSON.parse(sql(`SELECT json_agg(json_build_object('stored', convert_from(p.canonical_bytes, 'UTF8'))) FROM storyos.authoritative_heads h JOIN storyos.authoritative_revisions r ON (r.owner_user_id,r.project_id,r.manuscript_object_id,r.revision_id)=(h.owner_user_id,h.project_id,h.manuscript_object_id,h.current_revision_id) JOIN storyos.authoritative_payloads p ON (p.owner_user_id,p.project_id,p.payload_id)=(r.owner_user_id,r.project_id,r.payload_id) WHERE h.owner_user_id='${user}' AND h.project_id='${projectId}' AND h.manuscript_object_id='${chapters[2]}'`));
      const stored = rows[0].stored;
      let decoded; try { decoded = JSON.parse(stored); } catch { decoded = stored; }
      const database = typeof decoded === 'string' ? decoded : Array.isArray(decoded) ? decoded.map(b => b.text).join('\n') : decoded.blocks?.map(b => b.text).join('\n');
      observations.push({ label, expected, dom, server, stored, database, exact_equal: dom === database && dom === server && dom === expected });
    }
    await capture('insert');
    for (const [target, replacement] of [['𠀀', '界'], ['e\u0301', 'é'], ['👩‍👩‍👧‍👦', '👩🏽‍🚀'], ['，。！？', '「好」']]) {
      budget();
      const previous = await page.locator('[data-save-state]').getAttribute('data-authoritative-revision-id');
      await editor.evaluate((e, target) => {
        const node = e.lastElementChild.firstChild, start = node.textContent.lastIndexOf(target);
        e.focus(); const r = document.createRange(); r.setStart(node, start); r.setEnd(node, start + target.length);
        const s = getSelection(); s.removeAllRanges(); s.addRange(r);
      }, target);
      await page.keyboard.insertText(replacement);
      const start = expected.lastIndexOf(target);
      expected = expected.slice(0, start) + replacement + expected.slice(start + target.length);
      await page.waitForFunction(previous => document.querySelector('[data-save-state]')?.getAttribute('data-authoritative-revision-id') !== previous, previous);
      await saved(); await capture(`replace ${target} with ${replacement}`);
    }
    await open(); await saved(); await capture('reload');
    writeFileSync(resolve(out, 'coordinates.json'), JSON.stringify({ probe, utf16_units: probe.length, scalars: [...probe].length, observations }, null, 2));
    budget();
    await page.locator('[name="assistant-message"]').fill('Revise this passage: keep the voice.');
    const admittedResponse = page.waitForResponse(r => r.request().method() === 'POST' && r.url().endsWith('/agent-runs'));
    await page.locator('[data-writing-assistant-composer] button[type="submit"]').click();
    const admitted = await (await admittedResponse).json();
    worker();
    const inspect = page.locator('[data-assistant-inspect]');
    await inspect.last().waitFor();
    writeFileSync(resolve(out, 'web-proposal-setup.json'), JSON.stringify({ run_id: admitted.effect.run_id }));
    await sample('proposal-open', async () => {
      await inspect.last().click();
      await page.locator('[data-proposal-accept]').last().waitFor();
    });
    await sample('proposal-accept', async () => {
      const previous = await page.locator('[data-save-state]').getAttribute('data-authoritative-revision-id');
      await page.locator('[data-proposal-accept]').last().click();
      await page.waitForFunction(previous => document.querySelector('[data-save-state]')?.getAttribute('data-authoritative-revision-id') !== previous, previous);
      await saved();
    });
    let newVolume, newChapter;
    await sample('create-volume', async () => {
      await page.locator('[data-add-chapter]').click(); await page.locator('[data-create-volume-action]').click();
      await page.locator('[name="volume-title"]').fill('浏览器临时卷'); await page.locator('[name="volume-title"]').press('Enter');
      const row = page.locator('li[data-volume-id]').filter({ has: page.locator('[data-volume-title]', { hasText: '浏览器临时卷' }) });
      await row.waitFor(); newVolume = await row.getAttribute('data-volume-id');
    });
    await sample('create-chapter', async () => {
      await page.locator(`[data-create-chapter-menu="${newVolume}"]`).click(); await page.locator('[data-chapter-placement="append"]').click();
      await page.locator('[name="chapter-title"]').fill('浏览器临时章'); await page.locator('[name="chapter-title"]').press('Enter');
      const row = page.locator('button[data-chapter-title]', { hasText: '浏览器临时章' });
      await row.waitFor(); newChapter = await row.getAttribute('data-chapter-id');
    });
    await sample('reorder-chapter', async () => {
      const row = page.locator(`li[data-chapter-id="${chapters[1]}"]`), order = await row.getAttribute('data-chapter-order');
      await page.locator(`[data-chapter-menu="${chapters[1]}"]`).click(); await page.locator('[data-chapter-move="down"]').click();
      await page.waitForFunction(({ id, order }) => document.querySelector(`li[data-chapter-id="${id}"]`)?.getAttribute('data-chapter-order') !== order, { id: chapters[1], order });
    });
    await sample('reorder-volume', async () => {
      const order = await page.locator(`li[data-volume-id="${newVolume}"]`).getAttribute('data-volume-order');
      await page.locator(`[data-create-chapter-menu="${newVolume}"]`).click(); await page.locator('[data-volume-move="up"]').click();
      await page.waitForFunction(({ id, order }) => document.querySelector(`li[data-volume-id="${id}"]`)?.getAttribute('data-volume-order') !== order, { id: newVolume, order });
    });
    await sample('delete-chapter', async () => {
      await page.locator(`[data-chapter-menu="${newChapter}"]`).click(); await page.locator(`[data-delete-chapter="${newChapter}"]`).click();
      await page.locator(`[data-confirm-delete-chapter="${newChapter}"]`).click(); await page.locator(`li[data-chapter-id="${newChapter}"]`).waitFor({ state: 'detached' });
    });
    await sample('delete-volume', async () => {
      await page.locator(`[data-create-chapter-menu="${newVolume}"]`).click(); await page.locator(`[data-delete-volume="${newVolume}"]`).click();
      await page.locator(`[data-confirm-delete-volume="${newVolume}"]`).click(); await page.locator(`li[data-volume-id="${newVolume}"]`).waitFor({ state: 'detached' });
    });
    await sample('export', async () => {
      await page.locator('[data-readable-export] > button').click(); await page.locator('[data-readable-export] [data-export-id]').waitFor();
      worker(); await page.locator('[data-export-outcome="ready"]').waitFor();
    });
    await sample('session-recovery-after-structure', async () => { await open(); await saved(); });
  } catch (error) {
    writeFileSync(resolve(out, 'browser-error.json'), JSON.stringify({ error: String(error), body: await page.locator('body').innerText() }, null, 2));
    await page.screenshot({ path: resolve(out, 'browser-error.png') });
    throw error;
  } finally { await browser.close(); }
}
