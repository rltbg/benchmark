// Canvas scenario: N moving shapes (CustomPainter) + real-time 10k-point graph.
//
// Shapes only translate (fixed size, fixed per-shape gradient, fixed shadow), so the default
// renderer pre-renders every shape - with its own linear gradient and, for 1 shape in 10, its
// blurred drop shadow underneath - once into a sprite atlas (Picture.toImageSync) and draws the
// whole set each frame with ONE Canvas.drawRawAtlas call, in the original z-order, inside the
// rounded clip. This is the idiom documented for "many parts of an image" / sprites
// (api.flutter.dev Canvas.drawAtlas) and the "precalculate a static shape and cache it" advice
// of docs.flutter.dev/perf/best-practices. BENCH_CANVAS_MODE=direct (diagnostic) draws every
// shape with drawRect/drawCircle/drawRRect + gradient shader + MaskFilter.blur every frame.
import 'dart:io' show Platform;
import 'dart:math' as m;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

import 'model.dart' show fnv;

final bool canvasDirect = Platform.environment['BENCH_CANVAS_MODE'] == 'direct';
final int _diag = int.tryParse(Platform.environment['BENCH_DIAG'] ?? '') ?? 0; // TEMP

class Scene extends ChangeNotifier {
  final int n;
  final double ox, oy, rw, rh; // clip rrect (origin + size)
  final bool shadows, graph;
  final double gx, gy, gw, gh; // graph zone
  static const int graphPoints = 10000;
  static const double blurSigma = 4.0; // "flou 8 px" -> 8 px blur radius ~ 2 sigma

  late final Float32List x = Float32List(n),
      y = Float32List(n),
      vx = Float32List(n),
      vy = Float32List(n),
      sw = Float32List(n),
      sh = Float32List(n);
  late final Uint8List kind = Uint8List(n);
  late final List<Paint> fills;
  late final List<RRect> rrects;
  late final Uint8List hasShadow = Uint8List(n);
  final Paint shadowPaint = Paint()
    ..color = const Color(0x88000000)
    ..maskFilter = const MaskFilter.blur(BlurStyle.normal, blurSigma);
  final Paint bgPaint = Paint()..color = const Color(0xFF161B22);
  final Paint linePaint = Paint()
    ..color = const Color(0xFF58A6FF)
    ..style = PaintingStyle.stroke
    ..strokeWidth = 1.0;
  final Paint graphBg = Paint()..color = const Color(0xFF0D1117);
  final Paint atlasPaint = Paint()..filterQuality = FilterQuality.low;
  final Float32List pts = Float32List(graphPoints * 2);
  final Float32List ring = Float32List(graphPoints);
  int head = 0, k = 0;
  double lastT = 0;
  late final RRect clipRR;

  // atlas mode
  ui.Image? atlas;
  late final Float32List spriteRects = Float32List(4 * n); // l t r b in the atlas
  late final Float32List padLT = Float32List(n); // sprite origin offset vs. shape origin
  late final Float32List xforms = Float32List(4 * n); // RSTransform scos ssin tx ty
  int atlasW = 0, atlasH = 0;
  double atlasBuildMs = 0;

  Scene({
    required this.n,
    required this.ox,
    required this.oy,
    required this.rw,
    required this.rh,
    required this.shadows,
    required this.graph,
    this.gx = 0,
    this.gy = 0,
    this.gw = 0,
    this.gh = 0,
  }) {
    clipRR = RRect.fromRectAndRadius(Rect.fromLTWH(ox, oy, rw, rh), const Radius.circular(16));
    fills = List<Paint>.generate(n, (_) => Paint(), growable: false);
    rrects = List<RRect>.filled(n, RRect.zero, growable: false);
    for (var i = 0; i < n; i++) {
      final f = fnv(i * 5 + 1), g = fnv(i * 5 + 2), h = fnv(i * 5 + 3);
      kind[i] = i % 3;
      final w = 16.0 + (f % 33);
      final hh = kind[i] == 1 ? w : 16.0 + (g % 33);
      sw[i] = w;
      sh[i] = hh;
      x[i] = (h % 1000) / 1000.0 * (rw - w);
      y[i] = ((h >> 10) % 1000) / 1000.0 * (rh - hh);
      final sp = 60.0 + (fnv(i * 5 + 4) % 200);
      final ang = (fnv(i * 5 + 5) % 3600) / 3600.0 * 6.283185307;
      vx[i] = sp * m.cos(ang);
      vy[i] = sp * m.sin(ang);
      final c1 = Color(0xFF000000 | (f & 0xFFFFFF));
      final c2 = Color(0xFF000000 | (g & 0xFFFFFF));
      // one Paint + Shader per shape, created once (gradient in shape-local coordinates)
      fills[i].shader = ui.Gradient.linear(Offset.zero, Offset(w, hh), [c1, c2]);
      rrects[i] = RRect.fromRectAndRadius(Rect.fromLTWH(0, 0, w, hh), const Radius.circular(8));
      hasShadow[i] = (shadows && i % 10 == 0) ? 1 : 0;
      xforms[4 * i] = 1.0; // scos (scale 1, no rotation); ssin stays 0
    }
    if (!canvasDirect) _buildAtlas();
  }

  /// Paints shape i (and its shadow) with its local origin at the current canvas origin.
  void _paintShape(Canvas c, int i) {
    final w = sw[i], h = sh[i];
    if (hasShadow[i] != 0) {
      switch (kind[i]) {
        case 0:
          c.drawRect(Rect.fromLTWH(2, 2, w, h), shadowPaint);
        case 1:
          c.drawCircle(Offset(w / 2 + 2, w / 2 + 2), w / 2, shadowPaint);
        default:
          c.drawRRect(rrects[i].shift(const Offset(2, 2)), shadowPaint);
      }
    }
    switch (kind[i]) {
      case 0:
        c.drawRect(Rect.fromLTWH(0, 0, w, h), fills[i]);
      case 1:
        c.drawCircle(Offset(w / 2, w / 2), w / 2, fills[i]);
      default:
        c.drawRRect(rrects[i], fills[i]);
    }
  }

  void _buildAtlas() {
    final sw0 = Stopwatch()..start();
    // shelf packing; shadow margin = offset 2 + 3 sigma, 2 px transparent gutter against bleeding
    const gutter = 2.0;
    final maxW = n > 2000 ? 4096.0 : 2048.0;
    var cx = 0.0, cy = 0.0, rowH = 0.0;
    final ox0 = Float32List(n), oy0 = Float32List(n);
    for (var i = 0; i < n; i++) {
      final lt = hasShadow[i] != 0 ? 3 * blurSigma - 2 + 1 : 1.0; // left/top margin
      final rb = hasShadow[i] != 0 ? 3 * blurSigma + 2 + 1 : 1.0; // right/bottom margin
      final spw = sw[i] + lt + rb, sph = sh[i] + lt + rb;
      if (cx + spw > maxW) {
        cx = 0;
        cy += rowH + gutter;
        rowH = 0;
      }
      spriteRects[4 * i] = cx;
      spriteRects[4 * i + 1] = cy;
      spriteRects[4 * i + 2] = cx + spw;
      spriteRects[4 * i + 3] = cy + sph;
      padLT[i] = lt;
      ox0[i] = cx + lt;
      oy0[i] = cy + lt;
      cx += spw + gutter;
      rowH = m.max(rowH, sph);
    }
    atlasW = maxW.toInt();
    atlasH = (cy + rowH).ceil();
    final rec = ui.PictureRecorder();
    final c = Canvas(rec);
    for (var i = 0; i < n; i++) {
      c.save();
      c.translate(ox0[i], oy0[i]);
      _paintShape(c, i);
      c.restore();
    }
    final pic = rec.endRecording();
    atlas = pic.toImageSync(atlasW, atlasH);
    pic.dispose();
    atlasBuildMs = sw0.elapsedMicroseconds / 1000.0;
  }

  void tick(double nowSec) {
    final dt = lastT == 0 ? 0.0 : (nowSec - lastT).clamp(0.0, 0.1);
    lastT = nowSec;
    for (var i = 0; i < n; i++) {
      var px = x[i] + vx[i] * dt, py = y[i] + vy[i] * dt;
      final mx = rw - sw[i], my = rh - sh[i];
      if (px < 0) {
        px = -px;
        vx[i] = -vx[i];
      } else if (px > mx) {
        px = 2 * mx - px;
        vx[i] = -vx[i];
      }
      if (py < 0) {
        py = -py;
        vy[i] = -vy[i];
      } else if (py > my) {
        py = 2 * my - py;
        vy[i] = -vy[i];
      }
      x[i] = px;
      y[i] = py;
    }
    if (graph) {
      k++;
      final v = 50.0 + 35.0 * m.sin(k * 0.031) + 10.0 * m.sin(k * 0.173) + (fnv(k) % 100) / 25.0;
      ring[head] = v;
      head = (head + 1) % graphPoints;
      final step = gw / (graphPoints - 1);
      var j = 0;
      for (var p = 0; p < graphPoints; p++) {
        final idx = (head + p) % graphPoints;
        pts[j++] = p * step;
        pts[j++] = gh - ring[idx] * gh / 100.0;
      }
    }
    notifyListeners();
  }

  void paint(Canvas c) {
    c.save();
    if (_diag & 2 == 0) c.clipRRect(clipRR);
    c.drawRRect(clipRR, bgPaint);
    final a = atlas;
    if (a != null) {
      for (var i = 0; i < n; i++) {
        xforms[4 * i + 2] = ox + x[i] - padLT[i];
        xforms[4 * i + 3] = oy + y[i] - padLT[i];
      }
      c.drawRawAtlas(a, xforms, spriteRects, null, null, clipRR.outerRect, atlasPaint);
    } else {
      for (var i = 0; i < n; i++) {
        c.save();
        c.translate(ox + x[i], oy + y[i]);
        _paintShape(c, i);
        c.restore();
      }
    }
    c.restore();
    if (graph) {
      c.save();
      c.translate(gx, gy);
      c.drawRect(Rect.fromLTWH(0, 0, gw, gh), graphBg);
      c.clipRect(Rect.fromLTWH(0, 0, gw, gh));
      if (_diag & 1 == 0) c.drawRawPoints(ui.PointMode.polygon, pts, linePaint);
      if (_diag & 4 != 0) linePaint.strokeWidth = 0;
      c.restore();
    }
  }
}

class ScenePainter extends CustomPainter {
  final Scene s;
  // repaint: the Scene (a Listenable) triggers paint directly, skipping build and layout
  // (CustomPainter doc, "The most efficient way to trigger a repaint").
  ScenePainter(this.s) : super(repaint: s);
  @override
  void paint(Canvas canvas, Size size) => s.paint(canvas);
  @override
  bool shouldRepaint(covariant ScenePainter old) => !identical(old.s, s);
}
