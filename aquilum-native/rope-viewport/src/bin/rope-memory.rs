//! Headless Rust baseline for a bounded Rope-based Markdown model.
//! Uses the SAME test files as iced-content-cost but never creates a GPU
//! renderer, full-string edit widget, or a Tauri/WebView process.
use aquilum_native_rope_viewport::TextModel;
use std::{env, fs::File, hint::black_box, io::{self, BufReader, Write}, time::{Duration, Instant}, thread};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filename = env::args().nth(1).ok_or("Pass the Markdown fixture path")?;
    let file = File::open(&filename)?;
    let expected = file.metadata()?.len() as usize;
    let started = Instant::now();
    let model = TextModel::from_reader(BufReader::new(file))?;
    assert_eq!(model.len_bytes(), expected, "Rope must preserve exact UTF-8 bytes");
    let build_ms = started.elapsed().as_secs_f64()*1000.0;
    let lines = model.len_lines();

    const MAX_VIEW_CHARS: usize = 16384;
    const MAX_VIEW_LINES: usize = 256;
    let mut max_view_bytes=0usize;
    for line in [0, lines / 2, lines.saturating_sub(1)] {
        let view=model.window_at_line(line,MAX_VIEW_LINES,MAX_VIEW_CHARS)?;
        assert!(view.text.chars().count()<=MAX_VIEW_CHARS);
        max_view_bytes=max_view_bytes.max(view.text.len());
        black_box(view);
    }
    io::stdout().write_all(format!(
      "AQUILUM_ROPE_MODEL_READY bytes={} lines={} build_ms={:.3} view_limit_chars={} max_materialized_view_bytes={}\n",
      model.len_bytes(), lines, build_ms, MAX_VIEW_CHARS, max_view_bytes
    ).as_bytes())?;
    io::stdout().flush()?;
    black_box(&model);
    thread::sleep(Duration::from_secs(18));
    black_box(&model);
    Ok(())
}
