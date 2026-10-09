use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::fs;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[path = "../../rust/src/kernel.rs"]
mod kernel;

// Allocation counts are scoped to the timed scan loop, not to fixture loading,
// verification, warm-up, result formatting or native runtime startup.
static ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
struct CountAlloc;
#[global_allocator]
static ALLOCATOR: CountAlloc = CountAlloc;

unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

fn kb_in_status(key: &str) -> u64 {
    let status = fs::read_to_string("/proc/self/status").expect("Linux /proc process memory");
    let needle = format!("{key}:");
    status
        .lines()
        .find(|line| line.starts_with(&needle))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|number| number.parse::<u64>().ok())
        .expect("expected Linux memory counter")
}
fn rss_mib() -> f64 { kb_in_status("VmRSS") as f64 / 1024.0 }
fn peak_rss_mib() -> f64 { kb_in_status("VmHWM") as f64 / 1024.0 }
fn quantile(data: &[f64], fraction: f64) -> f64 {
    let mut sorted = data.to_vec();
    sorted.sort_unstable_by(|x, y| x.total_cmp(y));
    let index = ((fraction * sorted.len() as f64).ceil() as usize).saturating_sub(1);
    sorted[index]
}

fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 6, "usage: aquilum_native_scan file expected profile size_mb round");
    let filename = &args[1];
    let expected = args[2].parse::<u32>().expect("expected u32");
    let profile = &args[3];
    let mb = args[4].parse::<u32>().expect("size");
    let round = args[5].parse::<u32>().expect("round");
    let rss_before_load = rss_mib();
    let input = fs::read(filename).expect("read deterministic UTF-8 fixture");
    std::str::from_utf8(&input).expect("valid UTF8 bytes from JS fixture");
    let rss_after_load = rss_mib();

    let first = kernel::scan_bytes(&input);
    assert_eq!(first, expected, "native result differs from independent oracle");
    for _ in 0..10 {
        assert_eq!(black_box(kernel::scan_bytes(black_box(&input))), expected);
    }
    let rss_after_warm = rss_mib();
    let reps = match mb { 1 => 81, 5 => 61, 20 => 41, _ => 41 };
    let mut times = Vec::<f64>::with_capacity(reps);
    let alloc_start = ALLOC_CALLS.load(Ordering::Relaxed);
    let bytes_start = ALLOC_BYTES.load(Ordering::Relaxed);

    let mut checksum: u64 = 0;
    for _ in 0..reps {
        let started = Instant::now();
        let value = black_box(kernel::scan_bytes(black_box(&input)));
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(value, expected, "native result changed");
        checksum += value as u64;
        times.push(elapsed);
    }
    let calls_in_loops = ALLOC_CALLS.load(Ordering::Relaxed) - alloc_start;
    let bytes_in_loops = ALLOC_BYTES.load(Ordering::Relaxed) - bytes_start;
    let rss_after_scan = rss_mib();
    let peak = peak_rss_mib();

    let total: f64 = times.iter().sum();
    println!(
        concat!(
          "AQUILUM_NATIVE_COMPARE {{",
          "\"variant\":\"rust_native\",",
          "\"profile\":\"{}\",\"requestedMb\":{},\"round\":{},",
          "\"utf8Bytes\":{},\"expected\":{},\"repetitions\":{},",
          "\"p50Ms\":{:.6},\"p95Ms\":{:.6},\"p99Ms\":{:.6},\"meanMs\":{:.6},",
          "\"rssBeforeLoadMiB\":{:.3},\"rssAfterLoadMiB\":{:.3},",
          "\"rssAfterWarmMiB\":{:.3},\"rssAfterScanMiB\":{:.3},\"rssPeakMiB\":{:.3},",
          "\"nativeAllocCallsPerScan\":{:.6},\"nativeAllocatedBytesPerScan\":{:.3},",
          "\"checksum\":{}",
          "}}"
        ),
        profile, mb, round, input.len(), expected, reps,
        quantile(&times, 0.5), quantile(&times, 0.95), quantile(&times, 0.99), total / times.len() as f64,
        rss_before_load, rss_after_load, rss_after_warm, rss_after_scan, peak,
        calls_in_loops as f64 / reps as f64, bytes_in_loops as f64 / reps as f64,
        checksum,
    );
}
