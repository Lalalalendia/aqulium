use super::graph_view::GraphColorMode;
use super::shell::NativeWorkspaceShell;
use crate::app_core::{Core, CoreEvent, EventSink};
use crate::native_editor::{ColumnAlignment, KeyCommand, MAX_VIEWPORT_CHARS};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageCheck {
    pub stage: u8,
    pub name: &'static str,
    pub passed: bool,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutoverReport {
    pub stages: Vec<StageCheck>,
    pub ready_to_remove_webview2: bool,
}

#[derive(Default)]
struct CutoverSink(Mutex<Vec<CoreEvent>>);

impl EventSink for CutoverSink {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// Runs the Stage 7 Strangler Fig Cutover Verification Gate across Stages 0 through 6
/// in an isolated workspace under `temp_root` (headless model smoke only).
/// It NEVER certifies feature parity or authorizes removal of WebView2/Tauri.
pub fn verify_strangler_fig_cutover(temp_root: &Path) -> Result<CutoverReport, String> {
    let app_data = temp_root.join("cutover-app-data");
    let vault = temp_root.join("cutover-vault");
    fs::create_dir_all(&vault).map_err(|e| e.to_string())?;
    fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;

    let sink = Arc::new(CutoverSink::default());
    let core = Core::open(&app_data, Arc::clone(&sink) as Arc<dyn EventSink>);
    let mut shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "cutover-win", 1_700_000_000_000)?;

    let mut stages = Vec::with_capacity(7);

    // Stage 0 & 1: Core Adapter + Shell, Tabs, Navigation, Search, Backlinks, History
    shell
        .explorer
        .create_note(
            "Проект.md",
            "---\nstatus: active\n---\n# Проект\nСвязь со [[Вторая заметка|Второй]].\n- [ ] Первая задача\n",
        )
        .map_err(|e| format!("{e:?}"))?;
    shell
        .explorer
        .create_note("Вторая заметка.md", "# Вторая заметка\nОбратная ссылка на [[Проект]].\n")
        .map_err(|e| format!("{e:?}"))?;

    let _ = core.search.prepare(&vault.to_string_lossy());
    shell.open_note("Проект.md", false)?;
    shell.open_note("Вторая заметка.md", true)?;
    let back_target = shell.tabs.nav_history.go_back();
    let stage1_ok = shell.tabs.tabs().len() == 2 && back_target.as_deref() == Some("Проект.md");
    stages.push(StageCheck {
        stage: 1,
        name: "Stage 0–1: Core Adapter & Native Shell (Tabs, Navigation, UI State, File Tree)",
        passed: stage1_ok,
        details: format!("tabs={} back={:?}", shell.tabs.tabs().len(), back_target),
    });

    // Stage 2: Editor Keyboard, IME, Undo/Redo, Save & Reopen
    shell.open_note("Проект.md", false)?;
    {
        let editor = shell.active_editor_mut().ok_or("no active editor")?;
        editor.handle_key(KeyCommand::MoveDocEnd { select: false }).map_err(|e| e.to_string())?;
        editor.ime_preedit("черновик", Some((0, 8)));
        editor.ime_cancel();
        editor.ime_preedit("чист", Some((0, 4)));
        editor.ime_commit("Добавлено через IME.\n").map_err(|e| e.to_string())?;
        editor.handle_key(KeyCommand::Undo).map_err(|e| e.to_string())?;
        editor.handle_key(KeyCommand::Redo).map_err(|e| e.to_string())?;
        editor.save_now().map_err(|e| e.to_string())?;
        editor.reopen().map_err(|e| e.to_string())?;
    }
    let text_after_stage2 = shell.active_editor().unwrap().rope().to_string_full();
    let stage2_ok = text_after_stage2.ends_with("Добавлено через IME.\n");
    stages.push(StageCheck {
        stage: 2,
        name: "Stage 2: Safe Native Editor (Keyboard, IME, Undo/Redo, Save & Reopen)",
        passed: stage2_ok,
        details: format!("saved_len={}", text_after_stage2.len()),
    });

    // Stage 3: Large Document Rope + Bounded 8192-Char Viewport
    let big_chunk = "## Большой раздел\nТекст для проверки 8192-символьного окна GPUI + Rope 🦀.\n";
    let big_text = big_chunk.repeat(600);
    shell
        .explorer
        .create_note("Большая.md", &big_text)
        .map_err(|e| format!("{e:?}"))?;
    shell.open_note("Большая.md", true)?;
    let vp_chars = shell
        .active_editor()
        .unwrap()
        .viewport()
        .text
        .chars()
        .count();
    let stage3_ok = vp_chars <= MAX_VIEWPORT_CHARS;
    stages.push(StageCheck {
        stage: 3,
        name: "Stage 3: Large Document Rope & Bounded 8192-Char Viewport",
        passed: stage3_ok,
        details: format!("viewport_chars={vp_chars} (max={MAX_VIEWPORT_CHARS})"),
    });

    // Stage 4: Interactive Markdown (Tables, Tasks, WikiLinks, Dataview)
    shell
        .explorer
        .create_note(
            "Таблицы и Dataview.md",
            "| Имя | Статус |\n| --- | --- |\n| А | Б |\n- [ ] Задача в таблице\n```dataview\nTABLE status FROM \"\"\n```\n",
        )
        .map_err(|e| format!("{e:?}"))?;
    shell.open_note("Таблицы и Dataview.md", false)?;
    let (table_ok, task_ok, dv_count) = {
        let editor = shell.active_editor_mut().ok_or("no editor")?;
        let t_ok = editor.table_insert_row_below().map_err(|e| e.to_string())?;
        let _ = editor
            .table_set_column_alignment(ColumnAlignment::Center)
            .map_err(|e| e.to_string())?;
        let k_ok = editor.toggle_task_at_line(4).map_err(|e| e.to_string())?;
        editor.save_now().map_err(|e| e.to_string())?;
        let dv = editor.evaluate_dataview_blocks(&vault, 0);
        (t_ok, k_ok, dv.len())
    };
    let stage4_ok = table_ok && task_ok && dv_count == 1;
    stages.push(StageCheck {
        stage: 4,
        name: "Stage 4: Interactive Markdown Tables, Task Checkboxes & Dataview Engine",
        passed: stage4_ok,
        details: format!("table_ok={table_ok} task_ok={task_ok} dataview_blocks={dv_count}"),
    });

    // Stage 5: 2D Knowledge Graph & Settings/Templates
    shell.graph.refresh(&vault).map_err(|e| format!("{e:?}"))?;
    shell.graph.set_color_mode(GraphColorMode::Created);
    shell.settings.set_theme("dark")?;
    let stage5_ok = shell.settings.config().ui.theme == "dark";
    stages.push(StageCheck {
        stage: 5,
        name: "Stage 5: Native 2D Knowledge Graph, Settings & Templates",
        passed: stage5_ok,
        details: format!("graph_nodes={} theme={}", shell.graph.nodes().len(), shell.settings.config().ui.theme),
    });

    // Stage 6: Native Book Reader (FB2/EPUB) & Linked Reader Quotes
    let fb2_path = vault.join("Книга.fb2");
    fs::write(
        &fb2_path,
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<FictionBook><description><title-info>",
            "<book-title>Тестовая книга</book-title>",
            "<author><first-name>Иван</first-name><last-name>Авторов</last-name></author>",
            "</title-info></description>",
            "<body><section><title><p>Глава 1</p></title>",
            "<p>Первый абзац книги.</p><p>Второй важный абзац для цитаты.</p>",
            "</section></body></FictionBook>"
        ),
    )
    .map_err(|e| e.to_string())?;
    let quote_md = {
        let reader = shell.open_book("Книга.fb2", None)?;
        reader.navigate_to(0, 1, 1_700_000_005_000)?;
        reader.format_linked_quote("Второй важный абзац для цитаты.")
    };
    let stage6_ok = quote_md.contains("> [!quote] Тестовая книга — Иван Авторов")
        && quote_md.contains("aquilum-reader://Книга.fb2#cfi=/6/2!/4/4:0");
    stages.push(StageCheck {
        stage: 6,
        name: "Stage 6: Native Book Reader (FB2/EPUB), CFI Persistence & Linked Quotes",
        passed: stage6_ok,
        details: format!("quote_len={}", quote_md.len()),
    });

    // Stage checks exercise headless models only, not a real GPUI window,
    // keyboard/IME/accessibility input, reliable zipped EPUB, or comparative RAM.
    // Removing Tauri/WebView2 needs an independent product-parity gate.
    let ready_to_remove_webview2 = false;
    shell.shutdown(1_700_000_010_000);

    Ok(CutoverReport {
        stages,
        ready_to_remove_webview2,
    })
}
