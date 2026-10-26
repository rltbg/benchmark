//! Cœur vérifié. Deux implémentations, une seule active :
//! - `core-pulse` (défaut) : C généré par F* Pulse + KaRaMeL (`core-c/Core_Pulse.c`), compilé par la crate `cc` ;
//! - `core-rust` : crate `benchcore` du dépôt (`core/`), comparaison secondaire sur Linux.

pub const MAGIC: u8 = 0xA5;
pub const VERSION: u8 = 1;
pub const MAX_PAYLOAD: usize = 64;

#[cfg(feature = "core-rust")]
pub const NAME: &str = "rust";
#[cfg(all(feature = "core-pulse", not(feature = "core-rust")))]
pub const NAME: &str = "pulse-c";
#[cfg(not(any(feature = "core-pulse", feature = "core-rust")))]
compile_error!("activer core-pulse ou core-rust");

#[cfg(all(feature = "core-pulse", not(feature = "core-rust")))]
mod imp {
    unsafe extern "C" {
        fn Core_Pulse_hash(b: *mut u8, len: usize) -> u32;
        fn Core_Pulse_validate(b: *mut u8, len: usize) -> bool;
        fn Core_Pulse_parse(b: *mut u8, len: usize, out: *mut u8, cap: usize, out_len: *mut usize) -> bool;
    }
    // Les fonctions Pulse prennent `uint8_t *` non const mais ne modifient jamais `b` (spec Pulse : lecture seule).
    pub fn hash(b: &[u8]) -> u32 {
        unsafe { Core_Pulse_hash(b.as_ptr() as *mut u8, b.len()) }
    }
    pub fn validate(b: &[u8]) -> bool {
        unsafe { Core_Pulse_validate(b.as_ptr() as *mut u8, b.len()) }
    }
    pub fn parse(b: &[u8]) -> Option<Vec<u8>> {
        let mut out = vec![0u8; super::MAX_PAYLOAD];
        let mut n = 0usize;
        let ok = unsafe { Core_Pulse_parse(b.as_ptr() as *mut u8, b.len(), out.as_mut_ptr(), out.len(), &mut n) };
        if ok {
            out.truncate(n);
            Some(out)
        } else {
            None
        }
    }
}

#[cfg(feature = "core-rust")]
mod imp {
    pub fn hash(b: &[u8]) -> u32 {
        benchcore::hash_slice(b)
    }
    pub fn validate(b: &[u8]) -> bool {
        benchcore::validate_slice(b)
    }
    pub fn parse(b: &[u8]) -> Option<Vec<u8>> {
        if benchcore::validate_slice(b) { Some(b[3..].to_vec()) } else { None }
    }
}

pub use imp::{hash, parse, validate};

/// Met le texte en trame `[0xA5][0x01][n][octets UTF-8, n <= 64, tronqué]`.
pub fn frame(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let n = bytes.len().min(MAX_PAYLOAD);
    let mut f = Vec::with_capacity(3 + n);
    f.extend_from_slice(&[MAGIC, VERSION, n as u8]);
    f.extend_from_slice(&bytes[..n]);
    f
}

/// FNV-1a 32 bits des 4 octets petit-boutiste de i (via le cœur vérifié).
pub fn fnv_u32(i: u32) -> u32 {
    hash(&i.to_le_bytes())
}

/// Charge de calcul de `ms` millisecondes (équivalent de `core_heavy`) : hash répétés du cœur.
pub fn heavy(ms: u32) -> u64 {
    let t0 = crate::plat::now_ms();
    let buf = [0x5Au8; 64];
    let mut acc: u64 = 1469598103934665603;
    while crate::plat::now_ms() - t0 < ms as f64 {
        for _ in 0..1000 {
            acc = acc.wrapping_mul(1099511628211) ^ hash(&buf) as u64;
        }
    }
    acc
}

/// Autotest de démarrage : vecteurs FNV et trames valides/invalides.
pub fn selftest() -> Result<(), String> {
    for (s, want) in [("", 0x811C9DC5u32), ("a", 0xE40C292C), ("foobar", 0xBF9CF968)] {
        let got = hash(s.as_bytes());
        if got != want {
            return Err(format!("fnv({s:?})={got:08X} attendu {want:08X}"));
        }
    }
    let mk = |n: usize| -> Vec<u8> {
        let mut f = vec![MAGIC, VERSION, n as u8];
        f.extend(std::iter::repeat_n(0x41u8, n));
        f
    };
    for n in [0usize, 1, 5, 64] {
        let f = mk(n);
        if !validate(&f) {
            return Err(format!("trame valide de {n} octets refusée"));
        }
        match parse(&f) {
            Some(p) if p.len() == n => {}
            _ => return Err(format!("parse de {n} octets incorrect")),
        }
    }
    if validate(&mk(65)) {
        return Err("trame de 65 octets acceptée".into());
    }
    let mut bad_magic = mk(3);
    bad_magic[0] = 0x5A;
    if validate(&bad_magic) {
        return Err("mauvais magic accepté".into());
    }
    let mut bad_ver = mk(3);
    bad_ver[1] = 2;
    if validate(&bad_ver) {
        return Err("mauvaise version acceptée".into());
    }
    let mut bad_len = mk(5);
    bad_len.pop();
    if validate(&bad_len) || parse(&bad_len).is_some() {
        return Err("longueur incohérente (trop courte) acceptée".into());
    }
    let mut long_len = mk(5);
    long_len.push(0);
    if validate(&long_len) {
        return Err("longueur incohérente (trop longue) acceptée".into());
    }
    if validate(&[MAGIC, VERSION]) || validate(&[]) {
        return Err("trame de moins de 3 octets acceptée".into());
    }
    let f = frame("hello");
    if !validate(&f) || parse(&f).as_deref() != Some(b"hello") {
        return Err("aller-retour frame/parse de hello".into());
    }
    let long = "é".repeat(100);
    if frame(&long).len() != 3 + MAX_PAYLOAD || !validate(&frame(&long)) {
        return Err("troncature à 64 octets".into());
    }
    Ok(())
}
