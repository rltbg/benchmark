//! Cœur C-ABI commun du banc. Mêmes règles de trame que spec/Core.fst :
//! [0xA5][version=1][n][payload n octets], n <= 64.
#![allow(clippy::missing_safety_doc)]
use std::slice;

pub const MAGIC: u8 = 0xA5;
pub const VERSION: u8 = 1;
pub const MAX_PAYLOAD: usize = 64;

pub fn validate_slice(b: &[u8]) -> bool {
    b.len() >= 3 && b[0] == MAGIC && b[1] == VERSION && (b[2] as usize) <= MAX_PAYLOAD && b.len() - 3 == b[2] as usize
}
pub fn hash_slice(b: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &x in b { h = (h ^ x as u32).wrapping_mul(16777619); }
    h
}

/// 1 si la trame est valide, 0 sinon.
#[no_mangle]
pub unsafe extern "C" fn core_validate(p: *const u8, n: usize) -> i32 {
    if p.is_null() { return 0; }
    validate_slice(slice::from_raw_parts(p, n)) as i32
}
/// Copie le payload dans out (capacité cap), écrit sa longueur. Retourne 1 si ok, 0 sinon.
#[no_mangle]
pub unsafe extern "C" fn core_parse(p: *const u8, n: usize, out: *mut u8, cap: usize, out_len: *mut usize) -> i32 {
    if p.is_null() || out.is_null() || out_len.is_null() { return 0; }
    let b = slice::from_raw_parts(p, n);
    if !validate_slice(b) { return 0; }
    let m = b.len() - 3;
    if m > cap { return 0; }
    std::ptr::copy_nonoverlapping(b.as_ptr().add(3), out, m);
    *out_len = m;
    1
}
/// FNV-1a 32 bits.
#[no_mangle]
pub unsafe extern "C" fn core_hash(p: *const u8, n: usize) -> u32 {
    if p.is_null() { return 2166136261; }
    hash_slice(slice::from_raw_parts(p, n))
}
/// Appel lourd : calcule pendant `ms` millisecondes (horloge murale), renvoie un checksum.
#[no_mangle]
pub extern "C" fn core_heavy(ms: u32) -> u64 {
    let t = std::time::Instant::now();
    let mut acc: u64 = 1469598103934665603;
    let buf = [0x5Au8; 64];
    while t.elapsed().as_millis() < ms as u128 {
        for _ in 0..1000 { acc = acc.wrapping_mul(1099511628211) ^ hash_slice(&buf) as u64; }
    }
    acc
}
// ---------------------------------------------------------------- API v2 (bancs v3)
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::JoinHandle;

/// Appel sans argument ni travail : mesure le coût pur de la frontière FFI.
#[no_mangle]
pub extern "C" fn core_noop() -> u64 { 0 }
/// Appel avec 2 scalaires : coût FFI + passage d'arguments.
#[no_mangle]
pub extern "C" fn core_add(a: u64, b: u64) -> u64 { a.wrapping_add(b) }
/// Écrit n octets déterministes (xorshift32 de graine `seed`) dans `out` : tampon rempli côté natif.
#[no_mangle]
pub unsafe extern "C" fn core_fill(out: *mut u8, n: usize, seed: u32) {
    if out.is_null() { return; }
    let buf = slice::from_raw_parts_mut(out, n);
    let mut x = if seed == 0 { 0x2545F491 } else { seed };
    for b in buf.iter_mut() { x ^= x << 13; x ^= x >> 17; x ^= x << 5; *b = x as u8; }
}

/// Rappel natif -> UI : (user, index, prix). Appelé depuis le thread producteur du cœur.
pub type CoreCb = unsafe extern "C" fn(user: *mut core::ffi::c_void, index: u32, price: f64);
struct SendPtr(*mut core::ffi::c_void);
unsafe impl Send for SendPtr {}
static PRODUCED: AtomicU64 = AtomicU64::new(0);
static RUNNING: AtomicBool = AtomicBool::new(false);
static HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// Démarre un thread producteur qui appelle `cb` `rate` fois par seconde (par rafales d'1 ms),
/// avec index = x % range et prix = x / 100.0 (xorshift32, graine 0x2545F491 : même flux que le banc).
/// Retourne 1 si démarré, 0 si déjà en cours ou paramètres invalides.
#[no_mangle]
pub unsafe extern "C" fn core_producer_start(cb: Option<CoreCb>, user: *mut core::ffi::c_void, rate: u64, range: u32) -> i32 {
    let cb = match cb { Some(c) => c, None => return 0 };
    if rate == 0 || range == 0 || RUNNING.swap(true, Ordering::SeqCst) { return 0; }
    PRODUCED.store(0, Ordering::SeqCst);
    let user = SendPtr(user);
    let h = std::thread::spawn(move || {
        let user = user;
        let mut x: u32 = 0x2545F491;
        let t0 = std::time::Instant::now();
        let mut emitted: u64 = 0;
        while RUNNING.load(Ordering::Relaxed) {
            let due = (t0.elapsed().as_nanos() as u64).saturating_mul(rate) / 1_000_000_000;
            // rattrapage borné à 50 ms de retard pour ne pas rafaler indéfiniment après une pause
            let due = due.min(emitted + rate / 20 + 1);
            while emitted < due {
                x ^= x << 13; x ^= x >> 17; x ^= x << 5;
                cb(user.0, x % range, x as f64 / 100.0);
                emitted += 1;
            }
            PRODUCED.store(emitted, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_micros(500));
        }
        PRODUCED.store(emitted, Ordering::Relaxed);
    });
    *HANDLE.lock().unwrap() = Some(h);
    1
}
/// Arrête le producteur (attend la fin du thread) ; retourne le nombre d'appels émis.
#[no_mangle]
pub extern "C" fn core_producer_stop() -> u64 {
    RUNNING.store(false, Ordering::SeqCst);
    if let Some(h) = HANDLE.lock().unwrap().take() { let _ = h.join(); }
    PRODUCED.load(Ordering::SeqCst)
}
/// Nombre d'appels émis jusqu'ici.
#[no_mangle]
pub extern "C" fn core_producer_produced() -> u64 { PRODUCED.load(Ordering::Relaxed) }

#[cfg(test)]
mod t {
    use super::*;
    #[test] fn v2() {
        assert_eq!(core_noop(), 0); assert_eq!(core_add(2, 3), 5);
        let mut b = [0u8; 64]; unsafe { core_fill(b.as_mut_ptr(), 64, 1) }; let mut c = [0u8; 64]; unsafe { core_fill(c.as_mut_ptr(), 64, 1) };
        assert_eq!(b, c); assert!(b.iter().any(|&x| x != 0));
        use std::sync::atomic::AtomicU64 as A; static N: A = A::new(0);
        unsafe extern "C" fn cb(_u: *mut core::ffi::c_void, i: u32, _p: f64) { assert!(i < 1000); N.fetch_add(1, Ordering::Relaxed); }
        assert_eq!(unsafe { core_producer_start(Some(cb), core::ptr::null_mut(), 100_000, 1000) }, 1);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let n = core_producer_stop();
        assert!(n > 15_000 && n < 45_000, "n={}", n); assert_eq!(n, N.load(Ordering::Relaxed));
        assert_eq!(unsafe { core_producer_start(None, core::ptr::null_mut(), 1, 1) }, 0);
    }
    #[test] fn vectors() { assert_eq!(hash_slice(b""), 0x811C9DC5); assert_eq!(hash_slice(b"a"), 0xE40C292C); assert_eq!(hash_slice(b"foobar"), 0xBF9CF968); }
    #[test] fn frames() {
        let mut f = vec![0xA5, 1, 5]; f.extend_from_slice(b"hello");
        assert!(validate_slice(&f)); f[2] = 4; assert!(!validate_slice(&f));
        let mut g = vec![0xA5, 1, 65]; g.extend(vec![0u8; 65]); assert!(!validate_slice(&g));
    }
}
