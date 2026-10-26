/* Adaptateur C-ABI (benchcore.h) au-dessus des fonctions KaRaMeL. Écrit à la main (≈25 lignes).
   Renomme core_* en kr_* (via -D) pour pouvoir lier à côté du cœur Rust. Limite : n est size_t côté
   benchcore.h mais uint32_t côté Low* -> n > UINT32_MAX est refusé (validate/parse = 0, hash = offset FNV). */
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "Core_Low.h"
int32_t kr_validate(const uint8_t *p, size_t n) {
  if (p == NULL || n > UINT32_MAX) return 0;
  return Core_Low_validate((uint8_t *)p, (uint32_t)n) ? 1 : 0;
}
int32_t kr_parse(const uint8_t *p, size_t n, uint8_t *out, size_t cap, size_t *out_len) {
  if (p == NULL || out == NULL || out_len == NULL || n > UINT32_MAX) return 0;
  /* la capacité effective est bornée à 64 (le payload ne peut pas dépasser 64) */
  uint32_t c = cap > 64 ? 64 : (uint32_t)cap;
  uint32_t l = 0;
  if (!Core_Low_parse((uint8_t *)p, (uint32_t)n, out, c, &l)) return 0;
  *out_len = l;
  return 1;
}
uint32_t kr_hash(const uint8_t *p, size_t n) {
  if (p == NULL || n > UINT32_MAX) return 2166136261u;
  return Core_Low_hash((uint8_t *)p, (uint32_t)n);
}
