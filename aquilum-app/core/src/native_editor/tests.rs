use super::*;
use crate::app_core::{Core, CoreEvent, EventSink};
use crate::documents::merge::utf16_len;
use crate::documents::session::SYSTEM_CLIENT;
use crate::history::Source;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default)]
struct TestSink(Mutex<Vec<CoreEvent>>);

impl EventSink for TestSink {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

fn open_test_core() -> (tempfile::TempDir, Arc<Core>, Arc<TestSink>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Arc::new(TestSink::default());
    let core = Core::open(&dir.path().join("app-data"), Arc::clone(&sink) as Arc<dyn EventSink>);
    (dir, core, sink)
}

#[test]
fn rope_utf16_and_line_col_parity_with_multibyte_and_surrogates() {
    let sample = concat!(
        "# Заголовок заметки 🦀\n",
        "Вторая строка с [[ВикиСсылка|Алиас]] и эмодзи 🧪📚.\n",
        "Третья строка: e\u{0301}cole, İ, 가나다, 中文.\n",
        "> [!quote] Цитата из книги\n",
        "> [Источник](aquilum-reader://book-1#cfi=/6/4)\n"
    );
    // Repeat to span multiple 2-4 KiB rope chunks
    let full = sample.repeat(40);
    let mut rope = RopeBuffer::from_text(&full);

    assert!(rope.chunk_count() > 1, "expected multi-chunk rope");
    assert_eq!(rope.len_bytes(), full.len());
    assert_eq!(rope.len_chars(), full.chars().count());
    assert_eq!(rope.len_utf16(), utf16_len(&full));
    assert_eq!(rope.to_string_full(), full);

    // Check round-trip char <-> utf16 and char <-> line_col at sample points
    let mut cur_utf16 = 0;
    for (char_idx, ch) in full.chars().enumerate() {
        if char_idx % 97 == 0 || ch.len_utf16() == 2 {
            assert_eq!(rope.char_to_utf16(char_idx), cur_utf16);
            assert_eq!(rope.utf16_to_char(cur_utf16), char_idx);
            let lc = rope.char_to_line_col(char_idx);
            assert_eq!(rope.line_col_to_char(lc), char_idx);
        }
        cur_utf16 += ch.len_utf16();
    }

    // Mutate across chunk boundary with surrogate pairs
    let mid_char = rope.len_chars() / 2;
    let (edit, removed) = rope.replace_char_range(mid_char, mid_char + 15, "🚀 Новая вставка 🦀");
    assert_eq!(removed.chars().count(), 15);
    assert_eq!(edit.insert, "🚀 Новая вставка 🦀");
    let reconstructed = rope.to_string_full();
    assert_eq!(rope.len_utf16(), utf16_len(&reconstructed));
}

#[test]
fn keyboard_navigation_selection_and_preferred_column() {
    let (dir, core, _) = open_test_core();
    let note = dir.path().join("Навигация.md");
    fs::write(
        &note,
        "Первая длинная строка текста\nКороткая\nТретья длинная строка текста\n- [ ] Задача 1",
    )
    .unwrap();

    let mut editor = NativeEditorSession::open(Arc::clone(&core), &note, "gpui-test").unwrap();

    // Move to column 20 on line 0
    for _ in 0..20 {
        editor.handle_key(KeyCommand::MoveRight { select: false }).unwrap();
    }
    assert_eq!(editor.rope().char_to_line_col(editor.selection().head), LineCol { line: 0, column_chars: 20 });

    // Move down to short line 1 -> clamps to end of line 1 (8 chars), but remembers preferred column 20
    editor.handle_key(KeyCommand::MoveDown { select: false }).unwrap();
    assert_eq!(editor.rope().char_to_line_col(editor.selection().head), LineCol { line: 1, column_chars: 8 });

    // Move down to long line 2 -> restores preferred column 20!
    editor.handle_key(KeyCommand::MoveDown { select: false }).unwrap();
    assert_eq!(editor.rope().char_to_line_col(editor.selection().head), LineCol { line: 2, column_chars: 20 });

    // Word navigation and selection on Russian text
    editor.handle_key(KeyCommand::MoveLineStart { select: false }).unwrap();
    editor.handle_key(KeyCommand::MoveWordRight { select: true }).unwrap();
    let (sel_start, sel_end) = editor.selection().range();
    assert_eq!(editor.rope().slice_chars(sel_start, sel_end), "Третья");

    // Copy and paste
    editor.handle_key(KeyCommand::Copy).unwrap();
    assert_eq!(editor.clipboard(), "Третья");
    editor.handle_key(KeyCommand::MoveLineEnd { select: false }).unwrap();
    editor.handle_key(KeyCommand::Paste(String::new())).unwrap();
    assert!(editor.rope().to_string_full().contains("Третья длинная строка текстаТретья"));

    // Markdown task list auto-continuation on Enter at end of document
    editor.handle_key(KeyCommand::MoveDocEnd { select: false }).unwrap();
    editor.handle_key(KeyCommand::InsertNewline).unwrap();
    assert!(editor.rope().to_string_full().ends_with("- [ ] Задача 1\n- [ ] "));
}

#[test]
fn ime_composition_preedit_cancel_and_commit_safety() {
    let (dir, core, _) = open_test_core();
    let note = dir.path().join("IME.md");
    fs::write(&note, "Текст со [[Старая ссылка]] внутри.\n").unwrap();

    let mut editor = NativeEditorSession::open(Arc::clone(&core), &note, "gpui-ime").unwrap();
    // Select "[[Старая ссылка]]" (chars 9..26)
    editor.set_selection(Selection::new(9, 26));
    let version_before = editor.version();

    // Start IME preedit
    editor.ime_preedit("nihon", Some((0, 5)));
    assert!(editor.ime().is_composing());
    assert_eq!(editor.rope().to_string_full(), "Текст со nihon внутри.\n");
    assert_eq!(editor.version(), version_before, "preedit must not push uncommitted strokes to hub");
    assert_eq!(editor.history().undo_len(), 0, "preedit must not pollute undo stack");

    // Update IME preedit with CJK + emoji surrogate pair
    editor.ime_preedit("日本語🦀", Some((0, 4)));
    let vp = editor.viewport();
    assert!(vp.ime.is_some());
    assert_eq!(editor.rope().to_string_full(), "Текст со 日本語🦀 внутри.\n");

    // Cancel IME -> must restore "[[Старая ссылка]]" and original selection 9..26
    editor.ime_cancel();
    assert!(!editor.ime().is_composing());
    assert_eq!(editor.rope().to_string_full(), "Текст со [[Старая ссылка]] внутри.\n");
    assert_eq!(editor.selection().range(), (9, 26));
    assert_eq!(editor.history().undo_len(), 0);

    // Now run preedit -> commit
    editor.ime_preedit("нов", Some((0, 3)));
    editor.ime_commit("[[Новая заметка|Алиас]]").unwrap();
    assert!(!editor.ime().is_composing());
    assert_eq!(editor.rope().to_string_full(), "Текст со [[Новая заметка|Алиас]] внутри.\n");
    assert_eq!(editor.history().undo_len(), 1);
    assert_eq!(
        core.documents.read(&core, &note).unwrap().text,
        "Текст со [[Новая заметка|Алиас]] внутри.\n"
    );

    // Single Undo must revert the entire IME commit back to "[[Старая ссылка]]"
    editor.handle_key(KeyCommand::Undo).unwrap();
    assert_eq!(editor.rope().to_string_full(), "Текст со [[Старая ссылка]] внутри.\n");
    assert_eq!(
        core.documents.read(&core, &note).unwrap().text,
        "Текст со [[Старая ссылка]] внутри.\n"
    );
}

#[test]
fn transactional_undo_redo_with_coalescing_and_hub_sync() {
    let (dir, core, _) = open_test_core();
    let note = dir.path().join("Undo.md");
    fs::write(&note, "Начало: ").unwrap();

    let mut editor = NativeEditorSession::open(Arc::clone(&core), &note, "gpui-undo").unwrap();
    editor.handle_key(KeyCommand::MoveDocEnd { select: false }).unwrap();

    // Type "привет" within the 400ms coalescing window -> 1 undo entry
    for ch in "привет".chars() {
        editor.advance_clock_ms(40);
        editor.handle_key(KeyCommand::InsertText(ch.to_string())).unwrap();
    }
    assert_eq!(editor.rope().to_string_full(), "Начало: привет");
    assert_eq!(editor.history().undo_len(), 1);

    // Wait 600ms (> COALESCE_WINDOW_MS) and type " мир" -> 2nd undo entry
    editor.advance_clock_ms(600);
    for ch in " мир".chars() {
        editor.advance_clock_ms(30);
        editor.handle_key(KeyCommand::InsertText(ch.to_string())).unwrap();
    }
    assert_eq!(editor.rope().to_string_full(), "Начало: привет мир");
    assert_eq!(editor.history().undo_len(), 2);

    // Undo once -> removes " мир"
    editor.handle_key(KeyCommand::Undo).unwrap();
    assert_eq!(editor.rope().to_string_full(), "Начало: привет");
    assert_eq!(core.documents.read(&core, &note).unwrap().text, "Начало: привет");

    // Undo again -> removes "привет"
    editor.handle_key(KeyCommand::Undo).unwrap();
    assert_eq!(editor.rope().to_string_full(), "Начало: ");
    assert_eq!(core.documents.read(&core, &note).unwrap().text, "Начало: ");

    // Redo twice -> restores "Начало: привет мир"
    editor.handle_key(KeyCommand::Redo).unwrap();
    editor.handle_key(KeyCommand::Redo).unwrap();
    assert_eq!(editor.rope().to_string_full(), "Начало: привет мир");
    assert_eq!(core.documents.read(&core, &note).unwrap().text, "Начало: привет мир");
}

#[test]
fn save_reopen_external_merge_and_markdown_fidelity() {
    let (dir, core, _) = open_test_core();
    let note = dir.path().join("Маркдаун.md");
    let initial = concat!(
        "---\n",
        "title: Тестовая заметка\n",
        "tags: [rust, gpui]\n",
        "---\n",
        "# Заголовок 1\n",
        "> [!quote] Цитата из книги\n",
        "> Текст цитаты [Открыть](aquilum-reader://epub-1#cfi=/6/2)\n",
        "\n",
        "| Колонка А | Колонка Б |\n",
        "| --- | --- |\n",
        "| [[Заметка 1]] | `код` и **жирный** #тег |\n",
        "- [ ] Задача 1\n",
        "- [x] Задача 2\n"
    );
    fs::write(&note, initial).unwrap();

    let mut editor = NativeEditorSession::open(Arc::clone(&core), &note, "gpui-save").unwrap();
    let vp = editor.viewport();
    // Verify windowed Markdown scanner recognizes all key Aquilum constructs
    let kinds: Vec<MarkdownTokenKind> = vp.markdown_spans.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&MarkdownTokenKind::Frontmatter));
    assert!(kinds.contains(&MarkdownTokenKind::Heading(1)));
    assert!(kinds.contains(&MarkdownTokenKind::CalloutHeader));
    assert!(kinds.contains(&MarkdownTokenKind::ReaderQuote));
    assert!(kinds.contains(&MarkdownTokenKind::TableRow));
    assert!(kinds.contains(&MarkdownTokenKind::WikiLink));
    assert!(kinds.contains(&MarkdownTokenKind::InlineCode));
    assert!(kinds.contains(&MarkdownTokenKind::Bold));
    assert!(kinds.contains(&MarkdownTokenKind::Tag));
    assert!(kinds.contains(&MarkdownTokenKind::TaskUnchecked));
    assert!(kinds.contains(&MarkdownTokenKind::TaskChecked));

    // Append a new line, save immediately, and verify disk + clean state
    editor.handle_key(KeyCommand::MoveDocEnd { select: false }).unwrap();
    editor.handle_key(KeyCommand::InsertText("Новая строка в конце.\n".into())).unwrap();
    assert!(core.documents.is_dirty(&note).unwrap());
    editor.save_now().unwrap();
    assert!(!core.documents.is_dirty(&note).unwrap());

    let expected_after_save = format!("{initial}Новая строка в конце.\n");
    assert_eq!(fs::read_to_string(&note).unwrap(), expected_after_save);

    // Reopen and verify zero data loss
    editor.reopen().unwrap();
    assert_eq!(editor.rope().to_string_full(), expected_after_save);

    // Simulate an external edit via DocumentHub (e.g. MCP tool or external merge)
    let external_updated = expected_after_save.replace("# Заголовок 1", "# Обновлённый заголовок 1");
    core.documents
        .replace_text(&core, &note, &external_updated, SYSTEM_CLIENT, Source::Me, None)
        .unwrap();
    editor.sync_external().unwrap();
    assert_eq!(editor.rope().to_string_full(), external_updated);
}

#[cfg(windows)]
mod mem_counters {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
    }
    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut Counters, cb: u32) -> i32;
    }
    pub fn process_memory_mib() -> (f64, f64) {
        let mut c = Counters {
            cb: std::mem::size_of::<Counters>() as u32,
            ..Default::default()
        };
        let n = c.cb;
        let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, n) };
        if ok != 0 {
            (
                c.working_set_size as f64 / 1_048_576.0,
                c.private_usage as f64 / 1_048_576.0,
            )
        } else {
            (0.0, 0.0)
        }
    }
}

#[cfg(not(windows))]
mod mem_counters {
    pub fn process_memory_mib() -> (f64, f64) {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            let mut parts = statm.split_whitespace();
            let _size = parts.next();
            let resident_pages = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let share_pages = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let page_bytes = 4096.0;
            let rss_mib = (resident_pages as f64 * page_bytes) / 1_048_576.0;
            let priv_mib = (resident_pages.saturating_sub(share_pages) as f64 * page_bytes) / 1_048_576.0;
            return (rss_mib, priv_mib);
        }
        (0.0, 0.0)
    }
}

#[test]
fn large_5mb_document_viewport_window_and_ab_profile() {
    let (dir, core, _) = open_test_core();
    let note = dir.path().join("Large5MB.md");

    let paragraph = concat!(
        "## Раздел базы знаний Aquilum\n",
        "Быстрый локальный Markdown-редактор на Rust и GPUI. [[Связанная заметка|Переход]] и #производительность.\n",
        "> [!quote] Цитата из книги\n",
        "> Фрагмент текста со ссылкой [Открыть](aquilum-reader://book-42#cfi=/6/14) и проверкой Unicode 🦀.\n",
        "- [ ] Проверить клавиатуру, IME, выделение, Undo/Redo и сохранение на 5 МиБ.\n\n"
    );
    let target_bytes = 5_000_000;
    let body = paragraph.repeat(target_bytes / paragraph.len() + 1);
    fs::write(&note, &body).unwrap();

    let (ws_before, priv_before) = mem_counters::process_memory_mib();
    let open_start = Instant::now();
    let mut editor = NativeEditorSession::open(Arc::clone(&core), &note, "gpui-5mb").unwrap();
    let open_ms = open_start.elapsed().as_secs_f64() * 1000.0;

    let initial_metrics = editor.metrics();
    assert!(
        initial_metrics.viewport_len_chars <= MAX_VIEWPORT_CHARS,
        "viewport exceeded MAX_VIEWPORT_CHARS: {}",
        initial_metrics.viewport_len_chars
    );

    // Even when selecting the entire 5 MiB document (Ctrl+A), the GPUI viewport must remain <= 8192 chars!
    editor.handle_key(KeyCommand::SelectAll).unwrap();
    let select_all_vp = editor.viewport();
    assert!(
        select_all_vp.text.chars().count() <= MAX_VIEWPORT_CHARS,
        "SelectAll caused viewport to exceed 8192 chars"
    );
    assert_eq!(select_all_vp.selection.anchor_char_in_doc, 0);
    assert_eq!(select_all_vp.selection.head_char_in_doc, editor.rope().len_chars());

    // Reset caret and run 120 programmatic edits + viewport projections across the 5 MiB note
    editor.handle_key(KeyCommand::MoveDocStart { select: false }).unwrap();
    let total_lines = editor.rope().len_lines();
    let mut dispatch_samples_ms = Vec::with_capacity(120);
    let mut max_vp_chars = initial_metrics.viewport_len_chars;

    for i in 0..120 {
        let target_line = (i * (total_lines / 125)).min(total_lines.saturating_sub(1));
        let target_char = editor.rope().line_col_to_char(LineCol {
            line: target_line,
            column_chars: 0,
        });
        editor.set_selection(Selection::caret(target_char));
        editor.advance_clock_ms(500);

        let t0 = Instant::now();
        editor
            .handle_key(KeyCommand::InsertText(format!("Правка #{i}: GPUI+Rope тест.\n")))
            .unwrap();
        let vp = editor.viewport();
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

        assert!(vp.text.chars().count() <= MAX_VIEWPORT_CHARS);
        max_vp_chars = max_vp_chars.max(vp.text.chars().count());
        std::hint::black_box(&vp);
        dispatch_samples_ms.push(elapsed_ms);
    }

    dispatch_samples_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50_ms = dispatch_samples_ms[dispatch_samples_ms.len() / 2];
    let p95_ms = dispatch_samples_ms[(dispatch_samples_ms.len() * 95) / 100];

    // Undo the last 10 edits, Redo 5 edits, Save, and Reopen to prove 100% data integrity on 5 MiB
    for _ in 0..10 {
        editor.handle_key(KeyCommand::Undo).unwrap();
    }
    for _ in 0..5 {
        editor.handle_key(KeyCommand::Redo).unwrap();
    }
    let expected_text = editor.rope().to_string_full();
    editor.save_now().unwrap();
    let (ws_after, priv_after) = mem_counters::process_memory_mib();

    editor.reopen().unwrap();
    assert_eq!(editor.rope().len_bytes(), expected_text.len());
    assert_eq!(fs::read_to_string(&note).unwrap(), expected_text);

    let final_metrics = editor.metrics();
    let rope_mib = final_metrics.rope_allocated_bytes as f64 / 1_048_576.0;
    let vp_kib = final_metrics.viewport_len_bytes as f64 / 1024.0;

    println!(
        "AQUILUM_GPUI_ROPE_BENCH {{\"input_bytes\":{},\"rope_allocated_mib\":{:.3},\"viewport_max_chars\":{},\"viewport_kib\":{:.2},\"open_ms\":{:.3},\"dispatch_p50_ms\":{:.4},\"dispatch_p95_ms\":{:.4},\"ws_before_mib\":{:.3},\"ws_after_mib\":{:.3},\"priv_before_mib\":{:.3},\"priv_after_mib\":{:.3}}}",
        body.len(),
        rope_mib,
        max_vp_chars,
        vp_kib,
        open_ms,
        p50_ms,
        p95_ms,
        ws_before,
        ws_after,
        priv_before,
        priv_after
    );
}

#[test]
fn native_explicit_save_reports_missing_disk_file() {
    let (dir, core, _) = open_test_core();
    let note = dir.path().join("deleted-after-edit.md");
    fs::write(&note, "first version\n").unwrap();
    let mut editor = NativeEditorSession::open(Arc::clone(&core), &note, "save-error-test").unwrap();
    editor.handle_key(KeyCommand::InsertText("changed ".to_owned())).unwrap();
    fs::remove_file(&note).unwrap();
    assert!(editor.save_now().is_err(), "Ctrl+S must not claim success after the file disappears");
}
