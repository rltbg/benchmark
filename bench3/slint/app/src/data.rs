// Fonctions pures et jeux de données communs (spec v3 §4) : identiques dans les trois stacks.
use std::sync::atomic::{AtomicU64, Ordering};

pub fn fnv(i: u32) -> u32 {
    let mut h: u32 = 2166136261;
    for b in i.to_le_bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}
/// hauteur standard : 48 + 16*(fnv(i) % 6)
#[inline]
pub fn height(i: u32) -> u32 {
    48 + 16 * (fnv(i) % 6)
}
/// hauteur dense : 24 + 4*(fnv(i) % 6)
#[inline]
pub fn dense_height(i: u32) -> u32 {
    24 + 4 * (fnv(i) % 6)
}
pub fn xorshift(x: &mut u32) -> u32 {
    *x ^= *x << 13;
    *x ^= *x >> 17;
    *x ^= *x << 5;
    *x
}
pub const SEED: u32 = 0x2545F491;

pub fn sum_heights(n: usize, dense: bool) -> u64 {
    let f = if dense { dense_height } else { height };
    (0..n as u32).map(|i| f(i) as u64).sum()
}
/// empreinte du jeu de données de test : FNV-1a 32 bits du flux little-endian des hauteurs des n lignes
/// (4 octets par ligne) -> identique dans toute stack qui applique h(i) / hd(i).
pub fn checksum_heights(n: usize, dense: bool) -> u32 {
    let f = if dense { dense_height } else { height };
    let mut h: u32 = 2166136261;
    for i in 0..n as u32 {
        for b in f(i).to_le_bytes() {
            h = (h ^ b as u32).wrapping_mul(16777619);
        }
    }
    h
}

/// Sommes préfixes éparses (un point de contrôle toutes les `STEP` lignes) : O(N/STEP) mémoire,
/// position exacte d'une ligne en < STEP additions. 10 M lignes -> 156 k points (0,6 Mo).
pub const STEP: usize = 64;
pub struct Prefix {
    pub n: usize,
    pub dense: bool,
    ck: Vec<u32>,
    pub total: u64,
}
impl Prefix {
    pub fn build(n: usize, dense: bool) -> Prefix {
        let f = if dense { dense_height } else { height };
        let mut ck = Vec::with_capacity(n / STEP + 2);
        let mut s: u64 = 0;
        for i in 0..n {
            if i % STEP == 0 {
                ck.push(s as u32);
            }
            s += f(i as u32) as u64;
        }
        ck.push(s as u32); // garde (ne sert que si n % STEP == 0)
        Prefix { n, dense, ck, total: s }
    }
    #[inline]
    pub fn h(&self, i: usize) -> u32 {
        if self.dense { dense_height(i as u32) } else { height(i as u32) }
    }
    /// y du haut de la ligne i (i <= n)
    pub fn pos(&self, i: usize) -> u64 {
        if i >= self.n {
            return self.total;
        }
        let b = i / STEP;
        let mut y = self.ck[b] as u64;
        for k in b * STEP..i {
            y += self.h(k) as u64;
        }
        y
    }
    /// plus grande ligne i telle que pos(i) <= off
    pub fn row_at(&self, off: u64) -> usize {
        let off = off.min(self.total.saturating_sub(1));
        let nb = (self.n + STEP - 1) / STEP;
        let b = self.ck[..nb].partition_point(|&p| (p as u64) <= off).saturating_sub(1);
        let mut y = self.ck[b] as u64;
        let mut i = b * STEP;
        while i + 1 < self.n {
            let h = self.h(i) as u64;
            if y + h > off {
                break;
            }
            y += h;
            i += 1;
        }
        i
    }
}

/// Script de défilement déterministe : phase constante (v0 px/s pendant `const_s` s, 0 = aucune) puis
/// flings alternés (v0·exp(-2t), `fling_s` s chacun). `bounce` : aller-retour (réflexion aux bornes) au lieu d'un clamp.
#[derive(Clone)]
pub struct Script {
    pub const_v: f64,
    pub const_s: f64,
    pub fling_v: f64,
    pub fling_s: f64,
    pub bounce: bool,
}
impl Script {
    /// L1 : 30 000 px/s pendant 4 s puis flings de 150 000 px/s
    pub fn l1() -> Script { Script { const_v: 30000.0, const_s: 4.0, fling_v: 150000.0, fling_s: 1.5, bounce: false } }
    /// vitesse constante, aller-retour
    pub fn constant(v: f64) -> Script { Script { const_v: v, const_s: f64::INFINITY, fling_v: 0.0, fling_s: 1.5, bounce: true } }
    /// 4 s constantes puis 6 flings (style I1 `fling`)
    pub fn fling(v0: f64) -> Script { Script { const_v: 0.0, const_s: 0.0, fling_v: v0, fling_s: 1.5, bounce: false } }
    /// position non bornée (px) à t
    fn raw(&self, t: f64) -> f64 {
        let mut d = 0.0;
        let mut a = 0.0;
        if self.const_s > 0.0 {
            let e = t.min(self.const_s);
            d += self.const_v * e;
            a = e;
        }
        if t > a && self.fling_v != 0.0 {
            // flings alternés : intégrale de v0 exp(-2 u) sur [0, min(fling_s, ...)]
            let mut s = a;
            let mut dir = 1.0;
            while s < t {
                let e = t.min(s + self.fling_s);
                d += dir * self.fling_v / 2.0 * ((-2.0 * 0.0f64).exp() - (-2.0 * (e - s)).exp());
                s = e;
                dir = -dir;
            }
        }
        d
    }
}
/// État d'avancement : applique le script avec clamp ou réflexion à chaque pas (comme v2).
pub struct Scroller {
    pub script: Script,
    pub pos: f64,
    last_t: f64,
    prev_raw: f64,
    dir: f64,
}
impl Scroller {
    pub fn new(script: Script) -> Self { Scroller { script, pos: 0.0, last_t: 0.0, prev_raw: 0.0, dir: 1.0 } }
    pub fn reset(&mut self) { self.pos = 0.0; self.last_t = 0.0; self.prev_raw = 0.0; self.dir = 1.0; }
    pub fn advance(&mut self, t: f64, max: f64) -> f64 {
        let r = self.script.raw(t);
        let d = r - self.prev_raw;
        self.prev_raw = r;
        self.last_t = t;
        if self.script.bounce {
            let mut p = self.pos + d * self.dir;
            // réflexion aux bornes
            loop {
                if p > max {
                    p = 2.0 * max - p;
                    self.dir = -self.dir;
                } else if p < 0.0 {
                    p = -p;
                    self.dir = -self.dir;
                } else {
                    break;
                }
                if max <= 0.0 { p = 0.0; break; }
            }
            self.pos = p;
        } else {
            self.pos = (self.pos + d).clamp(0.0, max);
        }
        self.pos
    }
}

/// compteurs partagés (producteurs de fond)
#[derive(Default)]
pub struct Counters {
    pub generated: AtomicU64,
    pub dropped: AtomicU64,
}
impl Counters {
    pub fn add_gen(&self, n: u64) { self.generated.fetch_add(n, Ordering::Relaxed); }
    pub fn add_drop(&self, n: u64) { self.dropped.fetch_add(n, Ordering::Relaxed); }
    pub fn generated_n(&self) -> u64 { self.generated.load(Ordering::Relaxed) }
    pub fn drp(&self) -> u64 { self.dropped.load(Ordering::Relaxed) }
}

/// réflexion (rebond) d'une position non bornée p dans [0, l]
pub fn reflect(p: f32, l: f32) -> f32 {
    let m = p.rem_euclid(2.0 * l);
    if m > l { 2.0 * l - m } else { m }
}
