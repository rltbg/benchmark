#pragma once
// Pure functions shared by every stack (spec v3 section 4) + the exact height index used by the scripted scrolling.
#include <QtGlobal>
#include <algorithm>
#include <cstdint>
#include <utility>
#include <vector>

static inline uint32_t fnv(uint32_t i) {
    uint32_t h = 2166136261u;
    for (int k = 0; k < 4; ++k) { h ^= (i >> (8 * k)) & 255u; h *= 16777619u; }
    return h;
}
static inline int hStd(uint32_t id) { return 48 + 16 * int(fnv(id) % 6); }
static inline int hDense(uint32_t id) { return 24 + 4 * int(fnv(id) % 6); }
static inline uint32_t xorshift32(uint32_t x) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x; }

// Prefix sums of the row heights, one entry per block of 1024 rows (80 KB for 10 M rows): exact offset <-> row mapping.
struct HeightIndex {
    static constexpr int B = 1024;
    int n = 0; uint32_t base = 0; bool dense = false;
    std::vector<qint64> blk;     // blk[b] = offset of row b*B
    qint64 total = 0;            // sum of heights  (ops.sum_heights)
    uint32_t xorFnv = 0;         // XOR of fnv(base+i)  (ops.checksum_data of row data sets)
    int h(int i) const { return dense ? hDense(base + i) : hStd(base + i); }
    void build(int n_, uint32_t base_, bool dense_) {
        n = n_; base = base_; dense = dense_; blk.clear(); blk.reserve(size_t(n) / B + 2);
        qint64 off = 0; uint32_t x = 0;
        for (int i = 0; i < n; ++i) {
            if (i % B == 0) blk.push_back(off);
            const uint32_t f = fnv(base + i); x ^= f;
            off += dense ? 24 + 4 * int(f % 6) : 48 + 16 * int(f % 6);
        }
        total = off; xorFnv = x;
    }
    // index of the row containing offset y, and y's offset inside that row
    std::pair<int, double> rowAt(double y) const {
        if (n == 0) return {0, 0.0};
        const qint64 yi = qint64(std::max(0.0, y));
        const size_t b = size_t(std::upper_bound(blk.begin(), blk.end(), yi) - blk.begin()) - 1;
        int i = int(b) * B; qint64 off = blk[b];
        while (i < n - 1 && off + h(i) <= yi) { off += h(i); ++i; }
        return {i, y - double(off)};
    }
};
