// Sends one author message in a new Conversation of the Chapter 春网 in the packaged Web client.
// usage: PLAYWRIGHT_MODULE=<...> node panel.mjs <projectId> r0           (admit "准备检索。" and cancel it)
//        PLAYWRIGHT_MODULE=<...> node panel.mjs <projectId> ask <name> <message>   (wait, then screenshot)
import { writeFileSync } from "node:fs";

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const [projectId, mode, name, message] = process.argv.slice(2);
const origin = "http://127.0.0.1:8750";
const out = (file) => new URL(`./${file}`, import.meta.url).pathname;
const browser = await chromium.launch({ channel: "chrome" });
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
await context.addCookies([{ name: "storyos_session", value: "session-a", url: origin }]);
const page = await context.newPage();
try {
  await page.goto(`${origin}/projects/${projectId}`);
  const entry = page.getByRole("navigation", { name: "稿件目录" }).getByRole("button", { name: "春网", exact: true });
  await entry.click();
  await page.getByRole("heading", { name: "春网", exact: true }).waitFor();
  const takeOver = page.getByRole("button", { name: "接管写作" });
  if (await takeOver.isVisible()) {
    await takeOver.click();
    await takeOver.waitFor({ state: "hidden" });
    await entry.click();
    await page.getByRole("heading", { name: "春网", exact: true }).waitFor();
  }
  // With no earlier Conversation, the first message starts a new one, and the control is disabled.
  const newConversation = page.getByRole("button", { name: "新对话" }).first();
  if (await newConversation.isEnabled()) await newConversation.click();
  const composer = page.getByRole("textbox", { name: "给写作助手的消息" });
  await composer.fill(mode === "r0" ? "准备检索。" : message);
  await page.getByRole("button", { name: "发送" }).click();
  if (mode === "r0") {
    await page.getByRole("button", { name: "取消任务" }).click();
    await page.getByText("任务已取消。").waitFor({ timeout: 30000 });
    await page.screenshot({ path: out("r0.png") });
    console.log(new Date().toISOString(), "r0 cancelled");
  } else {
    await page.getByRole("button", { name: "取消任务" }).waitFor();
    await page.getByRole("button", { name: "取消任务" }).waitFor({ state: "hidden", timeout: 600000 });
    await page.waitForTimeout(3000);
    const panel = page.getByRole("complementary", { name: "写作助手对话" });
    writeFileSync(out(`${name}-panel.txt`), await panel.innerText());
    await panel.screenshot({ path: out(`${name}-panel.png`) });
    console.log(new Date().toISOString(), name, "settled");
  }
} catch (error) {
  await page.screenshot({ path: out("failure.png") });
  console.error(String(error).split("\n").slice(0, 4).join("\n"));
  process.exitCode = 1;
} finally {
  await browser.close();
}
