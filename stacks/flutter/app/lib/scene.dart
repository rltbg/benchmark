// Canvas scenario: N moving shapes (CustomPainter) + real-time 10k-point graph.
import 'dart:io' show Platform;
import 'dart:math' as m;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

import 'model.dart' show fnv;

class Scene extends ChangeNotifier {
  final int n;
  final double ox, oy, rw, rh; // clip rrect (origin + size)
  final bool shadows, graph;
  final double gx, gy, gw, gh; // graph zone
  static const int graphPoints = 10000;

  late final Float32List x = Float32List(n),
      y = Float32List(n),
      vx = Float32List(n),
      vy = Float32List(n),
      sw = Float32List(n),
      sh = Float32List(n);
  late final Uint8List kind = Uint8List(n);
  late final List<Paint> fills = List.filled(n, Paint(), growable: false);
  late final List<RRect> rrects = List.filled(n, RRect.zero, growable: false);
  late final Uint8List hasShadow = Uint8List(n);
  final Paint shadowPaint = Paint()
    ..color = const Color(0x88000000)
    ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 4.0); // blur 8 px -> sigma 4
  final Paint bgPaint = Paint()..color = const Color(0xFF161B22);
  final Paint linePaint = Paint()
    ..color = const Color(0xFF58A6FF)
    ..style = PaintingStyle.stroke
    ..strokeWidth = 1.0;
  final Paint graphBg = Paint()..color = const Color(0xFF0D1117);
  final Float32List pts = Float32List(graphPoints * 2);
  final Float32List ring = Float32List(graphPoints);
  int head = 0, k = 0;
  double lastT = 0;
  late final RRect clipRR;
  Paint? _shared;
  // Diagnostic only (default 0 = spec): 1 solid fill instead of gradient, 2 no clip, 4 no save/translate per shape
  static final int variant = int.tryParse(Platform.environment['FB_VARIANT'] ?? '') ?? 0;

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
      vx[i] = sp * _cos(ang);
      vy[i] = sp * _sin(ang);
      final c1 = Color(0xFF000000 | (f & 0xFFFFFF));
      final c2 = Color(0xFF000000 | (g & 0xFFFFFF));
      fills[i] = (variant & 8) != 0
          ? (_shared ??= (Paint()..shader = ui.Gradient.linear(Offset.zero, const Offset(48, 48), [c1, c2])))
          : (variant & 1) != 0
          ? (Paint()..color = c1)
          : (Paint()..shader = ui.Gradient.linear(Offset.zero, Offset(w, hh), [c1, c2])); // cached once
      rrects[i] = RRect.fromRectAndRadius(Rect.fromLTWH(0, 0, w, hh), const Radius.circular(8));
      hasShadow[i] = (shadows && i % 10 == 0) ? 1 : 0;
    }
  }

  static double _sin(double a) => m.sin(a);
  static double _cos(double a) => m.cos(a);
  static double _t(double a) => m.sin(a);

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
      final v = 50.0 + 35.0 * _t(k * 0.031) + 10.0 * _t(k * 0.173) + (fnv(k) % 100) / 25.0;
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
    if ((variant & 2) == 0) c.clipRRect(clipRR);
    c.drawRRect(clipRR, bgPaint);
    for (var i = 0; i < n; i++) {
      c.save();
      c.translate(ox + x[i], oy + y[i]);
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
      c.restore();
    }
    c.restore();
    if (graph) {
      c.save();
      c.translate(gx, gy);
      c.drawRect(Rect.fromLTWH(0, 0, gw, gh), graphBg);
      c.clipRect(Rect.fromLTWH(0, 0, gw, gh));
      c.drawRawPoints(ui.PointMode.polygon, pts, linePaint);
      c.restore();
    }
  }
}

class ScenePainter extends CustomPainter {
  final Scene s;
  ScenePainter(this.s) : super(repaint: s);
  @override
  void paint(Canvas canvas, Size size) => s.paint(canvas);
  @override
  bool shouldRepaint(covariant ScenePainter old) => true;
}
