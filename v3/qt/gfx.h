#pragma once
// CPU graphics scenarios (G1..G4): immediate-mode QPainter drawing inside Qt Quick items.
//  - QQuickPaintedItem (renderTarget Image): QPainter raster into a QImage that the scene graph then draws (works on every backend);
//  - QSGRenderNode (shapes only, software backend only): the node draws straight into the software renderer's QPainter.
// QSGGeometryNode-based items are not drawn by the software adaptation (docs: qtquick-visualcanvas-adaptations-software), so
// they are not used.
#include <QElapsedTimer>
#include <QHash>
#include <QImage>
#include <QImageReader>
#include <QLinearGradient>
#include <QPainter>
#include <QPainterPath>
#include <QQuickItem>
#include <QQuickPaintedItem>
#include <QQuickWindow>
#include <QSGRenderNode>
#include <QSGRendererInterface>
#include <QThreadPool>
#include <QtQml/qqmlregistration.h>
#include <cmath>
#include <functional>
#include <vector>
#include "common.h"

// Common interface used by the driver: advance the simulation by dt seconds, then request a repaint.
class GfxBase : public QQuickPaintedItem {
    Q_OBJECT
public:
    using QQuickPaintedItem::QQuickPaintedItem;
    virtual void step(double dt) = 0;
    virtual uint32_t dataChecksum() const = 0;     // XOR of fnv(i) over the data set (ops.checksum_data)
    virtual uint32_t stateChecksum() const = 0;
    virtual QString info() const { return {}; }
};

// ---------------- G1: shapes ----------------
struct ShapeSim {
    int n = 0; bool grad = false, shadow = false, clip = false;
    std::vector<float> x, y, vx, vy, w, h; std::vector<uint8_t> type; std::vector<QBrush> brush; std::vector<QColor> col;
    double W = 1000, H = 600;   // shadow: not rendered by Qt Quick software (no blur API in QPainter, MultiEffect/ShaderEffect not rendered): flag only, NOT emulated
    void init(int count, bool g, bool s, bool c) {
        n = count; grad = g; shadow = s; clip = c;
        x.resize(size_t(n)); y.resize(size_t(n)); vx.resize(size_t(n)); vy.resize(size_t(n)); w.resize(size_t(n)); h.resize(size_t(n));
        type.resize(size_t(n)); brush.clear(); col.clear();
        for (int i = 0; i < n; ++i) {
            const uint32_t u = uint32_t(i);
            const float sz = 16 + 8 * (fnv(u + 3) % 8);
            type[size_t(i)] = uint8_t(i % 3);
            w[size_t(i)] = sz; h[size_t(i)] = type[size_t(i)] == 0 ? sz * 0.75f : sz;
            x[size_t(i)] = float(fnv(u + 11) % uint32_t(1000 - sz)); y[size_t(i)] = float(fnv(u + 13) % uint32_t(600 - sz));
            const float sp = 40 + float(fnv(u + 17) % 160), a = float(fnv(u + 19) % 360) * float(M_PI / 180);
            vx[size_t(i)] = std::cos(a) * sp; vy[size_t(i)] = std::sin(a) * sp;
            const uint32_t f1 = fnv(u), f2 = fnv(u + 5);
            const QColor c1(int(f1 & 255), int((f1 >> 8) & 255), int((f1 >> 16) & 255));
            col.push_back(c1);
            if (grad) {
                QLinearGradient g(0, 0, 0, 1); g.setCoordinateMode(QGradient::ObjectBoundingMode);
                g.setColorAt(0, c1); g.setColorAt(1, QColor(int(f2 & 255), int((f2 >> 8) & 255), int((f2 >> 16) & 255)));
                brush.push_back(QBrush(g));
            }
        }
    }
    void step(double dt) {
        const float fdt = float(dt), fw = float(W), fh = float(H);
        for (size_t i = 0; i < size_t(n); ++i) {
            x[i] += vx[i] * fdt; y[i] += vy[i] * fdt;
            if (x[i] < 0) { x[i] = 0; vx[i] = -vx[i]; } else if (x[i] > fw - w[i]) { x[i] = fw - w[i]; vx[i] = -vx[i]; }
            if (y[i] < 0) { y[i] = 0; vy[i] = -vy[i]; } else if (y[i] > fh - h[i]) { y[i] = fh - h[i]; vy[i] = -vy[i]; }
        }
    }
    double checksumPos() const { double s = 0; for (int i = 0; i < n; ++i) s += double(x[size_t(i)]) + double(y[size_t(i)]); return s; }
    // Draw in item coordinates; the play area is 1000x600 centred horizontally at y = 5 (clamped to the item size).
    void paint(QPainter *p, double iw, double ih) {
        const double cw = std::min(1000.0, iw), ch = std::max(50.0, std::min(600.0, ih - 10));
        if (cw != W || ch != H) { W = cw; H = ch; for (int i = 0; i < n; ++i) { x[size_t(i)] = std::fmod(std::abs(x[size_t(i)]), std::max(1.0f, float(W) - w[size_t(i)])); y[size_t(i)] = std::fmod(std::abs(y[size_t(i)]), std::max(1.0f, float(H) - h[size_t(i)])); } }
        const QRectF cr((iw - cw) / 2, 5, cw, ch);
        p->setRenderHint(QPainter::Antialiasing, true); p->setPen(Qt::NoPen);
        p->save();
        if (clip) { QPainterPath cp; cp.addRoundedRect(cr, 24, 24); p->fillPath(cp, QColor(255, 255, 255)); p->setClipPath(cp, Qt::IntersectClip); }
        else p->fillRect(cr, QColor(255, 255, 255));
        p->translate(cr.topLeft());
        for (int i = 0; i < n; ++i) {
            const size_t k = size_t(i);
            if (grad) p->setBrush(brush[k]); else p->setBrush(col[k]);
            const QRectF r(x[k], y[k], w[k], h[k]);
            if (type[k] == 0) p->drawRect(r); else if (type[k] == 1) p->drawEllipse(r); else p->drawRoundedRect(r, 8, 8);
        }
        p->restore();
    }
};

class ShapesItem : public GfxBase {
    Q_OBJECT
    QML_ELEMENT
public:
    ShapesItem(QQuickItem *parent = nullptr) : GfxBase(parent) { setRenderTarget(QQuickPaintedItem::Image); setAntialiasing(true); }
    void paint(QPainter *p) override { sim.paint(p, width(), height()); }
    void step(double dt) override { sim.step(dt); update(); }
    uint32_t dataChecksum() const override { uint32_t x = 0; for (int i = 0; i < sim.n; ++i) x ^= fnv(uint32_t(i)); return x; }
    uint32_t stateChecksum() const override { return fnv(uint32_t(sim.checksumPos())); }
    ShapeSim sim;
};

// Variant impl=render: custom QSGRenderNode drawing straight into the software renderer's QPainter (no intermediate QImage).
class ShapesRenderNode : public QSGRenderNode {
public:
    StateFlags changedStates() const override { return {}; }
    RenderingFlags flags() const override { return BoundedRectRendering; }
    QRectF rect() const override { return QRectF(0, 0, w, h); }
    // software backend: the renderer exposes its QPainter as QSGRendererInterface::PainterResource; the node applies matrix(),
    // inheritedOpacity() and the clip given as a QRegion (QSGRenderNode::render docs).
    void render(const RenderState *state) override {
        auto *p = static_cast<QPainter *>(win->rendererInterface()->getResource(win, QSGRendererInterface::PainterResource));
        if (!p) return;
        p->resetTransform();
        if (const QRegion *cr = state->clipRegion(); cr && !cr->isEmpty()) p->setClipRegion(*cr, Qt::IntersectClip);
        p->setTransform(matrix() ? matrix()->toTransform() : QTransform(), false);
        p->setOpacity(inheritedOpacity());
        sim->paint(p, w, h);
    }
    ShapeSim *sim = nullptr; QQuickWindow *win = nullptr; double w = 0, h = 0;
};
class ShapesRenderItem : public QQuickItem {
    Q_OBJECT
    QML_ELEMENT
public:
    ShapesRenderItem(QQuickItem *parent = nullptr) : QQuickItem(parent) { setFlag(ItemHasContents, true); }
    QSGNode *updatePaintNode(QSGNode *old, UpdatePaintNodeData *) override {
        auto *nd = static_cast<ShapesRenderNode *>(old); if (!nd) nd = new ShapesRenderNode;
        nd->sim = &sim; nd->win = window(); nd->w = width(); nd->h = height();
        nd->markDirty(QSGNode::DirtyMaterial | QSGNode::DirtyGeometry | QSGNode::DirtyForceUpdate);
        return nd;
    }
    ShapeSim sim;
};

// ---------------- G2: Bezier paths ----------------
class PathsItem : public GfxBase {
    Q_OBJECT
    QML_ELEMENT
public:
    PathsItem(QQuickItem *parent = nullptr) : GfxBase(parent) { setRenderTarget(QQuickPaintedItem::Image); setAntialiasing(true); }
    // rebuild=true : every path is rebuilt each frame from animated control points (v0);
    // rebuild=false: paths are built once and translated by an animated offset (cached QPainterPath)
    void init(int count, bool dashed_, bool rebuild_) {
        n = count; dashed = dashed_; rebuild = rebuild_;
        px.resize(size_t(n) * 25 * 2); ox.resize(size_t(n)); oy.resize(size_t(n)); vx.resize(size_t(n)); vy.resize(size_t(n)); col.resize(size_t(n));
        for (int i = 0; i < n; ++i) {
            const uint32_t u = uint32_t(i);
            const float cx = float(fnv(u * 7 + 1) % 900), cy = float(fnv(u * 7 + 2) % 500);
            for (int k = 0; k < 25; ++k) {   // start point + 8 segments x 3 points
                px[(size_t(i) * 25 + size_t(k)) * 2] = cx + float(fnv(u * 131 + uint32_t(k) * 2) % 200);
                px[(size_t(i) * 25 + size_t(k)) * 2 + 1] = cy + float(fnv(u * 131 + uint32_t(k) * 2 + 1) % 120);
            }
            const float sp = 20 + float(fnv(u + 17) % 80), a = float(fnv(u + 19) % 360) * float(M_PI / 180);
            vx[size_t(i)] = std::cos(a) * sp; vy[size_t(i)] = std::sin(a) * sp; ox[size_t(i)] = oy[size_t(i)] = 0;
            const uint32_t f = fnv(u); col[size_t(i)] = QColor(int(f & 255), int((f >> 8) & 255), int((f >> 16) & 255));
        }
        if (!rebuild) { cached.resize(size_t(n)); for (int i = 0; i < n; ++i) cached[size_t(i)] = build(i, 0); }
    }
    QPainterPath build(int i, float ph) const {
        QPainterPath p; const float *q = &px[size_t(i) * 50];
        p.moveTo(q[0], q[1]);
        for (int s = 0; s < 8; ++s) {
            const float *a = q + 2 + s * 6;
            const float j = 6.f * std::sin(ph + float(s));
            p.cubicTo(a[0] + j, a[1], a[2], a[3] - j, a[4], a[5]);
        }
        return p;
    }
    void step(double dt) override {
        phase += float(dt);
        for (int i = 0; i < n; ++i) {
            const size_t k = size_t(i);
            ox[k] += vx[k] * float(dt); oy[k] += vy[k] * float(dt);
            if (ox[k] < -200 || ox[k] > 200) vx[k] = -vx[k];
            if (oy[k] < -150 || oy[k] > 150) vy[k] = -vy[k];
        }
        update();
    }
    void paint(QPainter *p) override {
        p->setRenderHint(QPainter::Antialiasing, true);
        p->translate((width() - 1000) / 2, 5); p->setClipRect(QRectF(0, 0, 1000, 600));
        p->fillRect(QRectF(0, 0, 1000, 600), QColor(255, 255, 255));
        QPen pen(QColor(0, 0, 0), 2); if (dashed) pen.setStyle(Qt::DashLine);
        p->setBrush(Qt::NoBrush);
        for (int i = 0; i < n; ++i) {
            pen.setColor(col[size_t(i)]); p->setPen(pen);
            if (rebuild) { p->save(); p->translate(ox[size_t(i)], oy[size_t(i)]); p->drawPath(build(i, phase)); p->restore(); }
            else { p->save(); p->translate(ox[size_t(i)], oy[size_t(i)]); p->drawPath(cached[size_t(i)]); p->restore(); }
        }
    }
    uint32_t dataChecksum() const override { uint32_t x = 0; for (int i = 0; i < n; ++i) x ^= fnv(uint32_t(i)); return x; }
    uint32_t stateChecksum() const override { double s = 0; for (int i = 0; i < n; ++i) s += double(ox[size_t(i)]) + double(oy[size_t(i)]); return fnv(uint32_t(std::abs(s))); }
    int n = 0; bool dashed = false, rebuild = false; float phase = 0;
    std::vector<float> px, ox, oy, vx, vy; std::vector<QColor> col; std::vector<QPainterPath> cached;
};

// ---------------- G3: real-time chart ----------------
class ChartItem : public GfxBase {
    Q_OBJECT
    QML_ELEMENT
public:
    ChartItem(QQuickItem *parent = nullptr) : GfxBase(parent) { setRenderTarget(QQuickPaintedItem::Image); }
    // points live in ring order with a fixed x (index * sx): new points overwrite the oldest ones, nothing is recomputed for
    // the others; paint() draws the two ring segments with a translation each (+ the segment joining them).
    void init(int count, bool decimate_) {
        n = count; decimate = decimate_; sx = 1000.0 / double(n);
        pts.resize(size_t(n)); for (int i = 0; i < n; ++i) pts[size_t(i)] = QPointF(i * sx, 150);
        head = 0; rng = 0x2545F491u;
    }
    void step(double) override {
        for (int k = 0; k < 1000; ++k) {   // 1000 points added per frame
            rng = xorshift32(rng); t += 0.002;
            const double v = 150 + 100 * std::sin(t) + (rng % 4000) / 100.0 - 20;
            pts[size_t(head)].setY(v); head = (head + 1) % n; ++added;
        }
        update();
    }
    void paint(QPainter *p) override {
        p->fillRect(QRectF(0, 0, width(), height()), QColor(24, 28, 40));
        p->translate((width() - 1000) / 2, 5);
        p->setRenderHint(QPainter::Antialiasing, false); p->setBrush(Qt::NoBrush); p->setPen(QPen(QColor(80, 220, 120), 1));
        const double shift = head * sx;
        // oldest point = index head (left edge), newest = index head-1 (right edge)
        p->save(); p->translate(-shift, 0); p->drawPolyline(pts.data() + head, n - head); p->restore();
        p->save(); p->translate(1000.0 - shift, 0); if (head > 0) p->drawPolyline(pts.data(), head); p->restore();
        if (head > 0 && head < n) p->drawLine(QPointF(pts[size_t(n - 1)].x() - shift, pts[size_t(n - 1)].y()), QPointF(0 + 1000.0 - shift, pts[0].y()));
    }
    uint32_t dataChecksum() const override { uint32_t x = 0; for (int i = 0; i < n; ++i) x ^= fnv(uint32_t(i)); return x; }
    uint32_t stateChecksum() const override { return fnv(uint32_t(added)) ^ fnv(uint32_t(pts[size_t(head)].y() * 100)); }
    int n = 0, head = 0; bool decimate = false; double sx = 1, t = 0; uint32_t rng = 1; uint64_t added = 0; std::vector<QPointF> pts;
};

// ---------------- G4: images ----------------
class ImagesItem : public GfxBase {
    Q_OBJECT
    QML_ELEMENT
public:
    ImagesItem(QQuickItem *parent = nullptr) : GfxBase(parent) { setRenderTarget(QQuickPaintedItem::Image); }
    // 600 PNG (256x256) decoded once on a QThreadPool (off the UI thread); `ready` is set when all are decoded.
    void load(const QString &dir, int count, const QString &big, std::function<void()> done) {
        n = count; bigSide = big.toInt();
        if (bigSide) { const QString f = dir + "/big" + big + ".png";
            QThreadPool::globalInstance()->start([this, f, done] { QImageReader r(f); r.setAllocationLimit(0); QImage im = r.read(); decodeMs = 0;
                bigImg = im.convertToFormat(QImage::Format_ARGB32_Premultiplied); ready = true; done(); }); return; }
        imgs.resize(600); auto *left = new std::atomic<int>(600); decodeNs = 0;
        for (int i = 0; i < 600; ++i)
            QThreadPool::globalInstance()->start([this, i, dir, left, done] {
                QElapsedTimer t; t.start();
                char nm[64]; snprintf(nm, sizeof nm, "/images/img%03d.png", i);
                imgs[size_t(i)] = QImage(dir + nm).convertToFormat(QImage::Format_ARGB32_Premultiplied);
                decodeNs += t.nsecsElapsed();
                if (left->fetch_sub(1) == 1) { ready = true; delete left; done(); } });
        for (int i = 0; i < n; ++i) { const uint32_t u = uint32_t(i);
            sc.push_back(0.15f + float(fnv(u + 1) % 100) / 100.f * 0.5f); rot0.push_back(float(fnv(u + 2) % 360)); rv.push_back(20.f + float(fnv(u + 3) % 100));
            op.push_back(0.3f + float(fnv(u + 4) % 70) / 100.f); cx.push_back(float(fnv(u + 5) % 1000)); cy.push_back(float(fnv(u + 6) % 600));
            vx.push_back(float(fnv(u + 7) % 120) - 60); vy.push_back(float(fnv(u + 8) % 120) - 60); }
    }
    void step(double dt) override {
        tt += dt;
        for (size_t i = 0; i < cx.size(); ++i) { cx[i] += vx[i] * float(dt); cy[i] += vy[i] * float(dt);
            if (cx[i] < 0 || cx[i] > 1000) vx[i] = -vx[i]; if (cy[i] < 0 || cy[i] > 600) vy[i] = -vy[i]; }
        update();
    }
    void paint(QPainter *p) override {
        if (!ready) return;
        p->setRenderHint(QPainter::SmoothPixmapTransform, true);
        p->translate((width() - 1000) / 2, 5); p->fillRect(QRectF(0, 0, 1000, 600), QColor(240, 240, 240)); p->setClipRect(QRectF(0, 0, 1000, 600));
        if (bigSide) {   // pan / zoom / rotation of one huge image
            const double zoom = 0.05 + 0.15 * (1 + std::sin(tt * 0.5)), side = double(bigImg.width());
            p->translate(500, 300); p->rotate(tt * 20); p->scale(zoom * 4096.0 / side * 1.0, zoom * 4096.0 / side * 1.0);
            p->translate(-side / 2 + std::sin(tt * 0.3) * side * 0.2, -side / 2 + std::cos(tt * 0.2) * side * 0.2);
            p->drawImage(0, 0, bigImg); return;
        }
        for (size_t i = 0; i < cx.size(); ++i) {
            p->save(); p->setOpacity(op[i]); p->translate(cx[i], cy[i]); p->rotate(rot0[i] + rv[i] * tt); p->scale(sc[i], sc[i]);
            p->drawImage(QPointF(-128, -128), imgs[i % 600]); p->restore();
        }
    }
    uint32_t dataChecksum() const override { uint32_t x = 0; for (int i = 0; i < n; ++i) x ^= fnv(uint32_t(i)); return x; }
    uint32_t stateChecksum() const override { double s = 0; for (size_t i = 0; i < cx.size(); ++i) s += double(cx[i]) + double(cy[i]); return fnv(uint32_t(s)); }
    QString info() const override { return bigSide ? QString("big=%1").arg(bigSide) : QString(); }
    int n = 0, bigSide = 0; std::atomic<bool> ready{false}; std::atomic<uint64_t> decodeNs{0}; double decodeMs = 0, tt = 0;
    std::vector<QImage> imgs; QImage bigImg; std::vector<float> sc, rot0, rv, op, cx, cy, vx, vy;
};
