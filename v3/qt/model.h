#pragma once
// List model (QAbstractListModel) + native->UI hand-off of update events (lock-free ring / queued invokeMethod).
#include <QAbstractListModel>
#include <QColor>
#include <QLocale>
#include <QMetaObject>
#include <QObject>
#include <atomic>
#include <chrono>
#include <cstring>
#include <thread>
#include <vector>
#include "common.h"

// One update event. kind 0: price of row `a` (model position) := price. kinds 1..3 are structural operations whose
// positions are drawn by the consumer inside [first visible row, rowCount): 1 insert, 2 remove, 3 move (a -> (uint32)price).
struct Ev { uint32_t a; uint32_t kind; double price; };

class RowModel : public QAbstractListModel {
    Q_OBJECT
public:
    enum Roles { TitleRole = Qt::UserRole + 1, SubtitleRole, PriceRole, DotRole, RowHRole, SeedRole };
    struct Row { uint32_t id; double price; };
    using QAbstractListModel::QAbstractListModel;

    int rowCount(const QModelIndex &p = {}) const override { return p.isValid() ? 0 : (explicitMode ? int(rows.size()) : n); }
    QHash<int, QByteArray> roleNames() const override {
        return {{TitleRole, "title"}, {SubtitleRole, "subtitle"}, {PriceRole, "price"}, {DotRole, "dot"}, {RowHRole, "rowH"}, {SeedRole, "seed"}};
    }
    QVariant data(const QModelIndex &ix, int role) const override {
        if (!ix.isValid()) return {};
        const int r = ix.row();
        uint32_t id; double price;
        if (explicitMode) { if (r >= int(rows.size())) return {}; id = rows[size_t(r)].id; price = rows[size_t(r)].price; }
        else { id = base + uint32_t(r); price = (fnv(id + 7) % 100000) / 100.0; }
        switch (role) {
        case TitleRole: ++rowsBound; return QStringLiteral("Item #") + QString::number(id);
        case SubtitleRole:   // "Quote " + i + " — " + (fnv(i+7) % 100000)/100.0, shortest round-trip formatting
            return QStringLiteral("Quote ") + QString::number(id) + QStringLiteral(" — ")
                   + QString::number((fnv(id + 7) % 100000) / 100.0, 'g', QLocale::FloatingPointShortest);
        case PriceRole: return price;
        case DotRole: { const uint32_t f = fnv(id); return QColor(int(f & 255), int((f >> 8) & 255), int((f >> 16) & 255)); }
        case RowHRole: return dense ? hDense(id) : hStd(id);
        case SeedRole: return fnv(id);
        }
        return {};
    }
    // ---- configuration (before a view is bound, or with a reset) ----
    void setImplicit(int count, uint32_t b, bool d) { explicitMode = false; rows.clear(); n = count; base = b; dense = d; }
    void resetImplicit(int count, uint32_t b) { beginResetModel(); setImplicit(count, b, dense); endResetModel(); }
    void makeExplicit() {    // materialise the current implicit rows (needed for insert/remove/move)
        rows.resize(size_t(n));
        for (int i = 0; i < n; ++i) { const uint32_t id = base + uint32_t(i); rows[size_t(i)] = {id, (fnv(id + 7) % 100000) / 100.0}; }
        nextId = base + uint32_t(n); explicitMode = true;
    }
    bool isExplicit() const { return explicitMode; }
    bool isDense() const { return dense; }

    // ---- UI thread: apply a batch of events (once per frame). Value updates are coalesced into dirty ranges and emitted as
    // grouped dataChanged(..., {PriceRole}) ; structural ops flush the ranges first because they shift positions. ----
    // returns the number of events consumed.
    size_t applyEvents(const Ev *ev, size_t cnt, int firstVisible, bool grouped = true) {
        if (!explicitMode) return 0;
        for (size_t k = 0; k < cnt; ++k) {
            const Ev &e = ev[k];
            const int size = int(rows.size());
            if (e.kind == 0) {
                if (size == 0) continue;
                const int i = int(e.a % uint32_t(size));
                rows[size_t(i)].price = e.price;
                if (grouped) { if (dirty.size() < size_t(size)) dirty.resize(size_t(size), 0); if (!dirty[size_t(i)]) { dirty[size_t(i)] = 1; ++dirtyCount; if (i < dMin) dMin = i; if (i > dMax) dMax = i; } }
                else { const QModelIndex q = index(i); emit dataChanged(q, q, {PriceRole}); }
                continue;
            }
            flushDirty();
            const int first = std::clamp(firstVisible, 0, std::max(0, size - 1));
            const int span = size - first;
            if (e.kind == 1) {
                const int pos = first + int(e.a % uint32_t(span + 1));
                const uint32_t id = nextId++;
                beginInsertRows({}, pos, pos); rows.insert(rows.begin() + pos, Row{id, (fnv(id + 7) % 100000) / 100.0}); endInsertRows();
            } else if (e.kind == 2 && span > 1) {
                const int pos = first + int(e.a % uint32_t(span));
                beginRemoveRows({}, pos, pos); rows.erase(rows.begin() + pos); endRemoveRows();
            } else if (e.kind == 3 && span > 1) {
                const int from = first + int(e.a % uint32_t(span)), to = first + int(uint32_t(e.price) % uint32_t(span));
                if (from == to) continue;
                // beginMoveRows destination = row before which the moved row is inserted (in the pre-move numbering)
                if (beginMoveRows({}, from, from, {}, to > from ? to + 1 : to)) {
                    const Row r = rows[size_t(from)];
                    if (to > from) std::rotate(rows.begin() + from, rows.begin() + from + 1, rows.begin() + to + 1);
                    else std::rotate(rows.begin() + to, rows.begin() + from, rows.begin() + from + 1);
                    (void)r; endMoveRows();
                }
            }
        }
        flushDirty();
        return cnt;
    }
    void flushDirty() {
        if (!dirtyCount) return;
        const int hi = std::min(dMax, int(rows.size()) - 1);
        int i = dMin;
        while (i <= hi) {
            if (!dirty[size_t(i)]) { ++i; continue; }
            int j = i; while (j + 1 <= hi && dirty[size_t(j + 1)]) ++j;
            emit dataChanged(index(i), index(j), {PriceRole});
            std::memset(&dirty[size_t(i)], 0, size_t(j - i + 1));
            i = j + 1;
        }
        dirtyCount = 0; dMin = INT32_MAX; dMax = -1;
    }
    size_t size() const { return explicitMode ? rows.size() : size_t(n); }
    double priceAt(int i) const { return rows[size_t(i)].price; }
    mutable uint64_t rowsBound = 0;   // row (re)binds of the title role: delegate creation or reuse
private:
    int n = 0; uint32_t base = 0; bool dense = false, explicitMode = false; uint32_t nextId = 0;
    std::vector<Row> rows;
    std::vector<uint8_t> dirty; size_t dirtyCount = 0; int dMin = INT32_MAX, dMax = -1;
};

// Single-producer single-consumer lock-free ring (power-of-two capacity). The producer thread push()es, the UI thread drains.
template <class T> class SpscRing {
public:
    explicit SpscRing(size_t pow2) : mask(pow2 - 1), buf(pow2) {}
    bool push(const T &v) {
        const size_t h = head.load(std::memory_order_relaxed);
        if (h - tailCache >= buf.size()) { tailCache = tail.load(std::memory_order_acquire); if (h - tailCache >= buf.size()) return false; }
        buf[h & mask] = v; head.store(h + 1, std::memory_order_release); return true;
    }
    size_t size() const { return head.load(std::memory_order_acquire) - tail.load(std::memory_order_acquire); }
    size_t popAll(std::vector<T> &out, size_t max = SIZE_MAX) {
        const size_t t = tail.load(std::memory_order_relaxed), h = head.load(std::memory_order_acquire);
        size_t n = std::min(h - t, max);
        for (size_t i = 0; i < n; ++i) out.push_back(buf[(t + i) & mask]);
        tail.store(t + n, std::memory_order_release); return n;
    }
private:
    size_t mask; std::vector<T> buf;
    alignas(64) std::atomic<size_t> head{0}; size_t tailCache = 0;
    alignas(64) std::atomic<size_t> tail{0};
};

// Native thread -> UI thread hand-off, three strategies (variant handoff=ring|invoke|batch):
//  ring   : lock-free SPSC ring written by the producer thread, drained by the UI tick once per frame (no event posted);
//  invoke : one QMetaObject::invokeMethod(Qt::QueuedConnection) per event (a QMetaCallEvent allocated and posted per event);
//  batch  : events collected in a producer-side vector, one queued invokeMethod per ~1 ms carrying the whole vector.
class UpdateSink {
public:
    enum Mode { Ring, Invoke, Batch };
    UpdateSink(Mode m, QObject *ctx, size_t bound = 1000000) : mode(m), ctx(ctx), bound(bound), ring(1u << 20) {}
    // ---- producer thread ----
    void push(const Ev &e) {
        const uint64_t g = genLocal + 1; genLocal = g; generated.store(g, std::memory_order_relaxed);
        switch (mode) {
        case Ring: if (ring.size() >= bound || !ring.push(e)) dropped.fetch_add(1, std::memory_order_relaxed); break;
        case Invoke:
            if (inflight.load(std::memory_order_relaxed) >= int64_t(bound)) { dropped.fetch_add(1, std::memory_order_relaxed); break; }
            inflight.fetch_add(1, std::memory_order_relaxed);
            QMetaObject::invokeMethod(ctx, [this, e] { inflight.fetch_sub(1, std::memory_order_relaxed); uiPending.push_back(e); }, Qt::QueuedConnection);
            break;
        case Batch: {
            local.push_back(e);
            const auto now = std::chrono::steady_clock::now();
            if (local.size() >= 4096 || now - lastFlush >= std::chrono::milliseconds(1)) flushBatch(now);
            break; }
        }
    }
    void flushBatch(std::chrono::steady_clock::time_point now = std::chrono::steady_clock::now()) {
        lastFlush = now; if (local.empty()) return;
        if (inflight.load(std::memory_order_relaxed) + int64_t(local.size()) > int64_t(bound)) { dropped.fetch_add(local.size(), std::memory_order_relaxed); local.clear(); return; }
        inflight.fetch_add(int64_t(local.size()), std::memory_order_relaxed);
        QMetaObject::invokeMethod(ctx, [this, v = std::move(local)]() mutable {
            inflight.fetch_sub(int64_t(v.size()), std::memory_order_relaxed);
            uiPending.insert(uiPending.end(), v.begin(), v.end()); }, Qt::QueuedConnection);
        local = std::vector<Ev>(); local.reserve(4096);
    }
    // ---- UI thread: take everything that arrived so far ----
    void drain(std::vector<Ev> &out) {
        out.clear();
        if (mode == Ring) ring.popAll(out);
        else out.swap(uiPending);
    }
    Mode mode; QObject *ctx; size_t bound;
    std::atomic<uint64_t> generated{0}, dropped{0}; std::atomic<int64_t> inflight{0};
private:
    SpscRing<Ev> ring; std::vector<Ev> uiPending, local; uint64_t genLocal = 0;
    std::chrono::steady_clock::time_point lastFlush = std::chrono::steady_clock::now();
};
