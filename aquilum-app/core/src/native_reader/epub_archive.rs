//! Native EPUB 2/3 ZIP, OPF and XHTML reader.
//!
//! Uses audited ZIP and XML libraries instead of maintaining a custom Deflate
//! implementation. Archives have strict entry and expanded-text limits.

use super::{make_cfi, BookChapter, BookFormat, BookMetadata, BookParagraph, NativeBookDocument};
use quick_xml::events::{BytesRef, BytesStart, BytesText, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

const MAX_FILES: usize = 10_000;
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

fn xml_attribute(tag: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for candidate in tag.attributes().flatten() {
        if candidate.key.as_ref() == key {
            let raw = std::str::from_utf8(candidate.value.as_ref()).ok()?;
            return quick_xml::escape::unescape(raw)
                .ok()
                .map(|value| value.into_owned());
        }
    }
    None
}

fn xml_text(text: &BytesText<'_>) -> Result<String, String> {
    let decoded = text.decode().map_err(|e| format!("invalid XML text: {e}"))?;
    quick_xml::escape::unescape(decoded.as_ref())
        .map(|value| value.into_owned())
        .map_err(|e| format!("invalid XML entity: {e}"))
}

fn xml_reference(reference: &BytesRef<'_>) -> Result<String, String> {
    let name = reference.decode().map_err(|e| format!("invalid XML reference: {e}"))?;
    quick_xml::escape::unescape(&format!("&{name};"))
        .map(|value| value.into_owned())
        .map_err(|e| format!("unsupported XML reference: {e}"))
}

fn entry_text(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    consumed: &mut usize,
) -> Result<String, String> {
    let entry = archive.by_name(name).map_err(|e| format!("missing EPUB entry {name}: {e}"))?;
    let size = entry.size();
    if size > MAX_ENTRY_BYTES || (*consumed as u64).saturating_add(size) > MAX_TOTAL_BYTES as u64 {
        return Err(format!("EPUB uncompressed size limit reached at {name}"));
    }
    let mut decoded = Vec::new();
    // Taking one extra byte ensures malicious or incorrect ZIP headers cannot
    // bypass the expanded-data limit.
    entry
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut decoded)
        .map_err(|e| format!("cannot decode EPUB entry {name}: {e}"))?;
    if decoded.len() as u64 > MAX_ENTRY_BYTES
        || decoded.len().saturating_add(*consumed) > MAX_TOTAL_BYTES
    {
        return Err(format!("EPUB decompression limit exceeded at {name}"));
    }
    *consumed += decoded.len();
    String::from_utf8(decoded).map_err(|_| format!("EPUB entry {name} is not UTF-8"))
}

fn package_path(container: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(container);
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(tag) | Event::Empty(tag)
                if tag.local_name().as_ref() == b"rootfile" =>
            {
                return xml_attribute(&tag, b"full-path")
                    .ok_or_else(|| "EPUB rootfile has no full-path attribute".to_owned());
            }
            Event::Eof => return Err("EPUB container has no rootfile".to_owned()),
            _ => {}
        }
    }
}

fn resolve_archive_href(opf_path: &str, href: &str) -> Result<String, String> {
    // Fragments identify locations inside chapters, not ZIP entry names.
    let relative = href.split('#').next().unwrap_or("");
    if relative.is_empty() || relative.starts_with('/') || relative.contains('\\') {
        return Err(format!("invalid EPUB manifest href: {href}"));
    }
    let prefix = opf_path.rsplit_once('/').map_or("", |(base, _)| base);
    let full = if prefix.is_empty() {
        relative.to_owned()
    } else {
        format!("{prefix}/{relative}")
    };
    let mut parts = Vec::new();
    for part in full.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("EPUB href escapes archive root".to_owned());
                }
            }
            item => parts.push(item),
        }
    }
    Ok(parts.join("/"))
}

struct Package {
    title: Option<String>,
    authors: Vec<String>,
    language: Option<String>,
    chapter_paths: Vec<String>,
}

fn parse_package(xml: &str, opf_path: &str) -> Result<Package, String> {
    let mut reader = Reader::from_str(xml);
    let mut in_metadata = false;
    let mut text_tag: Option<&'static str> = None;
    let mut metadata_text = String::new();
    let mut title = None;
    let mut authors = Vec::new();
    let mut language = None;
    let mut manifest: HashMap<String, String> = HashMap::new();
    let mut spine = Vec::new();
    loop {
        match reader.read_event().map_err(|e| format!("invalid OPF XML: {e}"))? {
            Event::Start(ref tag) | Event::Empty(ref tag) => {
                match tag.local_name().as_ref() {
                    b"metadata" => in_metadata = true,
                    b"title" if in_metadata => { text_tag = Some("title"); metadata_text.clear(); }
                    b"creator" if in_metadata => { text_tag = Some("author"); metadata_text.clear(); }
                    b"language" if in_metadata => { text_tag = Some("language"); metadata_text.clear(); },
                    b"item" => {
                        if let (Some(id), Some(href)) = (
                            xml_attribute(tag, b"id"), xml_attribute(tag, b"href"),
                        ) {
                            manifest.insert(id, href);
                        }
                    }
                    b"itemref" => {
                        if let Some(id) = xml_attribute(tag, b"idref") {
                            spine.push(id);
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(ref value) if text_tag.is_some() => {
                metadata_text.push_str(&xml_text(value)?);
            }
            Event::CData(ref value) if text_tag.is_some() => {
                metadata_text.push_str(&value.decode().map_err(|e| e.to_string())?);
            }
            Event::GeneralRef(ref value) if text_tag.is_some() => {
                metadata_text.push_str(&xml_reference(value)?);
            }
            Event::End(ref tag) => {
                match tag.local_name().as_ref() {
                    b"metadata" => in_metadata = false,
                    b"title" | b"creator" | b"language" => {
                        let value = metadata_text.trim().to_owned();
                        if !value.is_empty() {
                            match text_tag {
                                Some("title") => title = Some(value),
                                Some("author") => authors.push(value),
                                Some("language") => language = Some(value),
                                _ => {}
                            }
                        }
                        text_tag = None;
                        metadata_text.clear();
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if spine.is_empty() {
        return Err("EPUB OPF spine is empty".to_owned());
    }
    let mut chapter_paths = Vec::with_capacity(spine.len());
    for id in spine {
        let href = manifest
            .get(&id)
            .ok_or_else(|| format!("EPUB spine item is missing from manifest: {id}"))?;
        chapter_paths.push(resolve_archive_href(opf_path, href)?);
    }
    Ok(Package { title, authors, language, chapter_paths })
}

fn parse_xhtml(xhtml: &str, chapter_index: usize) -> Result<(Option<String>, Vec<BookParagraph>), String> {
    let mut reader = Reader::from_str(xhtml);
    let mut in_paragraph = false;
    let mut in_heading = false;
    let mut paragraph_text = String::new();
    let mut heading_text = String::new();
    let mut chapter_title = None;
    let mut paragraphs = Vec::new();
    loop {
        match reader.read_event().map_err(|e| format!("invalid EPUB XHTML: {e}"))? {
            Event::Start(ref tag) => {
                match tag.local_name().as_ref() {
                    b"p" => { in_paragraph = true; paragraph_text.clear(); }
                    b"h1" | b"h2" if chapter_title.is_none() => {
                        in_heading = true;
                        heading_text.clear();
                    }
                    b"br" if in_paragraph => paragraph_text.push(' '),
                    _ => {}
                }
            }
            Event::Empty(ref tag) if tag.local_name().as_ref() == b"br" && in_paragraph => {
                paragraph_text.push(' ');
            }
            Event::Text(ref value) => {
                let part = xml_text(value)?;
                if in_paragraph { paragraph_text.push_str(&part); }
                if in_heading { heading_text.push_str(&part); }
            }
            Event::CData(ref value) => {
                let part = value.decode().map_err(|e| e.to_string())?;
                if in_paragraph { paragraph_text.push_str(&part); }
                if in_heading { heading_text.push_str(&part); }
            }
            Event::GeneralRef(ref value) => {
                let part = xml_reference(value)?;
                if in_paragraph { paragraph_text.push_str(&part); }
                if in_heading { heading_text.push_str(&part); }
            }
            Event::End(ref tag) => {
                match tag.local_name().as_ref() {
                    b"p" if in_paragraph => {
                        let text = paragraph_text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            let idx = paragraphs.len();
                            paragraphs.push(BookParagraph {
                                index: idx,
                                text,
                                is_heading: false,
                                cfi: make_cfi(chapter_index, idx),
                            });
                        }
                        in_paragraph = false;
                    }
                    b"h1" | b"h2" if in_heading => {
                        let heading = heading_text.trim();
                        if !heading.is_empty() {
                            chapter_title = Some(heading.to_owned());
                        }
                        in_heading = false;
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((chapter_title, paragraphs))
}

/// Parse real binary EPUB archives, including Deflate and Data Descriptor ZIP
/// entries through the ZIP central directory and CRC checking provided by zip.
pub fn parse_epub_zip_bytes(
    path: &Path,
    relative_path: &str,
    bytes: &[u8],
) -> Result<NativeBookDocument, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("cannot open EPUB ZIP: {e}"))?;
    if archive.len() > MAX_FILES {
        return Err("EPUB ZIP contains too many entries".to_owned());
    }
    let mut consumed = 0;
    let opf_path = if archive.file_names().any(|name| name == "META-INF/container.xml") {
        package_path(&entry_text(&mut archive, "META-INF/container.xml", &mut consumed)?)?
    } else {
        archive.file_names()
            .find(|name| name.ends_with(".opf"))
            .map(str::to_owned)
            .ok_or_else(|| "EPUB contains no OPF package".to_owned())?
    };
    let opf_xml = entry_text(&mut archive, &opf_path, &mut consumed)?;
    let package = parse_package(&opf_xml, &opf_path)?;
    let mut chapters = Vec::new();
    let mut total_paragraphs = 0usize;
    if package.chapter_paths.len() > MAX_FILES {
        return Err("EPUB spine contains too many chapters".to_owned());
    }
    for chapter_path in package.chapter_paths {
        let xhtml = entry_text(&mut archive, &chapter_path, &mut consumed)?;
        let (heading, paragraphs) = parse_xhtml(&xhtml, chapters.len())?;
        if paragraphs.is_empty() { continue; }
        total_paragraphs += paragraphs.len();
        chapters.push(BookChapter {
            index: chapters.len(),
            title: heading.unwrap_or_else(|| format!("Глава {}", chapters.len() + 1)),
            paragraphs,
        });
    }
    if chapters.is_empty() {
        return Err("EPUB spine contains no readable paragraphs".to_owned());
    }
    let title = package.title.unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "EPUB Book".to_owned())
    });
    Ok(NativeBookDocument {
        path: path.to_path_buf(),
        relative_path: relative_path.to_owned(),
        format: BookFormat::Epub,
        metadata: BookMetadata { title, authors: package.authors, language: package.language },
        chapters,
        total_paragraphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    fn fixture() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let entries: &[(&str, &str)] = &[
            ("mimetype", "application/epub+zip"),
            ("META-INF/container.xml", r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#),
            ("OPS/book.opf", r#"<package><metadata><dc:title xmlns:dc="urn:dc">Роман &amp; истории</dc:title><dc:creator xmlns:dc="urn:dc">Автор</dc:creator><dc:language xmlns:dc="urn:dc">ru</dc:language></metadata><manifest><item id="second" href="two.xhtml"/><item id="first" href="first.xhtml"/></manifest><spine><itemref idref="first"/><itemref idref="second"/></spine></package>"#),
            ("OPS/two.xhtml", r#"<html><body><h1>Вторая</h1><p>Два</p></body></html>"#),
            ("OPS/first.xhtml", r#"<html><body><h1>Первая</h1><p>Раз <strong>два</strong> &amp; три</p></body></html>"#),
        ];
        for (name, content) in entries {
            // EPUB requires the first mimetype entry to be stored, not Deflated.
            let compression = if *name == "mimetype" {
                CompressionMethod::Stored
            } else {
                CompressionMethod::Deflated
            };
            let options = SimpleFileOptions::default().compression_method(compression);
            writer.start_file(*name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn native_epub_deflate_spine_metadata_unicode_and_cfi() {
        let bytes = fixture();
        let book = parse_epub_zip_bytes(Path::new("book.epub"), "Files/book.epub", &bytes).unwrap();
        assert_eq!(book.metadata.title, "Роман & истории");
        assert_eq!(book.metadata.authors, vec!["Автор"]);
        assert_eq!(book.total_paragraphs, 2);
        assert_eq!(book.chapters[0].title, "Первая");
        assert_eq!(book.chapters[0].paragraphs[0].text, "Раз два & три");
        assert_eq!(book.chapters[1].paragraphs[0].cfi, make_cfi(1, 0));
    }

    #[test]
    fn native_epub_rejects_corrupt_zip_and_missing_spine_file() {
        assert!(parse_epub_zip_bytes(Path::new("bad.epub"), "bad.epub", b"not a ZIP").is_err());
        let mut bytes = fixture();
        // Corrupt the deflated payload of a spine document while keeping the
        // central directory CRC unchanged: decompression must fail closed.
        let payload_start = {
            let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
            let file = archive.by_name("OPS/first.xhtml").unwrap();
            file.data_start() as usize
        };
        bytes[payload_start + 1] ^= 0x40;
        assert!(parse_epub_zip_bytes(Path::new("bad.epub"), "bad.epub", &bytes).is_err());
    }

    #[test]
    fn native_epub_rejects_archive_root_escape() {
        assert!(resolve_archive_href("OPS/content.opf", "../../else.xhtml").is_err());
        assert_eq!(
            resolve_archive_href("OPS/package.opf", "parts/../one.xhtml").unwrap(),
            "OPS/one.xhtml"
        );
    }
}
