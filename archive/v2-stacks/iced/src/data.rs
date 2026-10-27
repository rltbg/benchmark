//! Modele de donnees commun (spec BENCH-SPEC.md), generation du flux, script de defilement.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Instant;

pub const N_ROWS: usize = 100_000;

#[inline]
pub fn fnv(i: u32) -> u32 {
    let mut h: u32 = 2166136261;
    for b in i.to_le_bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}
#[inline]
pub fn row_h(i: u32) -> u32 {
    48 + 16 * (fnv(i) % 6)
}

pub struct ListModel {
    /// prefix[i] = somme des hauteurs des lignes 0..i (len = N+1)
    pub prefix: Vec<u32>,
    /// prix en centiemes
    pub prices: Vec<u32>,
    pub rows_drawn: u64,
    pub updates_applied: u64,
}
impl ListModel {
    pub fn new() -> Self {
        let mut prefix = Vec::with_capacity(N_ROWS + 1);
        let mut s = 0u32;
        prefix.push(0);
        for i in 0..N_ROWS as u32 {
            s += row_h(i);
            prefix.push(s);
        }
        let prices = (0..N_ROWS as u32).map(|i| fnv(i + 7) % 100000).collect();
        Self { prefix, prices, rows_drawn: 0, updates_applied: 0 }
    }
    pub fn total(&self) -> u32 {
        *self.prefix.last().unwrap()
    }
    /// Premiere ligne dont le bas depasse y (recherche binaire sur les sommes prefixes).
    pub fn row_at(&self, y: f64) -> usize {
        let p = self.prefix.partition_point(|&s| (s as f64) <= y);
        p.saturating_sub(1).min(N_ROWS - 1)
    }
}

/// Defilement scripte : 4 s a 3000 px/s puis flings alternes bas/haut (v0 20000 px/s, v=v0*exp(-2t) pendant 1,5 s).
pub fn list_offset(t: f64, maxoff: f64) -> f64 {
    if t < 4.0 {
        return (3000.0 * t).min(maxoff);
    }
    let mut pos = 12000.0f64.min(maxoff);
    let mut tt = t - 4.0;
    let mut k = 0;
    loop {
        let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
        let d = tt.min(1.5);
        let delta = 20000.0 * (1.0 - (-2.0 * d).exp()) / 2.0;
        pos = (pos + dir * delta).clamp(0.0, maxoff);
        if tt < 1.5 {
            return pos;
        }
        tt -= 1.5;
        k += 1;
    }
}

pub struct Stream {
    pub rx: Receiver<u32>,
    pub generated: Arc<AtomicU64>,
    pub dropped: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}
impl Stream {
    pub fn start(rate: u32) -> Self {
        let (tx, rx): (SyncSender<u32>, Receiver<u32>) = sync_channel(100_000);
        let generated = Arc::new(AtomicU64::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (g, d, s) = (generated.clone(), dropped.clone(), stop.clone());
        std::thread::spawn(move || {
            let t0 = Instant::now();
            let mut x: u32 = 0x2545F491;
            let mut made: u64 = 0;
            while !s.load(Ordering::Relaxed) {
                let target = (t0.elapsed().as_secs_f64() * rate as f64) as u64;
                while made < target {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    made += 1;
                    match tx.try_send(x) {
                        Ok(()) => {}
                        Err(TrySendError::Full(_)) => {
                            d.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(TrySendError::Disconnected(_)) => return,
                    }
                }
                g.store(made, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        Self { rx, generated, dropped, stop }
    }
    /// Applique tous les messages en attente (un lot par frame). Retourne le nombre applique.
    pub fn apply(&self, m: &mut ListModel) -> u64 {
        let mut n = 0;
        while let Ok(x) = self.rx.try_recv() {
            m.prices[(x % N_ROWS as u32) as usize] = x; // prix = x/100.0 (centiemes)
            n += 1;
        }
        m.updates_applied += n;
        n
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
