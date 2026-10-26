# Flutter (Linux desktop, GTK3 embedder, Dart AOT) - notes

## Versions
- Flutter 3.47.8 (stable, framework 737ba8f885, engine cf636b1dfb), Dart 3.13.7, DevTools 2.60.0 (SDK = `git clone --depth 1 -b stable` into `sdk/`, 3 s).
- Engine/Dart SDK artifacts downloaded from storage.googleapis.com and packages from pub.dev: **no blocked host encountered**. Packages: `ffi 2.2.0` (only direct dep besides flutter), vector_math 2.4.0, characters 1.4.1, material_color_utilities 0.13.0 (see logs/pubspec.lock).
- Analytics disabled: `flutter config --no-analytics`, `dart --disable-analytics`, `FLUTTER_SUPPRESS_ANALYTICS=true`. Flutter warns "running as root" (harmless).
- Build: `/home/user/eval/stacks/flutter/build.sh` (= `flutter pub get` + `flutter build linux --release -v`, `CMAKE_BUILD_PARALLEL_LEVEL=2 NINJAFLAGS=-j2`). Wall time **27.3 s** for the first app build (SDK artifacts already cached; max RSS 416 MB, logs/build-run1.out), 16 s for later rebuilds (logs/build-release.log = last build). The gen_snapshot AOT step is single-threaded. No compiler/analyzer warnings (`flutter analyze`: no issues).
- Run (harness): `--cmd /home/user/eval/stacks/flutter/app/build/linux/x64/release/bundle/benchapp` (bundle must stay intact, the binary is a 23 KB GTK launcher). Args reach `main(List<String>)` (verified; the runner template passes argv through FlDartProject). `exit(0)` after writing JSON works (rc=0).
- `smoke.sh SCENARIO DURATION TAG [args]` = my own Xvfb (random :200-:599) smoke test, outputs in logs/smoke-*.json.

## Bundle (build/linux/x64/release/bundle) - 21 659 001 bytes total
| file | bytes |
|---|---|
| benchapp (launcher) | 23 840 |
| lib/libflutter_linux_gtk.so (engine) | 17 256 536 |
| lib/libapp.so (AOT snapshot, **not** data/app.so in this version) | 3 376 008 |
| data/icudtl.dat | 864 880 |
| data/flutter_assets (NOTICES.Z 98 130, 2 unused shaders, manifests) | ~140 000 |
System libs from `ldd` (GTK3 stack: libgtk-3 8.2 MB, gdk, pango, cairo, harfbuzz, epoxy, fontconfig, X11/Xi/..., glib, ...) full list with sizes in logs/env-and-bundle.txt. Runtime needs GTK3 + libepoxy + a GL (Mesa llvmpipe here) installed.
Idle RSS (harness): ~249 MB, idle CPU mean 3.5 % of a core (peak 38 % at start-up). RSS during list: 267-282 MB.

## Rendering backend (important)
- This Flutter version (3.47) **always uses Impeller (OpenGLES/SDF) on Linux**: log line `Using the Impeller rendering backend (OpenGLESSDF)` under Xvfb + llvmpipe (`LIBGL_ALWAYS_SOFTWARE=1`). Skia is not selectable: `--enable-impeller`/`--no-enable-impeller` are not accepted by the Linux embedder from the command line; `FLUTTER_ENGINE_SWITCHES=1 FLUTTER_ENGINE_SWITCH_1=enable-impeller=false` is ignored (still Impeller, tested). The `--no-enable-impeller` opt-out is flagged deprecated in the engine strings.
- The embedder has a CPU path `FLUTTER_LINUX_RENDERER=software` (Skia raster + Cairo; prints "Using the software renderer ... not recommended"). It **works for scenes without text** (canvas, core: tested) but **aborts as soon as text is drawn**: `[FATAL:flutter/display_list/skia/dl_sk_dispatcher.cc(328)] Check failed: blob. Impeller DlText cannot be drawn to a Skia canvas.` (list smoke: rc=134, logs/smoke-sw.stdout). So the complete suite can only run on the default Impeller-GLES/llvmpipe path; `--env FLUTTER_LINUX_RENDERER=software` is usable for canvas and core only (report it as a variant).

## Frame measurement (exact)
`SchedulerBinding.addTimingsCallback` records every `FrameTiming` (vsyncStart, buildStart/Finish, rasterStart/Finish). `frame_ms` = difference between consecutive `rasterFinish` timestamps of frames whose rasterFinish lies in the measured window (clock verified identical to `Timeline.now`). rasterFinish = engine raster thread finished and handed the frame to the GTK GL presenter - it is NOT an X11 present/screen-update timestamp. Only frames really rasterized are reported (Flutter produces no frame when nothing is dirty; continuous redraw = `Ticker` that dirties the scene every tick, content really changes each frame in all animated scenarios; during warm-up scripted time is frozen at 0 for list/stream/text/images). Engine batches timings: app waits 800 ms after the window end. `ops` also contains build_ms / raster_ms / total_span_ms / vsync_to_rasterfinish_ms {n,p50,p95,p99,max,mean}, `frames_ticks` (Ticker callbacks) and `max_frame_stall_ms`. Observed cadence (list/stream: p50 ~20-25 ms with only ~9 ms of build+raster, min ~4 ms) suggests a ~60 Hz vsync-like pacing by the GTK embedder that cannot be disabled; I did not remove it ("no vsync pacing" of the spec is not achievable by the app).

## Scenario status (smoke 8-10 s, Impeller-GLES/llvmpipe, 4 vCPU shared)
| scenario | status | smoke result |
|---|---|---|
| idle | OK | 0 frames, RSS 249 MB, CPU 3.5 % |
| list | OK | sum_heights=8799584; ~28-40 fps, raster p50 7.7 ms/p95 21 ms, build p50 0.9 ms |
| stream (rate 10000) | OK | ~50 fps; generated 80148, applied 80142, dropped 0 |
| canvas 5000 shapes | OK functionally, **unplayable**: 1 frame in 8 s (raster 7.4 s/frame) | see below |
| core | OK functionally: calls_per_s ~4.5e7, heavy_calls 8/8 s; UI (1000 gradient shapes) 14 frames in 8 s (raster 580 ms) | software renderer variant: 49 fps |
| stress | OK functionally, **unplayable**: 1 frame in 10 s (raster 4.4 s) | |
| text | OK (partial semantics) | ~29 fps; time_to_first_display 260 ms, layout_ms 14.7 |
| images | OK | 600 decoded, ~35 fps |
| nav | OK | ~19 fps, 21 transitions + 5 modals in 10 s |

### Why canvas/stress are so slow (diagnosed with the `FB_VARIANT` env, diagnostic only, default 0 = spec)
Impeller-GLES on llvmpipe: gradient-filled draws cost ~0.5 ms each (1000 shapes: 580 ms/frame; solid colour fill `FB_VARIANT=1`: 57-65 ms; no clip (2) does not help; one single shared gradient Paint (8) is just as slow -> per-draw gradient cost, not cache misses). Also the 10 000-point 1 px polyline (drawRawPoints polygon, CPU tessellation of the stroke) alone costs ~80 ms raster per frame (canvas --shapes 10 solid: 11.7 fps). 5000 solid shapes + shadows + graph: 8.5 fps. With `FLUTTER_LINUX_RENDERER=software` (Skia CPU) canvas 5000 shapes = 8.4 fps (raster p50 105 ms). These are characteristics of the Linux engine on a software GL, not of the Dart code (build p50 < 8 ms). The spec-compliant variant (gradients) is what the binary runs by default.

## Implementation (lib/, 1256 lines of Dart; runner C++ untouched, template)
- bench.dart: CLI parse, timings, JSON; model.dart: fnv, heights, prefix sums, RowItem, price stream isolate, FFI workers; scene.dart: canvas; scenarios.dart: Host(list/stream/canvas/core/stress), text, images, nav.
- list: `ListView.builder` with real per-row heights (SizedBox(height)); `itemExtent`/`prototypeItem` not applicable (variable heights). (Flutter 3.24+ has `itemExtentBuilder`, which is O(index) per layout on this version - not used, brief asked for the builder.) Scroll position set with `ScrollController.jumpTo` from the closed-form script (4 s @3000 px/s + 6 flings, v0*exp(-2t)), clamped. Rows: circle, title, subtitle (repeated, clipped by maxLines), price `PriceCell` (registered State; stream update -> setState of the visible cell only). ops: rows_drawn = sum over ticks of visible rows (binary search on prefix sums), rows_built = itemBuilder invocations.
- stream: producer `Isolate` (xorshift32 seed 0x2545F491, index=x%100000, price=x/100) paced by a 0.7 ms sleep loop, sends `Uint32List` batches (one per ~1 ms, i.e. 1-10 updates) over a `SendPort`; bounded 100k in-flight window enforced producer-side via a shared-memory (dart:ffi calloc) applied counter acked by the UI; UI drains the queue once per frame. generated/dropped/applied are shared-memory counters (deltas over the window).
- canvas: one `CustomPainter` (repaint = Scene listenable, RepaintBoundary), per-shape `Paint` with a cached `ui.Gradient.linear` in local coordinates (save/translate/draw/restore), shadow = `MaskFilter.blur(normal, sigma 4)` offset (2,2) on 1 shape in 10, `clipRRect` 1000x600, graph = `drawRawPoints(PointMode.polygon)` 10 000 points updated per frame (one point added per frame).
- core: 2 worker Isolates; tight loop `core_validate+core_parse+core_hash` via `dart:ffi` leaf calls on the 8-byte "hello" frame (A5 01 05 'hello'), counter in shared memory; second isolate calls `core_heavy(100)` every 1 s; UI animates 1000 gradient shapes, no shadow/graph ("allege").
- stress: list(880 px) + stream 5000/s + canvas 5000 shapes in a 400x720 side panel (clip 380x600, graph 380x100) + core workers. `heap_samples` every 10 s contain `rss_kb` (ProcessInfo.currentRss) but `managed_heap_kb: null` (VM service / heap stats not available in product AOT).
- text: no virtualized huge-text widget in Flutter; the file is split into 9599 lines, `ListView.builder` of `Text` inside `SelectableRegion` (selectAll at t=5 s via `SelectableRegionState.selectAll()`, only built lines get selected - approximation). Scroll: triangle wave over 0..15000 px at 3000 px/s. time_to_first_display_ms measured from main() (excludes VM/engine start-up ~1.4 s to first frame incl. GTK/GL init); layout_ms = read+split + build duration of first frame (Flutter lays out lazily). Emoji/CJK/bidi text rendering was not visually inspected (no screenshot taken).
- images: custom loader (`File.readAsBytes` + `ui.instantiateImageCodec(targetWidth/Height 128)`, decode on engine worker threads, at most 4 loads in flight), `GridView.builder` 10 columns, scroll triangle wave at 3000 px/s, zoom 1->3->1 over 6 s via `Transform` with sinusoidal pan. decode_total_ms (~37 s for 600 in the last smoke) is a sum of wall times under heavy CPU contention (llvmpipe), not pure decode cost.
- nav: `Navigator` + `PageRouteBuilder` slide 300 ms, `RawDialogRoute` fade 200 ms, cycles push B, push C, pop, pop (x5 = 20 transitions) + modal after each cycle, repeated until the duration ends (smoke: transitions_done counts all); busy Isolate spins an integer loop (~1 core). Screens use plain solid-colour rounded cards (an earlier version with gradient+shadow cards gave 195 ms raster/frame on llvmpipe; changed because the spec does not define screen content).

## Deviations / limits
- Frame boundary = rasterFinish (see above), vsync pacing present, Skia unavailable, text scenarios impossible on the software renderer.
- Stream producer sends batches, not one message per update.
- Window: GTK default size 1280x720 (runner template), no window manager in Xvfb -> no decorations; Flutter shows the window after first frame. Atk/GSettings CRITICAL messages on stderr are harmless (no a11y bus / gsettings schema).
- Dart `Isolate.spawn` workers rather than OS threads; shared counters through native memory.
- Diagnostics env `FB_VARIANT` (bit 1 solid fill, 2 no clip, 8 shared gradient) exists in the binary but is off by default.
- Not run: official measurements, manual IME protocol, screenshot validation of rendering.
