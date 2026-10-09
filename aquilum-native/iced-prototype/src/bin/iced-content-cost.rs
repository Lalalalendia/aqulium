//! No-GUI RAM probe: only iced::widget::text_editor::Content::with_text.
//! Separates document model memory from GPU/shaping/render costs.
use iced::widget::text_editor;
use std::{env, fs, hint::black_box, io::{self, Write}, path::PathBuf, thread, time::{Duration, Instant}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).map(PathBuf::from).ok_or("Pass one UTF-8 Markdown file")?;
    let started = Instant::now();
    let input = fs::read_to_string(&path)?;
    let bytes = input.len();
    let lines = input.bytes().filter(|b| *b == b'\n').count() + 1;
    println!("AQUILUM_ICED_MODEL_READ bytes={bytes} lines={lines} ms={:.3}", started.elapsed().as_secs_f64()*1000.0);
    io::stdout().flush()?;
    let started_model = Instant::now();
    let content = text_editor::Content::with_text(black_box(&input));
    println!(
        "AQUILUM_ICED_MODEL_READY bytes={bytes} lines={lines} build_ms={:.3}",
        started_model.elapsed().as_secs_f64() * 1000.0
    );
    io::stdout().flush()?;
    black_box(&content);
    thread::sleep(Duration::from_secs(20));
    black_box(&content);
    Ok(())
}
