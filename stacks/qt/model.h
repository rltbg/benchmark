#pragma once
#include <chrono>
#include <QAbstractListModel>
#include <QColor>
#include <QHash>
#include <atomic>
#include <thread>
#include <vector>
#include "common.h"

struct Upd { uint32_t idx; double price; };

// 100k rows computed on the fly (no per-row storage); only price overrides are stored.
class BenchModel : public QAbstractListModel {
    Q_OBJECT
public:
    enum Roles { TitleRole = Qt::UserRole + 1, SubtitleRole, PriceRole, DotRole, RowHRole };
    using QAbstractListModel::QAbstractListModel;
    int rowCount(const QModelIndex &p = {}) const override { return p.isValid() ? 0 : ROWS; }
    QHash<int, QByteArray> roleNames() const override {
        return {{TitleRole, "title"}, {SubtitleRole, "subtitle"}, {PriceRole, "price"}, {DotRole, "dot"}, {RowHRole, "rowH"}};
    }
    QVariant data(const QModelIndex &ix, int role) const override {
        if (!ix.isValid()) return {};
        const int i = ix.row();
        switch (role) {
        case TitleRole: ++titleCalls; return QStringLiteral("Item #") + QString::number(i);
        case SubtitleRole: {
            const QString q = QStringLiteral("Quote ") + QString::number(i) + QStringLiteral(" — ")
                              + QString::number((fnv(i + 7) % 100000) / 100.0, 'f', 2) + QStringLiteral("   ");
            return q.repeated(6);
        }
        case PriceRole: { auto it = prices.constFind(i); return it == prices.constEnd() ? (fnv(i + 7) % 100000) / 100.0 : it.value(); }
        case DotRole: { uint32_t f = fnv(i); return QColor(f & 255, (f >> 8) & 255, (f >> 16) & 255); }
        case RowHRole: return rowHeight(i);
        }
        return {};
    }
    // called on the UI thread (via queued invokeMethod from the producer thread)
    void enqueue(std::vector<Upd> &&b) {
        if (pending.empty()) pending = std::move(b);
        else pending.insert(pending.end(), b.begin(), b.end());
    }
    // called once per frame on the UI thread: applies the whole batch
    void applyPending() {
        if (pending.empty()) return;
        for (const Upd &u : pending) {
            prices.insert(int(u.idx), u.price);
            const QModelIndex ix = index(int(u.idx));
            emit dataChanged(ix, ix, {PriceRole});
        }
        applied += pending.size();
        inflight.fetch_sub(int64_t(pending.size()), std::memory_order_relaxed);
        pending.clear();
    }
    uint64_t applied = 0;
    mutable uint64_t titleCalls = 0;   // row (re)binds: delegate creation or reuse
    std::atomic<int64_t> inflight{0};  // updates posted but not yet applied (bounded by 100k)
private:
    QHash<int, double> prices;
    std::vector<Upd> pending;
};

// Background producer: xorshift32 seed 0x2545F491, rate R updates/s, never blocks.
class Producer {
public:
    Producer(BenchModel *m, int rate) : model(m), rate(rate) {}
    void start() { th = std::thread([this] { run(); }); }
    void stop() { stopFlag = true; if (th.joinable()) th.join(); }
    std::atomic<uint64_t> generated{0}, dropped{0};
private:
    void run() {
        using clk = std::chrono::steady_clock;
        uint32_t x = 0x2545F491u;
        const auto t0 = clk::now();
        uint64_t made = 0;
        while (!stopFlag) {
            const double el = std::chrono::duration<double>(clk::now() - t0).count();
            const uint64_t due = uint64_t(el * rate);
            if (due > made) {
                const uint64_t n = due - made;
                std::vector<Upd> b; b.reserve(n);
                for (uint64_t k = 0; k < n; ++k) {
                    x ^= x << 13; x ^= x >> 17; x ^= x << 5;
                    b.push_back({x % 100000u, x / 100.0});
                }
                made = due; generated = made;
                if (model->inflight.load(std::memory_order_relaxed) + int64_t(n) > 100000) {
                    dropped += n;                       // bounded queue full: drop + count
                } else {
                    model->inflight.fetch_add(int64_t(n), std::memory_order_relaxed);
                    BenchModel *m = model;
                    QMetaObject::invokeMethod(m, [m, b = std::move(b)]() mutable { m->enqueue(std::move(b)); }, Qt::QueuedConnection);
                }
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
    }
    BenchModel *model; int rate; std::thread th; std::atomic<bool> stopFlag{false};
};
