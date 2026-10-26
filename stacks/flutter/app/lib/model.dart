// Common data model, price stream (background isolate), core FFI workers.
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:flutter/widgets.dart' hide Size;

const int kRows = 100000;

int fnv(int i) {
  var h = 0x811C9DC5;
  for (var k = 0; k < 4; k++) {
    h ^= (i >> (8 * k)) & 0xFF;
    h = (h * 0x01000193) & 0xFFFFFFFF;
  }
  return h;
}

final Int32List rowHeights = () {
  final l = Int32List(kRows);
  for (var i = 0; i < kRows; i++) {
    l[i] = 48 + 16 * (fnv(i) % 6);
  }
  return l;
}();

/// cum[i] = top offset of row i (cum[kRows] = sum of heights).
final Int64List rowTops = () {
  final l = Int64List(kRows + 1);
  for (var i = 0; i < kRows; i++) {
    l[i + 1] = l[i] + rowHeights[i];
  }
  return l;
}();

int rowAtOffset(double off) {
  var lo = 0, hi = kRows - 1;
  while (lo < hi) {
    final mid = (lo + hi + 1) >> 1;
    if (rowTops[mid] <= off) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  return lo;
}

final Float64List prices = () {
  final l = Float64List(kRows);
  for (var i = 0; i < kRows; i++) {
    l[i] = (fnv(i + 7) % 100000) / 100.0;
  }
  return l;
}();

// ---------------------------------------------------------------- shared memory counters
// 0 core calls, 1 core stop, 2 heavy calls, 3 stream applied, 4 stream generated,
// 5 stream dropped, 6 stream stop, 7 busy-thread stop, 8 core accumulator (published result),
// 9 heavy accumulator
final Pointer<Uint64> shm = calloc<Uint64>(16);

// ---------------------------------------------------------------- rows
final Map<int, _PriceCellState> livePrices = {};
int rowsBuilt = 0;

class RowItem extends StatelessWidget {
  final int i;
  const RowItem(this.i, {super.key});
  @override
  Widget build(BuildContext context) {
    rowsBuilt++;
    final h = rowHeights[i];
    final f = fnv(i);
    final sub = 'Quote $i — ${(fnv(i + 7) % 100000) / 100.0}';
    final lines = ((h - 28) / 16).floor().clamp(1, 6);
    return SizedBox(
      height: h.toDouble(),
      child: DecoratedBox(
        decoration: const BoxDecoration(
          border: Border(bottom: BorderSide(color: Color(0xFF333A44), width: 1)),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Row(children: [
            Container(
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: Color.fromARGB(255, f & 255, (f >> 8) & 255, (f >> 16) & 255),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Item #$i',
                      maxLines: 1,
                      style: const TextStyle(fontSize: 15, fontWeight: FontWeight.bold)),
                  Text(List.filled(6, sub).join('  '),
                      maxLines: lines,
                      overflow: TextOverflow.clip,
                      style: const TextStyle(fontSize: 12, color: Color(0xFF9AA4B2))),
                ],
              ),
            ),
            SizedBox(width: 90, child: PriceCell(i)),
          ]),
        ),
      ),
    );
  }
}

class PriceCell extends StatefulWidget {
  final int i;
  const PriceCell(this.i, {super.key});
  @override
  State<PriceCell> createState() => _PriceCellState();
}

class _PriceCellState extends State<PriceCell> {
  @override
  void initState() {
    super.initState();
    livePrices[widget.i] = this;
  }

  @override
  void didUpdateWidget(PriceCell old) {
    super.didUpdateWidget(old);
    if (old.i != widget.i) {
      livePrices.remove(old.i);
      livePrices[widget.i] = this;
    }
  }

  @override
  void dispose() {
    if (livePrices[widget.i] == this) livePrices.remove(widget.i);
    super.dispose();
  }

  void touch() => setState(() {});

  @override
  Widget build(BuildContext context) => Text(
        prices[widget.i].toStringAsFixed(2),
        textAlign: TextAlign.right,
        style: const TextStyle(
            fontSize: 14, color: Color(0xFF7CE38B), fontFeatures: [FontFeature.tabularFigures()]),
      );
}

// ---------------------------------------------------------------- price stream
class PriceStream {
  final int rate;
  final ReceivePort _rx = ReceivePort();
  final List<Uint32List> _queue = [];
  int appliedLocal = 0;

  PriceStream(this.rate) {
    _rx.listen((m) => _queue.add(m as Uint32List));
    Isolate.spawn(_producer, [_rx.sendPort, rate, shm.address]);
  }

  /// Apply all pending updates (called once per frame on the UI isolate).
  void drain() {
    if (_queue.isEmpty) return;
    for (final chunk in _queue) {
      for (var k = 0; k < chunk.length; k++) {
        final x = chunk[k];
        final idx = x % kRows;
        prices[idx] = x / 100.0;
        livePrices[idx]?.touch();
      }
      appliedLocal += chunk.length;
    }
    _queue.clear();
    shm[3] = appliedLocal;
  }

  void stop() {
    shm[6] = 1;
  }
}

void _producer(List a) {
  final SendPort out = a[0];
  final int rate = a[1];
  final Pointer<Uint64> s = Pointer<Uint64>.fromAddress(a[2]);
  var x = 0x2545F491;
  var attempted = 0, enq = 0, dropped = 0;
  final sw = Stopwatch()..start();
  while (s[6] == 0) {
    final due = (sw.elapsedMicroseconds * rate / 1e6).floor() - attempted;
    if (due > 0) {
      final chunk = Uint32List(due);
      final inFlight = enq - s[3];
      if (inFlight + due > 100000) {
        // bounded queue (100k) full: drop and count, never block the producer
        for (var k = 0; k < due; k++) {
          x ^= (x << 13) & 0xFFFFFFFF;
          x ^= x >> 17;
          x ^= (x << 5) & 0xFFFFFFFF;
        }
        dropped += due;
      } else {
        for (var k = 0; k < due; k++) {
          x ^= (x << 13) & 0xFFFFFFFF;
          x ^= x >> 17;
          x ^= (x << 5) & 0xFFFFFFFF;
          chunk[k] = x;
        }
        out.send(chunk);
        enq += due;
      }
      attempted += due;
      s[4] = attempted;
      s[5] = dropped;
    }
    sleep(const Duration(microseconds: 700));
  }
}

// ---------------------------------------------------------------- core FFI workers
// size_t is mapped with the ABI-specific `Size` type (dart:ffi), not IntPtr.
typedef _ValC = Int32 Function(Pointer<Uint8>, Size);
typedef _ValD = int Function(Pointer<Uint8>, int);
typedef _ParC = Int32 Function(Pointer<Uint8>, Size, Pointer<Uint8>, Size, Pointer<Size>);
typedef _ParD = int Function(Pointer<Uint8>, int, Pointer<Uint8>, int, Pointer<Size>);
typedef _HashC = Uint32 Function(Pointer<Uint8>, Size);
typedef _HashD = int Function(Pointer<Uint8>, int);
typedef _HeavyC = Uint64 Function(Uint32);
typedef _HeavyD = int Function(int);

class CoreWorkers {
  static void start(String lib) {
    Isolate.spawn(_tight, [lib, shm.address]);
    Isolate.spawn(_heavy, [lib, shm.address]);
  }

  static void stop() {
    shm[1] = 1;
  }
}

void _tight(List a) {
  final lib = DynamicLibrary.open(a[0] as String);
  final s = Pointer<Uint64>.fromAddress(a[1]);
  final validate = lib.lookupFunction<_ValC, _ValD>('core_validate', isLeaf: true);
  final parse = lib.lookupFunction<_ParC, _ParD>('core_parse', isLeaf: true);
  final hash = lib.lookupFunction<_HashC, _HashD>('core_hash', isLeaf: true);
  final f = calloc<Uint8>(8);
  f.asTypedList(8).setAll(0, [0xA5, 1, 5, 0x68, 0x65, 0x6c, 0x6c, 0x6f]);
  final o = calloc<Uint8>(64);
  final ol = calloc<Size>(1);
  var n = 0, acc = 0;
  while (s[1] == 0) {
    for (var k = 0; k < 4096; k++) {
      acc += validate(f, 8);
      acc += parse(f, 8, o, 64, ol) + ol.value;
      acc += hash(f, 8);
    }
    n += 4096;
    s[0] = n;
    s[8] = acc; // publish the accumulated result so the calls cannot be elided
  }
}

void _heavy(List a) {
  final lib = DynamicLibrary.open(a[0] as String);
  final s = Pointer<Uint64>.fromAddress(a[1]);
  final heavy = lib.lookupFunction<_HeavyC, _HeavyD>('core_heavy');
  final sw = Stopwatch()..start();
  var next = 0;
  var acc = 0;
  while (s[1] == 0) {
    if (sw.elapsedMilliseconds >= next) {
      acc ^= heavy(100);
      s[9] = acc;
      s[2] = s[2] + 1;
      next += 1000;
    } else {
      sleep(const Duration(milliseconds: 5));
    }
  }
}

/// nav scenario: background thread keeping ~1 core busy with integer maths.
void startBusy() {
  Isolate.spawn(_busy, shm.address);
}

void _busy(int addr) {
  final s = Pointer<Uint64>.fromAddress(addr);
  var x = 12345, n = 0;
  while (s[7] == 0) {
    for (var k = 0; k < 1000000; k++) {
      x = (x * 1103515245 + 12345) & 0x7FFFFFFF;
    }
    n++;
  }
  if (x == -1) print(n);
}
