// Harness side: CLI parsing, portable paths, measure markers, frame timings, JSON output.
import 'dart:async';
import 'dart:convert';
import 'dart:developer' show Timeline;
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/scheduler.dart';

/// Versions injected by the flutter tool as dart-defines (FLUTTER_VERSION etc.).
const String kFlutterVersion = String.fromEnvironment('FLUTTER_VERSION', defaultValue: '?');
const String kFlutterFw = String.fromEnvironment('FLUTTER_FRAMEWORK_REVISION', defaultValue: '?');
const String kFlutterEngine = String.fromEnvironment('FLUTTER_ENGINE_REVISION', defaultValue: '?');
const String kFfiVersion = String.fromEnvironment('BENCH_FFI_VERSION', defaultValue: '?');

/// Renderer chosen by the runner (linux/runner/my_application.cc) through the documented
/// fl_dart_project_set_enable_impeller() API; BENCH_FLUTTER_IMPELLER=0 selects Skia (OpenGL).
bool get impellerEnabled => Platform.environment['BENCH_FLUTTER_IMPELLER'] != '0';

/// Repository root: $BENCH_ROOT, else the first ancestor of the executable containing core/benchcore.h.
String benchRoot() {
  final env = Platform.environment['BENCH_ROOT'];
  if (env != null && env.isNotEmpty) return env;
  var d = File(Platform.resolvedExecutable).parent;
  while (true) {
    if (File('${d.path}/core/benchcore.h').existsSync()) return d.path;
    final p = d.parent;
    if (p.path == d.path) return Directory.current.path;
    d = p;
  }
}

/// C-ABI core: $BENCHCORE_LIB, else $BENCH_ROOT/core/target/release/libbenchcore.so.
String coreLibPath() {
  final env = Platform.environment['BENCHCORE_LIB'];
  if (env != null && env.isNotEmpty) return env;
  return '${benchRoot()}/core/target/release/libbenchcore.so';
}

class Cfg {
  String scenario = 'idle';
  double duration = 10;
  String out = 'flutter-bench.json';
  int rate = 1000;
  int shapes = 5000;
  String? assetsArg;
  bool durationGiven = false;

  String get assets => assetsArg ?? '${benchRoot()}/assets';

  static Cfg parse(List<String> a) {
    final c = Cfg();
    for (var i = 0; i < a.length; i++) {
      String v() => a[++i];
      switch (a[i]) {
        case '--scenario':
          c.scenario = v();
        case '--duration':
          c.duration = double.parse(v());
          c.durationGiven = true;
        case '--out':
          c.out = v();
        case '--rate':
          c.rate = int.parse(v());
        case '--shapes':
          c.shapes = int.parse(v());
        case '--assets':
          c.assetsArg = v();
      }
    }
    if (c.scenario == 'stress' && !c.durationGiven) c.duration = 600;
    return c;
  }
}

double pct(List<double> s, double p) {
  if (s.isEmpty) return 0;
  final k = ((p / 100 * s.length).ceil() - 1).clamp(0, s.length - 1);
  return s[k];
}

Map<String, double> stats(List<double> v) {
  final s = List<double>.from(v)..sort();
  if (s.isEmpty) return {'n': 0};
  return {
    'n': s.length.toDouble(),
    'p50': pct(s, 50),
    'p95': pct(s, 95),
    'p99': pct(s, 99),
    'max': s.last,
    'mean': s.reduce((a, b) => a + b) / s.length,
  };
}

class Bench {
  static late Bench I;
  final Cfg cfg;
  final int mainUs = Timeline.now;
  int startUs = 0, endUs = 0;
  bool measuring = false;
  bool ended = false;
  int ticks = 0; // ticker callbacks inside the measured window
  // [vsyncStart, buildStart, buildFinish, rasterStart, rasterFinish]
  final List<List<int>> frames = [];
  final List<void Function()> startHooks = [];
  final List<void Function(Map<String, Object?> ops)> endHooks = [];
  final Map<String, Object?> ops = {};
  final List<String> extraNotes = [];
  Object? unsupported;
  Map<String, Object?> Function(List<List<int>> inWindow)? finishExtra;

  Bench(this.cfg);

  /// Seconds since measure start (0 during warm-up).
  double get t => measuring ? (Timeline.now - startUs) / 1e6 : 0.0;

  void begin() {
    SchedulerBinding.instance.addTimingsCallback(_onTimings);
    Timer(const Duration(seconds: 2), _startMeasure);
  }

  void _onTimings(List<ui.FrameTiming> ts) {
    for (final f in ts) {
      frames.add([
        f.timestampInMicroseconds(ui.FramePhase.vsyncStart),
        f.timestampInMicroseconds(ui.FramePhase.buildStart),
        f.timestampInMicroseconds(ui.FramePhase.buildFinish),
        f.timestampInMicroseconds(ui.FramePhase.rasterStart),
        f.timestampInMicroseconds(ui.FramePhase.rasterFinish),
      ]);
    }
  }

  Future<void> _marker(String m) async {
    stdout.writeln(m);
    await stdout.flush();
  }

  Future<void> _startMeasure() async {
    await _marker('BENCH:measure_start');
    startUs = Timeline.now;
    measuring = true;
    for (final h in startHooks) {
      h();
    }
    Timer(Duration(microseconds: (cfg.duration * 1e6).round()), _endMeasure);
  }

  Future<void> _endMeasure() async {
    endUs = Timeline.now;
    measuring = false;
    ended = true;
    for (final h in endHooks) {
      h(ops);
    }
    await _marker('BENCH:measure_end');
    // FrameTiming is batched in release mode and delivered "within about 1 second"
    // (PlatformDispatcher.onReportTimings doc): wait until a timing past the window
    // has arrived, at most 1.5 s.
    final sw = Stopwatch()..start();
    while (sw.elapsedMilliseconds < 1500 && !frames.any((f) => f[4] > endUs)) {
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    _finish();
  }

  void _finish() {
    final inW = frames.where((f) => f[4] >= startUs && f[4] <= endUs).toList()
      ..sort((a, b) => a[4].compareTo(b[4]));
    final fm = <double>[];
    for (var i = 1; i < inW.length; i++) {
      fm.add((inW[i][4] - inW[i - 1][4]) / 1000.0);
    }
    ops['frames_ticks'] = ticks;
    ops['build_ms'] = stats([for (final f in inW) (f[2] - f[1]) / 1000.0]);
    ops['raster_ms'] = stats([for (final f in inW) (f[4] - f[3]) / 1000.0]);
    ops['vsync_to_rasterfinish_ms'] = stats([for (final f in inW) (f[4] - f[0]) / 1000.0]);
    // Dart: no public GC statistics API in product (AOT release) mode, the VM service is absent.
    ops['gc'] = null;
    // Spec v2.1 pacing: the native cadence is the engine's 60 Hz vsync (fallback waiter of the
    // Linux embedder); there is no public API to uncap it, so `uncapped` degrades to "60-only".
    final pacingReq = Platform.environment['BENCH_PACING'] ?? '60';
    ops['pacing'] = pacingReq == 'uncapped' ? '60-only' : '60';
    if (finishExtra != null) ops.addAll(finishExtra!(inW));
    final renderer = impellerEnabled ? 'Impeller (OpenGL ES backend, default)' : 'Skia (OpenGL), via fl_dart_project_set_enable_impeller(FALSE)';
    final notes = 'Flutter Dart-AOT release, Linux GTK3 embedder, renderer: $renderer, GL = Mesa (llvmpipe under Xvfb). '
        'frame_ms = interval between consecutive FrameTiming rasterFinish timestamps (SchedulerBinding.addTimingsCallback; '
        'raster thread finished the frame and handed it to the GTK GL compositor; latest public post-render signal, not an X11 present timestamp). '
        'Only frames the engine rasterized are counted (Flutter produces no frame when nothing is dirty; every animated frame changes content). '
        'Pacing (BENCH_PACING, requested=${Platform.environment['BENCH_PACING'] ?? '60'}): native Flutter cadence = Ticker/SchedulerBinding frames driven by the engine vsync; '
        'the Linux GTK embedder registers no vsync callback, so the engine uses its fallback vsync waiter with a fixed 1/60 s tick '
        '(engine source shell/common/vsync_waiter_fallback.cc). No public/documented API uncaps it, so uncapped -> pacing "60-only". '
        'gc = null: Dart exposes no GC counters in AOT release. Warm-up 2 s; script time frozen at 0 during warm-up. '
        '${extraNotes.join(' ')}';
    final doc = {
      'stack': 'flutter',
      'version': 'Flutter $kFlutterVersion (framework $kFlutterFw, engine $kFlutterEngine), Dart ${Platform.version.split(' ').first}, '
          'package:ffi $kFfiVersion, ${impellerEnabled ? 'Impeller-GLES' : 'Skia-GL'}',
      'scenario': cfg.scenario,
      'duration_s': cfg.duration,
      'frames': inW.length,
      'frame_ms': fm,
      'ops': ops,
      'checksum': (ops['checksum'] ?? '0').toString(),
      'notes': notes,
      'unsupported': unsupported,
    };
    File(cfg.out).writeAsStringSync(jsonEncode(doc));
    exit(0);
  }
}
