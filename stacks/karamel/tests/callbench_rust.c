#include <stdio.h>
#include <time.h>
#include "benchcore.h"
int main(void){ uint8_t f[8]={0xA5,1,5,'h','e','l','l','o'}; uint8_t o[64]; size_t ol; struct timespec a,b; uint64_t N=100000000ULL, acc=0;
 clock_gettime(CLOCK_MONOTONIC,&a);
 for(uint64_t i=0;i<N;i++){ f[3]=(uint8_t)i; acc+=core_validate(f,8)+core_hash(f,8); core_parse(f,8,o,64,&ol); acc+=ol; }
 clock_gettime(CLOCK_MONOTONIC,&b); double s=(b.tv_sec-a.tv_sec)+(b.tv_nsec-a.tv_nsec)/1e9;
 printf("Rust lien statique (re-mesure): %.1f M triples(validate+hash+parse)/s (acc=%llu)\n", N/s/1e6,(unsigned long long)acc); return 0;}
