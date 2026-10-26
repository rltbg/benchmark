#pragma once
// Bench driver (spec v3): scenario scripts, measurement, background threads, JSON output.
// Exposed to QML as the singleton `App` of module `Bench` (QML_SINGLETON + QQmlEngine::setExternalSingletonInstance, Qt 6.12).
#include <QAbstractListModel>
#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMutex>
#include <QQuickTextDocument>
#include <QQuickView>
#include <QQuickWindow>
#include <QTextCursor>
#include <QTextDocument>
#include <QTimer>
#include <QtQml/qqmlregistration.h>
#include <malloc.h>
#include <unistd.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <thread>
#include "common.h"
#include "gfx.h"
#include "model.h"

extern "C" {
#include "benchcore.h"
}

static const double WARMUP = 2.0;
inline const std::chrono::steady_clock::time_point g_procStart = std::chrono::steady_clock::now();
static inline qint64 nowNs() { return std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now() - g_procStart).count(); }

struct Args {
    QString scenario = "idle", out = "out.json", assets, backend, shot;
    QMap<QString, QString> var;
    bool paced = false, pace60 = true; double duration = 12; int n = -1;
    QString v(const QString &k, const QString &def = {}) const { return var.value(k, def); }
};

// ---- labels grid (T2): N text labels whose text changes every frame ----
class LabelModel : public QAbstractListModel {
    Q_OBJECT
public:
    using QAbstractListModel::QAbstractListModel;
    int rowCount(const QModelIndex &p = {}) const override { return p.isValid() ? 0 : int(texts.size()); }
    QHash<int, QByteArray> roleNames() const override { return {{Qt::UserRole + 1, "label"}}; }
    QVariant data(const QModelIndex &ix, int role) const override { return role == Qt::UserRole + 1 && ix.isValid() ? QVariant(texts[size_t(ix.row())]) : QVariant(); }
    void init(int n) { beginResetModel(); texts.assign(size_t(n), QString()); endResetModel(); update(0); }
    // counter + 8 characters derived from fnv(i + counter)
    void update(uint32_t counter, bool signal = true) {
        static const char alpha[] = "abcdefghijklmnopqrstuvwxyz0123456789";
        const size_t n = texts.size();
        for (size_t i = 0; i < n; ++i) {
            const uint32_t f = fnv(uint32_t(i) + counter);
            QString s = QString::number(counter); s.reserve(s.size() + 9); s += QLatin1Char(' ');
            for (int k = 0; k < 8; ++k) s += QLatin1Char(alpha[(f >> (k * 3)) % 36 ? (f >> (k * 3)) % 36 : 1]);
            texts[i] = s;
        }
        if (signal && n) emit dataChanged(index(0), index(int(n) - 1), {Qt::UserRole + 1});
    }
    std::vector<QString> texts;
};

// ---- bindings grid (U2): N QObjects with a bindable int property, labels bound to it in QML ----
class Cell : public QObject {
    Q_OBJECT
    Q_PROPERTY(int value READ value WRITE setValue NOTIFY valueChanged BINDABLE bindableValue)
public:
    explicit Cell(QObject *p = nullptr) : QObject(p) {}
    int value() const { return m_value; }
    void setValue(int v) { m_value = v; }
    QBindable<int> bindableValue() { return &m_value; }
signals:
    void valueChanged();
private:
    Q_OBJECT_BINDABLE_PROPERTY(Cell, int, m_value, &Cell::valueChanged)
};
class CellModel : public QAbstractListModel {
    Q_OBJECT
public:
    using QAbstractListModel::QAbstractListModel;
    int rowCount(const QModelIndex &p = {}) const override { return p.isValid() ? 0 : int(cells.size()); }
    QHash<int, QByteArray> roleNames() const override { return {{Qt::UserRole + 1, "cell"}}; }
    QVariant data(const QModelIndex &ix, int role) const override { return role == Qt::UserRole + 1 && ix.isValid() ? QVariant::fromValue<QObject *>(cells[size_t(ix.row())]) : QVariant(); }
    void init(int n) { beginResetModel(); cells.clear(); for (int i = 0; i < n; ++i) cells.push_back(new Cell(this)); endResetModel(); }
    std::vector<Cell *> cells;
};

// Exact scripted scroll of one ListView (see OPTIMIZATION-LOG L1).
struct ListCtl {
    QQuickItem *lv = nullptr; const HeightIndex *hi = nullptr; double pos = 0, shown = 0, prevD = 0; qint64 realigns = 0, jumps = 0;
    double viewH() const { return lv->height(); }
    void reset(double d) { pos = shown = 0; prevD = d; }
    void drive(double d) {
        if (!lv) return;
        const double vh = viewH();
        const double maxOff = hi ? double(hi->total) - vh : lv->property("contentHeight").toDouble() - vh;
        const double target = std::clamp(pos + (d - prevD), 0.0, std::max(0.0, maxOff)); prevD = d;
        const double delta = target - shown; pos = shown = target;
        double cy = lv->property("contentY").toDouble();
        const double maxStep = vh * 0.5;
        if (std::abs(delta) <= maxStep) { cy += delta; lv->setProperty("contentY", cy); }
        else if (hi) {
            // big jump (fling): ask the view for the exact row, then add the offset inside the row (positionViewAtIndex is public)
            const auto [idx, rem] = hi->rowAt(pos);
            QMetaObject::invokeMethod(lv, "positionViewAtIndex", Q_ARG(int, idx), Q_ARG(int, 0 /*ListView.Beginning*/));
            cy = lv->property("contentY").toDouble() + rem; lv->setProperty("contentY", cy); ++jumps;
        } else {
            double rest = delta;
            while (rest != 0) { const double st = std::clamp(rest, -maxStep, maxStep); cy += st; rest -= st; lv->setProperty("contentY", cy); }
        }
        if (hi) {   // residual drift of the estimated contentHeight: re-align the exact top row with ListView.itemAtIndex()
            const auto [idx, rem] = hi->rowAt(pos);
            QQuickItem *it = nullptr;
            QMetaObject::invokeMethod(lv, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, it), Q_ARG(int, idx));
            if (it) { const double err = (it->y() - cy) + rem; if (std::abs(err) > 0.01) { ++realigns; lv->setProperty("contentY", cy + err); } }
        }
    }
    double topRowError() const {
        if (!lv || !hi) return 0;
        const auto [idx, rem] = hi->rowAt(pos); QQuickItem *it = nullptr;
        QMetaObject::invokeMethod(lv, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, it), Q_ARG(int, idx));
        return it ? (it->y() - lv->property("contentY").toDouble()) + rem : 1e9;
    }
    int firstVisible() const { int i = 0; QMetaObject::invokeMethod(lv, "indexAt", Q_RETURN_ARG(int, i), Q_ARG(qreal, 100.0), Q_ARG(qreal, lv->property("contentY").toDouble() + 1)); return std::max(0, i); }
};

class Bench : public QObject {
    Q_OBJECT
    struct Flow;
    QML_NAMED_ELEMENT(App)
    QML_SINGLETON
    QML_UNCREATABLE("provided from C++ with QQmlEngine::setExternalSingletonInstance")
    Q_PROPERTY(QString scenario READ scenario CONSTANT)
    Q_PROPERTY(QString pane READ pane CONSTANT)
    Q_PROPERTY(int n READ n CONSTANT)
    Q_PROPERTY(QObject *model READ modelObj CONSTANT)
    Q_PROPERTY(QObject *model2 READ model2Obj CONSTANT)
    Q_PROPERTY(QObject *labels READ labelsObj CONSTANT)
    Q_PROPERTY(QObject *cells READ cellsObj CONSTANT)
    Q_PROPERTY(bool squareWhite READ squareWhite NOTIFY squareChanged)
    Q_PROPERTY(bool interactive READ interactive CONSTANT)
    Q_PROPERTY(int richK READ richK CONSTANT)
    Q_PROPERTY(bool layerBlur READ layerBlur CONSTANT)
public:
    enum Sc { Idle, ListScale, ListRich, ListMutate, ListReset, TextBig, TextLabels, TextEdit, TextRich, GfxShapes, GfxPaths, GfxChart, GfxImages, GfxLayers,
              UiTree, UiBind, UiAnim, FfiCalls, FfiCallbacks, FfiBlock, LatLoad, LatWheel, Soak, MemMin, Unknown };
    explicit Bench(const Args &a) : a(a) { init(); }
    QString scenario() const { return a.scenario; }
    QString pane() const { return paneName; }
    int n() const { return N; }
    QObject *modelObj() { return &model; }
    QObject *model2Obj() { return &model2; }
    QObject *labelsObj() { return &labelModel; }
    QObject *cellsObj() { return &cellModel; }
    bool squareWhite() const { return white; }
    bool interactive() const { return sc == LatWheel; }
    int richK() const { return sc == ListRich ? N : 0; }
    bool layerBlur() const { return a.v("effect") == "blur"; }
    void setView(QQuickView *v) { view = v; }
    QQuickView *view = nullptr;
    ImagesItem *imagesItem = nullptr;

    // ---------- scenario table ----------
    static Sc parse(const QString &s) {
        static const QMap<QString, Sc> m{{"idle", Idle}, {"list-scale", ListScale}, {"list-rich", ListRich}, {"list-mutate", ListMutate}, {"list-reset", ListReset},
            {"text-big", TextBig}, {"text-labels", TextLabels}, {"text-edit", TextEdit}, {"text-rich", TextRich}, {"gfx-shapes", GfxShapes}, {"gfx-paths", GfxPaths},
            {"gfx-chart", GfxChart}, {"gfx-images", GfxImages}, {"gfx-layers", GfxLayers}, {"ui-tree", UiTree}, {"startup-scale", UiTree}, {"ui-bind", UiBind}, {"ui-anim", UiAnim},
            {"ffi-calls", FfiCalls}, {"ffi-callbacks", FfiCallbacks}, {"ffi-block", FfiBlock}, {"latency-load", LatLoad}, {"latency-wheel", LatWheel}, {"soak", Soak}, {"mem-min", MemMin}};
        return m.value(s, Unknown);
    }
    // Ticked scenarios redraw continuously (each frameSwapped queues a tick that advances the script and calls update());
    // native ones only render what Qt itself schedules (animations, resize, typing) and count every frame.
    bool ticked() const {
        switch (sc) {
        case Idle: case TextEdit: case UiTree: case UiAnim: case GfxLayers: return false;
        case LatLoad: case LatWheel: { const QString l = a.v("load", "idle"); return l == "fling" || l == "mutate"; }
        default: return true;
        }
    }
    void init() {
        sc = parse(a.scenario); N = a.n;
        if (N < 0) { static const QMap<Sc, int> def{{ListScale, 100000}, {ListRich, 6}, {ListMutate, 10000}, {ListReset, 100000}, {TextBig, 1}, {TextLabels, 1000}, {TextEdit, 10}, {TextRich, 1000},
            {GfxShapes, 1000}, {GfxPaths, 100}, {GfxChart, 10000}, {GfxImages, 100}, {GfxLayers, 8}, {UiTree, 1000}, {UiBind, 1000}, {UiAnim, 1000}, {FfiCalls, 8}, {FfiCallbacks, 10000},
            {FfiBlock, 100}, {LatLoad, 100000}, {LatWheel, 100000}, {Soak, 0}, {MemMin, 0}}; N = def.value(sc, 0); }
        switch (sc) {
        case Idle: case ListScale: case ListRich: case ListMutate: case ListReset: case FfiCallbacks: case FfiBlock: case LatLoad: case LatWheel: paneName = "list"; break;
        case TextBig: case TextEdit: case TextRich: paneName = "text"; break;
        case TextLabels: case UiBind: paneName = "labels"; break;
        case GfxShapes: case GfxPaths: case GfxChart: case GfxImages: case FfiCalls: paneName = "gfx"; break;
        case GfxLayers: paneName = "layers"; break;
        case UiTree: paneName = "tree"; break;
        case UiAnim: paneName = "anim"; break;
        case Soak: case MemMin: paneName = "composite"; break;
        default: paneName = "list";
        }
        // list data set
        const bool dense = sc == ListRich;
        int rows = 100000;
        if (sc == ListScale || sc == ListReset || sc == LatLoad || sc == LatWheel) rows = N;
        if (sc == ListReset) { rows = N; }
        model.setImplicit(rows, 0, dense); hindex.build(rows, 0, dense);
        model2.setImplicit(100000, 0, false);
        if (sc == ListMutate || sc == FfiCallbacks || sc == Soak) { model.makeExplicit(); if (sc == Soak) model2.makeExplicit(); }
        if (sc == LatLoad && a.v("load") == "mutate") model.makeExplicit();
        if (sc == TextLabels) labelModel.init(N);
        if (sc == UiBind) cellModel.init(N);
        needReady = (sc == GfxImages || sc == UiTree || sc == TextBig || sc == TextRich || sc == TextEdit || sc == TextLabels || sc == UiBind || sc == UiAnim || sc == GfxLayers || sc == Soak || sc == MemMin);
        ready = !needReady;
        listCtl[0].hi = (model.isExplicit() ? nullptr : &hindex);
        const QString load = a.v("load", "idle");
        profile = (sc == ListScale || ((sc == LatLoad || sc == LatWheel) && load == "fling")) ? Fling : sc == ListRich ? Rich20k : Slow3k;
        xorData = hindex.xorFnv;
    }
    // ---------- called from QML ----------
    Q_INVOKABLE void registerItem(const QString &role, QObject *o) {
        items[role] = o;
        if (role == "list") listCtl[0].lv = qobject_cast<QQuickItem *>(o);
        else if (role == "list2") listCtl[1].lv = qobject_cast<QQuickItem *>(o);
        else if (role == "gfx" || role == "gfx2") {
            if (auto *g = qobject_cast<GfxBase *>(o)) gfx.push_back(g); else if (auto *r = qobject_cast<ShapesRenderItem *>(o)) renderItems.push_back(r);
            initGfx(o);
        } else if (role == "textedit") te = o;
    }
    Q_INVOKABLE void setReady() { if (!ready) { ready = true; } }
    Q_INVOKABLE void toggleSquare() { white = !white; if (measuring) ++keyEvents; emit squareChanged(); }
    Q_INVOKABLE void focusEditor() { if (te) QMetaObject::invokeMethod(te, "forceActiveFocus"); }
    Q_INVOKABLE QString variant(const QString &k, const QString &def = {}) const { return a.v(k, def); }
    // T1/T3/T4: fill the TextEdit (QML calls this once the item exists)
    Q_INVOKABLE void loadText(QObject *edit, const QString &mode);
    Q_INVOKABLE void layoutDone(QObject *flick) { (void)flick; }
signals:
    void squareChanged();
    void frame();
public:
    void start() {
        startNs = nowNs();
        stall.setTimerType(Qt::PreciseTimer); stall.setInterval(4);
        QObject::connect(&stall, &QTimer::timeout, this, [this] { stallTick(); });
        stall.start();
        startBackground();
        QObject::connect(view, &QQuickWindow::frameSwapped, this, [this] { onFrameSwapped(); }, Qt::DirectConnection);
        QObject::connect(view, &QWindow::widthChanged, this, [this] { onResize(); });
        QObject::connect(view, &QWindow::heightChanged, this, [this] { onResize(); });
        if (sc == TextEdit) QTimer::singleShot(0, this, [this] { focusEditor(); });
        if (ticked()) view->update();
    }

private:
    // ---------- init helpers ----------
    void initGfx(QObject *o) {
        const bool g1 = sc == GfxShapes || sc == FfiCalls || sc == Soak;
        const QString f = a.v("feature", sc == Soak ? "gradient" : "solid");
        if (auto *s = qobject_cast<ShapesItem *>(o)) {
            const int cnt = sc == FfiCalls ? 1000 : (sc == Soak ? 500 : N);
            const bool all = f == "all";
            s->sim.init(cnt, f == "gradient" || all, f == "shadow" || all, f == "clip" || all);
        } else if (auto *r = qobject_cast<ShapesRenderItem *>(o)) {
            const bool all = f == "all";
            r->sim.init(N, f == "gradient" || all, f == "shadow" || all, f == "clip" || all);
        } else if (auto *p = qobject_cast<PathsItem *>(o)) p->init(N, a.v("feature") == "dashed", a.v("rebuild") == "1");
        else if (auto *c = qobject_cast<ChartItem *>(o)) c->init(N, false);
        else if (auto *im = qobject_cast<ImagesItem *>(o)) {
            imagesItem = im;
            im->load(a.assets, N, a.v("big"), [this] { QMetaObject::invokeMethod(this, [this] { setReady(); }, Qt::QueuedConnection); });
        }
        (void)g1;
    }
    void startBackground();
    void stopBackground();
    void flowApply(Flow &f);
    void startGen(Flow &f, uint64_t rate, bool structural);
    void ffiWorker();
    void startFfiWorkers(int k);
    void doReset();
    void onResize() {
        if (!measuring || resizePendingNs) return;
        resizePendingNs = nowNs(); ++resizes;
    }
    void stallTick() {
        const qint64 now = nowNs();
        if (measuring && lastStallNs) stallMax = std::max(stallMax, (now - lastStallNs) / 1e6);
        lastStallNs = now;
        if (!ticked()) lifecycle(now);
    }
    // warm-up / measurement window bookkeeping; returns true once finished
    bool lifecycle(qint64 now) {
        if (finished) return true;
        const double el = (now - startNs) / 1e9;
        if (!measuring && ready && el >= WARMUP) beginMeasure(now);
        if (measuring && (now - t0Ns) / 1e9 >= a.duration) { finish(); return true; }
        return false;
    }
    void onFrameSwapped() {   // emitted on the GUI thread by the basic/software render loops
        const qint64 now = nowNs();
        if (ready && !setupDone) { setupDone = true; setupMs = now / 1e6; if (ttfdBase >= 0 && ttfdMs < 0) ttfdMs = (now - ttfdBase) / 1e6; }
        if (ttfdBase >= 0 && ttfdMs < 0 && textShown) ttfdMs = (now - ttfdBase) / 1e6;
        if (resizePendingNs) { if (measuring) resizeMs.push_back((now - resizePendingNs) / 1e6); resizePendingNs = 0; }
        if (resetPendingNs) { if (measuring) rebuildMs.push_back((now - resetPendingNs) / 1e6); resetPendingNs = 0; }
        {
            QMutexLocker l(&fm);
            if (!ticked() || ticks.exchange(false)) {
                if (measuring && lastNs >= 0) frames.push_back((now - lastNs) / 1e6);
                if (measuring) lastNs = now;
            }
            lastFrameNs = now;
        }
        if (ticked() && !tickPending.exchange(true)) QMetaObject::invokeMethod(this, &Bench::scheduleTick, Qt::QueuedConnection);
    }
    // BENCH_PACING=uncapped: tick right away. 60 (default): next point of a fixed 16.667 ms grid, QTimer::singleShot(nanoseconds, PreciseTimer).
    void scheduleTick() {
        if (!a.pace60) { tick(); return; }
        const qint64 now = nowNs();
        if (nextSlotNs == 0) nextSlotNs = now;
        nextSlotNs += PERIOD_NS;
        if (nextSlotNs <= now) nextSlotNs += ((now - nextSlotNs) / PERIOD_NS + 1) * PERIOD_NS;
        QTimer::singleShot(std::chrono::nanoseconds(nextSlotNs - now), Qt::PreciseTimer, this, [this] { tick(); });
    }
    void tick();
    void scenarioTick(double t, double dt);
    void beginMeasure(qint64 now);
    void finish();
    double cumD(double t) const {   // cumulative scripted scroll distance (unclamped)
        if (profile == Fling) {
            if (t <= 4.0) return 30000.0 * t;
            double d = 120000.0; const double tt = t - 4.0; const int k = int(tt / 1.5);
            for (int i = 0; i < k; ++i) d += (i % 2 == 0 ? 1 : -1) * 75000.0 * (1 - std::exp(-3.0));
            const double tau = tt - 1.5 * k;
            return d + (k % 2 == 0 ? 1 : -1) * 75000.0 * (1 - std::exp(-2 * tau));
        }
        if (profile == Rich20k) return 20000.0 * t;
        return 3000.0 * t;
    }
    static double pingpong(double t, double v, double mx) { if (mx <= 0) return 0; const double u = std::fmod(v * t, 2 * mx); return u <= mx ? u : 2 * mx - u; }
    QString graphicsApiName() const {
        switch (view->rendererInterface()->graphicsApi()) {
        case QSGRendererInterface::Software: return "software";
        case QSGRendererInterface::OpenGL: return "opengl(rhi)";
        default: return QString::number(int(view->rendererInterface()->graphicsApi()));
        }
    }
    QString notes() const;
public:
    static QString version() {
        return QString("Qt %1 (runtime %2, aqtinstall linux_gcc_64 from download.qt.io), g++ %3, CMake 4.4.4, Ninja 1.13.2")
            .arg(QT_VERSION_STR, qVersion(), __VERSION__);
    }
    Args a; Sc sc = Unknown; int N = 0; QString paneName;
    RowModel model, model2; LabelModel labelModel; CellModel cellModel; HeightIndex hindex;
    ListCtl listCtl[2]; QHash<QString, QObject *> items; std::vector<GfxBase *> gfx; QObject *te = nullptr;
    QTimer stall;
    qint64 startNs = 0, t0Ns = 0, lastTickNs = 0, lastNs = -1, lastFrameNs = 0, lastStallNs = 0, nextSlotNs = 0, resizePendingNs = 0, resetPendingNs = 0, ttfdBase = -1;
    static constexpr qint64 PERIOD_NS = 16666667;
    QMutex fm; std::vector<double> frames, resizeMs, rebuildMs, resetCallMs;
    bool measuring = false, finished = false, needReady = false, ready = true, setupDone = false, white = false, textShown = false, selected = false;
    double setupMs = 0, ttfdMs = -1, layoutMs = 0, stallMax = 0; int keyEvents = 0, resizes = 0, resetsDone = 0; double lastResetT = 0;
    std::atomic<bool> ticks{false}, tickPending{false}, stopAll{false};
    // update flows (L3 / F2 / I1 mutate / M1): native or Qt-side producer -> UpdateSink -> UI-thread batch application
    struct Flow { UpdateSink *sink = nullptr; RowModel *model = nullptr; ListCtl *ctl = nullptr; std::thread gen; std::vector<Ev> buf;
                  uint64_t applied = 0, applied0 = 0, structural = 0, structural0 = 0, gen0 = 0, drop0 = 0, signals = 0; uint64_t rate = 0; bool core = false; };
    Flow flows[2]; int nflows = 0;
    enum Profile { Fling, Rich20k, Slow3k } profile = Slow3k;
    HeightIndex hidx[2]; int curIdx = 0; std::thread idxThread; int resetGen = 0; double nextResetT = 0; uint32_t xorData = 0;
    // F1 / F3 / loads
    std::vector<std::thread> workers; std::atomic<uint64_t> heavyCalls{0}, busyAcc{0}; uint64_t heavy0 = 0;
    struct FnStat { std::atomic<uint64_t> calls{0}, bytes{0}, ns{0}; };
    FnStat fs[2][4];     // [phase][noop,add,hash,fill]
    std::atomic<int> phase{0}; qint64 phaseSwitchNs = 0; bool phase2Started = false; QElapsedTimer uiHeavyTimer; double uiHeavyLastT = -1;
    int textBytes = 0; std::atomic<bool> textLoading{false};
    uint64_t counter = 0, rowsBound0 = 0; double lastScriptT = 0; bool ffiStarted = false; std::vector<QQuickItem *> renderItems; QJsonArray notRendered; QString unsupported;
    bool heavyOnUi() const { return a.v("where") == "ui" || a.var.contains("ui"); }
};
