use aquilum_core::app_core::{Core, CoreEvent, EventSink};
use aquilum_core::native_shell::{verify_strangler_fig_cutover, NativeWorkspaceShell};
use aquilum_native_gpui::GpuiRopeEditorView;
use std::path::PathBuf;
use std::sync::Arc;

struct StdoutEventSink;

impl EventSink for StdoutEventSink {
    fn emit(&self, event: CoreEvent) {
        if std::env::var("AQUILUM_VERBOSE_EVENTS").is_ok() {
            println!("[aquilum-native-gpui:event] {}", event.name());
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(first_arg) = args.next() else {
        eprintln!(
            "Usage:\n  aquilum-native-gpui <path-to-note-or-workspace> [--app-data <dir>]\n  aquilum-native-gpui --verify-model-smoke\n  aquilum-native-gpui --verify-cutover (always blocked)"
        );
        std::process::exit(1);
    };

    if first_arg == "--verify-cutover" || first_arg == "--verify-model-smoke" {
        let temp_dir = std::env::temp_dir().join(format!("aquilum-cutover-{}", std::process::id()));
        let report = verify_strangler_fig_cutover(&temp_dir).expect("cutover verification failed");
        for stage in &report.stages {
            println!(
                "AQUILUM_CUTOVER_STAGE stage={} passed={} name={:?} details={:?}",
                stage.stage, stage.passed, stage.name, stage.details
            );
        }
        let model_ok = report.stages.iter().all(|stage| stage.passed);
        println!("AQUILUM_NATIVE_MODEL_SMOKE passed={model_ok}");
        println!("AQUILUM_CUTOVER_READY ready={}", report.ready_to_remove_webview2);
        let _ = std::fs::remove_dir_all(&temp_dir);
        if first_arg == "--verify-cutover" {
            // Always fail closed until independently verified physical GUI and EPUB parity.
            std::process::exit(2);
        }
        if !model_ok {
            std::process::exit(1);
        }
        return;
    }

    let target_path = PathBuf::from(first_arg);
    let app_data_dir = match args.next().as_deref() {
        Some("--app-data") => args.next().map(PathBuf::from).unwrap_or_else(default_app_data_dir),
        _ => default_app_data_dir(),
    };

    let core = Core::open(&app_data_dir, Arc::new(StdoutEventSink));
    if target_path.is_dir() {
        let shell = NativeWorkspaceShell::open(Arc::clone(&core), &target_path, "gpui-main", 0)
            .expect("failed to open workspace in NativeWorkspaceShell");
        println!(
            "AQUILUM_NATIVE_SHELL_MODEL_READY workspace={:?} tabs={}",
            shell.workspace_root(),
            shell.tabs.tabs().len()
        );
        shell.shutdown(0);
    } else {
        let view = GpuiRopeEditorView::open(Arc::clone(&core), &target_path)
            .expect("failed to open note in GpuiRopeEditorView");
        let frame = view.render_frame();
        println!(
            "AQUILUM_NATIVE_VIEWPORT_MODEL_READY title={:?} version={} total_bytes={} total_chars={} total_lines={} rope_mib={:.3} viewport_chars={} viewport_lines={}..{}",
            frame.title,
            frame.version,
            frame.metrics.rope_len_bytes,
            frame.metrics.rope_len_chars,
            frame.metrics.rope_len_lines,
            frame.metrics.rope_allocated_bytes as f64 / 1_048_576.0,
            frame.metrics.viewport_len_chars,
            frame.viewport.start_line + 1,
            frame.viewport.end_line + 1,
        );
        core.shutdown();
    }
}

fn default_app_data_dir() -> PathBuf {
    std::env::temp_dir().join("aquilum-native-gpui-data")
}
