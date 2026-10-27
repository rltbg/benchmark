// Cœur C-ABI v2 (core/benchcore.h) appelé par les fonctions `extern "C"` de la crate rlib `benchcore`.
// Les pointeurs de fonction sont opacifiés par `black_box` pour que la LTO fat n'inline pas le cœur dans
// la boucle appelante : chaque appel est un vrai appel avec l'ABI C, comme depuis un autre langage.
use std::ffi::c_void;
use std::hint::black_box;

pub type ValidateFn = unsafe extern "C" fn(*const u8, usize) -> i32;
pub type ParseFn = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32;
pub type HashFn = unsafe extern "C" fn(*const u8, usize) -> u32;
pub type HeavyFn = unsafe extern "C" fn(u32) -> u64;
pub type NoopFn = extern "C" fn() -> u64;
pub type AddFn = extern "C" fn(u64, u64) -> u64;
pub type FillFn = unsafe extern "C" fn(*mut u8, usize, u32);
pub type CoreCb = unsafe extern "C" fn(*mut c_void, u32, f64);
pub type ProdStartFn = unsafe extern "C" fn(Option<CoreCb>, *mut c_void, u64, u32) -> i32;
pub type ProdStopFn = extern "C" fn() -> u64;
pub type ProdCountFn = extern "C" fn() -> u64;

#[derive(Clone, Copy)]
pub struct Core {
    pub validate: ValidateFn,
    pub parse: ParseFn,
    pub hash: HashFn,
    pub heavy: HeavyFn,
    pub noop: NoopFn,
    pub add: AddFn,
    pub fill: FillFn,
    pub producer_start: ProdStartFn,
    pub producer_stop: ProdStopFn,
    pub producer_produced: ProdCountFn,
}
impl Core {
    pub fn get() -> Core {
        Core {
            validate: black_box(benchcore::core_validate as ValidateFn),
            parse: black_box(benchcore::core_parse as ParseFn),
            hash: black_box(benchcore::core_hash as HashFn),
            heavy: black_box(benchcore::core_heavy as HeavyFn),
            noop: black_box(benchcore::core_noop as NoopFn),
            add: black_box(benchcore::core_add as AddFn),
            fill: black_box(benchcore::core_fill as FillFn),
            producer_start: black_box(benchcore::core_producer_start as ProdStartFn),
            producer_stop: black_box(benchcore::core_producer_stop as ProdStopFn),
            producer_produced: black_box(benchcore::core_producer_produced as ProdCountFn),
        }
    }
}
