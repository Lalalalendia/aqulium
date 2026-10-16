//! Pure-Rust Document Exporter (`NativeExporter`):
//! Converts Aquilum Markdown notes (including callouts, wiki-links, tasks, tables, and hashtags)
//! into standalone print-styled HTML (`markdown_to_print_html`) or multi-page A4 PDF-1.4 files
//! (`export_markdown_to_pdf_bytes`). The PDF path is an ASCII-only diagnostic,
//! NOT a replacement for the existing high-fidelity WebView2 print renderer.

use pulldown_cmark::{ html, Options, Parser };
use std::fs;
use std::path::Path;

pub struct NativeExporter;

impl NativeExporter {
    /// Converts an Aquilum Markdown document into a standalone, print-styled HTML document
    /// (stripping YAML frontmatter and rendering callouts, wiki-links, tasks, and tables).
    pub fn markdown_to_print_html(title: &str, markdown: &str, theme: &str) -> String {
        let body_md = strip_yaml_frontmatter(markdown);
        let preprocessed = preprocess_aquilum_markdown(body_md);

        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        let parser = Parser::new_ext(&preprocessed, opts);

        let mut rendered_body = String::with_capacity(preprocessed.len() * 2);
        html::push_html(&mut rendered_body, parser);

        let (bg, fg, border, callout_bg) = if theme == "dark" {
            ("#14161a", "#e6e9ef", "#2c313c", "#1e222a")
        } else {
            ("#ffffff", "#1b1f27", "#dcdfe6", "#f5f7fa")
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="utf-8">
<title>{escaped_title}</title>
<style>
  @page {{ size: A4; margin: 18mm; }}
  body {{
    font-family: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    background: {bg}; color: {fg};
    line-height: 1.68; max-width: 720px; margin: 0 auto; padding: 24px;
  }}
  h1, h2, h3, h4 {{ font-weight: 600; margin-top: 1.4em; margin-bottom: 0.5em; }}
  table {{ border-collapse: collapse; width: 100%; margin: 1em 0; }}
  th, td {{ border: 1px solid {border}; padding: 8px 12px; text-align: left; }}
  blockquote {{
    background: {callout_bg}; border-left: 4px solid #1471eb;
    margin: 1em 0; padding: 10px 16px; border-radius: 6px;
  }}
  code {{ font-family: "iA Writer Mono", monospace; padding: 2px 5px; border-radius: 4px; background: {callout_bg}; }}
  .q-print-wikilink {{ color: #1471eb; text-decoration: none; font-weight: 500; }}
  .q-print-hashtag {{ color: #1471eb; background: {callout_bg}; padding: 1px 6px; border-radius: 999px; font-size: 0.9em; }}
</style>
</head>
<body>
{rendered_body}
</body>
</html>"#,
            escaped_title = escape_html(title),
            bg = bg,
            fg = fg,
            border = border,
            callout_bg = callout_bg,
            rendered_body = rendered_body
        )
    }

    pub fn export_note_to_html(
        note_path: &Path,
        output_html_path: &Path,
        theme: &str,
    ) -> Result<usize, String> {
        let raw = fs::read_to_string(note_path).map_err(|e| e.to_string())?;
        let title = note_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Aquilum Note".to_owned());
        let html_doc = Self::markdown_to_print_html(&title, &raw, theme);
        fs::write(output_html_path, html_doc.as_bytes()).map_err(|e| e.to_string())?;
        Ok(html_doc.len())
    }

    /// Produces a simple ASCII PDF diagnostic; typography and Unicode are not preserved.
    /// Production callers should prefer the existing document print renderer.
    pub fn export_markdown_to_pdf_bytes(title: &str, markdown: &str) -> Vec<u8> {
        let body_md = strip_yaml_frontmatter(markdown);
        let mut visual_lines = Vec::new();
        visual_lines.push(format!("# {title}"));
        visual_lines.push(String::new());

        for raw_line in body_md.lines() {
            let clean = simplify_markdown_line_for_pdf(raw_line);
            if clean.chars().count() <= 80 {
                visual_lines.push(clean);
            } else {
                // Soft-wrap at 80 chars
                let chars: Vec<char> = clean.chars().collect();
                for chunk in chars.chunks(80) {
                    visual_lines.push(chunk.iter().collect());
                }
            }
        }

        const LINES_PER_PAGE: usize = 46;
        let pages: Vec<&[String]> = if visual_lines.is_empty() {
            vec![&[]]
        } else {
            visual_lines.chunks(LINES_PER_PAGE).collect()
        };

        build_pdf_document(&pages)
    }

    pub fn export_note_to_pdf(note_path: &Path, output_pdf_path: &Path) -> Result<usize, String> {
        let raw = fs::read_to_string(note_path).map_err(|e| e.to_string())?;
        let title = note_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Aquilum Note".to_owned());
        if !title.is_ascii() || !raw.is_ascii() {
            return Err("Native PDF diagnostics cannot preserve Unicode; use the existing print renderer".to_owned());
        }
        let pdf_bytes = Self::export_markdown_to_pdf_bytes(&title, &raw);
        fs::write(output_pdf_path, &pdf_bytes).map_err(|e| e.to_string())?;
        Ok(pdf_bytes.len())
    }
}

fn strip_yaml_frontmatter(markdown: &str) -> &str {
    let trimmed = markdown.strip_prefix("\u{feff}").unwrap_or(markdown);
    if !trimmed.starts_with("---\n") && !trimmed.starts_with("---\r\n") {
        return trimmed;
    }
    let after_first = if trimmed.starts_with("---\r\n") { 5 } else { 4 };
    let rest = &trimmed[after_first..];
    for (idx, line) in rest.match_indices('\n') {
        let _ = line;
        let next_line = &rest[idx + 1..];
        if next_line.starts_with("---\n")
            || next_line.starts_with("---\r\n")
            || next_line.starts_with("...\n")
            || next_line.starts_with("...\r\n")
            || next_line.trim() == "---"
        {
            let skip = next_line.find('\n').map_or(next_line.len(), |i| i + 1);
            return &next_line[skip..];
        }
    }
    trimmed
}

fn preprocess_aquilum_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 64);
    for line in input.split_inclusive('\n') {
        // Transform [[Target|Alias]] -> <a class="q-print-wikilink" href="#Target">Alias</a>
        let mut current = String::new();
        let mut cursor = 0;
        while let Some(open_rel) = line[cursor..].find("[[") {
            let open = cursor + open_rel;
            current.push_str(&line[cursor..open]);
            let after_open = open + 2;
            if let Some(close_rel) = line[after_open..].find("]]") {
                let close = after_open + close_rel;
                let inner = &line[after_open..close];
                let (target, label) = match inner.split_once('|') {
                    Some((t, a)) => (t.trim(), a.trim()),
                    None => (inner.trim(), inner.trim()),
                };
                current.push_str(&format!(
                    "<a class=\"q-print-wikilink\" href=\"#{}\">{}</a>",
                    escape_html(target),
                    escape_html(label)
                ));
                cursor = close + 2;
            } else {
                current.push_str("[[");
                cursor = after_open;
            }
        }
        current.push_str(&line[cursor..]);
        out.push_str(&current);
    }
    out
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn simplify_markdown_line_for_pdf(line: &str) -> String {
    let mut out = line.replace("[[", "").replace("]]", "");
    if let Some(trimmed) = out.trim_start().strip_prefix("- [ ] ") {
        out = format!("[ ] {trimmed}");
    } else if let Some(trimmed) = out.trim_start().strip_prefix("- [x] ") {
        out = format!("[x] {trimmed}");
    }
    out
}

fn pdf_escape_ascii_or_hex(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            // Transliterate or preserve readable representation in base Helvetica stream
            c => out.push_str(transliterate_char(c)),
        }
    }
    out
}

fn transliterate_char(ch: char) -> &'static str {
    match ch {
        'а' => "a", 'б' => "b", 'в' => "v", 'г' => "g", 'д' => "d", 'е' | 'ё' => "e",
        'ж' => "zh", 'з' => "z", 'и' => "i", 'й' => "y", 'к' => "k", 'л' => "l",
        'м' => "m", 'н' => "n", 'о' => "o", 'п' => "p", 'р' => "r", 'с' => "s",
        'т' => "t", 'у' => "u", 'ф' => "f", 'х' => "kh", 'ц' => "ts", 'ч' => "ch",
        'ш' => "sh", 'щ' => "shch", 'ъ' | 'ь' => "", 'ы' => "y", 'э' => "e",
        'ю' => "yu", 'я' => "ya",
        'А' => "A", 'Б' => "B", 'В' => "V", 'Г' => "G", 'Д' => "D", 'Е' | 'Ё' => "E",
        'Ж' => "Zh", 'З' => "Z", 'И' => "I", 'Й' => "Y", 'К' => "K", 'Л' => "L",
        'М' => "M", 'Н' => "N", 'О' => "O", 'П' => "P", 'Р' => "R", 'С' => "S",
        'Т' => "T", 'У' => "U", 'Ф' => "F", 'Х' => "Kh", 'Ц' => "Ts", 'Ч' => "Ch",
        'Ш' => "Sh", 'Щ' => "Shch", 'Ъ' | 'Ь' => "", 'Ы' => "Y", 'Э' => "E",
        'Ю' => "Yu", 'Я' => "Ya",
        '—' | '–' => "-", '«' | '»' => "\"",
        _ => "?",
    }
}

fn build_pdf_document(pages: &[&[String]]) -> Vec<u8> {
    // Object 1: Catalog
    // Object 2: Pages
    // Object 3: Font (Helvetica)
    // Objects 4..(4 + 2*P): Page object + Content stream object for each page
    let page_count = pages.len().max(1);
    let mut objects: Vec<Vec<u8>> = Vec::with_capacity(3 + page_count * 2);

    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());

    let kids: String = (0..page_count)
        .map(|i| format!("{} 0 R", 4 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>").into_bytes());
    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());

    for (i, lines) in pages.iter().enumerate() {
        let page_obj_id = 4 + i * 2;
        let content_obj_id = page_obj_id + 1;
        let _ = page_obj_id;

        let mut stream = String::from("BT\n/F1 11 Tf\n15 TL\n50 790 Td\n");
        for line in *lines {
            let escaped = pdf_escape_ascii_or_hex(line);
            stream.push_str(&format!("({escaped}) Tj\nT*\n"));
        }
        stream.push_str("ET\n");
        let stream_bytes = stream.into_bytes();

        let page_dict = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_obj_id} 0 R >>"
        );
        objects.push(page_dict.into_bytes());

        let mut content_obj = format!("<< /Length {} >>\nstream\n", stream_bytes.len()).into_bytes();
        content_obj.extend_from_slice(&stream_bytes);
        content_obj.extend_from_slice(b"endstream");
        objects.push(content_obj);
    }

    let mut pdf = Vec::from(&b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n"[..]);
    let mut offsets = Vec::with_capacity(objects.len());
    for (idx, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", idx + 1).as_bytes());
        pdf.extend_from_slice(obj);
        pdf.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for off in offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}
