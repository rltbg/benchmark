#pragma once
#include <cstdint>
static inline uint32_t fnv(uint32_t i) {
    uint32_t h = 2166136261u;
    for (int k = 0; k < 4; ++k) { h ^= (i >> (8 * k)) & 255u; h *= 16777619u; }
    return h;
}
static inline int rowHeight(int i) { return 48 + 16 * int(fnv(uint32_t(i)) % 6); }
static constexpr int ROWS = 100000;
