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
#[cfg(test)]
mod t {
    use super::*;
    #[test] fn vectors() { assert_eq!(hash_slice(b""), 0x811C9DC5); assert_eq!(hash_slice(b"a"), 0xE40C292C); assert_eq!(hash_slice(b"foobar"), 0xBF9CF968); }
    #[test] fn frames() {
        let mut f = vec![0xA5, 1, 5]; f.extend_from_slice(b"hello");
        assert!(validate_slice(&f)); f[2] = 4; assert!(!validate_slice(&f));
        let mut g = vec![0xA5, 1, 65]; g.extend(vec![0u8; 65]); assert!(!validate_slice(&g));
    }
}
