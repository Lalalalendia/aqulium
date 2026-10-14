use super::*;
use crate::app_core::{Core, CoreEvent, EventSink};
use crate::native_editor::{
    parse_table_at_line, toggle_dataview_task, ColumnAlignment, InteractiveLinkTarget, KeyCommand,
    Selection,
};
use std::fs;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct ShellTestSink(Mutex<Vec<CoreEvent>>);

impl EventSink for ShellTestSink {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[test]
fn stage1_shell_tabs_navigation_and_sticky_ui_state() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    let mut shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "main-win", 1_000).unwrap();
    shell.explorer.create_note("Alpha.md", "# Alpha\nСтрока 1\nСтрока 2\n").unwrap();
    shell.explorer.create_note("Beta.md", "# Beta\nТекст второй заметки\n").unwrap();

    shell.open_note("Alpha.md", false).unwrap();
    shell
        .active_editor_mut()
        .unwrap()
        .set_selection(Selection::caret(8));

    shell.open_note("Beta.md", true).unwrap();
    assert_eq!(shell.tabs.tabs().len(), 2);
    assert_eq!(shell.tabs.nav_history.current(), Some("Beta.md"));

    // Navigate back and forward
    let prev = shell.tabs.nav_history.go_back().unwrap();
    assert_eq!(prev, "Alpha.md");
    let next = shell.tabs.nav_history.go_forward().unwrap();
    assert_eq!(next, "Beta.md");

    // Persist session and sticky Yrs cursor positions to ui-state.sqlite3
    assert!(shell.save_session(2_000).unwrap());
    shell.shutdown(3_000);
}

#[test]
fn stage4_interactive_markdown_tables_tasks_wikilinks_and_dataview() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    let mut shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "main-win", 1_000).unwrap();
    shell
        .explorer
        .create_note(
            "Задачи.md",
            "---\nstatus: in-progress\n---\n# Задачи\n- [ ] Сделать таблицу\n| Модуль | Статус |\n| --- | --- |\n| Core | Готово |\n\nПерейти в [[Целевая заметка|Цель]].\n```dataview\nTABLE status FROM \"\"\n```\n",
        )
        .unwrap();

    shell.open_note("Задачи.md", false).unwrap();
    {
        let editor = shell.active_editor_mut().unwrap();
        // Toggle task on line 4 ("- [ ] Сделать таблицу" -> "- [x] Сделать таблицу")
        assert!(editor.toggle_task_at_line(4).unwrap());
        assert!(editor.rope().to_string_full().contains("- [x] Сделать таблицу"));

        // Place cursor on the table (line 5) and insert a column & align right
        let (table_start, _, _) = editor.rope().line_char_bounds(5);
        editor.set_selection(Selection::caret(table_start + 2));
        assert!(editor.table_insert_column_after().unwrap());
        assert!(editor.table_set_column_alignment(ColumnAlignment::Right).unwrap());

        let parsed = parse_table_at_line(editor.rope(), 5).expect("parsed table");
        assert_eq!(parsed.col_count(), 3);
        assert_eq!(parsed.alignments[0], ColumnAlignment::Right);

        // Undo restores the previous table step cleanly
        editor.handle_key(KeyCommand::Undo).unwrap();
        editor.save_now().unwrap();
    }

    // Toggle task via Dataview task helper
    assert!(toggle_dataview_task(&core, &vault, "Задачи.md", 5).unwrap());
    assert!(fs::read_to_string(vault.join("Задачи.md"))
        .unwrap()
        .contains("- [ ] Сделать таблицу"));

    shell.shutdown(2_000);
}

#[test]
fn stage6_book_reader_fb2_cfi_and_quote_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    let fb2 = concat!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
        "<FictionBook><description><title-info>",
        "<book-title>Архитектура Rust</book-title>",
        "<author><first-name>Павел</first-name><last-name>Соколов</last-name></author>",
        "<lang>ru</lang>",
        "</title-info></description>",
        "<body>",
        "<section><title><p>Глава 1. Введение</p></title><p>Абзац 1.1</p><p>Абзац 1.2</p></section>",
        "<section><title><p>Глава 2. Память и Rope</p></title><p>Абзац 2.1</p><p>Ключевая мысль про Rope и окно 8192 символа.</p></section>",
        "</body></FictionBook>"
    );
    fs::write(vault.join("rust-arch.fb2"), fb2).unwrap();

    let mut shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "reader-win", 1_000).unwrap();
    let quote_block = {
        let reader = shell.open_book("rust-arch.fb2", None).unwrap();
        assert_eq!(reader.book().metadata.title, "Архитектура Rust");
        assert_eq!(reader.book().chapters.len(), 2);
        reader.navigate_to(1, 1, 1_500).unwrap();
        assert_eq!(reader.current_cfi(), "/6/4!/4/4:0");
        reader.format_linked_quote("Ключевая мысль про Rope и окно 8192 символа.")
    };

    shell.explorer.create_note("Конспект.md", &quote_block).unwrap();
    shell.open_note("Конспект.md", false).unwrap();

    // Place cursor on the aquilum-reader:// line and follow link back into the book!
    {
        let editor = shell.active_editor_mut().unwrap();
        let (line2_start, _, _) = editor.rope().line_char_bounds(2);
        editor.set_selection(Selection::caret(line2_start + 5));
    }
    let followed = shell.follow_link_at_cursor(false).unwrap();
    assert_eq!(
        followed,
        Some(InteractiveLinkTarget::ReaderQuote {
            book_path: "rust-arch.fb2".to_owned(),
            cfi: "/6/4!/4/4:0".to_owned(),
        })
    );
    assert_eq!(shell.reader("rust-arch.fb2").unwrap().current_position(), (1, 1));
    fs::write(vault.join("unpacked.epub"), b"PK\x03\x04binary").unwrap();
    assert!(shell.open_book("unpacked.epub", None).is_err(), "EPUB needs ZIP/OPF/spine parsing");
    shell.shutdown(2_000);
}

#[test]
fn stage7_strangler_fig_smoke_never_authorizes_webview_removal() {
    let dir = tempfile::tempdir().unwrap();
    let report = verify_strangler_fig_cutover(dir.path()).expect("cutover verification");
    for stage in &report.stages {
        assert!(stage.passed, "Stage {} ({}) failed: {}", stage.stage, stage.name, stage.details);
    }
    assert!(!report.ready_to_remove_webview2, "headless stages cannot certify real GUI or EPUB parity");
}

#[test]
fn phase2_in_page_search_replace_inline_formatting_and_smart_dashes() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    let mut shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "search-win", 1_000).unwrap();
    shell
        .explorer
        .create_note(
            "Поиск.md",
            "Первый ключ в тексте.\nВторой КЛЮЧ для замены.\nТретий ключ в конце.\n",
        )
        .unwrap();
    shell.open_note("Поиск.md", false).unwrap();

    let editor = shell.active_editor_mut().unwrap();
    let mut search_state = editor.find_in_page("ключ", false);
    assert_eq!(search_state.matches.len(), 3);
    assert_eq!(search_state.current_index, Some(0));

    // Next match selects "КЛЮЧ" on line 2
    let m2 = editor.page_search_next(&mut search_state).unwrap();
    assert_eq!(editor.rope().slice_chars(m2.0, m2.1), "КЛЮЧ");

    // Replace current match -> "Второй токен для замены."
    assert!(editor.page_search_replace_current(&mut search_state, "токен").unwrap());
    assert!(editor.rope().to_string_full().contains("Второй токен для замены."));

    // Replace remaining 2 matches of "ключ" -> "маркер"
    let replaced_count = editor.page_search_replace_all("ключ", "маркер", false).unwrap();
    assert_eq!(replaced_count, 2);
    assert!(!editor.rope().to_string_full().to_lowercase().contains("ключ"));

    // Toggle bold wrap (`**`) and unwrap around "токен"
    let state2 = editor.find_in_page("токен", false);
    assert_eq!(state2.matches.len(), 1);
    editor.toggle_inline_wrap("**").unwrap();
    assert!(editor.rope().to_string_full().contains("**токен**"));
    editor.toggle_inline_wrap("**").unwrap();
    assert!(!editor.rope().to_string_full().contains("**токен**"));

    // Smart dashes: typing "-" then "-" produces "—"
    editor.handle_key(KeyCommand::MoveDocEnd { select: false }).unwrap();
    editor.insert_text_with_typography("-").unwrap();
    editor.insert_text_with_typography("-").unwrap();
    assert!(editor.rope().to_string_full().ends_with("—"));
}

#[test]
fn phase2_book_library_import_companion_page_and_progress_sync() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    let external_book = dir.path().join("sample-book.fb2");
    fs::write(
        &external_book,
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<FictionBook><description><title-info>",
            "<book-title>Философия Rust</book-title>",
            "<author><first-name>Анна</first-name><last-name>Котова</last-name></author>",
            "</title-info></description>",
            "<body><section><title><p>Глава 1</p></title>",
            "<p>П1</p><p>П2</p><p>П3</p><p>П4</p>",
            "</section></body></FictionBook>"
        ),
    )
    .unwrap();

    let shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "lib-win", 1_000).unwrap();
    let ws_id = shell.tabs.workspace_id();
    let imported = shell
        .book_library
        .import_book_and_create_page(&vault, ws_id, &external_book)
        .unwrap();

    assert_eq!(imported.title, "Философия Rust");
    assert_eq!(imported.author, "Анна Котова");
    assert_eq!(imported.total_pages, 4);
    assert_eq!(imported.companion_note_rel_path.as_deref(), Some("Философия Rust.md"));

    // Sync progress 3/4 into the companion note frontmatter
    let companion_path = vault.join("Философия Rust.md");
    assert!(shell
        .book_library
        .sync_progress_to_companion_note(&companion_path, 3, 4)
        .unwrap());
    assert!(fs::read_to_string(&companion_path)
        .unwrap()
        .contains("pages: \"3/4\""));

    let books = shell.book_library.list_books(&vault, ws_id);
    assert_eq!(books.len(), 1);
}

#[test]
fn phase2_trash_deletions_restore_and_retention_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();

    let shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "trash-win", 1_000).unwrap();
    shell.explorer.create_note("Временная.md", "Текст для корзины\n").unwrap();
    assert!(vault.join("Временная.md").is_file());

    shell.explorer.trash_note("Временная.md").unwrap();
    assert!(!vault.join("Временная.md").exists());

    let state = shell.trash.state().unwrap();
    assert_eq!(state.count, 1);

    let page = shell.trash.deletions_page(0, 20).unwrap();
    assert_eq!(page.total, 1);
    let deletion_id = page.deletions[0].id.clone();

    shell.trash.restore_deletion(&deletion_id).unwrap();
    assert!(vault.join("Временная.md").is_file());
}

#[test]
fn phase2_native_html_and_pdf_export_without_webview2() {
    let dir = tempfile::tempdir().unwrap();
    let note_path = dir.path().join("Отчёт.md");
    fs::write(
        &note_path,
        "---\ntitle: Отчёт\n---\n# Заголовок отчёта\nСвязь с [[База|Базой знаний]].\n- [x] Готово\n| А | Б |\n| --- | --- |\n| 1 | 2 |\n",
    )
    .unwrap();

    let html_out = dir.path().join("Отчёт.html");
    let pdf_out = dir.path().join("Отчёт.pdf");

    let html_bytes = NativeExporter::export_note_to_html(&note_path, &html_out, "dark").unwrap();
    assert!(html_bytes > 200);
    let html_text = fs::read_to_string(&html_out).unwrap();
    assert!(html_text.contains("<a class=\"q-print-wikilink\" href=\"#База\">Базой знаний</a>"));
    assert!(html_text.contains("<table>"));
    assert!(!html_text.contains("title: Отчёт"));

    assert!(NativeExporter::export_note_to_pdf(&note_path, &pdf_out).is_err(), "Cyrillic must not be silently transliterated");
    let ascii_note = dir.path().join("example.md");
    fs::write(&ascii_note, "# Example\nASCII PDF only.\n").unwrap();
    let pdf_bytes = NativeExporter::export_note_to_pdf(&ascii_note, &pdf_out).unwrap();
    assert!(pdf_bytes > 250);
    let raw_pdf = fs::read(&pdf_out).unwrap();
    assert!(raw_pdf.starts_with(b"%PDF-1.4\n"));
    assert!(raw_pdf.ends_with(b"%%EOF\n"));
}

#[test]
fn phase2_shortcuts_theme_palette_ui_scale_and_i18n() {
    assert_eq!(match_shortcut("N", true, false, false), Some(ShortcutId::NewFile));
    assert_eq!(match_shortcut("F", true, true, false), Some(ShortcutId::FocusMode));
    assert_eq!(match_shortcut("F", true, false, false), Some(ShortcutId::PageSearch));
    assert_eq!(match_shortcut("S", true, true, false), Some(ShortcutId::Strikethrough));

    let palette = ThemePalette::resolve("dark", "#1471eb");
    assert!(palette.is_dark);
    assert_eq!(palette.accent_rgb, (20, 113, 235));

    let mut scale = UiScaleState::default();
    scale.zoom_in();
    assert_eq!(scale.scale, 1.1);
    scale.zoom_reset();
    assert_eq!(scale.scale, 1.0);
    assert!(scale.toggle_focus_mode());

    assert_eq!(t(Locale::Ru, "sidebar.books"), "Библиотека книг");
    assert_eq!(t(Locale::En, "sidebar.books"), "Book Library");
}

#[test]
fn phase3_frontmatter_inspector_and_media_embeds() {
    use crate::native_editor::{
        format_image_embed, parse_frontmatter, parse_image_embed, FrontmatterValue, ImageAlign,
        ImageCrop, MediaKind,
    };

    let doc = "---\nПуть к файлу: \"Files/book.fb2\"\nread_percent: \"45%\"\ntags: [rust, gpui]\n---\n# Заголовок\n";
    let fm = parse_frontmatter(doc).expect("parsed frontmatter");
    let resolved = fm.resolve_book_fields();
    assert_eq!(resolved.book_file.as_deref(), Some("Files/book.fb2"));
    assert_eq!(resolved.read_percent, Some(45));

    let embed = parse_image_embed("![[Files/diagram.png|480|left|crop=10,20,80,90]]").expect("embed");
    assert_eq!(embed.src, "Files/diagram.png");
    assert_eq!(embed.width, Some(480));
    assert_eq!(embed.align, ImageAlign::Left);
    assert_eq!(
        embed.crop,
        Some(ImageCrop {
            left: 10.0,
            top: 20.0,
            right: 80.0,
            bottom: 90.0,
        })
    );
    assert_eq!(embed.kind, MediaKind::Image);
    assert_eq!(
        format_image_embed(&embed),
        "![[Files/diagram.png|480|left|crop=10,20,80,90]]"
    );

    let dir = tempfile::tempdir().unwrap();
    let sink = Arc::new(ShellTestSink::default());
    let core = Core::open(&dir.path().join("data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let mut shell = NativeWorkspaceShell::open(Arc::clone(&core), &vault, "fm-win", 1_000).unwrap();
    shell.explorer.create_note("Медиа.md", "# Медиа\n").unwrap();
    shell.open_note("Медиа.md", false).unwrap();
    {
        let ed = shell.active_editor_mut().unwrap();
        ed.set_frontmatter_property("status", FrontmatterValue::Text("ready".into()))
            .unwrap();
        let inserted = ed
            .import_and_insert_media(&vault, "pic.png", b"\x89PNG\r\n\x1a\n", Some(320))
            .unwrap();
        assert_eq!(inserted, "![[Files/pic.png|320]]");
    }
    shell.shutdown(2_000);
}

#[test]
fn phase3_graph_spatial_pick_grid_label_candidates_and_date_filter() {
    use std::collections::HashMap;
    let nodes = vec![
        DecodedGraphNode {
            index: 0,
            path: "A.md".into(),
            title: "A".into(),
            x: 0.0,
            y: 0.0,
            created_days: 100.0,
            modified_days: 110.0,
            degree: 5,
            color_rgb: (20, 113, 235),
        },
        DecodedGraphNode {
            index: 1,
            path: "B.md".into(),
            title: "B".into(),
            x: 50.0,
            y: 40.0,
            created_days: 200.0,
            modified_days: 210.0,
            degree: 2,
            color_rgb: (20, 113, 235),
        },
    ];
    let grid = PickGrid::build(&nodes).expect("pick grid");
    assert_eq!(grid.pick_node(&nodes, 1.0, -1.0, 1.0, 1.0, 1.0), Some(0));
    assert_eq!(grid.pick_node(&nodes, 49.5, 40.5, 1.0, 1.0, 1.0), Some(1));

    let mut fades = HashMap::new();
    fades.insert(0, 0.9);
    fades.insert(1, 0.5);
    let candidates = collect_label_candidates(
        &nodes,
        Some(&grid),
        0.0,
        0.0,
        1.0,
        1.0,
        1.0,
        150.0,
        Some(0),
        800.0,
        600.0,
        &fades,
        LABEL_LIMIT,
    );
    assert_eq!(candidates[0].node, 0, "hovered node must be pinned at index 0");

    let mut filter = GraphDateFilter::default();
    filter.adopt(&nodes, 150.0);
    assert_eq!(filter.visible_count, 1);
    filter.retarget(&nodes, 50.0);
    assert_eq!(filter.visible_count, 2);
    while filter.advance(0.05) {}
    assert_eq!(filter.alpha, vec![255, 255]);
}

#[test]
fn phase3_window_state_tab_reorder_and_history_diff() {
    let dir = tempfile::tempdir().unwrap();
    let wm = NativeWindowStateManager::new(dir.path());
    wm.observe(1440.0, 900.0, false);
    wm.persist().unwrap();

    let wm2 = NativeWindowStateManager::new(dir.path());
    assert_eq!(
        wm2.current(),
        NativeWindowPlacement {
            width: 1440.0,
            height: 900.0,
            maximized: false,
        }
    );

    let boxes = [
        TabBox { left: 0.0, width: 100.0 },
        TabBox { left: 104.0, width: 100.0 },
        TabBox { left: 208.0, width: 100.0 },
    ];
    let layout = reorder_layout(&boxes, 0, 130.0);
    assert_eq!(layout.target, 1);
    assert!(resting_offset(&boxes, 0, 1) > 0.0);

    let diff = compute_version_diff("Строка 1\nСтарая 2\nСтрока 3", "Строка 1\nНовая 2\nСтрока 3");
    assert!(diff.iter().any(|l| l.kind == HistoryDiffKind::Removed && l.text == "Старая 2"));
    assert!(diff.iter().any(|l| l.kind == HistoryDiffKind::Added && l.text == "Новая 2"));
}

