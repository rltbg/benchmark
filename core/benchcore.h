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
/* ---- API v2 (bancs v3) ---- */
uint64_t core_noop(void);                                   /* coût pur de la frontière FFI */
uint64_t core_add(uint64_t a, uint64_t b);
void     core_fill(uint8_t *out, size_t n, uint32_t seed);  /* écrit n octets déterministes dans out */
/* Rappel natif -> UI : appelé depuis le thread producteur du cœur */
typedef void (*core_cb)(void *user, uint32_t index, double price);
/* Démarre un thread qui appelle cb `rate` fois/s (index = x % range, prix = x/100.0, xorshift32 0x2545F491).
   Retourne 1 si démarré. */
int32_t  core_producer_start(core_cb cb, void *user, uint64_t rate, uint32_t range);
uint64_t core_producer_stop(void);       /* arrête, joint le thread, retourne le nombre d'appels émis */
uint64_t core_producer_produced(void);
#ifdef __cplusplus
}
#endif
#endif
