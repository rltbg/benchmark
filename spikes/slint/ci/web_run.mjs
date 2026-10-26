// Charge la page dans Chrome headless, récupère la console (SPIKE:*), captures et tas JS.
import { chromium } from "playwright-core";
import fs from "node:fs";
const out = process.env.OUT;
const logs = [];
const browser = await chromium.launch({
  executablePath: process.env.CHROME,
  args: ["--no-sandbox", "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--enable-precise-memory-info"],
});
const page = await browser.newPage({ viewport: { width: 1024, height: 720 } });
page.on("console", (m) => logs.push(m.text()));
page.on("pageerror", (e) => logs.push("PAGEERROR " + e.message));
const t0 = Date.now();
await page.goto("http://127.0.0.1:8123/index.html?autorun=1");
const has = (p) => logs.some((l) => l.includes(p));
const wait = async (p, s) => { const end = Date.now() + s * 1000; while (Date.now() < end) { if (has(p)) return true; await new Promise((r) => setTimeout(r, 100)); } return false; };
const metrics = [];
if (await wait("SPIKE:ready", 90)) metrics.push(`cold_start_ms_to_ready=${Date.now() - t0}`);
const heap = async () => page.evaluate(() => performance.memory ? performance.memory.usedJSHeapSize : -1);
if (await wait("SPIKE:tab index=1", 20)) { await page.waitForTimeout(1500); await page.screenshot({ path: `${out}/shot-1-list.png` }); }
if (await wait("SPIKE:tab index=2", 20)) { await page.waitForTimeout(2000); await page.screenshot({ path: `${out}/shot-2-canvas.png` }); metrics.push(`js_heap_bytes_canvas=${await heap()}`); }
if (await wait("SPIKE:done", 30)) { await page.screenshot({ path: `${out}/shot-3-core.png` }); metrics.push(`js_heap_bytes_end=${await heap()}`); }
fs.writeFileSync(`${out}/run.log`, logs.join("\n") + "\n");
fs.writeFileSync(`${out}/run-metrics.txt`, metrics.join("\n") + "\n");
await browser.close();
console.log(metrics.join(" "));
