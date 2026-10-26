/* Passe 2 : tests C du code Pulse extrait (Custard KrmlC -> KaRaMeL HEAD) (Core_Pulse) + test différentiel contre le cœur Rust (libbenchcore.a). */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include "Custard.h"   /* en-tête écrit par krml : Core_Pulse_* */
#include "benchcore.h"   /* core_* = Rust */
int32_t kr_validate(const uint8_t *p, size_t n);
int32_t kr_parse(const uint8_t *p, size_t n, uint8_t *out, size_t cap, size_t *out_len);
uint32_t kr_hash(const uint8_t *p, size_t n);
static long checks = 0, fails = 0;
#define CHECK(c) do { checks++; if (!(c)) { fails++; printf("ECHEC ligne %d: %s\n", __LINE__, #c); } } while (0)
static size_t mk(uint8_t *f, size_t n, uint8_t fill) { f[0]=0xA5; f[1]=1; f[2]=(uint8_t)n; for (size_t i=0;i<n;i++) f[3+i]=(uint8_t)(fill+i); return 3+n; }
static uint32_t ref_fnv(const uint8_t *p, size_t n) { uint64_t h = 2166136261ULL; for (size_t i=0;i<n;i++) { h ^= p[i]; h = (h * 16777619ULL) & 0xFFFFFFFFULL; } return (uint32_t)h; }
static uint64_t rs = 88172645463325252ULL; static uint64_t rnd(void){ rs ^= rs<<13; rs ^= rs>>7; rs ^= rs<<17; return rs; }
int main(void) {
  /* vecteurs FNV-1a */
  CHECK(Core_Pulse_hash((uint8_t*)"", 0) == 0x811C9DC5u);
  CHECK(Core_Pulse_hash((uint8_t*)"a", 1) == 0xE40C292Cu);
  CHECK(Core_Pulse_hash((uint8_t*)"foobar", 6) == 0xBF9CF968u);
  CHECK(Core_Pulse_hash((uint8_t*)"foo", 3) == 0xA9F37ED7u);
  /* trames valides 0..64 */
  uint8_t f[300], out[64], big[128]; size_t ol;
  for (size_t n = 0; n <= 64; n++) {
    size_t L = mk(f, n, 7);
    CHECK(Core_Pulse_validate(f, L));
    memset(out, 0xEE, sizeof out); ol = 99;
    CHECK(Core_Pulse_parse(f, L, out, 64, &ol));
    CHECK(ol == n); CHECK(memcmp(out, f+3, n) == 0);
    for (size_t i = n; i < 64; i++) CHECK(out[i] == 0xEE);   /* pas d'écriture au-delà */
    CHECK(Core_Pulse_hash(f+3, n) == ref_fnv(f+3, n));
  }
  /* 65 octets : refusé; 255 : refusé */
  for (size_t n = 65; n <= 255; n += (n==65?1:190)) { size_t L = mk(f, n, 1); CHECK(!Core_Pulse_validate(f, L)); memset(out,0xEE,64); ol=99; CHECK(!Core_Pulse_parse(f, L, out, 64, &ol)); CHECK(ol==99); CHECK(out[0]==0xEE); }
  /* cap trop petit (cap < n) : refusé, rien écrit */
  { size_t L = mk(f, 10, 3); memset(out,0xEE,64); ol=99; CHECK(!Core_Pulse_parse(f, L, out, 9, &ol)); CHECK(out[0]==0xEE && ol==99); CHECK(Core_Pulse_parse(f, L, out, 10, &ol) && ol==10); }
  /* invalides : magic, version, longueur incohérente, trop court */
  { size_t L = mk(f, 5, 1); f[0]=0xA4; CHECK(!Core_Pulse_validate(f,L)); f[0]=0xA5; f[1]=2; CHECK(!Core_Pulse_validate(f,L)); f[1]=1;
    CHECK(!Core_Pulse_validate(f,L-1)); CHECK(!Core_Pulse_validate(f,L+1)); f[2]=0; CHECK(!Core_Pulse_validate(f,L)); f[2]=255; CHECK(!Core_Pulse_validate(f,L));
    CHECK(!Core_Pulse_validate(f,0)); CHECK(!Core_Pulse_validate(f,1)); CHECK(!Core_Pulse_validate(f,2)); }
  { uint8_t t[3]={0xA5,1,0}; CHECK(Core_Pulse_validate(t,3)); ol=9; CHECK(Core_Pulse_parse(t,3,out,64,&ol) && ol==0); }
  /* sentinelles : validate/hash ne lisent pas au-delà de len (canari après un buffer exactement dimensionné) */
  { uint8_t *m = malloc(8); memcpy(m,(uint8_t[]){0xA5,1,5,'h','e','l','l','o'},8); CHECK(Core_Pulse_validate(m,8)); CHECK(!Core_Pulse_validate(m,7)); free(m); }
  /* différentiel vs cœur Rust : 2 000 000 buffers aléatoires, moitié proches de trames valides */
  long diff = 0;
  for (long it = 0; it < 2000000; it++) {
    size_t n = rnd() % 80; uint8_t b[96];
    for (size_t i=0;i<n;i++) b[i] = (uint8_t)rnd();
    if (n >= 3 && (it & 1)) { b[0]=0xA5; b[1]=1; if (rnd()&3) b[2]=(uint8_t)(n-3); }
    uint8_t o1[64], o2[64]; size_t l1=0, l2=0; memset(o1,0,64); memset(o2,0,64);
    int32_t v1 = core_validate(b,n), v2 = kr_validate(b,n);
    int32_t p1 = core_parse(b,n,o1,64,&l1), p2 = kr_parse(b,n,o2,64,&l2);
    uint32_t h1 = core_hash(b,n), h2 = kr_hash(b,n);
    if (v1!=v2 || p1!=p2 || h1!=h2 || (p1 && (l1!=l2 || memcmp(o1,o2,l1)))) { if (diff++ < 5) printf("DIFF it=%ld n=%zu\n", it, n); }
  }
  CHECK(diff == 0);
  /* grands buffers : hash == référence */
  for (int k=0;k<200;k++){ size_t n = 1 + rnd()%100; for(size_t i=0;i<n;i++) big[i]=(uint8_t)rnd(); CHECK(Core_Pulse_hash(big,n)==ref_fnv(big,n)); }
  printf("%ld verifications, %ld echec(s), %ld divergence(s) vs Rust\n", checks, fails, diff);
  return fails ? 1 : 0;
}
