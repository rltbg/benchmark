// Modèle de liste : 100 000 lignes, hauteur h(i)=48+16*(fnv(i)%6) (fnv = cœur vérifié), prix mis à jour 1000/s
// par une tâche de fond (xorshift32, graine 0x2545F491 : index = x % 100000, prix = x / 100.0).
#pragma once
#include <QAbstractListModel>
#include <QElapsedTimer>
#include <QThread>
#include <QTimer>
#include <QVector>
#include <QPair>
#include <cstdint>
#include <QtQml/qqmlregistration.h>
extern "C" {
#include "Custard.h"
}

inline uint32_t fnvOfIndex(uint32_t i) {
    uint8_t b[4] = {uint8_t(i), uint8_t(i >> 8), uint8_t(i >> 16), uint8_t(i >> 24)};   // little-endian
    return Core_Pulse_hash(b, 4);
}

using PriceBatch = QVector<QPair<int, double>>;
Q_DECLARE_METATYPE(PriceBatch)

class PriceFeed : public QObject {
    Q_OBJECT
public:
    explicit PriceFeed(QObject *parent = nullptr) : QObject(parent) {}
public slots:
    void start() {
        m_clock.start();
        m_timer = new QTimer(this);
        m_timer->setTimerType(Qt::PreciseTimer);
        m_timer->setInterval(16);
        connect(m_timer, &QTimer::timeout, this, &PriceFeed::tick);
        m_timer->start();
    }
signals:
    void batch(PriceBatch updates);
private:
    void tick() {
        const qint64 due = m_clock.elapsed();            // 1 mise à jour par ms écoulée
        PriceBatch out;
        while (m_done < due) {
            m_x ^= m_x << 13; m_x ^= m_x >> 17; m_x ^= m_x << 5;   // xorshift32
            out.append({int(m_x % 100000u), m_x / 100.0});
            ++m_done;
        }
        if (!out.isEmpty()) emit batch(out);
    }
    QElapsedTimer m_clock;
    QTimer *m_timer = nullptr;
    uint32_t m_x = 0x2545F491u;
    qint64 m_done = 0;
};

class PriceModel : public QAbstractListModel {
    Q_OBJECT
    QML_ELEMENT
public:
    enum Roles { TitleRole = Qt::UserRole + 1, PriceRole, HeightRole };
    explicit PriceModel(QObject *parent = nullptr) : QAbstractListModel(parent), m_prices(100000) {
        for (int i = 0; i < 100000; ++i) m_prices[i] = (fnvOfIndex(uint32_t(i) ^ 0x9E3779B9u) % 100000u) / 100.0;
        qRegisterMetaType<PriceBatch>();
    }
    ~PriceModel() override { stopFeed(); }
    int rowCount(const QModelIndex &p = {}) const override { return p.isValid() ? 0 : 100000; }
    QVariant data(const QModelIndex &ix, int role) const override {
        if (!ix.isValid()) return {};
        const int i = ix.row();
        switch (role) {
        case TitleRole: return QStringLiteral("Item #%1").arg(i);
        case PriceRole: return QString::number(m_prices[i], 'f', 2);
        case HeightRole: return 48 + 16 * int(fnvOfIndex(uint32_t(i)) % 6u);
        }
        return {};
    }
    QHash<int, QByteArray> roleNames() const override {
        return {{TitleRole, "title"}, {PriceRole, "price"}, {HeightRole, "rowHeight"}};
    }
    // Lance la tâche de fond (fil dédié si Qt a les threads, sinon minuterie sur le fil principal : wasm monofil).
    Q_INVOKABLE void startFeed() {
        if (m_feed) return;
        m_feed = new PriceFeed;
        connect(m_feed, &PriceFeed::batch, this, &PriceModel::apply, Qt::QueuedConnection);
#if QT_CONFIG(thread)
        m_thread = new QThread(this);
        m_feed->moveToThread(m_thread);
        connect(m_thread, &QThread::started, m_feed, &PriceFeed::start);
        connect(m_thread, &QThread::finished, m_feed, &QObject::deleteLater);
        m_thread->start();
#else
        m_feed->setParent(this);
        m_feed->start();
#endif
    }
    qint64 updates() const { return m_updates; }
private slots:
    void apply(PriceBatch b) {
        for (const auto &u : b) {
            m_prices[u.first] = u.second;
            const QModelIndex ix = index(u.first);
            emit dataChanged(ix, ix, {PriceRole});
        }
        m_updates += b.size();
    }
private:
    void stopFeed() {
#if QT_CONFIG(thread)
        if (m_thread) { m_thread->quit(); m_thread->wait(); }
#endif
    }
    QVector<double> m_prices;
    PriceFeed *m_feed = nullptr;
#if QT_CONFIG(thread)
    QThread *m_thread = nullptr;
#endif
    qint64 m_updates = 0;
};
