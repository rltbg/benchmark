// usage : node web-run.mjs <dossier-statique> <outdir>
// Sert le dossier (MIME application/wasm), ouvre Chrome headless sur /?autorun=1, journalise la console, capture a chaque SPIKE:shot.
import { chromium } from 'playwright-core';
import http from 'node:http'; import fs from 'node:fs'; import path from 'node:path';
const [dir, out] = process.argv.slice(2); fs.mkdirSync(out, { recursive: true });
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.css': 'text/css', '.dat': 'application/octet-stream', '.png': 'image/png', '.ico': 'image/x-icon' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(req.url.split('?')[0]); if (p.endsWith('/')) p += 'index.html';
  const f = path.join(dir, p);
  if (!f.startsWith(path.resolve(dir)) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end('404'); return; }
  res.writeHead(200, { 'Content-Type': mime[path.extname(f)] || 'application/octet-stream' }); fs.createReadStream(f).pipe(res);
});
await new Promise(r => server.listen(0, '127.0.0.1', r)); const port = server.address().port;
const browser = await chromium.launch({ executablePath: process.env.CHROME || '/usr/bin/google-chrome', headless: true,
  args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-precise-memory-info'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
const lines = []; const t0 = Date.now(); let readyMs = null; let done = false; const shots = new Set(); const pending = [];
const log = fs.createWriteStream(path.join(out, 'console.log'));
page.on('console', m => {
  const t = m.text(); log.write(`[${Date.now() - t0}] ${m.type()}: ${t}\n`);
  if (t.startsWith('SPIKE:')) {
    lines.push(t);
    if (t.startsWith('SPIKE:ready') && readyMs === null) readyMs = Date.now() - t0;
    if (t.startsWith('SPIKE:done')) done = true;
    const m2 = t.match(/^SPIKE:shot (\S+)/);
    if (m2 && !shots.has(m2[1])) { shots.add(m2[1]); pending.push((async () => { await new Promise(r => setTimeout(r, 300)); await page.screenshot({ path: path.join(out, m2[1] + '.png') }); })()); }
  }
});
page.on('pageerror', e => log.write(`[${Date.now() - t0}] PAGEERROR: ${e.message}\n`));
page.on('requestfailed', r => log.write(`[${Date.now() - t0}] REQFAILED: ${r.url()} ${r.failure()?.errorText}\n`));
await page.goto(`http://127.0.0.1:${port}/?autorun=1`);
const deadline = Date.now() + 170000;
while (!done && Date.now() < deadline) await new Promise(r => setTimeout(r, 500));
await Promise.all(pending);
let heap = null;
try { heap = await page.evaluate(() => ({ used: performance.memory?.usedJSHeapSize, total: performance.memory?.totalJSHeapSize })); } catch (e) { heap = { error: String(e) }; }
let cdp = null;
try { const c = await page.context().newCDPSession(page); await c.send('Performance.enable'); cdp = Object.fromEntries((await c.send('Performance.getMetrics')).metrics.map(x => [x.name, x.value])); } catch (e) { cdp = { error: String(e) }; }
await page.screenshot({ path: path.join(out, 'final.png') });
fs.writeFileSync(path.join(out, 'run.log'), lines.join('\n') + '\n');
fs.writeFileSync(path.join(out, 'metrics.json'), JSON.stringify({ ready_ms_from_navigation: readyMs, done, wall_ms: Date.now() - t0, js_heap: heap, cdp }, null, 1));
console.log(JSON.stringify({ readyMs, done, heap }));
await browser.close(); server.close();
