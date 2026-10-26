//! State shared between the GTK main thread (measurement, JSON), the NSH
//! engine thread (where the Rust components run) and the background threads.

use std::{
    collections::VecDeque,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    },
    time::Instant,
};

pub struct Config {
    pub scenario: String,
    pub duration: f64,
    pub out: String,
    pub rate: u32,
    pub shapes: usize,
    pub assets: String,
}

pub static CFG: OnceLock<Config> = OnceLock::new();
pub static T0: OnceLock<Instant> = OnceLock::new();

pub fn cfg() -> &'static Config {
    CFG.get().expect("config")
}

/// Nanoseconds since process start (monotonic).
pub fn now_ns() -> u64 {
    T0.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// Nanoseconds (process clock) at which the measured window started; 0 before.
pub static START_NS: AtomicU64 = AtomicU64::new(0);
pub static STOP: AtomicBool = AtomicBool::new(false);

/// Seconds since the measured window started (None during warm-up).
pub fn measured_t() -> Option<f64> {
    let s = START_NS.load(Relaxed);
    if s == 0 {
        None
    } else {
        Some((now_ns().saturating_sub(s)) as f64 / 1e9)
    }
}

pub static GENERATED: AtomicU64 = AtomicU64::new(0);
pub static APPLIED: AtomicU64 = AtomicU64::new(0);
pub static DROPPED: AtomicU64 = AtomicU64::new(0);
pub static ROWS_BOUND: AtomicU64 = AtomicU64::new(0);
pub static FINAL_OFFSET: AtomicU64 = AtomicU64::new(0);
pub static CONTENT_H: AtomicU64 = AtomicU64::new(0);
pub static CALLS: AtomicU64 = AtomicU64::new(0);
pub static HEAVY: AtomicU64 = AtomicU64::new(0);
pub static CORE_SINK: AtomicU64 = AtomicU64::new(0);
pub static IMAGES_DECODED: AtomicU64 = AtomicU64::new(0);
pub static DECODE_US: AtomicU64 = AtomicU64::new(0);
pub static TRANSITIONS: AtomicU64 = AtomicU64::new(0);
pub static TEXT_FIRST_MS: AtomicU64 = AtomicU64::new(0);
pub static TEXT_LAYOUT_MS: AtomicU64 = AtomicU64::new(0);
pub static TEXT_BYTES: AtomicU64 = AtomicU64::new(0);
/// Process ms (+1) at which the 1 MB text was first laid out (engine); 0 before.
pub static TEXT_LAID_MS: AtomicU64 = AtomicU64::new(0);
pub static EMPTY_PAINTS: AtomicU64 = AtomicU64::new(0);
pub static KEY_EVENTS: AtomicU64 = AtomicU64::new(0);
pub static MODALS: AtomicU64 = AtomicU64::new(0);
/// Folded into the checksum (row prices applied, key toggles...).
pub static CHECK: AtomicU64 = AtomicU64::new(0);
/// Which of the 600 images finished loading at least once (bit set).
pub static IMAGES_SEEN: Mutex<[u64; 10]> = Mutex::new([0; 10]);

pub fn distinct_images() -> u32 {
    IMAGES_SEEN.lock().unwrap().iter().map(|w| w.count_ones()).sum()
}
pub static SCRIPT_NOTE: Mutex<String> = Mutex::new(String::new());

/// The stream's bounded queue (100 000 updates): (row, price in cents, i.e. x of the spec's x / 100.0).
pub static QUEUE: Mutex<VecDeque<(u32, u32)>> = Mutex::new(VecDeque::new());
pub const QUEUE_CAP: usize = 100_000;

/// Zeroes the counters that accumulate from process start, at the start of the measured window
/// (the producer and the core threads start with the measured window, so they count nothing before).
pub fn reset_counters() {
    for a in [&CALLS, &HEAVY, &ROWS_BOUND, &APPLIED, &GENERATED, &DROPPED, &TRANSITIONS, &MODALS, &KEY_EVENTS, &EMPTY_PAINTS, &IMAGES_DECODED, &DECODE_US] {
        a.store(0, Relaxed);
    }
}

pub fn fnv(i: u32) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in i.to_le_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

pub fn row_h(i: u32) -> u32 {
    48 + 16 * (fnv(i) % 6)
}

pub fn sum_heights() -> u64 {
    (0..100_000u32).map(|i| row_h(i) as u64).sum()
}

pub fn f64_store(a: &AtomicU64, v: f64) {
    a.store(v.to_bits(), Relaxed);
}

pub fn f64_load(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Relaxed))
}

/// Background producer (xorshift32 seed 0x2545F491): `rate` updates per second
/// into the bounded queue; never blocks on the consumer (drop + count when full).
pub fn spawn_producer(rate: u32) {
    std::thread::Builder::new()
        .name("producer".into())
        .spawn(move || {
            let mut x: u32 = 0x2545_F491;
            let start = Instant::now();
            let mut made: u64 = 0;
            while !STOP.load(Relaxed) {
                let due = (start.elapsed().as_secs_f64() * rate as f64) as u64;
                if due > made {
                    let mut batch = Vec::with_capacity((due - made) as usize);
                    while made < due {
                        x ^= x << 13;
                        x ^= x >> 17;
                        x ^= x << 5;
                        // price = x / 100.0 (spec), kept in cents: x
                        batch.push((x % 100_000, x));
                        made += 1;
                    }
                    GENERATED.fetch_add(batch.len() as u64, Relaxed);
                    let mut q = QUEUE.lock().unwrap();
                    for u in batch {
                        if q.len() >= QUEUE_CAP {
                            DROPPED.fetch_add(1, Relaxed);
                        } else {
                            q.push_back(u);
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_micros(500));
            }
        })
        .unwrap();
}

/// Core worker: validate + parse + hash on the "hello" frame in a tight loop,
/// through the C-ABI symbols of libbenchcore (linked as a Rust rlib).
pub fn spawn_core_threads() {
    std::thread::Builder::new()
        .name("core-worker".into())
        .spawn(|| {
            use std::hint::black_box;
            let frame = hello_frame();
            let mut out = [0u8; 64];
            let mut acc: u64 = 0;
            let mut n: u64 = 0;
            // Calls go through black-boxed function pointers so that LTO cannot
            // inline or fold the C-ABI functions of the rlib.
            let validate: unsafe extern "C" fn(*const u8, usize) -> i32 = black_box(benchcore::core_validate);
            let parse: unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32 =
                black_box(benchcore::core_parse);
            let hash: unsafe extern "C" fn(*const u8, usize) -> u32 = black_box(benchcore::core_hash);
            while !STOP.load(Relaxed) {
                for _ in 0..1000 {
                    unsafe {
                        let fp = black_box(frame.as_ptr());
                        let v = validate(fp, frame.len());
                        let mut ol: usize = 0;
                        let p = parse(black_box(frame.as_ptr()), frame.len(), out.as_mut_ptr(), out.len(), &mut ol);
                        let h = hash(black_box(frame.as_ptr()), frame.len());
                        acc = acc.wrapping_add(black_box(v as u64 ^ (p as u64) ^ h as u64 ^ ol as u64));
                    }
                    n += 1;
                }
                CALLS.fetch_add(1000, Relaxed);
            }
            CORE_SINK.fetch_add(acc ^ n, Relaxed);
        })
        .unwrap();
    std::thread::Builder::new()
        .name("core-heavy".into())
        .spawn(|| {
            let mut acc = 0u64;
            while !STOP.load(Relaxed) {
                acc = acc.wrapping_add(benchcore::core_heavy(100));
                HEAVY.fetch_add(1, Relaxed);
                // one call per second (100 ms of work + 900 ms rest)
                for _ in 0..18 {
                    if STOP.load(Relaxed) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
            CORE_SINK.fetch_add(acc, Relaxed);
        })
        .unwrap();
}

/// 8-byte "hello" frame: [0xA5][version=1][n=5]["hello"].
pub fn hello_frame() -> [u8; 8] {
    [0xA5, 1, 5, b'h', b'e', b'l', b'l', b'o']
}
