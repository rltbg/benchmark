#include "bench.h"
#include <QImageReader>
#include <QMetaProperty>
#include <fcntl.h>
#include <functional>
#include <memory>

// ======================= background producers / workers =======================
static void coreCb(void *user, uint32_t index, double price) {   // runs on the core's producer thread
    static_cast<UpdateSink *>(user)->push(Ev{index, 0, price});
}

void Bench::startGen(Flow &f, uint64_t rate, bool structuralOps) {
    f.rate = rate;
    f.gen = std::thread([this, &f, rate, structuralOps] {
        using clk = std::chrono::steady_clock;
        uint32_t x = 0x2545F491u; const auto t0 = clk::now(); uint64_t made = 0, madeS = 0;
        while (!stopAll) {
            const uint64_t ns = uint64_t(std::chrono::duration_cast<std::chrono::nanoseconds>(clk::now() - t0).count());
            uint64_t due = ns / 1000 * rate / 1000000;   // ns * rate / 1e9 without overflow
            due = std::min<uint64_t>(due, made + rate / 20 + 1);   // bounded catch-up (50 ms), same rule as the native producer
            for (; made < due; ++made) { x = xorshift32(x); f.sink->push(Ev{x % 100000u, 0, x / 100.0}); }
            if (structuralOps) {
                uint64_t dueS = std::min<uint64_t>(ns / 1000 * (rate / 100) / 1000000, madeS + rate / 2000 + 1);
                for (; madeS < dueS; ++madeS) {
                    x = xorshift32(x); const uint32_t r1 = x; x = xorshift32(x); const uint32_t r2 = x; x = xorshift32(x);
                    f.sink->push(Ev{r1, 1 + x % 3, double(r2)});
                }
            }
            if (f.sink->mode == UpdateSink::Batch) f.sink->flushBatch();
            std::this_thread::sleep_for(std::chrono::microseconds(500));
        }
    });
}

void Bench::startBackground() {
    const QString handoff = a.v("handoff", "ring");
    const UpdateSink::Mode mode = handoff == "invoke" ? UpdateSink::Invoke : handoff == "batch" ? UpdateSink::Batch : UpdateSink::Ring;
    auto mkFlow = [&](Flow &f, RowModel *m, ListCtl *c) { f.sink = new UpdateSink(mode, this); f.model = m; f.ctl = c; };
    switch (sc) {
    case ListMutate: mkFlow(flows[0], &model, &listCtl[0]); nflows = 1; startGen(flows[0], uint64_t(N), true); break;
    case LatLoad: case LatWheel:
        if (a.v("load") == "mutate") { mkFlow(flows[0], &model, &listCtl[0]); nflows = 1; startGen(flows[0], 100000, true); }
        else if (a.v("load") == "heavy") workers.emplace_back([this] { while (!stopAll) { busyAcc ^= core_heavy(100); ++heavyCalls; } });
        break;
    case FfiCallbacks:
        mkFlow(flows[0], &model, &listCtl[0]); nflows = 1; flows[0].core = true;
        core_producer_start(coreCb, flows[0].sink, uint64_t(N), 100000);
        break;
    case Soak:
        mkFlow(flows[0], &model, &listCtl[0]); mkFlow(flows[1], &model2, &listCtl[1]); nflows = 2;
        startGen(flows[0], 20000, true);
        flows[1].core = true; core_producer_start(coreCb, flows[1].sink, 50000, 100000);
        break;
    case FfiBlock:
        if (!heavyOnUi())
            workers.emplace_back([this] { while (!stopAll) { busyAcc ^= core_heavy(uint32_t(N)); ++heavyCalls; } });
        break;
    default: break;
    }
}

void Bench::stopBackground() {
    stopAll = true;
    for (int i = 0; i < nflows; ++i) { if (flows[i].core) core_producer_stop(); if (flows[i].gen.joinable()) flows[i].gen.join(); }
    for (auto &t : workers) t.join();
    workers.clear();
}

// Drain the hand-off and apply the whole batch on the UI thread (once per frame).
void Bench::flowApply(Flow &f) {
    f.sink->drain(f.buf);
    if (f.buf.empty()) return;
    const int first = f.ctl->lv ? f.ctl->firstVisible() : 0;
    const size_t k = f.model->applyEvents(f.buf.data(), f.buf.size(), first, a.v("group", "1") != "0");
    f.applied += k;
    for (const Ev &e : f.buf) if (e.kind) ++f.structural;
}

// G1 idiom A (impl=items): one QML Rectangle per shape, moved from C++ each frame (QQuickItem::setPosition); Item.clip is rectangular only.
void Bench::shapesReady() {
    if (!shapesHost || !shapeItems.empty()) return;
    const QString f = a.v("feature", "solid"); const bool all = f == "all";
    itemsSim.init(N, f == "gradient" || all, false, false);
    shapeItems.clear();
    for (QQuickItem *c : shapesHost->childItems()) {
        if (c->inherits("QQuickRepeater")) continue;
        const size_t i = shapeItems.size(); if (i >= size_t(N)) break;
        c->setWidth(itemsSim.w[i]); c->setHeight(itemsSim.h[i]);
        c->setProperty("radius", itemsSim.type[i] == 0 ? 0.0 : itemsSim.type[i] == 1 ? double(itemsSim.w[i]) / 2 : 8.0);
        shapeItems.push_back(c);
    }
    setReady();
}
void Bench::stepItems(double dt) {
    itemsSim.step(dt);
    for (size_t i = 0; i < shapeItems.size(); ++i) shapeItems[i]->setPosition(QPointF(double(itemsSim.x[i]), double(itemsSim.y[i])));
}

void Bench::startFfiWorkers(int k) { for (int i = 0; i < k; ++i) workers.emplace_back([this] { ffiWorker(); }); }
void Bench::ffiWorker() {
    using clk = std::chrono::steady_clock;
    const int bytes = std::max(1, N);
    std::vector<uint8_t> buf(static_cast<size_t>(bytes), uint8_t(0)); core_fill(buf.data(), size_t(bytes), 1);   // buffer created once, on the stack side
    const QString sel = a.v("fn", "all");
    const bool use[4] = {sel == "all" || sel == "noop", sel == "all" || sel == "add", sel == "all" || sel == "hash", sel == "all" || sel == "fill"};
    const uint64_t perBatchBuf = std::max<uint64_t>(1, std::min<uint64_t>(200000, (4u << 20) / uint64_t(bytes)));
    uint64_t acc = 0; uint32_t seed = 1;
    while (!stopAll) {
        for (int f = 0; f < 4 && !stopAll; ++f) {
            if (!use[f]) continue;
            const uint64_t cnt = f < 2 ? 50000 : perBatchBuf;
            const auto t0 = clk::now();
            switch (f) {
            case 0: for (uint64_t i = 0; i < cnt; ++i) acc += core_noop(); break;
            case 1: for (uint64_t i = 0; i < cnt; ++i) acc = core_add(acc, i); break;
            case 2: for (uint64_t i = 0; i < cnt; ++i) acc += core_hash(buf.data(), size_t(bytes)); break;
            case 3: for (uint64_t i = 0; i < cnt; ++i) core_fill(buf.data(), size_t(bytes), ++seed); acc += buf[0]; break;
            }
            const uint64_t ns = uint64_t(std::chrono::duration_cast<std::chrono::nanoseconds>(clk::now() - t0).count());
            FnStat &s = fs[phase.load(std::memory_order_relaxed)][f];
            s.calls.fetch_add(cnt, std::memory_order_relaxed); s.ns.fetch_add(ns, std::memory_order_relaxed);
            if (f >= 2) s.bytes.fetch_add(cnt * uint64_t(bytes), std::memory_order_relaxed);
        }
    }
    busyAcc ^= acc;
}

// ======================= text =======================
void Bench::loadText(QObject *edit, const QString &mode) {
    te = edit;
    auto *qd = edit->property("textDocument").value<QQuickTextDocument *>();
    QTextDocument *doc = qd ? qd->textDocument() : nullptr;
    if (sc == TextRich) {   // N spans built with the QTextDocument API (no HTML parsing): alternating fonts/sizes/colours, CJK, emoji, Arabic/Hebrew
        QElapsedTimer t; t.start(); ttfdBase = nowNs();
        static const QStringList words{"lorem", "ipsum", "日本語テキスト", "dolor", "😀🚀", "مرحبا بالعالم", "sit", "שלום עולם", "漢字", "amet", "한국어", "consectetur"};
        static const QStringList fams{"DejaVu Sans", "DejaVu Serif", "DejaVu Sans Mono", "Noto Sans"};
        if (doc) {
            doc->setUndoRedoEnabled(false);
            QTextCursor c(doc);
            for (int i = 0; i < N; ++i) {
                const uint32_t f = fnv(uint32_t(i));
                QTextCharFormat cf; cf.setFontFamilies({fams[i % 4]}); cf.setFontPointSize(8 + (i % 5) * 2.0);
                cf.setFontWeight(i % 4 == 0 ? QFont::Bold : QFont::Normal); cf.setFontItalic(i % 7 == 0);
                cf.setForeground(QColor(int(f & 127), int((f >> 8) & 127), int((f >> 16) & 127)));
                c.insertText(words[i % words.size()] + QLatin1Char(' '), cf);
                if (i % 12 == 11) c.insertBlock();
            }
        }
        (void)edit->property("contentHeight");
        layoutMs = t.nsecsElapsed() / 1e6; textShown = true; textBytes = doc ? doc->characterCount() : 0;
        QMetaObject::invokeMethod(this, [this] { setReady(); }, Qt::QueuedConnection);
        return;
    }
    const bool edit3 = sc == TextEdit || sc == Soak;
    const QString path = a.assets + (edit3 ? QStringLiteral("/text1mb.txt") : QString("/text%1mb.txt").arg(std::max(1, N)));
    ttfdBase = nowNs();
    const QString lmode = a.v("load", "async");
    if (doc && !edit3) doc->setUndoRedoEnabled(false);   // read-only: no undo stack (QTextDocument docs)
    if (lmode == "sync") {   // v0: file read, UTF-8 decoding, document build and layout all on the UI thread
        QElapsedTimer t; t.start();
        QFile f(path); if (!f.open(QIODevice::ReadOnly)) { fprintf(stderr, "cannot open %s\n", qPrintable(path)); return; }
        const QString txt = QString::fromUtf8(f.readAll()); textBytes = int(f.size());
        edit->setProperty("text", txt); (void)edit->property("contentHeight");
        layoutMs = t.nsecsElapsed() / 1e6; textShown = true; setReady();
        if (edit3) { edit->setProperty("cursorPosition", txt.size() / 2); }
        return;
    }
    if (lmode == "chunked" && doc) {   // progressive: worker decodes, the UI thread appends 256 KB chunks with QTextCursor, one per event-loop turn
        textLoading = true;
        std::thread([this, edit, doc, path] {
            QFile f(path); auto *txt = new QString;
            if (f.open(QIODevice::ReadOnly)) { *txt = QString::fromUtf8(f.readAll()); textBytes = int(f.size()); }
            QMetaObject::invokeMethod(this, [this, edit, doc, txt] {
                auto step = std::make_shared<std::function<void(qsizetype)>>();
                *step = [this, edit, doc, txt, step](qsizetype pos) {
                    QTextCursor cur(doc); cur.movePosition(QTextCursor::End);
                    const qsizetype len = std::min<qsizetype>(262144, txt->size() - pos);
                    cur.insertText(txt->mid(pos, len)); (void)edit->property("contentHeight");
                    if (!textShown) textShown = true;
                    if (pos + len < txt->size()) QTimer::singleShot(0, this, [step, pos, len] { (*step)(pos + len); });
                    else { textLoading = false; layoutMs = (nowNs() - ttfdBase) / 1e6; delete txt; setReady(); }
                };
                (*step)(0);
            }, Qt::QueuedConnection);
        }).detach();
        return;
    }
    // default: file read + UTF-8 decoding on a worker thread; the UI thread only builds the document (+ layout)
    textLoading = true;
    std::thread([this, edit, path, edit3] {
        QFile f(path); QString txt;
        if (f.open(QIODevice::ReadOnly)) { txt = QString::fromUtf8(f.readAll()); textBytes = int(f.size()); }
        QMetaObject::invokeMethod(this, [this, edit, txt = std::move(txt), edit3]() mutable {
            QElapsedTimer t; t.start();
            edit->setProperty("text", txt); (void)edit->property("contentHeight");
            layoutMs = t.nsecsElapsed() / 1e6; textShown = true; textLoading = false;
            if (edit3) {
                edit->setProperty("cursorPosition", txt.size() / 2);
                (void)edit->property("contentHeight");
                if (QObject *fl = items.value("flick")) {
                    const QRectF cr = edit->property("cursorRectangle").toRectF();
                    fl->setProperty("contentY", std::max(0.0, cr.y() - fl->property("height").toDouble() / 2));
                }
            }
            setReady();
        }, Qt::QueuedConnection);
    }).detach();
}

// ======================= tick / scripts =======================
void Bench::tick() {
    tickPending = false;
    if (finished) return;
    const qint64 now = nowNs();
    if (lifecycle(now)) return;
    const double t = measuring ? (now - t0Ns) / 1e9 : (now - startNs) / 1e9;
    double dt = lastTickNs ? (now - lastTickNs) / 1e9 : 0.016; lastTickNs = now; dt = std::min(dt, 0.1);
    scenarioTick(t, dt);
    emit frame();
    ticks = true;
    view->update();
}

void Bench::doReset() {
    // L4: full model replacement with N fresh rows (new ids -> new heights). The height index of the next generation was built on a
    // worker thread while waiting, so only the model reset itself runs on the UI thread.
    ++resetGen; if (idxThread.joinable()) idxThread.join();
    curIdx ^= 1;
    QElapsedTimer t; t.start();
    resetPendingNs = nowNs();
    model.resetImplicit(N, uint32_t(resetGen) * uint32_t(N));
    listCtl[0].hi = &hidx[curIdx];
    if (measuring) resetCallMs.push_back(t.nsecsElapsed() / 1e6);
    QMetaObject::invokeMethod(listCtl[0].lv, "positionViewAtBeginning");
    listCtl[0].reset(cumD(lastScriptT));
    const uint32_t nb = uint32_t(resetGen + 1) * uint32_t(N); const int other = curIdx ^ 1; const int cnt = N;
    idxThread = std::thread([this, other, nb, cnt] { hidx[other].build(cnt, nb, false); });
}

void Bench::scenarioTick(double t, double dt) {
    lastScriptT = t;
    switch (sc) {
    case ListScale: case ListRich: listCtl[0].drive(cumD(t)); break;
    case ListMutate: case FfiCallbacks: flowApply(flows[0]); listCtl[0].drive(cumD(t)); break;
    case ListReset:
        if (measuring && t >= nextResetT) { doReset(); nextResetT = t + 2.0; }
        listCtl[0].drive(cumD(t)); break;
    case FfiBlock:
        listCtl[0].drive(cumD(t));
        if (heavyOnUi() && (uiHeavyLastT < 0 || t - uiHeavyLastT >= 1.0)) {
            busyAcc ^= core_heavy(uint32_t(N)); if (measuring) ++heavyCalls; uiHeavyLastT = t;   // blocks the UI thread: the freeze is the result
        }
        break;
    case LatLoad: case LatWheel: {
        const QString load = a.v("load", "idle");
        if (load == "fling" || load == "mutate") { if (nflows) flowApply(flows[0]); listCtl[0].drive(cumD(t)); }
        break; }
    case TextBig: case TextRich: {
        QObject *fl = items.value("flick"); if (!fl || !textShown) break;
        const double mx = fl->property("contentHeight").toDouble() - fl->property("height").toDouble();
        fl->setProperty("contentY", pingpong(t, sc == TextBig ? 30000 : 10000, mx));
        if (sc == TextBig && measuring && !selected && t >= 5.0 && te) { selected = true; QMetaObject::invokeMethod(te, "selectAll"); }
        break; }
    case TextLabels: labelModel.update(uint32_t(++counter)); break;
    case UiBind: { ++counter; for (Cell *c : cellModel.cells) c->setValue(counter); break; }
    case GfxShapes: case GfxPaths: case GfxChart: case GfxImages:
        for (GfxBase *g : gfx) g->step(dt);
        if (!shapeItems.empty()) stepItems(dt);
        for (QQuickItem *r : renderItems) { auto *ri = static_cast<ShapesRenderItem *>(r); ri->sim.step(dt); ri->update(); }
        break;
    case FfiCalls:
        for (GfxBase *g : gfx) g->step(dt);
        if (measuring && !workers.size() && !ffiStarted) { ffiStarted = true; phase = 0; startFfiWorkers(a.v("threads") == "4" ? 4 : 1); if (a.v("threads") == "4") phase2Started = true; }
        if (measuring && !phase2Started && a.v("threads").isEmpty() && t >= a.duration / 2) { phase2Started = true; phase = 1; phaseSwitchNs = nowNs(); startFfiWorkers(3); }
        break;
    case Soak: case MemMin:
        if (nflows) { flowApply(flows[0]); flowApply(flows[1]); }
        listCtl[0].drive(sc == Soak ? 3000.0 * t : cumD(t));
        if (sc == Soak) listCtl[1].drive(3000.0 * t);
        for (GfxBase *g : gfx) g->step(dt);
        if (QObject *fl = items.value("flick"); fl && sc == MemMin && textShown) {
            const double mx = fl->property("contentHeight").toDouble() - fl->property("height").toDouble();
            fl->setProperty("contentY", pingpong(t, 30000, mx));
        }
        break;
    default: break;
    }
}

void Bench::beginMeasure(qint64 now) {
    { QMutexLocker l(&fm); frames.clear(); lastNs = -1; measuring = true; }
    t0Ns = now; stallMax = 0; lastStallNs = now; keyEvents = 0; resizeMs.clear(); rebuildMs.clear(); resetCallMs.clear(); resizes = 0;
    nextResetT = 2.0; counter = 0; selected = false;
    for (auto &c : listCtl) if (c.lv) { c.pos = c.shown = c.prevD = 0; QMetaObject::invokeMethod(c.lv, "positionViewAtBeginning"); c.realigns = c.jumps = 0; }
    if (listCtl[0].lv && listCtl[0].hi && !a.v("from").isEmpty()) {   // variant from=<fraction of the list>: start the script deep inside the list (float32 matrix precision check)
        ListCtl &c = listCtl[0]; const double off = std::clamp(a.v("from").toDouble(), 0.0, 0.99) * double(c.hi->total);
        const auto [idx, rem] = c.hi->rowAt(off);
        QMetaObject::invokeMethod(c.lv, "positionViewAtIndex", Q_ARG(int, idx), Q_ARG(int, 0));
        c.lv->setProperty("contentY", c.lv->property("contentY").toDouble() + rem); c.pos = c.shown = off; startOffset = off;
    }
    for (int i = 0; i < nflows; ++i) { Flow &f = flows[i]; f.gen0 = f.sink->generated; f.drop0 = f.sink->dropped; f.applied0 = f.applied; f.structural0 = f.structural; }
    heavy0 = heavyCalls; rowsBound0 = model.rowsBound; lastTickNs = 0;
    if (sc == ListReset) { hidx[1].build(N, uint32_t(N), false); curIdx = 0; hidx[0] = hindex; }   // generation 0 = initial index (kept), 1 prebuilt
    fputs("BENCH:measure_start\n", stdout); fflush(stdout);
}

// ======================= output =======================
static QJsonArray jarr(const std::vector<double> &v) { QJsonArray a; for (double d : v) a.push_back(d); return a; }

void Bench::finish() {
    if (finished) return; finished = true;
    const qint64 endNs = nowNs();
    for (int i = 0; i < nflows; ++i) flowApply(flows[i]);   // updates still in flight at the end of the window are applied before counting
    fputs("BENCH:measure_end\n", stdout); fflush(stdout);
    const double dur = measuring ? (endNs - t0Ns) / 1e9 : a.duration;
    QJsonObject ops; const Sc s = sc;
    ops["n"] = N;
    { QJsonObject v; for (auto it = a.var.begin(); it != a.var.end(); ++it) v[it.key()] = it.value(); ops["variant"] = v; }
    ops["ui_stall_ms_max"] = stallMax; ops["ui_stall_ms_max_incl_warmup"] = stallMaxAll; ops["setup_ms"] = setupMs;
    uint32_t ck = 0;
    const bool listy = s == ListScale || s == ListRich || s == ListMutate || s == ListReset || s == FfiCallbacks || s == FfiBlock || s == LatLoad || s == LatWheel || s == Idle || s == Soak || s == MemMin;
    if (listy) {
        ops["sum_heights"] = qint64(hindex.total); ops["rows_bound"] = qint64(model.rowsBound - rowsBound0);
        ops["checksum_data"] = QString::asprintf("%08x", hindex.xorFnv); xorData = hindex.xorFnv;
        ops["final_offset"] = listCtl[0].pos; ops["start_offset"] = startOffset;
        if (listCtl[0].hi && listCtl[0].lv) { ops["top_row_y_error_px"] = listCtl[0].topRowError(); ops["list_realigns"] = qint64(listCtl[0].realigns); ops["list_jumps"] = qint64(listCtl[0].jumps); }
        if (listCtl[0].lv) ops["listview_contentHeight_estimate"] = listCtl[0].lv->property("contentHeight").toDouble();
        ck ^= fnv(uint32_t(listCtl[0].pos));
    }
    if (nflows) {
        uint64_t g = 0, ap = 0, dr = 0, st = 0;
        for (int i = 0; i < nflows; ++i) { g += flows[i].sink->generated - flows[i].gen0; dr += flows[i].sink->dropped - flows[i].drop0; ap += flows[i].applied - flows[i].applied0; st += flows[i].structural - flows[i].structural0; }
        ops["updates_generated"] = qint64(g); ops["updates_applied"] = qint64(ap); ops["updates_dropped"] = qint64(dr); ops["structural_applied"] = qint64(st);
        ops["handoff"] = a.v("handoff", "ring");
        ops["model_rows_end"] = qint64(model.size());
        if (s == FfiCallbacks) ops["producer_produced_total"] = qint64(core_producer_produced());
        ck ^= fnv(uint32_t(g)) ^ fnv(uint32_t(ap));
        if (s == Soak) { QJsonObject o; for (int i = 0; i < 2; ++i) { o[QString("flow%1_generated").arg(i)] = qint64(flows[i].sink->generated - flows[i].gen0); o[QString("flow%1_applied").arg(i)] = qint64(flows[i].applied - flows[i].applied0); } ops["soak"] = o; }
    }
    if (s == ListReset) { ops["resets"] = qint64(resetCallMs.size()); ops["reset_call_ms"] = jarr(resetCallMs); ops["rebuild_ms"] = jarr(rebuildMs);
        double mx = 0; for (double d : rebuildMs) mx = std::max(mx, d); ops["rebuild_ms_max"] = mx; }
    if (s == TextBig || s == TextEdit || s == TextRich) { ops["time_to_first_display_ms"] = ttfdMs; ops["layout_ms"] = layoutMs; ops["text_bytes"] = textBytes; ops["load"] = a.v("load", "async");
        if (s == TextBig) ops["selected_all"] = selected; }
    if (s == TextEdit || s == Soak) ops["key_events"] = keyEvents; else ops["key_events"] = keyEvents;
    if (s == UiTree) { ops["resize_frame_ms"] = jarr(resizeMs); ops["resizes"] = resizes; }
    if (!gfx.empty()) {
        uint32_t x = 0; for (GfxBase *g : gfx) { x ^= g->dataChecksum(); ck ^= g->stateChecksum(); }
        for (QQuickItem *r : renderItems) { auto *ri = static_cast<ShapesRenderItem *>(r); for (int i = 0; i < ri->sim.n; ++i) x ^= fnv(uint32_t(i)); ck ^= fnv(uint32_t(ri->sim.checksumPos())); }
        xorData = x; ops["checksum_data"] = QString::asprintf("%08x", x); ops["state_checksum"] = QString::asprintf("%08x", ck);
        QString info = gfx[0]->info(); if (!info.isEmpty()) ops["info"] = info;
        if (imagesItem && imagesItem->bigSide == 0) { ops["images_decoded"] = 600; ops["decode_total_ms"] = imagesItem->decodeNs / 1e6; }
    }
    if (!shapeItems.empty()) { uint32_t x = 0; for (int i = 0; i < N; ++i) x ^= fnv(uint32_t(i)); ops["checksum_data"] = QString::asprintf("%08x", x); ops["state_checksum"] = QString::asprintf("%08x", fnv(uint32_t(itemsSim.checksumPos()))); }
    if (s == GfxShapes || s == FfiCalls || s == Soak) {
        const QString f = a.v("feature", "solid"); QJsonArray nr; const bool items = a.v("impl") == "items";
        if (f == "shadow" || f == "all") nr.push_back("drop shadow (blur 8 px): no QPainter blur API; MultiEffect/ShaderEffect not rendered by the software adaptation; not emulated");
        if ((f == "clip" || f == "all") && items) nr.push_back("rounded clip (Item.clip is rectangular only; rectangular clip used)");
        ops["not_rendered"] = nr; ops["shapes_idiom"] = items ? "items (one Rectangle per shape)" : a.v("impl") == "render" ? "QSGRenderNode" : "QQuickPaintedItem";
    } else ops["not_rendered"] = notRendered;
    if (s == GfxLayers && a.v("effect") == "blur") { QJsonArray nr; nr.push_back("MultiEffect blur 4 px (ShaderEffect-based: not rendered by the software adaptation)"); ops["not_rendered"] = nr; }
    if (s == TextLabels || s == UiBind || s == UiTree || s == UiAnim || s == GfxLayers) { uint32_t x = 0; for (int i = 0; i < N; ++i) x ^= fnv(uint32_t(i)); ops["checksum_data"] = QString::asprintf("%08x", x); xorData = x; }
    if (s == UiBind) ops["counter_final"] = qint64(counter);
    if (s == FfiCalls) {
        static const char *names[4] = {"noop", "add", "hash", "fill"};
        const double secs[2] = {phase2Started && a.v("threads").isEmpty() ? (phaseSwitchNs - t0Ns) / 1e9 : dur, std::max(0.0, (endNs - phaseSwitchNs) / 1e9)};
        QJsonArray ph; const int nph = phase2Started && a.v("threads").isEmpty() ? 2 : 1;
        for (int p = 0; p < nph; ++p) {
            QJsonObject o; o["threads"] = (nph == 2) ? (p == 0 ? 1 : 4) : (a.v("threads") == "4" ? 4 : 1); o["seconds"] = secs[p];
            for (int f = 0; f < 4; ++f) {
                const uint64_t c = fs[p][f].calls, ns = fs[p][f].ns, by = fs[p][f].bytes;
                if (!c) continue;
                QJsonObject fo; fo["calls"] = qint64(c); fo["calls_per_s"] = c / std::max(secs[p], 1e-9); fo["ns_per_call_busy"] = double(ns) / double(c);
                if (f >= 2) fo["mb_per_s"] = by / 1e6 / std::max(secs[p], 1e-9);
                o[names[f]] = fo;
            }
            ph.push_back(o);
        }
        ops["ffi_phases"] = ph; ops["buffer_bytes"] = std::max(1, N);
    }
    if (s == FfiBlock || s == LatLoad || s == LatWheel) { ops["heavy_calls"] = qint64(heavyCalls - heavy0); ops["heavy_ms"] = (s == FfiBlock) ? N : 100; ops["heavy_where"] = heavyOnUi() ? "ui" : "bg"; }
    if (s == LatLoad || s == LatWheel) ops["load"] = a.v("load", "idle");
    ops["heap_inuse_bytes_end"] = qint64(mallinfo2().uordblks); ops["alloc_bytes"] = QJsonValue::Null;
    ops["graphics_api"] = graphicsApiName();
    ops["pacing"] = !ticked() ? (a.pace60 ? "60-native" : "native") : (a.pace60 ? "60" : "uncapped");
    ops["frame_driver"] = ticked() ? "tick" : "qt";
    if (!a.shot.isEmpty()) view->grabWindow().save(a.shot);
    QJsonArray fa; { QMutexLocker l(&fm); for (double f : frames) fa.push_back(f); }
    QJsonObject root{{"stack", "qt"}, {"version", version()}, {"scenario", a.scenario}, {"duration_s", dur}, {"frames", fa.size()}, {"frame_ms", fa},
                     {"ops", ops}, {"checksum", QString::asprintf("%08x", ck)}, {"notes", notes()}, {"unsupported", unsupported.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(unsupported)}};
    QFile f(a.out);
    if (f.open(QIODevice::WriteOnly)) { f.write(QJsonDocument(root).toJson(QJsonDocument::Compact)); f.close(); }
    else fprintf(stderr, "cannot write %s\n", qPrintable(a.out));
    fflush(stdout);
    _exit(0);   // skip destructors: tearing down 10^5..10^6 QML items / a 100 MB QTextDocument takes seconds and measures nothing
}

QString Bench::notes() const {
    QString n = QString("Qt %1; scene graph backend=%2; platform=%3. frame_ms = interval between consecutive QQuickWindow::frameSwapped "
                        "(software: emitted by the software render loop right after QBackingStore::flush of the dirty region (xcb, MIT-SHM); RHI OpenGL: after the GL swapBuffers of the basic render loop (Mesa llvmpipe)). "
                        "QT_QPA_UPDATE_IDLE_TIME=%4 (documented in QWindow::requestUpdate, recommended for benchmarking in the Scene Graph docs). ")
                    .arg(qVersion(), graphicsApiName(), QGuiApplication::platformName(), QString::fromLatin1(qgetenv("QT_QPA_UPDATE_IDLE_TIME")));
    if (ticked()) n += a.pace60 ? "Pacing 60: each tick (script + QQuickWindow::update()) is scheduled on a fixed 16.667 ms grid with QTimer::singleShot(nanoseconds, Qt::PreciseTimer); a late frame moves to the next slot (missed frame). Only frames following a tick are counted. "
                                : "Pacing uncapped: next tick queued right after each frameSwapped. ";
    else n += "Frames are rendered by Qt only when the scene changes (animations on Qt's 16 ms animation timer, resize, typing, cursor blink); every frameSwapped while measuring is counted. ";
    n += "ui_stall_ms_max = longest gap between two firings of a 4 ms Qt::PreciseTimer on the UI thread (measure window only). setup_ms = process start (static init) -> first frameSwapped after the scenario data/items exist. "
         "_exit(0) after writing the JSON (no destructor teardown). ops.alloc_bytes null: no cheap public allocation counter (heap_inuse_bytes_end from mallinfo2 given instead). ";
    return n;
}
