//! Collapsible & Typed YAML Frontmatter Inspector/Mutator (`frontmatter.ts` port)
//! and Media / Image Embed Parser, Formatter & Binary Importer (`imageEmbeds.ts` port).

use super::editor::{EditorError, NativeEditorSession};
use super::history::EditKind;
use crate::files::gate;
use std::collections::BTreeMap;
use std::path::Path;

pub const FM_BOOK_COVER: &str = "Book_cover";
pub const FM_PAGE_COVER: &str = "Page_cover";
pub const FM_BOOK_FILE: &str = "Путь к файлу";
pub const FM_READER_POSITION: &str = "reader_position";
pub const FM_READ_PERCENT: &str = "read_percent";
pub const FM_PAGE_COVER_POSITION: &str = "page_cover_position";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrontmatterValue {
    Text(String),
    List(Vec<String>),
}

impl FrontmatterValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            Self::List(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedBookFields {
    pub book_cover_url: Option<String>,
    pub page_cover_url: Option<String>,
    pub page_cover_position: Option<String>,
    pub book_file: Option<String>,
    pub reader_position: Option<String>,
    pub read_percent: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFrontmatter {
    pub end_char: usize,
    pub raw_yaml: String,
    pub fields: BTreeMap<String, FrontmatterValue>,
    pub field_order: Vec<String>,
}

impl ParsedFrontmatter {
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(FrontmatterValue::as_str)
    }

    pub fn resolve_book_fields(&self) -> ResolvedBookFields {
        let pick = |primary: &str, aliases: &[&str]| -> Option<String> {
            for k in std::iter::once(primary).chain(aliases.iter().copied()) {
                if let Some(v) = self.get_str(k) {
                    let t = v.trim();
                    if !t.is_empty() {
                        return Some(t.to_owned());
                    }
                }
            }
            None
        };
        let read_percent = pick(FM_READ_PERCENT, &[]).and_then(|s| {
            let clean = s.trim().trim_end_matches('%');
            clean.parse::<f64>().ok().map(|n| n.round().clamp(0.0, 100.0) as u8)
        });
        ResolvedBookFields {
            book_cover_url: pick(FM_BOOK_COVER, &["cover_url"]),
            page_cover_url: pick(FM_PAGE_COVER, &["page_cover_url"]),
            page_cover_position: pick(FM_PAGE_COVER_POSITION, &[]),
            book_file: pick(FM_BOOK_FILE, &["book_file", "book"]),
            reader_position: pick(FM_READER_POSITION, &[]),
            read_percent,
        }
    }

    pub fn serialize_block(&self) -> String {
        let mut out = String::from("---\n");
        for key in &self.field_order {
            if let Some(val) = self.fields.get(key) {
                match val {
                    FrontmatterValue::Text(s) => {
                        if s.contains(':') || s.contains('#') || s.starts_with(' ') || s.ends_with(' ') {
                            out.push_str(&format!("{key}: \"{}\"\n", s.replace('"', "\\\"")));
                        } else {
                            out.push_str(&format!("{key}: {s}\n"));
                        }
                    }
                    FrontmatterValue::List(items) => {
                        out.push_str(&format!("{key}: [{}]\n", items.join(", ")));
                    }
                }
            }
        }
        out.push_str("---\n");
        out
    }
}

pub fn parse_frontmatter(doc: &str) -> Option<ParsedFrontmatter> {
    let body = doc.strip_prefix('\u{feff}').unwrap_or(doc);
    let prefix_chars = if doc.starts_with('\u{feff}') { 1 } else { 0 };
    let after_open = if let Some(r) = body.strip_prefix("---\n") {
        (r, 4)
    } else if let Some(r) = body.strip_prefix("---\r\n") {
        (r, 5)
    } else {
        return None;
    };

    let (rest, open_bytes) = after_open;
    let mut yaml_end_byte = None;
    let mut close_fence_bytes = 0;

    let mut pos = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" || trimmed == "..." {
            yaml_end_byte = Some(pos);
            close_fence_bytes = line.len();
            break;
        }
        pos += line.len();
    }

    let yaml_end = yaml_end_byte?;
    let raw_yaml = rest[..yaml_end].to_owned();
    let total_block_bytes = open_bytes + yaml_end + close_fence_bytes;
    let end_char = prefix_chars + body[..total_block_bytes].chars().count();

    let mut fields = BTreeMap::new();
    let mut field_order = Vec::new();

    for line in raw_yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((raw_k, raw_v)) = trimmed.split_once(':') {
            let key = raw_k.trim().to_owned();
            if key.is_empty() {
                continue;
            }
            let val_trimmed = raw_v.trim();
            let value = if val_trimmed.starts_with('[') && val_trimmed.ends_with(']') {
                let inner = &val_trimmed[1..val_trimmed.len() - 1];
                let items = inner
                    .split(',')
                    .map(|s| s.trim().trim_matches(['"', '\'']).to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                FrontmatterValue::List(items)
            } else {
                let unquoted = val_trimmed
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .or_else(|| val_trimmed.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                    .unwrap_or(val_trimmed);
                FrontmatterValue::Text(unquoted.to_owned())
            };
            if !fields.contains_key(&key) {
                field_order.push(key.clone());
            }
            fields.insert(key, value);
        }
    }

    Some(ParsedFrontmatter {
        end_char,
        raw_yaml,
        fields,
        field_order,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageAlign {
    Left,
    Center,
    Right,
}

impl ImageAlign {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Left => Self::Center,
            Self::Center => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageCrop {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageEmbed {
    pub src: String,
    pub width: Option<u32>,
    pub align: ImageAlign,
    pub crop: Option<ImageCrop>,
    pub kind: MediaKind,
    pub wiki: bool,
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "avif", "bmp", "svg"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "ogv", "ogg", "mov", "m4v", "mkv"];

fn media_kind_of(src: &str) -> Option<MediaKind> {
    let base = src.split(['?', '#']).next().unwrap_or(src);
    let (_, ext) = base.rsplit_once('.')?;
    let lower = ext.to_ascii_lowercase();
    if VIDEO_EXTS.contains(&lower.as_str()) {
        Some(MediaKind::Video)
    } else if IMAGE_EXTS.contains(&lower.as_str()) {
        Some(MediaKind::Image)
    } else {
        None
    }
}

fn round_crop(v: f32) -> f32 {
    ((v * 10.0).round() / 10.0).clamp(0.0, 100.0)
}

fn parse_crop(raw: &str) -> Option<ImageCrop> {
    let parts: Vec<f32> = raw
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok())
        .collect::<Option<Vec<_>>>()?;
    if parts.len() != 4 {
        return None;
    }
    let (left, top, right, bottom) = (
        round_crop(parts[0]),
        round_crop(parts[1]),
        round_crop(parts[2]),
        round_crop(parts[3]),
    );
    if right <= left || bottom <= top {
        return None;
    }
    Some(ImageCrop {
        left,
        top,
        right,
        bottom,
    })
}

fn parse_embed_params(src: String, params: &[&str], kind: MediaKind, wiki: bool) -> ImageEmbed {
    let mut width = None;
    let mut align = ImageAlign::Center;
    let mut crop = None;

    for raw in params {
        let p = raw.trim();
        if p.is_empty() {
            continue;
        }
        if let Ok(w) = p.parse::<u32>() {
            if w > 0 {
                width = Some(w);
            }
            continue;
        }
        match p {
            "left" => {
                align = ImageAlign::Left;
                continue;
            }
            "center" => {
                align = ImageAlign::Center;
                continue;
            }
            "right" => {
                align = ImageAlign::Right;
                continue;
            }
            _ => {}
        }
        if let Some(c_str) = p.strip_prefix("crop=") {
            crop = parse_crop(c_str);
        }
    }

    ImageEmbed {
        src,
        width,
        align,
        crop,
        kind,
        wiki,
    }
}

pub fn parse_image_embed(line: &str) -> Option<ImageEmbed> {
    let trimmed = line.trim();
    if let Some(inner) = trimmed.strip_prefix("![[").and_then(|s| s.strip_suffix("]]")) {
        let mut parts = inner.split('|');
        let src = parts.next()?.trim().to_owned();
        let kind = media_kind_of(&src)?;
        let params: Vec<&str> = parts.collect();
        return Some(parse_embed_params(src, &params, kind, true));
    }
    if let Some(rest) = trimmed.strip_prefix("![") {
        let (alt_part, url_part) = rest.split_once("](")?;
        let raw_url = url_part.strip_suffix(')')?.trim();
        let src = raw_url
            .strip_prefix('<')
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(raw_url)
            .trim()
            .to_owned();
        if src.is_empty() {
            return None;
        }
        let kind = media_kind_of(&src).unwrap_or(MediaKind::Image);
        let params: Vec<&str> = alt_part.split('|').collect();
        return Some(parse_embed_params(src, &params, kind, false));
    }
    None
}

pub fn format_image_embed(embed: &ImageEmbed) -> String {
    let mut params = Vec::new();
    if let Some(w) = embed.width {
        params.push(w.to_string());
    }
    if embed.align != ImageAlign::Center {
        params.push(embed.align.as_str().to_owned());
    }
    if let Some(c) = embed.crop {
        params.push(format!("crop={},{},{},{}", c.left, c.top, c.right, c.bottom));
    }
    if embed.wiki {
        if params.is_empty() {
            format!("![[{}]]", embed.src)
        } else {
            format!("![[{}|{}]]", embed.src, params.join("|"))
        }
    } else {
        let target = if embed.src.chars().any(char::is_whitespace) {
            format!("<{}>", embed.src)
        } else {
            embed.src.clone()
        };
        format!("![{}]({target})", params.join("|"))
    }
}

impl NativeEditorSession {
    pub fn parsed_frontmatter(&self) -> Option<ParsedFrontmatter> {
        let head_slice = self.rope().slice_chars(0, 4096.min(self.rope().len_chars()));
        parse_frontmatter(&head_slice)
    }

    pub fn set_frontmatter_property(
        &mut self,
        key: &str,
        value: FrontmatterValue,
    ) -> Result<(), EditorError> {
        if let Some(mut fm) = self.parsed_frontmatter() {
            if !fm.fields.contains_key(key) {
                fm.field_order.push(key.to_owned());
            }
            fm.fields.insert(key.to_owned(), value);
            let serialized = fm.serialize_block();
            self.replace_char_range_atomic(0, fm.end_char, &serialized, EditKind::Replace)
        } else {
            let mut fm = ParsedFrontmatter {
                end_char: 0,
                raw_yaml: String::new(),
                fields: BTreeMap::new(),
                field_order: vec![key.to_owned()],
            };
            fm.fields.insert(key.to_owned(), value);
            let serialized = fm.serialize_block();
            self.replace_char_range_atomic(0, 0, &serialized, EditKind::Replace)
        }
    }

    pub fn import_and_insert_media(
        &mut self,
        workspace_root: &Path,
        file_name: &str,
        bytes: &[u8],
        width: Option<u32>,
    ) -> Result<String, EditorError> {
        let files_folder = self.core().settings.get_config().files.folder;
        let dest_dir = workspace_root.join(&files_folder);
        let _ = std::fs::create_dir_all(&dest_dir);
        let dest_path = dest_dir.join(file_name);
        let _ = gate::create_binary(self.core(), &dest_path, bytes);

        let rel_src = format!("{files_folder}/{file_name}");
        let embed = ImageEmbed {
            src: rel_src.clone(),
            width,
            align: ImageAlign::Center,
            crop: None,
            kind: media_kind_of(&rel_src).unwrap_or(MediaKind::Image),
            wiki: true,
        };
        let markdown = format_image_embed(&embed);
        let (from_char, to_char) = self.selection().range();
        self.replace_char_range_atomic(from_char, to_char, &markdown, EditKind::Paste)?;
        Ok(markdown)
    }
}
