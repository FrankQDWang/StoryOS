// Types Chapters of the sample into the packaged Web client, as an author types them.
// The editor sends createChapter and applyAuthorEdit over HTTP. No SQL.
// usage: PLAYWRIGHT_MODULE=<apps/web/node_modules/playwright/index.mjs> node type-sample.mjs <projectId> <first> <last>
import { readFileSync } from "node:fs";

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const [projectId, first, last] = process.argv.slice(2);
const sample = JSON.parse(readFileSync(new URL("./sample.json", import.meta.url), "utf8"));
const origin = "http://127.0.0.1:8750";
const browser = await chromium.launch({ channel: "chrome" });
const context = await browser.newContext();
await context.addCookies([{ name: "storyos_session", value: "session-a", url: origin }]);
const page = await context.newPage();
try {
  await page.goto(`${origin}/projects/${projectId}`);
  await page.getByRole("button", { name: "目录菜单" }).waitFor();
  for (let number = Number(first); number <= Number(last); number += 1) {
    const chapter = sample[number - 1];
    const text = chapter.paragraphs.join("\n");
    const entry = page.getByRole("navigation", { name: "稿件目录" })
      .getByRole("button", { name: chapter.title, exact: true });
    if (await entry.count() === 0) {
      await page.getByRole("button", { name: "目录菜单" }).click();
      await page.getByRole("button", { name: "创建章" }).click();
      await page.getByPlaceholder("章标题").fill(chapter.title);
      await page.getByPlaceholder("章标题").press("Enter");
      await entry.waitFor();
    }
    // The Server allows 10 anti-forgery challenges for each Project in a minute, so a refused open waits.
    for (let attempt = 1; ; attempt += 1) {
      await entry.click();
      try {
        await page.getByRole("heading", { name: chapter.title, exact: true }).waitFor({ timeout: 5000 });
        break;
      } catch (error) {
        if (attempt === 4) throw error;
        await page.waitForTimeout(20000);
      }
    }
    const takeOver = page.getByRole("button", { name: "接管写作" });
    if (await takeOver.isVisible()) {
      await takeOver.click();
      await takeOver.waitFor({ state: "hidden" });
      await entry.click();
    }
    await page.getByRole("heading", { name: chapter.title, exact: true }).waitFor();
    await page.waitForTimeout(1000);
    const editor = page.locator(".manuscript-editor[contenteditable=true]");
    await editor.waitFor({ timeout: 10000 });
    await editor.click();
    if ((await editor.innerText()).trim() !== "") throw new Error(`Chapter ${number} is not empty`);
    for (const [index, paragraph] of chapter.paragraphs.entries()) {
      if (index > 0) await page.keyboard.press("Enter");
      await page.keyboard.insertText(paragraph);
    }
    await page.getByText(`本章 ${text.length} 字`).waitFor({ timeout: 30000 });
    await page.getByText("已保存", { exact: true }).waitFor({ timeout: 60000 });
    console.log(new Date().toISOString(), number, chapter.title, text.length, "saved");
    await page.waitForTimeout(30000);
  }
} catch (error) {
  await page.screenshot({ path: new URL("./failure.png", import.meta.url).pathname });
  console.error(String(error).split("\n").slice(0, 4).join("\n"));
  process.exitCode = 1;
} finally {
  await browser.close();
}
