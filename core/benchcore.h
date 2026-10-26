#ifndef BENCHCORE_H
#define BENCHCORE_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
int32_t  core_validate(const uint8_t *p, size_t n);
int32_t  core_parse(const uint8_t *p, size_t n, uint8_t *out, size_t cap, size_t *out_len);
uint32_t core_hash(const uint8_t *p, size_t n);
uint64_t core_heavy(uint32_t ms);
#ifdef __cplusplus
}
#endif
#endif
