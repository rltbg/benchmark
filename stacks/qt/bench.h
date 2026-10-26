#pragma once
// Bench driver: scenario scripts, measurement, background threads, JSON output.
// Exposed to QML as the singleton `App` of module `Bench` (QML_SINGLETON + QQmlEngine::setExternalSingletonInstance, Qt 6.12).
#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QGuiApplication>
#include <QImageReader>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMutex>
#include <QQuickAsyncImageProvider>
#include <QQuickView>
#include <QQuickWindow>
#include <QRunnable>
#include <QThreadPool>
#include <QTimer>
#include <QtQml/qqmlregistration.h>
#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdio>
#include <thread>
#include "canvas.h"
#include "model.h"

extern "C" {
#include "benchcore.h"
}

static const double WARMUP = 2.0;

struct Args {
    QString scenario = "idle", out = "out.json", assets, canvasImpl = "painted", backend, shot;
    bool paced = false; double duration = 10; int rate = 1000; int shapes = 5000;
};

// ---- images: QQuickAsyncImageProvider + QThreadPool (the idiom the QQuickImageProvider docs recommend instead of the
// single per-engine loader thread). Decode + downscale with QImageReader::setScaledSize, off the UI thread. ----
struct DecodeStats { std::atomic<uint64_t> decoded{0}, decodeNs{0}; };
class ImgResponse : public QQuickImageResponse, public QRunnable {
public:
    ImgResponse(QString path, QSize req, DecodeStats *st) : path(std::move(path)), req(req), st(st) { setAutoDelete(false); }
    void run() override {
        QElapsedTimer t; t.start();
        QImageReader r(path);
        if (req.isValid()) r.setScaledSize(req);
        img = r.read();
        st->decodeNs += t.nsecsElapsed(); ++st->decoded;
        emit finished();
    }
    QQuickTextureFactory *textureFactory() const override { return QQuickTextureFactory::textureFactoryForImage(img); }
    QString path; QSize req; DecodeStats *st; QImage img;
};
class ImgProvider : public QQuickAsyncImageProvider {
public:
    explicit ImgProvider(QString dir) : dir(std::move(dir)) {}
    QQuickImageResponse *requestImageResponse(const QString &id, const QSize &req) override {
        auto *r = new ImgResponse(dir + "/images/" + id + ".png", req, &stats);
        pool.start(r);
        return r;
    }
    QString dir; DecodeStats stats; QThreadPool pool;
};

class Bench : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(App)
    QML_SINGLETON
    QML_UNCREATABLE("provided from C++ with QQmlEngine::setExternalSingletonInstance")
    Q_PROPERTY(QString scenario READ scenario CONSTANT)
    Q_PROPERTY(QString canvasImpl READ canvasImpl CONSTANT)
    Q_PROPERTY(int shapes READ shapes CONSTANT)
    Q_PROPERTY(QObject *model READ modelObj CONSTANT)
public:
    explicit Bench(const Args &a) : a(a), model(this) { clock.start(); }
    QString scenario() const { return a.scenario; }
    QString canvasImpl() const { return a.canvasImpl; }
    int shapes() const { return a.shapes; }
    QObject *modelObj() { return &model; }
    ImgProvider *imgProv = nullptr;

    void setView(QQuickView *v) { view = v; }
    // Continuous scenarios: redraw continuously (each frameSwapped queues a tick that advances the script and calls update()).
    // Timer scenarios (idle, nav): nothing is redrawn by us; Qt renders only when something changes (nav: StackView/Popup
    // transitions driven by Qt's animation timer), warm-up end and measurement end come from QTimer::singleShot.
    bool continuous() const { return a.scenario != "idle" && a.scenario != "nav"; }
    void start() {
        startNs = clock.nsecsElapsed();
        const QString s = a.scenario;
        if (s == "stream" || s == "stress") { producer = new Producer(&model, s == "stress" ? 5000 : a.rate); producer->start(); }
        if (s == "core" || s == "stress") startCore();
        if (s == "nav") startBusy();
        QObject::connect(view, &QQuickWindow::frameSwapped, this, [this] { onFrameSwapped(); }, Qt::DirectConnection);
        if (!continuous()) {
            QTimer::singleShot(int(WARMUP * 1000), this, [this] { beginMeasure(clock.nsecsElapsed()); });
            QTimer::singleShot(int((WARMUP + a.duration) * 1000), this, [this] { finish(); });
            return;
        }
        view->update();
    }

    Q_INVOKABLE void registerItem(const QString &role, QObject *o) {
        items[role] = o;
        if (role == "canvas") {
            auto *it = qobject_cast<QQuickItem *>(o);
            canvases.push_back(it);
            const bool light = a.scenario == "core";
            const int n = light ? 1000 : a.shapes;
            if (auto *c = qobject_cast<CanvasItem *>(o)) c->sim.init(n, !light);
            else if (auto *r = qobject_cast<CanvasRenderItem *>(o)) r->sim.init(n, !light);
        }
    }
    Q_INVOKABLE void loadText(QObject *te) {
        QElapsedTimer t; t.start();
        textStartNs = clock.nsecsElapsed();
        QFile f(a.assets + "/text1mb.txt");
        if (!f.open(QIODevice::ReadOnly)) { fprintf(stderr, "cannot open %s\n", qPrintable(f.fileName())); return; }
        const QString txt = QString::fromUtf8(f.readAll());
        te->setProperty("text", txt);
        (void)te->property("contentHeight");   // forces the synchronous document layout
        layoutMs = t.nsecsElapsed() / 1e6;
        textDoneNs = clock.nsecsElapsed();
    }
    Q_INVOKABLE void transitionDone() { if (measuring) ++transitions; }
    Q_INVOKABLE void modalDone() { if (measuring) ++modals; }
    Q_INVOKABLE void keyEvent() { if (measuring) ++keyEvents; }
signals:
    void frame();
private:
    void onFrameSwapped() {   // emitted on the GUI thread by the basic/software render loops (render thread for 'threaded')
        const qint64 now = clock.nsecsElapsed();
        {
            QMutexLocker l(&fm);
            // continuous scenarios: only frames whose content our tick changed are samples (extra frames, e.g. triggered by an
            // async image completing, are merged); nav: every frame is caused by an animation step, i.e. a changed scene.
            if (!continuous() || ticked.exchange(false)) {
                if (measuring && lastNs >= 0) frames.push_back((now - lastNs) / 1e6);
                if (measuring) lastNs = now;
            }
            lastFrameNs = now;
        }
        if (continuous() && !tickPending.exchange(true)) QMetaObject::invokeMethod(this, &Bench::tick, Qt::QueuedConnection);
    }
    void tick() {
        tickPending = false;
        if (finished) return;
        const qint64 now = clock.nsecsElapsed();
        const double el = (now - startNs) / 1e9;
        if (!measuring && el >= WARMUP) beginMeasure(now);
        const double t = measuring ? (now - t0Ns) / 1e9 : el;
        if (measuring && t >= a.duration) { finish(); return; }
        double dt = lastTickNs ? (now - lastTickNs) / 1e9 : 0.016; lastTickNs = now; dt = std::min(dt, 0.1);
        if (!ttfdDone && textDoneNs > 0 && lastFrameNs > textDoneNs) { ttfdMs = (lastFrameNs - textStartNs) / 1e6; ttfdDone = true; }
        model.applyPending();
        const QString s = a.scenario;
        if (s == "list" || s == "stream" || s == "stress" || s == "input") driveList(t);
        if (s == "images") driveImages(t);
        if (s == "text") driveText(t);
        if (s == "canvas" || s == "core" || s == "stress") for (QQuickItem *c : canvases) stepCanvas(c, dt);
        emit frame();
        ticked = true;
        view->update();
    }
    void beginMeasure(qint64 now) {
        { QMutexLocker l(&fm); frames.clear(); lastNs = continuous() ? now : -1; measuring = true; }
        t0Ns = now;
        listPos = 0; prevD = 0; titleCalls0 = model.titleCalls;
        if (producer) { gen0 = producer->generated; drop0 = producer->dropped; app0 = model.applied; }
        calls0 = calls; heavy0 = heavy; transitions = modals = 0; keyEvents = 0; listRealigns = 0;
        if (imgProv) { dec0 = imgProv->stats.decoded; decNs0 = imgProv->stats.decodeNs; }
        textSelected = false;
        for (QQuickItem *c : canvases) { if (auto *ci = qobject_cast<CanvasItem *>(c)) ci->sim.gHead = 0; }
        fputs("BENCH:measure_start\n", stdout); fflush(stdout);
    }
    // ---------- scripts ----------
    static double flingD(double t) {   // signed cumulative distance of the list script (unclamped)
        if (t <= 4.0) return 3000.0 * t;
        double d = 12000.0;
        const double tt = t - 4.0; int k = int(tt / 1.5);
        for (int i = 0; i < k; ++i) d += (i % 2 == 0 ? 1 : -1) * 10000.0 * (1 - std::exp(-3.0));
        const double tau = tt - 1.5 * k;
        d += (k % 2 == 0 ? 1 : -1) * 10000.0 * (1 - std::exp(-2 * tau));
        return d;
    }
    // index of the row containing exact offset y, and y's offset inside that row
    std::pair<int, double> rowAt(double y) const {
        const auto it = std::upper_bound(prefix.begin(), prefix.end(), qint64(y));
        const int i = std::clamp(int(it - prefix.begin()) - 1, 0, ROWS - 1);
        return {i, y - double(prefix[i])};
    }
    void driveList(double t) {
        QObject *lv = items.value("list"); if (!lv) return;
        if (prefix.empty()) { prefix.resize(ROWS + 1); prefix[0] = 0; for (int i = 0; i < ROWS; ++i) prefix[i + 1] = prefix[i] + rowHeight(i); }
        const double viewH = lv->property("height").toDouble();
        const double maxOff = double(prefix[ROWS]) - viewH;
        const double d = flingD(t);
        const double target = std::clamp(listPos + (d - prevD), 0.0, maxOff); prevD = d;
        const double delta = target - shownPos; listPos = shownPos = target;   // shownPos survives the t=0 restart
        // ListView docs: with variable delegate heights contentHeight is only an estimate and positioning by absolute
        // contentY is "unreliable"; positionViewAtIndex() was tried and is not exact either here (the anchored row ends up
        // hundreds of px off once reused delegates take their new height). Exact method used: move contentY by the exact
        // pixel delta, in steps no larger than half the viewport so that ListView always lays out new rows adjacent to
        // existing ones (big steps only happen after a slow frame, or when the script restarts at t=0).
        double cy = lv->property("contentY").toDouble();
        double rest = delta;
        const double maxStep = viewH * 0.5;
        while (rest != 0) {
            const double st = std::clamp(rest, -maxStep, maxStep);
            cy += st; rest -= st;
            lv->setProperty("contentY", cy);
        }
        // Residual drift (a few px per 10^5 px scrolled, from delegates resized after layout): re-align the exact top row
        // using the public ListView.itemAtIndex(); no-op when the view already matches the exact offset.
        const auto [idx, rem] = rowAt(listPos);
        QQuickItem *it = nullptr;
        QMetaObject::invokeMethod(lv, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, it), Q_ARG(int, idx));
        if (it) {
            const double err = (it->y() - cy) + rem;
            if (std::abs(err) > 0.01) { ++listRealigns; lv->setProperty("contentY", cy + err); }
        }
    }
    static double pingpong(double t, double v, double mx) {
        if (mx <= 0) return 0;
        const double u = std::fmod(v * t, 2 * mx); return u <= mx ? u : 2 * mx - u;
    }
    void driveImages(double t) {
        QObject *g = items.value("grid"), *z = items.value("zoomer"); if (!g) return;
        const double viewH = g->property("height").toDouble();
        g->setProperty("contentY", pingpong(t, 3000, 7680 - viewH));
        const double ph = std::fmod(t, 6.0) / 6.0;
        if (z) z->setProperty("scale", 1.0 + 2.0 * (1.0 - std::abs(2 * ph - 1)));
    }
    void driveText(double t) {
        QObject *fl = items.value("flick"), *te = items.value("textedit"); if (!fl) return;
        const double mx = fl->property("contentHeight").toDouble() - fl->property("height").toDouble();
        fl->setProperty("contentY", pingpong(t, 3000, mx));
        if (measuring && !textSelected && t >= 5.0 && te) { textSelected = true; QMetaObject::invokeMethod(te, "selectAll"); }
    }
    void stepCanvas(QQuickItem *c, double dt) {
        if (auto *ci = qobject_cast<CanvasItem *>(c)) { ci->sim.step(dt); ci->update(); }
        else if (auto *r = qobject_cast<CanvasRenderItem *>(c)) { r->sim.step(dt); r->update(); }
    }
    // ---------- background threads ----------
    void startCore() {
        coreThreads.emplace_back([this] {
            uint8_t f[8] = {0xA5, 1, 5, 'h', 'e', 'l', 'l', 'o'}; uint8_t o[64]; size_t ol; uint64_t acc = 0;
            while (!stopAll) {
                for (int i = 0; i < 2000; ++i) {   // 1 call = validate + parse + hash; results accumulated and published
                    acc += uint32_t(core_validate(f, 8)) + core_hash(f, 8);
                    acc += uint32_t(core_parse(f, 8, o, 64, &ol)) + ol;
                }
                calls.fetch_add(2000, std::memory_order_relaxed); coreAcc.store(acc, std::memory_order_relaxed);
            }
        });
        coreThreads.emplace_back([this] {
            using clk = std::chrono::steady_clock; auto next = clk::now();
            while (!stopAll) {
                next += std::chrono::seconds(1);
                coreAcc ^= core_heavy(100); ++heavy;
                while (!stopAll && clk::now() < next) std::this_thread::sleep_for(std::chrono::milliseconds(10));
            }
        });
    }
    void startBusy() {
        coreThreads.emplace_back([this] { uint64_t x = 1; while (!stopAll) { for (int i = 0; i < 100000; ++i) x = x * 6364136223846793005ULL + 1442695040888963407ULL; busyAcc.store(x, std::memory_order_relaxed); } });
    }
    // ---------- output ----------
    void finish() {
        if (finished) return; finished = true;
        const qint64 endNs = clock.nsecsElapsed();
        fputs("BENCH:measure_end\n", stdout); fflush(stdout);
        const double dur = measuring ? (endNs - t0Ns) / 1e9 : a.duration;
        QJsonObject ops; const QString s = a.scenario;
        ops["sum_heights"] = [] { qint64 sum = 0; for (int i = 0; i < ROWS; ++i) sum += rowHeight(i); return sum; }();
        uint32_t ck = 0;
        if (s == "list" || s == "stream" || s == "stress" || s == "input") {
            ops["final_offset"] = listPos; ops["rows_drawn"] = qint64(model.titleCalls - titleCalls0);
            if (QObject *lv = items.value("list")) {
                double cy = lv->property("contentY").toDouble(); int topIdx = -1;
                QMetaObject::invokeMethod(lv, "indexAt", Q_RETURN_ARG(int, topIdx), Q_ARG(qreal, 100.0), Q_ARG(qreal, cy + 0.5));
                const auto [ei, erem] = rowAt(listPos);
                ops["top_index_expected"] = ei; ops["top_index_listview"] = topIdx;
                QQuickItem *it = nullptr;
                QMetaObject::invokeMethod(lv, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, it), Q_ARG(int, ei));
                // exactness check: offset of the expected top row inside the viewport must be -remainder
                if (it) ops["top_row_y_error_px"] = (it->y() - cy) + erem;
                ops["list_realigns"] = qint64(listRealigns);
                ops["listview_contentHeight_estimate"] = lv->property("contentHeight").toDouble();
            }
            ck ^= fnv(uint32_t(listPos));
        }
        if (s == "input") ops["key_events"] = qint64(keyEvents);
        if (producer) {
            producer->stop();
            ops["updates_generated"] = qint64(producer->generated - gen0); ops["updates_dropped"] = qint64(producer->dropped - drop0);
            ops["updates_applied"] = qint64(model.applied - app0);
        }
        if (s == "canvas" || s == "core" || s == "stress") {
            double sum = 0; int n = 0;
            for (QQuickItem *c : canvases) {
                if (auto *ci = qobject_cast<CanvasItem *>(c)) { sum += ci->sim.checksumPos(); n = ci->sim.n; }
                else if (auto *r = qobject_cast<CanvasRenderItem *>(c)) { sum += r->sim.checksumPos(); n = r->sim.n; }
            }
            if (a.canvasImpl == "qml") n = a.shapes;
            ops["shapes"] = n; ops["graph_points"] = (s == "core") ? 0 : 10000;
            ops["not_rendered"] = QJsonArray();   // rects, circles, rounded rects, gradients, shadows (pre-blurred sprites), clip, polyline: all drawn
            ck ^= fnv(uint32_t(sum));
        }
        if (s == "core" || s == "stress") {
            stopAll = true; for (auto &t : coreThreads) t.join(); coreThreads.clear();
            const uint64_t c = calls - calls0;
            ops["calls"] = qint64(c); ops["calls_per_s"] = c / std::max(dur, 1e-9); ops["heavy_calls"] = qint64(heavy - heavy0);
            double mx = 0; { QMutexLocker l(&fm); for (double f : frames) mx = std::max(mx, f); }
            ops["max_frame_stall_ms"] = mx; ck ^= fnv(uint32_t(coreAcc));
        }
        if (s == "images" && imgProv) {
            ops["images_decoded"] = qint64(imgProv->stats.decoded - dec0); ops["decode_total_ms"] = (imgProv->stats.decodeNs - decNs0) / 1e6;
            ops["images_decoded_incl_warmup"] = qint64(imgProv->stats.decoded); ops["decode_total_ms_incl_warmup"] = imgProv->stats.decodeNs / 1e6;
        }
        if (s == "text") { ops["time_to_first_display_ms"] = ttfdMs; ops["layout_ms"] = layoutMs; ck ^= fnv(uint32_t(textSelected)); }
        if (s == "nav") { ops["transitions_done"] = qint64(transitions); ops["modals_done"] = qint64(modals); }
        if (s == "idle") ops["rows_drawn"] = qint64(model.titleCalls);
        ops["graphics_api"] = graphicsApiName();
        if (!a.shot.isEmpty()) view->grabWindow().save(a.shot);
        QJsonArray fa; { QMutexLocker l(&fm); for (double f : frames) fa.push_back(f); }
        QJsonObject root{{"stack", "qt"}, {"version", version()}, {"scenario", s}, {"duration_s", dur}, {"frames", fa.size()}, {"frame_ms", fa},
                         {"ops", ops}, {"checksum", QString::asprintf("%08x", ck)}, {"notes", notes()}, {"unsupported", QJsonValue::Null}};
        QFile f(a.out);
        if (f.open(QIODevice::WriteOnly)) { f.write(QJsonDocument(root).toJson(QJsonDocument::Compact)); f.close(); }
        else fprintf(stderr, "cannot write %s\n", qPrintable(a.out));
        stopAll = true; for (auto &t : coreThreads) t.join(); coreThreads.clear();
        if (producer) producer->stop();
        QCoreApplication::exit(0);
    }
    QString graphicsApiName() const {
        switch (view->rendererInterface()->graphicsApi()) {
        case QSGRendererInterface::Software: return "software";
        case QSGRendererInterface::OpenGL: return "opengl(rhi)";
        default: return QString::number(int(view->rendererInterface()->graphicsApi()));
        }
    }
public:
    static QString version() {
        return QString("Qt %1 (runtime %2, aqtinstall linux_gcc_64 from download.qt.io), g++ %3, CMake 4.4.4, Ninja 1.13.2")
            .arg(QT_VERSION_STR, qVersion(), __VERSION__);
    }
private:
    QString notes() const {
        QString n = QString("Qt %1; scene graph backend=%2; platform=%3. frame_ms = interval between consecutive QQuickWindow::frameSwapped "
                            "(software: emitted by the software render loop right after QBackingStore::flush of the dirty region to the X window (xcb, MIT-SHM); "
                            "RHI OpenGL: after the GL swapBuffers of the basic render loop (Mesa llvmpipe)). QT_QPA_UPDATE_IDLE_TIME=%4 (documented in QWindow::requestUpdate "
                            "and recommended for benchmarking in the Qt Quick Scene Graph docs; removes the default 5 ms update pacing unless --paced). ")
                        .arg(qVersion(), graphicsApiName(), QGuiApplication::platformName(), QString::fromLatin1(qgetenv("QT_QPA_UPDATE_IDLE_TIME")));
        n += continuous() ? "Continuous redraw: each frameSwapped queues a tick that advances the script and calls QQuickWindow::update(); only frames following a tick are counted. "
                          : "No redraw requested by the app: frames are rendered by Qt only when the scene changes. ";
        n += "First 2 s warm-up discarded (scripts restart at t=0). ";
        const QString s = a.scenario;
        if (s == "idle" || s == "list" || s == "stream" || s == "stress" || s == "input")
            n += "List = QML ListView (reuseItems, cacheBuffer 600, delegate height = row-height role) over a C++ QAbstractListModel (rows computed in data()). "
                 "Scripted offset is exact (prefix sums of h(i)); the view is anchored with positionViewAtIndex()+remainder and then scrolled by exact pixel deltas "
                 "(ListView's contentHeight is only an estimate for variable heights). rows_drawn = data(TitleRole) calls (delegate creations + reuse rebinds). ";
        if (s == "stream" || s == "stress") n += "Producer = std::thread (xorshift32), batches posted with QMetaObject::invokeMethod(functor, Qt::QueuedConnection); bounded queue = 100k in-flight updates (excess dropped+counted in the producer); UI applies the whole pending batch once per frame, one dataChanged(ix, ix, {PriceRole}) per update. ";
        if (s == "canvas" || s == "core" || s == "stress") n += QString("Canvas impl=%1 (").arg(a.canvasImpl) + (a.canvasImpl == "render" ? "QSGRenderNode drawing with the software renderer's QPainter" : a.canvasImpl == "qml" ? "QML Canvas Context2D, smoke only" : "QQuickPaintedItem, renderTarget Image (QPainter raster into a QImage, then drawn by the scene graph)") + "); gradient fill via per-shape QLinearGradient(ObjectBoundingMode), antialiased rounded clip path; drop shadows: QPainter has no blur API, so a pre-blurred sprite (3 box-blur passes approximating an 8 px blur) cached per (type,size) is drawn at offset (2,2) under 1 shape in 10. Graph: 10000-pt drawPolyline. ";
        if (s == "text") n += "TextEdit (textFormat PlainText, readOnly, selectByMouse) in a Flickable; selectAll() at t=5 s. layout_ms = set text + force synchronous layout (contentHeight). time_to_first_display_ms = file load start -> first frameSwapped after the text was set. ";
        if (s == "images") n += "GridView of 600 Image{asynchronous, sourceSize 128x128} through a QQuickAsyncImageProvider decoding with QImageReader::setScaledSize on a QThreadPool (timed in the worker); zoom = item scale 1->3->1 / 6 s; scroll ping-pong 3000 px/s. images_decoded counts re-decodes after cache eviction. ";
        if (s == "nav") n += "QtQuick.Controls (Basic style) StackView pushItem/popCurrentItem with 300 ms slide transitions + modal Popup with 200 ms fade, cycle [push,push,open,close,pop,pop]; a C++ thread busy-loops one core. Qt Quick animations are driven by Qt's default animation timer (16 ms) in the software and basic render loops (no public API to change it; QAnimationDriver is \\internal), so nav frames are capped at ~60/s. ";
        if (s == "input") n += "Square 200x200 at (100,100) over the scrolling list; QML Keys.onPressed (Key_Space, auto-repeat ignored) toggles #000000/#FFFFFF. ";
        return n;
    }

    Args a; QQuickView *view = nullptr; BenchModel model; Producer *producer = nullptr;
    QHash<QString, QObject *> items; std::vector<QQuickItem *> canvases; std::vector<qint64> prefix;
    QElapsedTimer clock; qint64 startNs = 0; QMutex fm; std::vector<double> frames; qint64 lastNs = -1, lastFrameNs = 0, t0Ns = 0, lastTickNs = 0;
    bool measuring = false, finished = false, textSelected = false, ttfdDone = false;
    std::atomic<bool> ticked{false}, tickPending{false}, stopAll{false};
    double listPos = 0, shownPos = 0, prevD = 0, layoutMs = 0, ttfdMs = 0; qint64 textStartNs = 0, textDoneNs = 0;
    uint64_t titleCalls0 = 0, gen0 = 0, drop0 = 0, app0 = 0, calls0 = 0, heavy0 = 0, dec0 = 0, decNs0 = 0;
    std::atomic<uint64_t> calls{0}, heavy{0}, coreAcc{0}, busyAcc{0};
    qint64 listRealigns = 0;
    int transitions = 0, modals = 0, keyEvents = 0;
    std::vector<std::thread> coreThreads;
};
