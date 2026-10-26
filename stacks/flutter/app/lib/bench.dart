// Harness: CLI parsing, frame-timing collection, JSON output.
import 'dart:async';
import 'dart:convert';
import 'dart:developer' show Timeline;
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/scheduler.dart';

class Cfg {
  String scenario = 'idle';
  double duration = 10;
  String out = '/tmp/flutter-bench.json';
  int rate = 1000;
  int shapes = 5000;
  String assets = '/home/user/eval/assets';
  String core = '/home/user/eval/core/target/release/libbenchcore.so';
  bool durationGiven = false;

  static Cfg parse(List<String> a) {
    final c = Cfg();
    for (var i = 0; i < a.length; i++) {
      String v() => a[++i];
      switch (a[i]) {
        case '--scenario': c.scenario = v();
        case '--duration': c.duration = double.parse(v()); c.durationGiven = true;
        case '--out': c.out = v();
        case '--rate': c.rate = int.parse(v());
        case '--shapes': c.shapes = int.parse(v());
        case '--assets': c.assets = v();
        case '--core': c.core = v();
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
  int ticks = 0; // ticker callbacks inside the measured window
  // [vsyncStart, buildStart, buildFinish, rasterStart, rasterFinish]
  final List<List<int>> frames = [];
  final List<void Function()> startHooks = [];
  final List<void Function(Map<String, Object?> ops)> endHooks = [];
  final Map<String, Object?> ops = {};
  final List<String> extraNotes = [];
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

  void _startMeasure() {
    startUs = Timeline.now;
    measuring = true;
    for (final h in startHooks) {
      h();
    }
    Timer(Duration(microseconds: (cfg.duration * 1e6).round()), _endMeasure);
  }

  void _endMeasure() {
    endUs = Timeline.now;
    measuring = false;
    for (final h in endHooks) {
      h(ops);
    }
    // FrameTiming reports are batched by the engine: wait for the tail.
    Timer(const Duration(milliseconds: 800), _finish);
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
    ops['total_span_ms'] = stats([for (final f in inW) (f[4] - f[1]) / 1000.0]);
    ops['vsync_to_rasterfinish_ms'] = stats([for (final f in inW) (f[4] - f[0]) / 1000.0]);
    ops['clock_check_main_to_firstframe_ms'] =
        frames.isEmpty ? null : (frames.first[4] - mainUs) / 1000.0;
    if (finishExtra != null) ops.addAll(finishExtra!(inW));
    final impeller = Platform.environment['FLUTTER_ENGINE_SWITCH_1'] ?? 'default-backend';
    final notes = 'Flutter Dart-AOT release, Linux GTK embedder. '
        'frame_ms = interval between consecutive FrameTiming rasterFinish timestamps (engine raster thread finished the frame, '
        'frame handed to the GTK/GL presenter; NOT a guaranteed screen-update timestamp; no visibility into X11 present). '
        'Only frames the engine actually rasterized are reported (a frame with no dirty layer is never produced, so no empty frames are counted); '
        'continuous redraw by a Ticker that dirties the scene every tick. Frames filtered to rasterFinish within the measured window '
        '[start,end] after a 2 s warm-up; timings are batched by the engine so 800 ms extra were waited. '
        'Engine switches env: $impeller. ${extraNotes.join(' ')}';
    final doc = {
      'stack': 'flutter',
      'scenario': cfg.scenario,
      'duration_s': cfg.duration,
      'frames': inW.length,
      'frame_ms': fm,
      'ops': ops,
      'checksum': (ops['checksum'] ?? '0').toString(),
      'notes': notes,
    };
    File(cfg.out).writeAsStringSync(jsonEncode(doc));
    exit(0);
  }
}
