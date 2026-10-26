// Canvas : 2 000 formes animées (1/3 rectangles, 1/3 cercles, 1/3 rectangles arrondis), dégradé linéaire à 2 couleurs, rebond.
#pragma once
#include <QQuickPaintedItem>
#include <QPainter>
#include <QLinearGradient>
#include <QElapsedTimer>
#include <QRandomGenerator>
#include <QVector>
#include <QQuickWindow>
#include <QtQml/qqmlregistration.h>

class ShapesItem : public QQuickPaintedItem {
    Q_OBJECT
    QML_ELEMENT
public:
    explicit ShapesItem(QQuickItem *parent = nullptr) : QQuickPaintedItem(parent) {
        QRandomGenerator rng(12345);
        m_s.resize(2000);
        for (int i = 0; i < m_s.size(); ++i) {
            Shape &s = m_s[i];
            s.kind = i % 3;
            s.w = 10 + rng.bounded(30); s.h = s.kind == 1 ? s.w : 10 + rng.bounded(30);
            s.x = rng.bounded(1000) / 1000.0; s.y = rng.bounded(1000) / 1000.0;   // normalisés, étalés au premier rendu
            s.vx = (rng.bounded(200) - 100) * 1.2; s.vy = (rng.bounded(200) - 100) * 1.2;   // px/s
            s.c1 = QColor::fromHsv(rng.bounded(360), 200, 255);
            s.c2 = QColor::fromHsv(rng.bounded(360), 220, 160);
        }
        m_clock.start();
        setAntialiasing(true);
    }
    void paint(QPainter *p) override {
        const qint64 now = m_clock.nsecsElapsed();
        const double dt = qMin(0.1, (now - m_last) / 1e9);
        m_last = now;
        const double W = width(), H = height();
        if (!m_placed && W > 0 && H > 0) { for (Shape &s : m_s) { s.x *= qMax(1.0, W - s.w); s.y *= qMax(1.0, H - s.h); } m_placed = true; }
        for (Shape &s : m_s) {
            s.x += s.vx * dt; s.y += s.vy * dt;
            if (s.x < 0) { s.x = 0; s.vx = -s.vx; } else if (s.x + s.w > W) { s.x = qMax(0.0, W - s.w); s.vx = -s.vx; }
            if (s.y < 0) { s.y = 0; s.vy = -s.vy; } else if (s.y + s.h > H) { s.y = qMax(0.0, H - s.h); s.vy = -s.vy; }
            QLinearGradient g(s.x, s.y, s.x + s.w, s.y + s.h);
            g.setColorAt(0, s.c1); g.setColorAt(1, s.c2);
            p->setBrush(g); p->setPen(Qt::NoPen);
            const QRectF r(s.x, s.y, s.w, s.h);
            if (s.kind == 0) p->drawRect(r);
            else if (s.kind == 1) p->drawEllipse(r);
            else p->drawRoundedRect(r, 8, 8);
        }
        // boucle d'animation : prochaine image tant que l'onglet est visible
        if (isVisible()) QMetaObject::invokeMethod(this, [this] { update(); }, Qt::QueuedConnection);
    }
protected:
    void itemChange(ItemChange c, const ItemChangeData &d) override {
        QQuickPaintedItem::itemChange(c, d);
        if (c == ItemVisibleHasChanged && d.boolValue) { m_last = m_clock.nsecsElapsed(); update(); }
    }
private:
    struct Shape { int kind; double x, y, w, h, vx, vy; QColor c1, c2; };
    QVector<Shape> m_s;
    QElapsedTimer m_clock;
    qint64 m_last = 0;
    bool m_placed = false;
};
