// Qt 6 (Qt Quick / QML + C++) implementation of the reference benchmark app.
#include <QGuiApplication>
#include <QQuickView>
#include <QQuickWindow>
#include <QQuickImageProvider>
#include <QAnimationDriver>
#include <QQmlContext>
#include <QQmlEngine>
#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QImage>
#include <QFile>
#include <QMutex>
#include <QTimer>
#include <atomic>
#include <thread>
#include <cstdio>
#include <cmath>
#include "model.h"
#include "canvas.h"

extern "C" {
#include "benchcore.h"
}

static const double WARMUP = 2.0;

struct Args {
    QString scenario = "idle", out = "out.json", assets = "/home/user/eval/assets", canvasImpl = "painted", backend, shot;
    bool paced = false; double duration = 10; int rate = 1000; int shapes = 5000;
};

// ---- image provider: decode off the UI thread (called from Qt's image loader thread when Image.asynchronous) ----
class ImgProvider : public QQuickImageProvider {
public:
    ImgProvider(const QString &dir) : QQuickImageProvider(QQmlImageProviderBase::Image), dir(dir) {}
    QImage requestImage(const QString &id, QSize *size, const QSize &req) override {
        QElapsedTimer t; t.start();
        QImage img(dir + "/images/" + id + ".png");
        if (req.isValid() && !img.isNull()) img = img.scaled(req, Qt::KeepAspectRatio, Qt::SmoothTransformation);
        if (size) *size = img.size();
        decodeNs += t.nsecsElapsed(); ++decoded;
        return img;
    }
    QString dir; std::atomic<uint64_t> decoded{0}, decodeNs{0};
};

// Qt's software render loop advances animations from a 16 ms timer; the RHI render loops use a frame-driven
// QAnimationDriver. Use a frame-driven one here too so that animations progress on every (unthrottled) frame.
class FrameDriver : public QAnimationDriver {
public:
    FrameDriver() : QAnimationDriver(nullptr) {}
    void start() override { if (!running) { running = true; t.start(); } QAnimationDriver::start(); }
    void stop() override { running = false; QAnimationDriver::stop(); }
    qint64 elapsed() const override { return running ? t.elapsed() : 0; }
    bool running = false; QElapsedTimer t;
};

class Bench : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString scenario READ scenario CONSTANT)
    Q_PROPERTY(QString canvasImpl READ canvasImpl CONSTANT)
    Q_PROPERTY(int shapes READ shapes CONSTANT)
    Q_PROPERTY(QObject *model READ modelObj CONSTANT)
public:
    Bench(const Args &a) : a(a), model(this) { clock.start(); }
    QString scenario() const { return a.scenario; }
    QString canvasImpl() const { return a.canvasImpl; }
    int shapes() const { return a.shapes; }
    QObject *modelObj() { return &model; }
    ImgProvider *imgProv = nullptr;

    void setView(QQuickView *v) { view = v; }
    void start() {
        startNs = clock.nsecsElapsed();
        if (a.scenario != "idle" && (a.backend == "software" || qgetenv("QT_QUICK_BACKEND") == "software")) { drv = new FrameDriver; drv->install(); }
        const QString s = a.scenario;
        if (s == "stream" || s == "stress") { producer = new Producer(&model, s == "stress" ? 5000 : a.rate); producer->start(); }
        if (s == "core" || s == "stress") startCore();
        if (s == "nav") startBusy();
        if (s == "idle") {
            QTimer::singleShot(int((WARMUP + a.duration) * 1000), this, [this] { finish(); });
            return;
        }
        QObject::connect(view, &QQuickWindow::frameSwapped, this, [this] {
            const qint64 now = clock.nsecsElapsed();
            {
                QMutexLocker l(&fm);
                // only frames requested by our tick() count; extra frames triggered by e.g. async image completion are not separate samples
                if (ticked.exchange(false)) {
                    if (measuring && lastNs >= 0) frames.push_back((now - lastNs) / 1e6);
                    lastNs = now;
                }
                lastFrameNs = now;
            }
            if (!tickPending.exchange(true)) QMetaObject::invokeMethod(this, &Bench::tick, Qt::QueuedConnection);
        }, Qt::DirectConnection);
        view->update();
    }

    Q_INVOKABLE void registerItem(const QString &role, QObject *o) {
        items[role] = o;
        if (role == "canvas") {
            auto *it = qobject_cast<QQuickItem *>(o);
            canvases.push_back(it);
            const bool light = a.scenario == "core";
            const int n = light ? 1000 : a.shapes;
            const bool graph = !light;
            if (auto *c = qobject_cast<CanvasItem *>(o)) c->sim.init(n, graph);
            else if (auto *r = qobject_cast<CanvasRenderItem *>(o)) r->sim.init(n, graph);
        }
    }
    Q_INVOKABLE void loadText(QObject *te) {
        QElapsedTimer t; t.start();
        textStartNs = clock.nsecsElapsed();
        QFile f(a.assets + "/text1mb.txt");
        f.open(QIODevice::ReadOnly);
        const QString txt = QString::fromUtf8(f.readAll());
        te->setProperty("text", txt);
        (void)te->property("contentHeight");   // forces the synchronous document layout
        layoutMs = t.nsecsElapsed() / 1e6;
        textDoneNs = clock.nsecsElapsed();
    }
    Q_INVOKABLE void transitionDone() { if (measuring) ++transitions; }
    Q_INVOKABLE void modalDone() { if (measuring) ++modals; }
signals:
    void frame();
private slots:
    void tick() {
        tickPending = false;
        const qint64 now = clock.nsecsElapsed();
        if (a.scenario == "nav" && now / 1000000 == lastTickNs / 1000000 && lastTickNs) {
            // animation time (ms resolution) did not advance: nothing would change on screen; retry in 1 ms instead of rendering an empty frame
            tickPending = true; QTimer::singleShot(1, this, [this] { tickPending = false; tick(); }); return;
        }
        const double el = (now - startNs) / 1e9;
        if (!measuring && el >= WARMUP) beginMeasure(now);
        const double t = measuring ? (now - t0Ns) / 1e9 : el;
        if (measuring && t >= a.duration) { finish(); return; }
        double dt = lastTickNs ? (now - lastTickNs) / 1e9 : 0.016; lastTickNs = now; dt = std::min(dt, 0.1);
        if (!ttfdDone && textDoneNs > 0 && lastFrameNs > textDoneNs) { ttfdMs = (lastFrameNs - textStartNs) / 1e6; ttfdDone = true; }
        model.applyPending();
        if (drv && drv->running) drv->advance();
        const QString s = a.scenario;
        if (s == "list" || s == "stream" || s == "stress") driveList(t);
        if (s == "images") driveImages(t);
        if (s == "text") driveText(t);
        if (s == "canvas" || s == "core" || s == "stress") for (QQuickItem *c : canvases) stepCanvas(c, dt);
        emit frame();
        ticked = true;
        view->update();
    }
private:
    void beginMeasure(qint64 now) {
        { QMutexLocker l(&fm); frames.clear(); lastNs = now; measuring = true; }
        t0Ns = now;
        listPos = 0; prevD = 0; titleCalls0 = model.titleCalls;
        if (producer) { gen0 = producer->generated; drop0 = producer->dropped; app0 = model.applied; }
        calls0 = calls; heavy0 = heavy; transitions = modals = 0;
        if (imgProv) { dec0 = imgProv->decoded; decNs0 = imgProv->decodeNs; }
        textSelected = false;
        for (QQuickItem *c : canvases) { if (auto *ci = qobject_cast<CanvasItem *>(c)) ci->sim.gHead = 0; }
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
    void driveList(double t) {
        QObject *lv = items.value("list"); if (!lv) return;
        const double viewH = lv->property("height").toDouble();
        const double maxOff = double(8799584) - viewH;
        const double d = flingD(t);
        listPos = std::clamp(listPos + (d - prevD), 0.0, maxOff); prevD = d;
        // ListView only knows an *estimated* contentHeight for variable-height delegates; never scroll past it.
        const double hi = lv->property("originY").toDouble() + lv->property("contentHeight").toDouble() - viewH;
        lv->setProperty("contentY", std::min(listPos, std::max(hi, 0.0)));
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
                for (int i = 0; i < 2000; ++i) {
                    acc += core_validate(f, 8) + core_hash(f, 8);
                    core_parse(f, 8, o, 64, &ol); acc += ol;
                }
                calls.fetch_add(2000, std::memory_order_relaxed); coreAcc = acc;
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
        coreThreads.emplace_back([this] { volatile uint64_t x = 1; while (!stopAll) { for (int i = 0; i < 100000; ++i) x = x * 6364136223846793005ULL + 1442695040888963407ULL; busyAcc = x; } });
    }
    // ---------- output ----------
    void finish() {
        if (finished) return; finished = true;
        const double dur = measuring ? (clock.nsecsElapsed() - t0Ns) / 1e9 : a.duration;
        QJsonObject ops; const QString s = a.scenario;
        ops["sum_heights"] = [] { qint64 sum = 0; for (int i = 0; i < ROWS; ++i) sum += rowHeight(i); return sum; }();
        uint32_t ck = 0;
        if (s == "list" || s == "stream" || s == "stress") {
            ops["final_offset"] = listPos; ops["rows_drawn"] = qint64(model.titleCalls - titleCalls0);
            if (QObject *lv = items.value("list")) { ops["listview_contentY"] = lv->property("contentY").toDouble(); ops["listview_contentHeight_estimate"] = lv->property("contentHeight").toDouble(); }
            ck ^= fnv(uint32_t(listPos));
        }
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
            if (a.canvasImpl == "qml") n = a.shapes; ops["shapes"] = n; ops["graph_points"] = (s == "core") ? 0 : 10000; ops["shadows_rendered"] = true;
            ck ^= fnv(uint32_t(sum));
        }
        if (s == "core" || s == "stress") {
            stopAll = true; for (auto &t : coreThreads) t.join(); coreThreads.clear();
            const uint64_t c = calls - calls0;
            ops["calls"] = qint64(c); ops["calls_per_s"] = c / std::max(dur, 1e-9); ops["heavy_calls"] = qint64(heavy - heavy0);
            double mx = 0; for (double f : frames) mx = std::max(mx, f);
            ops["max_frame_stall_ms"] = mx; ck ^= fnv(uint32_t(coreAcc));
        }
        if (s == "images" && imgProv) { ops["images_decoded"] = qint64(imgProv->decoded - dec0); ops["decode_total_ms"] = (imgProv->decodeNs - decNs0) / 1e6; ops["images_decoded_incl_warmup"] = qint64(imgProv->decoded); ops["decode_total_ms_incl_warmup"] = imgProv->decodeNs / 1e6; }
        if (s == "text") { ops["time_to_first_display_ms"] = ttfdMs; ops["layout_ms"] = layoutMs; ck ^= fnv(uint32_t(textSelected)); }
        if (s == "nav") { ops["transitions_done"] = qint64(transitions); ops["modals_done"] = qint64(modals); }
        if (s == "idle") { ops["rows_drawn"] = qint64(model.titleCalls); }
        ops["qt_version"] = QString(qVersion());
        ops["graphics_api"] = int(view->rendererInterface()->graphicsApi());
        if (!a.shot.isEmpty()) view->grabWindow().save(a.shot);
        QJsonArray fa; { QMutexLocker l(&fm); for (double f : frames) fa.push_back(f); }
        QJsonObject root{{"stack", "qt"}, {"scenario", s}, {"duration_s", dur}, {"frames", fa.size()}, {"frame_ms", fa}, {"ops", ops},
                         {"checksum", QString::asprintf("%08x", ck)}, {"notes", notes()}};
        QFile f(a.out); f.open(QIODevice::WriteOnly); f.write(QJsonDocument(root).toJson(QJsonDocument::Compact)); f.close();
        stopAll = true; for (auto &t : coreThreads) t.join(); coreThreads.clear();
        if (producer) producer->stop();
        QCoreApplication::exit(0);
    }
    QString notes() const {
        QString n = QString("Qt %1; scene graph backend=%2 (graphicsApi enum %3); platform=%4. frame_ms = interval between consecutive QQuickWindow::frameSwapped emissions "
                            "(software: after QSGSoftwareRenderLoop rendered the dirty regions and flushed the QBackingStore to X (xcb/SHM); RHI/GL: after swapBuffers returned on the render thread); "
                            "QT_QPA_UPDATE_IDLE_TIME=%5 (Qt default 5 ms pacing of update requests removed unless --paced); redraw requested continuously (window->update() from a queued tick after every frameSwapped; no vsync/timer pacing). First 2 s warm-up discarded (scripts restart at t=0). ")
                        .arg(qVersion()).arg(qgetenv("QT_QUICK_BACKEND").isEmpty() ? "default/RHI" : QString::fromLatin1(qgetenv("QT_QUICK_BACKEND")))
                        .arg(int(view->rendererInterface()->graphicsApi())).arg(QGuiApplication::platformName()).arg(QString::fromLatin1(qgetenv("QT_QPA_UPDATE_IDLE_TIME")));
        const QString s = a.scenario;
        if (s == "idle" || s == "list" || s == "stream" || s == "stress")
            n += "List = QML ListView (reuseItems, cacheBuffer 600) over C++ QAbstractListModel (rows computed in data()); delegate height = row height role; ListView estimates contentHeight for variable heights so scripted offsets are in exact sum-of-heights space but contentY is clamped to the ListView estimate. rows_drawn = number of data(TitleRole) calls (delegate creations + reuse rebinds). ";
        if (s == "stream" || s == "stress") n += "Producer = std::thread (xorshift32), batches posted with QMetaObject::invokeMethod(Qt::QueuedConnection); bounded in-flight counter 100k (drop+count in producer); UI applies the whole batch once per frame with one dataChanged per update. ";
        if (s == "canvas" || s == "core" || s == "stress") n += QString("Canvas impl=%1 (").arg(a.canvasImpl) + (a.canvasImpl == "render" ? "custom QSGRenderNode drawing straight into the software renderer's QPainter" : a.canvasImpl == "qml" ? "QML Canvas (JS Context2D) smoke only" : "QQuickPaintedItem, QPainter into QImage target") + "); gradient fill via per-shape QLinearGradient(ObjectBoundingMode) brushes, antialiased rounded clip path; drop shadows = pre-blurred (box x3, 8px) sprite per (type,size), drawn for 1 shape in 10 (QPainter has no blur primitive). Graph: 10000-pt drawPolyline per frame. ";
        if (s == "text") n += "TextEdit (readOnly, selectByMouse) in a Flickable, selectAll() called at t=5s. layout_ms = setting text + forcing synchronous layout (contentHeight). time_to_first_display_ms = from start of file load until first frameSwapped after text set. ";
        if (s == "images") n += "GridView of 600 Image{asynchronous:true,sourceSize 128x128} via QQuickImageProvider (decode+smooth downscale off UI thread, timed inside provider); zoom via item scale 1->3->1 / 6s; scroll ping-pong 3000px/s. ";
        if (s == "nav") n += "StackView push/pop (300 ms slide) + Popup modal (200 ms fade) cycles [push,push,modal,pop,pop] while a C++ thread busy-loops one core. ";
        return n;
    }

    Args a; QQuickView *view = nullptr; BenchModel model; Producer *producer = nullptr;
    QHash<QString, QObject *> items; std::vector<QQuickItem *> canvases;
    QElapsedTimer clock; qint64 startNs = 0; QMutex fm; std::vector<double> frames; qint64 lastNs = -1, lastFrameNs = 0, t0Ns = 0, lastTickNs = 0;
    bool measuring = false, finished = false, textSelected = false, ttfdDone = false;
    std::atomic<bool> ticked{false}, tickPending{false}, stopAll{false};
    double listPos = 0, prevD = 0, layoutMs = 0, ttfdMs = 0; qint64 textStartNs = 0, textDoneNs = 0;
    uint64_t titleCalls0 = 0, gen0 = 0, drop0 = 0, app0 = 0, calls0 = 0, heavy0 = 0, dec0 = 0, decNs0 = 0;
    std::atomic<uint64_t> calls{0}, heavy{0}, coreAcc{0}; uint64_t busyAcc = 0;
    FrameDriver *drv = nullptr;
    int transitions = 0, modals = 0;
    std::vector<std::thread> coreThreads;
};

int main(int argc, char **argv) {
    Args a;
    for (int i = 1; i < argc; ++i) {
        const QString k = argv[i]; auto nx = [&] { return i + 1 < argc ? QString(argv[++i]) : QString(); };
        if (k == "--scenario") a.scenario = nx(); else if (k == "--duration") a.duration = nx().toDouble();
        else if (k == "--out") a.out = nx(); else if (k == "--rate") a.rate = nx().toInt();
        else if (k == "--shapes") a.shapes = nx().toInt(); else if (k == "--assets") a.assets = nx();
        else if (k == "--canvas-impl") a.canvasImpl = nx(); else if (k == "--backend") a.backend = nx(); else if (k == "--shot") a.shot = nx(); else if (k == "--paced") a.paced = true;
    }
    // Qt paces idle update requests by QT_QPA_UPDATE_IDLE_TIME (default 5 ms): disable it for continuous un-throttled redraw (--paced keeps Qt default)
    if (!a.paced && !qEnvironmentVariableIsSet("QT_QPA_UPDATE_IDLE_TIME")) qputenv("QT_QPA_UPDATE_IDLE_TIME", "0");
    // Software Qt Quick backend by default; --backend opengl (or QSG_RHI_BACKEND/QT_QUICK_BACKEND in env) selects RHI.
    if (a.backend.isEmpty()) a.backend = (qEnvironmentVariableIsSet("QT_QUICK_BACKEND") || qEnvironmentVariableIsSet("QSG_RHI_BACKEND")) ? "env" : "software";
    if (a.backend == "software") QQuickWindow::setSceneGraphBackend("software");
    else if (a.backend == "opengl") QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
    QGuiApplication app(argc, argv);
    qmlRegisterType<CanvasItem>("Bench", 1, 0, "CanvasItem");
    qmlRegisterType<CanvasRenderItem>("Bench", 1, 0, "CanvasRenderItem");
    Bench bench(a);
    QQuickView view;
    bench.setView(&view);
    view.engine()->rootContext()->setContextProperty("bench", &bench);
    if (a.scenario == "images") { bench.imgProv = new ImgProvider(a.assets); view.engine()->addImageProvider("bench", bench.imgProv); }
    view.setResizeMode(QQuickView::SizeRootObjectToView);
    view.setSource(QUrl("qrc:/qml/main.qml"));
    if (view.status() != QQuickView::Ready) { for (auto &e : view.errors()) fprintf(stderr, "%s\n", qPrintable(e.toString())); return 2; }
    view.setMinimumSize(QSize(1280, 720)); view.setMaximumSize(QSize(1280, 720)); view.resize(1280, 720);
    view.setTitle("bench-qt");
    view.show();
    bench.start();
    return app.exec();
}
#include "main.moc"
