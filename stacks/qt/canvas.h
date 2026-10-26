#pragma once
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>
#include <QQuickPaintedItem>
#include <QQuickWindow>
#include <QSGRenderNode>
#include <QSGRendererInterface>
#include <QPainter>
#include <QPainterPath>
#include <QLinearGradient>
#include <QImage>
#include <QHash>
#include <cmath>
#include <vector>
#include "common.h"

// Shared simulation + immediate-mode QPainter drawing used by both canvas items.
struct ShapeSim {
    int n = 0; bool withGraph = true;
    std::vector<float> x, y, vx, vy, w, h;
    std::vector<uint8_t> type;
    std::vector<QBrush> brush;
    std::vector<float> gr;            // graph ring buffer (10000 points)
    int gHead = 0; uint32_t rng = 0x12345678u; double tAcc = 0;
    double sumPos = 0;
    QHash<int, QImage> shadows;       // blurred shadow sprites keyed by (type,size)
    double W = 1000, H = 600;         // clipped play area

    void init(int count, bool graph) {
        n = count; withGraph = graph;
        x.resize(n); y.resize(n); vx.resize(n); vy.resize(n); w.resize(n); h.resize(n); type.resize(n); brush.resize(n);
        for (int i = 0; i < n; ++i) {
            const float s = 16 + 8 * (fnv(i + 3) % 8);
            type[i] = i % 3;
            w[i] = s; h[i] = type[i] == 0 ? s * 0.75f : s;
            x[i] = fnv(i + 11) % int(1000 - s); y[i] = fnv(i + 13) % int(600 - s);
            const float sp = 40 + fnv(i + 17) % 160, a = (fnv(i + 19) % 360) * float(M_PI / 180);
            vx[i] = std::cos(a) * sp; vy[i] = std::sin(a) * sp;
            const uint32_t f1 = fnv(i), f2 = fnv(i + 5);
            QLinearGradient g(0, 0, 0, 1); g.setCoordinateMode(QGradient::ObjectBoundingMode);
            g.setColorAt(0, QColor(f1 & 255, (f1 >> 8) & 255, (f1 >> 16) & 255));
            g.setColorAt(1, QColor(f2 & 255, (f2 >> 8) & 255, (f2 >> 16) & 255));
            brush[i] = QBrush(g);
        }
        gr.assign(10000, 50.f);
    }
    void setArea(double aw, double ah) {
        if (aw == W && ah == H) return;
        W = aw; H = ah;
        for (int i = 0; i < n; ++i) { x[i] = std::fmod(std::abs(x[i]), std::max(1.0, W - w[i])); y[i] = std::fmod(std::abs(y[i]), std::max(1.0, H - h[i])); }
    }
    void step(double dt) {
        for (int i = 0; i < n; ++i) {
            x[i] += vx[i] * dt; y[i] += vy[i] * dt;
            if (x[i] < 0) { x[i] = 0; vx[i] = -vx[i]; } else if (x[i] > W - w[i]) { x[i] = W - w[i]; vx[i] = -vx[i]; }
            if (y[i] < 0) { y[i] = 0; vy[i] = -vy[i]; } else if (y[i] > H - h[i]) { y[i] = H - h[i]; vy[i] = -vy[i]; }
        }
        if (withGraph) {   // one new point per frame
            rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
            tAcc += 0.05;
            gr[gHead] = 50.f + 30.f * std::sin(tAcc) + (rng % 2000) / 100.f - 10.f;
            gHead = (gHead + 1) % int(gr.size());
        }
    }
    double checksumPos() const { double s = 0; for (int i = 0; i < n; ++i) s += x[i] + y[i]; return s; }

    const QImage &shadowFor(int i) {
        const int key = type[i] * 1000 + int(w[i]);
        auto it = shadows.find(key);
        if (it != shadows.end()) return *it;
        const int pad = 14, iw = int(w[i]) + 2 * pad, ih = int(h[i]) + 2 * pad;
        QImage img(iw, ih, QImage::Format_ARGB32_Premultiplied); img.fill(0);
        { QPainter p(&img); p.setRenderHint(QPainter::Antialiasing); p.setPen(Qt::NoPen); p.setBrush(QColor(0, 0, 0, 110));
          const QRectF r(pad, pad, w[i], h[i]);
          if (type[i] == 0) p.drawRect(r); else if (type[i] == 1) p.drawEllipse(r); else p.drawRoundedRect(r, 8, 8); }
        // 8px blur ~ 3 box passes of radius 3 (separable) on the alpha channel (black shadow => premultiplied = alpha<<24)
        std::vector<float> a(iw * ih), b(iw * ih);
        for (int yy = 0; yy < ih; ++yy) { const QRgb *l = reinterpret_cast<const QRgb *>(img.constScanLine(yy)); for (int xx = 0; xx < iw; ++xx) a[yy * iw + xx] = qAlpha(l[xx]); }
        const int R = 3;
        for (int pass = 0; pass < 3; ++pass) {
            for (int yy = 0; yy < ih; ++yy) for (int xx = 0; xx < iw; ++xx) { float s = 0; for (int k = -R; k <= R; ++k) { int q = xx + k; if (q >= 0 && q < iw) s += a[yy * iw + q]; } b[yy * iw + xx] = s / (2 * R + 1); }
            for (int yy = 0; yy < ih; ++yy) for (int xx = 0; xx < iw; ++xx) { float s = 0; for (int k = -R; k <= R; ++k) { int q = yy + k; if (q >= 0 && q < ih) s += b[q * iw + xx]; } a[yy * iw + xx] = s / (2 * R + 1); }
        }
        for (int yy = 0; yy < ih; ++yy) { QRgb *l = reinterpret_cast<QRgb *>(img.scanLine(yy)); for (int xx = 0; xx < iw; ++xx) l[xx] = uint32_t(a[yy * iw + xx] + 0.5f) << 24; }
        return shadows.insert(key, img).value();
    }

    // Draw everything in item coordinates (0,0 = item top-left), item size iw x ih.
    void paint(QPainter *p, double iw, double ih) {
        const double cw = std::min(1000.0, iw), ch = std::max(50.0, std::min(600.0, ih - 115));
        setArea(cw, ch);
        const QRectF cr((iw - cw) / 2, 5, cw, ch);
        p->setRenderHint(QPainter::Antialiasing, true);
        p->setPen(Qt::NoPen);
        QPainterPath clip; clip.addRoundedRect(cr, 24, 24);
        p->fillPath(clip, QColor(255, 255, 255));
        p->save();
        p->setClipPath(clip, Qt::IntersectClip);
        p->translate(cr.topLeft());
        for (int i = 0; i < n; ++i) {
            if (i % 10 == 0) { const QImage &s = shadowFor(i); p->drawImage(QPointF(x[i] + 2 - 14, y[i] + 2 - 14), s); }
            p->setBrush(brush[i]);
            const QRectF r(x[i], y[i], w[i], h[i]);
            if (type[i] == 0) p->drawRect(r); else if (type[i] == 1) p->drawEllipse(r); else p->drawRoundedRect(r, 8, 8);
        }
        p->restore();
        if (withGraph) {
            const QRectF g(cr.left(), cr.bottom() + 10, cw, 100);
            p->fillRect(g, QColor(24, 28, 40));
            static thread_local std::vector<QPointF> pts;
            const int N = int(gr.size()); pts.resize(N);
            const double sx = cw / N;
            for (int k = 0; k < N; ++k) pts[k] = QPointF(g.left() + k * sx, g.bottom() - gr[(gHead + k) % N] * 0.9);
            p->setBrush(Qt::NoBrush); p->setPen(QPen(QColor(80, 220, 120), 1));
            p->drawPolyline(pts.data(), N);
        }
    }
};

// Variant A: QQuickPaintedItem (QPainter into a QImage, uploaded as a QSGPainterNode). Works on every backend.
class CanvasItem : public QQuickPaintedItem {
    Q_OBJECT
    QML_ELEMENT
public:
    CanvasItem(QQuickItem *parent = nullptr) : QQuickPaintedItem(parent) { setRenderTarget(QQuickPaintedItem::Image); setAntialiasing(true); }
    void paint(QPainter *p) override { sim.paint(p, width(), height()); }
    ShapeSim sim;
};

// Variant B: custom QSGRenderNode drawing straight into the software renderer's QPainter (no intermediate image).
class CanvasRenderNode : public QSGRenderNode {
public:
    StateFlags changedStates() const override { return {}; }
    RenderingFlags flags() const override { return BoundedRectRendering; }
    QRectF rect() const override { return QRectF(0, 0, w, h); }
    // Software backend only: the renderer exposes its QPainter (QSGRendererInterface::PainterResource) and saves/restores
    // it around render(). The node applies matrix(), inheritedOpacity() and the clip region given as a plain QRegion
    // (QSGRenderNode docs: "software ... use no scissor or stencil. There the clip region is provided as an ordinary QRegion").
    void render(const RenderState *state) override {
        auto *ri = win->rendererInterface();
        auto *p = static_cast<QPainter *>(ri->getResource(win, QSGRendererInterface::PainterResource));
        if (!p) return;
        p->resetTransform();   // clip region is in window coordinates: set it before applying the node matrix
        if (const QRegion *cr = state->clipRegion(); cr && !cr->isEmpty()) p->setClipRegion(*cr, Qt::IntersectClip);
        p->setTransform(matrix() ? matrix()->toTransform() : QTransform(), false);
        p->setOpacity(inheritedOpacity());
        sim->paint(p, w, h);
    }
    ShapeSim *sim = nullptr; QQuickWindow *win = nullptr; double w = 0, h = 0;
};
class CanvasRenderItem : public QQuickItem {
    Q_OBJECT
    QML_ELEMENT
public:
    CanvasRenderItem(QQuickItem *parent = nullptr) : QQuickItem(parent) { setFlag(ItemHasContents, true); }
    QSGNode *updatePaintNode(QSGNode *old, UpdatePaintNodeData *) override {
        auto *nd = static_cast<CanvasRenderNode *>(old);
        if (!nd) nd = new CanvasRenderNode;
        nd->sim = &sim; nd->win = window(); nd->w = width(); nd->h = height();
        nd->markDirty(QSGNode::DirtyMaterial | QSGNode::DirtyGeometry | QSGNode::DirtyForceUpdate);
        return nd;
    }
    ShapeSim sim;
};
