/* Adaptateur C-ABI (benchcore.h) au-dessus des fonctions Pulse extraites (Core_Pulse_*). Écrit à la main.
   Passe 2 : len est un size_t de bout en bout (SZ.t en Pulse) -> plus de limite n <= UINT32_MAX ni de clamp de cap.
   Seuls restent les tests de NULL (benchcore.h les spécifie ; la spec Pulse suppose des tableaux vivants). */
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "Custard.h"
int32_t kr_validate(const uint8_t *p, size_t n) {
  if (p == NULL) return 0;
  return Core_Pulse_validate((uint8_t *)p, n) ? 1 : 0;
}
int32_t kr_parse(const uint8_t *p, size_t n, uint8_t *out, size_t cap, size_t *out_len) {
  if (p == NULL || out == NULL || out_len == NULL) return 0;
  return Core_Pulse_parse((uint8_t *)p, n, out, cap, out_len) ? 1 : 0;
}
uint32_t kr_hash(const uint8_t *p, size_t n) {
  if (p == NULL) return 2166136261u;
  return Core_Pulse_hash((uint8_t *)p, n);
}
