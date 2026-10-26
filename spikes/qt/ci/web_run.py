#!/usr/bin/env python3
"""Sert le dossier wasm (en-têtes COOP/COEP pour le multithread) et le charge dans Chrome headless (Playwright, Chrome système).
usage: web_run.py <dossier-wasm> <page.html> <dossier-sortie> <étiquette>
Écrit run-<étiquette>.log (console du navigateur), <étiquette>-tab*.png, run-<étiquette>.json."""
import http.server, json, os, socketserver, sys, threading, time, functools
from playwright.sync_api import sync_playwright

root, page, out, label = sys.argv[1:5]
os.makedirs(out, exist_ok=True)

class H(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()
    def log_message(self, *a): pass
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

socketserver.TCPServer.allow_reuse_address = True
srv = socketserver.ThreadingTCPServer(("127.0.0.1", 0), functools.partial(H, directory=root))
port = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()

lines, t_ready, t_done = [], None, None
result = {"label": label}
with sync_playwright() as p:
    b = p.chromium.launch(channel="chrome", headless=True, args=[
        "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--enable-precise-memory-info"])
    ctx = b.new_context(viewport={"width": 960, "height": 640})
    pg = ctx.new_page()
    t0 = time.time()
    def on_console(m):
        global t_ready, t_done
        txt = m.text
        lines.append(f"[{m.type}] {txt}")
        if "SPIKE:ready" in txt and t_ready is None: t_ready = time.time() - t0
        if "SPIKE:done" in txt and t_done is None: t_done = time.time() - t0
    pg.on("console", on_console)
    pg.on("pageerror", lambda e: lines.append(f"[pageerror] {e}"))
    pg.goto(f"http://127.0.0.1:{port}/{page}?autorun=1")
    result["cross_origin_isolated"] = pg.evaluate("window.crossOriginIsolated")
    shots = {}
    def wait_marker(sub, timeout):
        end = time.time() + timeout
        while time.time() < end:
            if any(sub in l for l in lines): return True
            time.sleep(0.1)
        return False
    ok_ready = wait_marker("SPIKE:ready", 90)
    if ok_ready:
        time.sleep(2.5);  pg.screenshot(path=f"{out}/{label}-tab1-list.png")
        time.sleep(3.0);  pg.screenshot(path=f"{out}/{label}-tab2-canvas.png")
        wait_marker("SPIKE:heavy done", 30)
        time.sleep(0.5);  pg.screenshot(path=f"{out}/{label}-tab3-core.png")
    ok_done = wait_marker("SPIKE:done", 30)
    try:
        result["js_heap"] = pg.evaluate("({used: performance.memory.usedJSHeapSize, total: performance.memory.totalJSHeapSize})")
    except Exception as e:
        result["js_heap_error"] = str(e)
    result.update(ready_s=t_ready, done_s=t_done, ok_ready=ok_ready, ok_done=ok_done)
    b.close()
open(f"{out}/run-{label}.log", "w").write("\n".join(lines) + "\n")
json.dump(result, open(f"{out}/run-{label}.json", "w"), indent=1)
print(json.dumps(result))
print("\n".join(l for l in lines if "SPIKE:" in l or "pageerror" in l or "[error]" in l)[:4000])
sys.exit(0 if result.get("ok_done") else 1)
