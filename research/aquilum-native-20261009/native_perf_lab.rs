//! Isolated performance experiment. Not part of upstream Aquilum.
use crate::app_core::{Core, CoreEvent};
use crate::history::Source;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const SWITCH: &str = "AQUILUM_BENCH_SKIP_CLEAN_COMPACT";

fn create_workspace(bytes: usize) -> (tempfile::TempDir, Arc<Core>, PathBuf, String) {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("appdata");
    let vault = temp.path().join("vault");
    fs::create_dir_all(&app_data).unwrap();
    fs::create_dir_all(&vault).unwrap();
    let mut content = String::with_capacity(bytes + 128);
    while content.len() < bytes {
        content.push_str("## Chapter: Performance validation\nA passage of ordinary words and 1234567890.\n");
    }
    let note = vault.join("Chapter.md");
    fs::write(&note, &content).unwrap();
    let core = Core::open(&app_data, Arc::new(|_: CoreEvent| {}));
    std::env::remove_var(SWITCH);
    let opened = core.documents.open(&core, &note, 0).expect("initialize document");
    assert_eq!(opened.text, content);
    core.documents.release(&core, &note);
    (temp, core, note, content)
}

fn run_cycle(core: &Arc<Core>, path: &PathBuf, expected_len: usize) -> (f64, f64) {
    let started = Instant::now();
    let opened = core.documents.open(core, path, 0).expect("open note");
    let open_ms = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(opened.text.len(), expected_len);
    let started = Instant::now();
    core.documents.release(core, path);
    let close_ms = started.elapsed().as_secs_f64() * 1000.0;
    (open_ms, close_ms)
}

fn percentiles(values: &[f64]) -> (f64, f64, f64) {
    assert!(!values.is_empty());
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let p95 = sorted[((sorted.len() - 1) * 95).div_ceil(100)];
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (mean, median, p95)
}

#[test]
fn native_roundtrip_correctness() {
    let (_temp, core, note, original) = create_workspace(2048);
    std::env::set_var(SWITCH, "1");
    let first = core.documents.open(&core, &note, 0).unwrap();
    assert_eq!(first.text, original);
    core.documents.release(&core, &note);
    fs::write(&note, "External editing of this Markdown note.\n").unwrap();
    let external = core.documents.open(&core, &note, 0).unwrap();
    assert_eq!(external.text, "External editing of this Markdown note.\n");
    let next = "External edit plus agent content.\n";
    core.documents.replace_text(&core, &note, next, "lab-agent", Source::Me, Some(external.version)).unwrap();
    core.documents.release(&core, &note);
    assert_eq!(fs::read_to_string(&note).unwrap(), next);
    let reopened = core.documents.open(&core, &note, 0).unwrap();
    assert_eq!(reopened.text, next);
    core.documents.release(&core, &note);
    std::env::remove_var(SWITCH);
    core.shutdown();
}

#[test]
#[ignore = "run with cargo test --release benchmark_clean_document_hub -- --ignored --nocapture --test-threads=1"]
fn benchmark_clean_document_hub() {
    println!("AQUILUM_BENCH_HEADER,size_bytes,strategy,open_mean_ms,open_p50_ms,open_p95_ms,close_mean_ms,close_p50_ms,close_p95_ms,total_mean_ms,total_p50_ms,total_p95_ms,iterations");
    for size in [64_000, 512_000, 2_000_000] {
        let (_temp, core, note, text) = create_workspace(size);
        let mut open = [Vec::<f64>::new(), Vec::<f64>::new()];
        let mut close = [Vec::<f64>::new(), Vec::<f64>::new()];
        let mut total = [Vec::<f64>::new(), Vec::<f64>::new()];
        for round in 0..24 {
            let order = if round % 2 == 0 { [0, 1] } else { [1, 0] };
            for strategy in order {
                if strategy == 1 { std::env::set_var(SWITCH, "1"); }
                else { std::env::remove_var(SWITCH); }
                let (a, b) = run_cycle(&core, &note, text.len());
                if round >= 4 {
                    open[strategy].push(a);
                    close[strategy].push(b);
                    total[strategy].push(a + b);
                }
            }
        }
        for strategy in 0..2 {
            let o = percentiles(&open[strategy]);
            let c = percentiles(&close[strategy]);
            let t = percentiles(&total[strategy]);
            println!("AQUILUM_BENCH_RESULT,{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{}", size, if strategy == 0 { "baseline" } else { "skip_clean_compact" }, o.0, o.1, o.2, c.0, c.1, c.2, t.0, t.1, t.2, total[strategy].len());
        }
        core.shutdown();
    }
    std::env::remove_var(SWITCH);
}
