// Scenario widgets: list / stream / canvas / core / stress (Host), text, images, nav.
import 'dart:async';
import 'dart:developer' show Timeline;
import 'dart:ffi' show Uint64Pointer;
import 'dart:io';
import 'dart:math' as m;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import 'bench.dart';
import 'model.dart';
import 'scene.dart';

const _bg = Color(0xFF0D1117);
const _bar = Color(0xFF21262D);

String hexOf(int v) => (v & 0xFFFFFFFF).toRadixString(16);

Widget _titleBar(String s) => Container(
    height: 36,
    color: _bar,
    padding: const EdgeInsets.symmetric(horizontal: 12),
    alignment: Alignment.centerLeft,
    child: Text(s, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.bold)));

Widget _statusBar(String s) => Container(
    height: 24,
    color: _bar,
    padding: const EdgeInsets.symmetric(horizontal: 12),
    alignment: Alignment.centerLeft,
    child: Text(s, style: const TextStyle(fontSize: 12, color: Color(0xFF9AA4B2))));

/// Scripted scroll offset: 4 s at 3000 px/s, then 6 alternating flings
/// (v0 = 20000 px/s, v(t) = v0*exp(-2t), 1.5 s each), then rest.
double scriptedOffset(double t) {
  if (t <= 0) return 0;
  if (t < 4) return 3000 * t;
  var off = 12000.0;
  var tt = t - 4;
  for (var k = 0; k < 6 && tt > 0; k++) {
    final dir = k.isEven ? 1.0 : -1.0;
    final d = tt < 1.5 ? tt : 1.5;
    off += dir * 20000 / 2 * (1 - m.exp(-2 * d));
    tt -= 1.5;
  }
  return off;
}

// ===================================================================== Host
class Host extends StatefulWidget {
  final Cfg cfg;
  const Host(this.cfg, {super.key});
  @override
  State<Host> createState() => _HostState();
}

class _HostState extends State<Host> with SingleTickerProviderStateMixin {
  late final Cfg cfg = widget.cfg;
  late final String sc = cfg.scenario;
  late final bool hasList = const {'idle', 'list', 'stream', 'stress', 'input'}.contains(sc);
  late final bool hasInput = sc == 'input';
  final ValueNotifier<bool> squareWhite = ValueNotifier(false);
  final FocusNode focus = FocusNode(debugLabel: 'bench');
  int keyEvents = 0, keyEvents0 = 0;
  late final bool animated = sc != 'idle';
  late final bool hasStream = sc == 'stream' || sc == 'stress';
  late final bool hasCore = sc == 'core' || sc == 'stress';
  final ScrollController ctrl = ScrollController();
  PriceStream? ps;
  Scene? scene;
  Ticker? ticker;
  final sw = Stopwatch()..start();
  double viewH = 660;
  int rowsDrawn = 0, rowsBuilt0 = 0, gen0 = 0, drop0 = 0, app0 = 0, calls0 = 0, heavy0 = 0;
  final List<Map<String, Object?>> heap = [];
  Timer? heapTimer;

  @override
  void initState() {
    super.initState();
    final b = Bench.I;
    if (hasStream) ps = PriceStream(sc == 'stress' ? 5000 : cfg.rate);
    if (hasCore) CoreWorkers.start(coreLibPath());
    if (sc == 'canvas') {
      scene = Scene(
          n: cfg.shapes, ox: 20, oy: 10, rw: 1000, rh: 600, shadows: true, graph: true,
          gx: 20, gy: 615, gw: 1000, gh: 100);
    } else if (sc == 'core') {
      scene = Scene(n: 1000, ox: 20, oy: 10, rw: 1000, rh: 600, shadows: false, graph: false);
    } else if (sc == 'stress') {
      scene = Scene(
          n: cfg.shapes, ox: 10, oy: 10, rw: 380, rh: 600, shadows: true, graph: true,
          gx: 10, gy: 615, gw: 380, gh: 100);
      viewH = 660;
      heapTimer = Timer.periodic(const Duration(seconds: 10), (_) {
        heap.add({
          't': b.measuring ? b.t : -1,
          'rss_kb': ProcessInfo.currentRss ~/ 1024,
          'managed_heap_kb': null,
        });
      });
    }
    if (animated) ticker = createTicker(_tick)..start();
    b.startHooks.add(() {
      rowsDrawn = 0;
      rowsBuilt0 = rowsBuilt;
      gen0 = shm[4];
      drop0 = shm[5];
      app0 = shm[3];
      calls0 = shm[0];
      heavy0 = shm[2];
      keyEvents0 = keyEvents;
    });
    b.endHooks.add((ops) {
      final secs = (b.endUs - b.startUs) / 1e6;
      ops['sum_heights'] = rowTops[kRows];
      if (hasList) {
        final off = ctrl.hasClients ? ctrl.offset : 0.0;
        ops['final_offset'] = off;
        ops['rows_drawn'] = rowsDrawn;
        ops['rows_built'] = rowsBuilt - rowsBuilt0;
        ops['checksum'] = hexOf(fnv(off.round()));
      }
      if (hasStream) {
        ops['updates_generated'] = shm[4] - gen0;
        ops['updates_dropped'] = shm[5] - drop0;
        ops['updates_applied'] = shm[3] - app0;
        ops['rate'] = sc == 'stress' ? 5000 : cfg.rate;
      }
      if (hasInput) {
        ops['key_events'] = keyEvents - keyEvents0;
        ops['square_white'] = squareWhite.value;
      }
      if (scene != null) {
        ops['shapes'] = scene!.n;
        ops['not_rendered'] = <String>[];
        ops['canvas_mode'] = canvasDirect ? 'direct' : 'atlas';
        if (!canvasDirect) {
          ops['atlas_px'] = [scene!.atlasW, scene!.atlasH];
          ops['atlas_pages'] = scene!.pages.length;
          ops['atlas_record_ms'] = scene!.atlasBuildMs;
        }
        ops['checksum'] = hexOf(fnv((scene!.x[0] * 100).round() ^ (scene!.y[scene!.n - 1] * 100).round()));
      }
      if (hasCore) {
        ops['calls_per_s'] = (shm[0] - calls0) / secs;
        ops['heavy_calls'] = shm[2] - heavy0;
        ops['core_lib'] = coreLibPath();
        ops['core_acc'] = shm[8];
      }
      if (sc == 'stress') ops['heap_samples'] = heap;
      ps?.stop();
      if (hasCore) CoreWorkers.stop();
    });
    b.finishExtra = (inW) {
      var mx = 0.0;
      for (var i = 1; i < inW.length; i++) {
        mx = m.max(mx, (inW[i][4] - inW[i - 1][4]) / 1000.0);
      }
      return {'max_frame_stall_ms': mx};
    };
    if (sc == 'stress') {
      b.extraNotes.add('managed_heap_kb is null: the Dart VM service/heap stats are not available in product AOT release; rss_kb (ProcessInfo.currentRss, whole process) is given instead.');
    }
    if (hasList) {
      b.extraNotes.add('list = ListView.builder(itemExtentBuilder: known per-row heights h(i)) (SliverVariedExtentList); '
          'scroll set by ScrollController.jumpTo in a Ticker callback from the closed-form script.');
    }
    if (hasInput) {
      b.extraNotes.add('input: Focus(autofocus) onKeyEvent, KeyDownEvent of LogicalKeyboardKey.space toggles a 200x200 square at (100,100) '
          'black/white (ValueListenableBuilder -> ColoredBox), drawn over the scrolling list; key repeats ignored.');
    }
    if (scene != null) {
      b.extraNotes.add(canvasDirect
          ? 'canvas (BENCH_CANVAS_MODE=direct): per shape drawRect/drawCircle/drawRRect with its own gradient Paint, MaskFilter.blur(sigma 4) shadow at (2,2) for 1 in 10, clipRRect 1000x600 r16, graph drawRawPoints(polygon) 10000 pts stroke 1 px.'
          : 'canvas: every shape (own 2-colour linear gradient, and for 1 in 10 its MaskFilter.blur(sigma 4) shadow at offset (2,2)) is recorded once into a sprite atlas '
              '(PictureRecorder + Picture.toImageSync, ${scene!.pages.length} page(s) ${scene!.atlasW} px wide, ${scene!.atlasH} px total height, sprites assigned to pages in index order); each frame = one Canvas.drawRawAtlas per page, pages in order = shapes in z-order '
              '(translation-only RSTransforms, bilinear sampling) inside clipRRect 1000x600 r16; graph drawRawPoints(polygon) 10000 pts stroke 1 px rebuilt per frame.');
    }
  }

  void _tick(Duration _) {
    final b = Bench.I;
    if (b.measuring) b.ticks++;
    if (hasList && ctrl.hasClients) {
      final maxOff = rowTops[kRows] - viewH;
      final off = scriptedOffset(b.t).clamp(0.0, maxOff.toDouble());
      if (off != ctrl.offset) ctrl.jumpTo(off);
      if (b.measuring) rowsDrawn += rowAtOffset(off + viewH) - rowAtOffset(off) + 1;
    }
    ps?.drain();
    scene?.tick(sw.elapsedMicroseconds / 1e6);
  }

  @override
  void dispose() {
    ticker?.dispose();
    heapTimer?.cancel();
    focus.dispose();
    super.dispose();
  }

  Widget _list(double w) {
    return SizedBox(
      width: w,
      child: Column(children: [
        _titleBar('Benchmark - $sc'),
        Expanded(
          child: ListView.builder(
            controller: ctrl,
            physics: const ClampingScrollPhysics(),
            itemCount: kRows,
            // heights are known in advance: the documented idiom for varied extents
            itemExtentBuilder: (i, _) => i < kRows ? rowHeights[i].toDouble() : null,
            itemBuilder: (c, i) => RowItem(i),
          ),
        ),
        _statusBar('$kRows rows - ${hasStream ? "streaming" : "static"}'),
      ]),
    );
  }

  @override
  Widget build(BuildContext context) {
    Widget panel(double w, double h) => RepaintBoundary(
        child: SizedBox(width: w, height: h, child: CustomPaint(painter: ScenePainter(scene!))));
    switch (sc) {
      case 'canvas':
      case 'core':
        return ColoredBox(color: _bg, child: panel(1280, 720));
      case 'stress':
        return ColoredBox(
            color: _bg,
            child: Row(children: [_list(880), SizedBox(width: 400, child: panel(400, 720))]));
      case 'input':
        return Focus(
          focusNode: focus,
          autofocus: true,
          onKeyEvent: (node, e) {
            if (e is KeyDownEvent && e.logicalKey == LogicalKeyboardKey.space) {
              keyEvents++;
              squareWhite.value = !squareWhite.value;
              return KeyEventResult.handled;
            }
            return e.logicalKey == LogicalKeyboardKey.space ? KeyEventResult.handled : KeyEventResult.ignored;
          },
          child: ColoredBox(
            color: _bg,
            child: Stack(children: [
              _list(1280),
              Positioned(
                left: 100,
                top: 100,
                width: 200,
                height: 200,
                child: ValueListenableBuilder<bool>(
                  valueListenable: squareWhite,
                  builder: (c, white, _) =>
                      ColoredBox(color: white ? const Color(0xFFFFFFFF) : const Color(0xFF000000)),
                ),
              ),
            ]),
          ),
        );
      default:
        return ColoredBox(color: _bg, child: _list(1280));
    }
  }
}

// ===================================================================== text
class TextScenario extends StatefulWidget {
  final Cfg cfg;
  const TextScenario(this.cfg, {super.key});
  @override
  State<TextScenario> createState() => _TextState();
}

class _TextState extends State<TextScenario> with SingleTickerProviderStateMixin {
  late final List<String> lines;
  final ScrollController ctrl = ScrollController();
  final GlobalKey<SelectableRegionState> key = GlobalKey();
  final FocusNode focus = FocusNode();
  late final Ticker ticker;
  double readMs = 0;
  int firstBuildUs = 0;
  bool selected = false;

  @override
  void initState() {
    super.initState();
    final b = Bench.I;
    final sw = Stopwatch()..start();
    lines = File('${widget.cfg.assets}/text1mb.txt').readAsStringSync().split('\n');
    readMs = sw.elapsedMicroseconds / 1000.0;
    ticker = createTicker((_) {
      if (b.measuring) b.ticks++;
      final t = b.t;
      if (ctrl.hasClients) {
        final p = (3000 * t) % 30000;
        final off = p < 15000 ? p : 30000 - p;
        if (off != ctrl.offset) ctrl.jumpTo(off.toDouble());
        // when idle at t=0 (warm-up) still dirty the scene with a no-op jump? no: nothing changes
      }
      if (!selected && t >= 5) {
        selected = true;
        key.currentState?.selectAll();
      }
    })..start();
    b.endHooks.add((ops) {
      ops['final_offset'] = ctrl.hasClients ? ctrl.offset : 0.0;
      ops['lines'] = lines.length;
      ops['select_all_done'] = selected;
      // checked on the verify_gui capture: CJK, Hangul, Hebrew/Arabic (RTL) and accents render;
      // emoji come out as monochrome DejaVu Sans glyphs (fontconfig fallback order) and the
      // ZWJ family sequence is not composed into one glyph.
      ops['not_rendered'] = ['colour emoji (monochrome glyphs)', 'emoji ZWJ sequence composition'];
      ops['checksum'] = hexOf(fnv(lines.length ^ (ctrl.hasClients ? ctrl.offset.round() : 0)));
    });
    b.finishExtra = (inW) {
      final r = <String, Object?>{};
      for (final f in b.frames) {
        if (f[4] >= firstBuildUs) {
          r['time_to_first_display_ms'] = (f[4] - b.mainUs) / 1000.0;
          r['layout_ms'] = readMs + (f[2] - f[1]) / 1000.0;
          break;
        }
      }
      r['read_split_ms'] = readMs;
      return r;
    };
    b.extraNotes.add('text: Flutter has no virtualized single big text view; the file is split into lines (ListView.builder of Text) inside SelectableRegion. '
        'time_to_first_display_ms = main() entry -> rasterFinish of the first frame that built the text (excludes engine/VM startup before main). '
        'layout_ms = file read+split + buildDuration of that first frame (Flutter lays out lazily, only the viewport). '
        'Scroll = triangle wave 0..15000 px at 3000 px/s. Select-all at t=5 s via SelectableRegionState.selectAll(): with a lazy list only built lines are selected.');
  }

  @override
  void dispose() {
    ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (firstBuildUs == 0) firstBuildUs = Timeline.now;
    return ColoredBox(
      color: _bg,
      child: SelectableRegion(
        key: key,
        focusNode: focus,
        selectionControls: emptyTextSelectionControls,
        child: ListView.builder(
          controller: ctrl,
          itemCount: lines.length,
          itemBuilder: (c, i) => Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 2),
            child: Text(lines[i].isEmpty ? ' ' : lines[i], style: const TextStyle(fontSize: 14)),
          ),
        ),
      ),
    );
  }
}

// ===================================================================== images
/// Decode statistics. Images are shown with the documented idiom
/// Image(image: ResizeImage(FileImage(f), width: 128, height: 128)): the engine decodes them
/// at 128x128 on its own image-decoding threads (not the UI isolate) and ImageCache keeps them.
class ImageStats {
  final Set<int> decoded = {};
  final Map<int, int> requestedUs = {};
  double decodeMs = 0;
}

class ImageCell extends StatelessWidget {
  final int i;
  final String dir;
  final ImageStats st;
  const ImageCell(this.i, this.dir, this.st, {super.key});

  @override
  Widget build(BuildContext context) {
    st.requestedUs.putIfAbsent(i, () => Timeline.now);
    return Image(
      image: ResizeImage(FileImage(File('$dir/images/img${i.toString().padLeft(3, '0')}.png')),
          width: 128, height: 128),
      width: 128,
      height: 128,
      fit: BoxFit.cover,
      filterQuality: FilterQuality.low,
      gaplessPlayback: true,
      frameBuilder: (c, child, frame, sync) {
        if (frame != null && st.decoded.add(i)) {
          st.decodeMs += (Timeline.now - st.requestedUs[i]!) / 1000.0;
        }
        return frame == null ? const ColoredBox(color: Color(0xFF21262D)) : child;
      },
    );
  }
}

class ImagesScenario extends StatefulWidget {
  final Cfg cfg;
  const ImagesScenario(this.cfg, {super.key});
  @override
  State<ImagesScenario> createState() => _ImagesState();
}

class _ImagesState extends State<ImagesScenario> with SingleTickerProviderStateMixin {
  final ImageStats store = ImageStats();
  late final String dir = widget.cfg.assets;
  final ScrollController ctrl = ScrollController();
  late final Ticker ticker;
  final ValueNotifier<Matrix4> xf = ValueNotifier(Matrix4.identity());

  @override
  void initState() {
    super.initState();
    final b = Bench.I;
    ticker = createTicker((_) {
      if (b.measuring) b.ticks++;
      final t = b.t;
      if (ctrl.hasClients && ctrl.position.hasContentDimensions) {
        final mx = ctrl.position.maxScrollExtent;
        if (mx > 0) {
          final p = (3000 * t) % (2 * mx);
          final off = p < mx ? p : 2 * mx - p;
          if (off != ctrl.offset) ctrl.jumpTo(off);
        }
      }
      var s = 1.0;
      if (t > 0 && t < 6) s = 1 + 2 * (1 - (t - 3).abs() / 3);
      final dx = (s - 1) * 120 * m.sin(t * 2), dy = (s - 1) * 60 * m.cos(t * 1.5);
      xf.value = Matrix4.identity()
        ..translateByDouble(640.0 + dx, 360.0 + dy, 0.0, 1.0)
        ..scaleByDouble(s, s, 1.0, 1.0)
        ..translateByDouble(-640.0, -360.0, 0.0, 1.0);
    })..start();
    b.endHooks.add((ops) {
      ops['images_decoded'] = store.decoded.length;
      ops['decode_total_ms'] = store.decodeMs;
      ops['final_offset'] = ctrl.hasClients ? ctrl.offset : 0.0;
      ops['checksum'] = hexOf(fnv(store.decoded.length));
    });
    b.extraNotes.add('images: Image(ResizeImage(FileImage, width 128, height 128)) per cell (decode at 128x128 by the engine off the UI isolate, kept in ImageCache). '
        'decode_total_ms = sum over images of wall time from first build of the cell to its first frame (file read + decode + scheduling, incl. CPU contention with llvmpipe); counters cover the whole run incl. warm-up. '
        'GridView.builder 10 columns x 128 px; scroll = triangle wave over the grid at 3000 px/s; zoom 1->3->1 over 6 s (Transform) with a sinusoidal pan; during warm-up the script is frozen at t=0.');
  }

  @override
  void dispose() {
    ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ColoredBox(
        color: _bg,
        child: ClipRect(
          child: ValueListenableBuilder<Matrix4>(
            valueListenable: xf,
            builder: (c, mtx, child) => Transform(transform: mtx, child: child),
            child: GridView.builder(
              controller: ctrl,
              gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(crossAxisCount: 10),
              itemCount: 600,
              itemBuilder: (c, i) => ImageCell(i, dir, store),
            ),
          ),
        ),
      );
}

// ===================================================================== nav
final GlobalKey<NavigatorState> navKey = GlobalKey<NavigatorState>();

class NavScreen extends StatelessWidget {
  final int n;
  const NavScreen(this.n, {super.key});
  @override
  Widget build(BuildContext context) {
    final base = [const Color(0xFF1F6FEB), const Color(0xFF8957E5), const Color(0xFF238636)][n % 3];
    return ColoredBox(
      color: _bg,
      child: Column(children: [
        _titleBar('Screen ${n + 1}'),
        Expanded(
          child: ListView.builder(
            itemCount: 60,
            itemBuilder: (c, i) => Container(
              height: 64,
              margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(10),
                color: base,
              ),
              child: Text('Card $i on screen ${n + 1}', style: const TextStyle(fontSize: 16)),
            ),
          ),
        ),
      ]),
    );
  }
}

Future<void> _waitStatus(Animation<double> a, AnimationStatus s) {
  if (a.status == s) return Future.value();
  final c = Completer<void>();
  void l(AnimationStatus st) {
    if (st == s) {
      a.removeStatusListener(l);
      c.complete();
    }
  }

  a.addStatusListener(l);
  return c.future;
}

class NavScenario extends StatefulWidget {
  final Cfg cfg;
  const NavScenario(this.cfg, {super.key});
  @override
  State<NavScenario> createState() => _NavState();
}

class _NavState extends State<NavScenario> {
  int transitions = 0, modals = 0, batches = 0;
  bool running = false;

  @override
  void initState() {
    super.initState();
    final b = Bench.I;
    startBusy();
    b.startHooks.add(() {
      running = true;
      _drive();
    });
    b.endHooks.add((ops) {
      running = false;
      shm[7] = 1;
      ops['transitions_done'] = transitions;
      ops['modals_done'] = modals;
      ops['batches_of_20'] = batches;
      ops['checksum'] = hexOf(fnv(transitions * 1000 + modals));
    });
    b.extraNotes.add('nav: Navigator with PageRouteBuilder slide (300 ms) and RawDialogRoute fade (200 ms); script = push B, push C, pop, pop (4 transitions) x5 = 20 per batch, '
        'one modal open+close after every cycle; batches repeat until the duration ends. Frames exist only while animating (no idle frames). '
        'A background Dart isolate spins an integer loop (~1 core).');
  }

  PageRouteBuilder<void> _slide(int n) => PageRouteBuilder<void>(
        transitionDuration: const Duration(milliseconds: 300),
        reverseTransitionDuration: const Duration(milliseconds: 300),
        pageBuilder: (c, a, s) => NavScreen(n),
        transitionsBuilder: (c, a, s, child) => SlideTransition(
          position: Tween(begin: const Offset(1, 0), end: Offset.zero)
              .animate(CurvedAnimation(parent: a, curve: Curves.easeOutCubic)),
          child: child,
        ),
      );

  Future<void> _push(int n) async {
    final r = _slide(n);
    navKey.currentState!.push(r);
    await Future<void>.delayed(Duration.zero);
    await _waitStatus(r.animation!, AnimationStatus.completed);
    transitions++;
  }

  Future<void> _pop() async {
    final nav = navKey.currentState!;
    // find top route's animation before popping
    late Animation<double> anim;
    nav.popUntil((route) {
      if (route is TransitionRoute) anim = route.animation!;
      return true;
    });
    nav.pop();
    await _waitStatus(anim, AnimationStatus.dismissed);
    transitions++;
  }

  Future<void> _modal() async {
    final r = RawDialogRoute<void>(
      transitionDuration: const Duration(milliseconds: 200),
      barrierColor: const Color(0x99000000),
      pageBuilder: (c, a, s) => Center(
          child: Container(
              width: 400,
              height: 240,
              decoration: BoxDecoration(color: _bar, borderRadius: BorderRadius.circular(16)),
              alignment: Alignment.center,
              child: const Text('Modal', style: TextStyle(fontSize: 24)))),
      transitionBuilder: (c, a, s, child) => FadeTransition(opacity: a, child: child),
    );
    navKey.currentState!.push(r);
    await Future<void>.delayed(Duration.zero);
    await _waitStatus(r.animation!, AnimationStatus.completed);
    navKey.currentState!.pop();
    await _waitStatus(r.animation!, AnimationStatus.dismissed);
    modals++;
  }

  Future<void> _drive() async {
    while (running) {
      for (var cyc = 0; cyc < 5 && running; cyc++) {
        await _push(1);
        if (!running) break;
        await _push(2);
        if (!running) break;
        await _pop();
        if (!running) break;
        await _pop();
        if (!running) break;
        await _modal();
      }
      if (running) batches++;
    }
  }

  @override
  Widget build(BuildContext context) => const NavScreen(0);
}
