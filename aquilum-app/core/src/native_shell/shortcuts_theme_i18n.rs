//! Official Aquilum Keyboard Shortcut Catalog & Dispatcher (`ShortcutId`),
//! Theme Palette & Design Tokens (`ThemePalette`), UI Scaling (`UiScaleState`),
//! and RU/EN Localization (`Locale`, `t`).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShortcutId {
    NewFile,
    GlobalSearch,
    NewFromTemplate,
    FocusMode,
    PageSearch,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Bold,
    Italic,
    Strikethrough,
    Indent,
    Outdent,
    LineBreak,
    PageSearchNext,
    PageSearchPrevious,
    ReaderNextPage,
    ReaderPreviousPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutToken {
    pub id: ShortcutId,
    pub key: &'static str,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}

pub const SHORTCUTS: &[ShortcutToken] = &[
    ShortcutToken { id: ShortcutId::NewFile, key: "N", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::GlobalSearch, key: "O", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::NewFromTemplate, key: "U", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::FocusMode, key: "F", primary: true, shift: true, alt: false },
    ShortcutToken { id: ShortcutId::PageSearch, key: "F", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::ZoomIn, key: "=", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::ZoomOut, key: "-", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::ZoomReset, key: "0", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::Bold, key: "B", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::Italic, key: "I", primary: true, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::Strikethrough, key: "S", primary: true, shift: true, alt: false },
    ShortcutToken { id: ShortcutId::Indent, key: "Tab", primary: false, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::Outdent, key: "Tab", primary: false, shift: true, alt: false },
    ShortcutToken { id: ShortcutId::LineBreak, key: "Enter", primary: false, shift: true, alt: false },
    ShortcutToken { id: ShortcutId::PageSearchNext, key: "Enter", primary: false, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::PageSearchPrevious, key: "Enter", primary: false, shift: true, alt: false },
    ShortcutToken { id: ShortcutId::ReaderNextPage, key: "ArrowRight", primary: false, shift: false, alt: false },
    ShortcutToken { id: ShortcutId::ReaderPreviousPage, key: "ArrowLeft", primary: false, shift: false, alt: false },
];

pub fn match_shortcut(key: &str, primary: bool, shift: bool, alt: bool) -> Option<ShortcutId> {
    SHORTCUTS
        .iter()
        .find(|s| s.key.eq_ignore_ascii_case(key) && s.primary == primary && s.shift == shift && s.alt == alt)
        .map(|s| s.id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    Ru,
    En,
}

impl Locale {
    pub fn from_code(code: &str) -> Self {
        if code.trim().to_ascii_lowercase().starts_with("en") {
            Self::En
        } else {
            Self::Ru
        }
    }
}

pub fn t(locale: Locale, key: &str) -> &'static str {
    match (locale, key) {
        (Locale::Ru, "sidebar.files") => "Файлы",
        (Locale::En, "sidebar.files") => "Files",
        (Locale::Ru, "sidebar.search") => "Поиск",
        (Locale::En, "sidebar.search") => "Search",
        (Locale::Ru, "sidebar.templates") => "Шаблоны",
        (Locale::En, "sidebar.templates") => "Templates",
        (Locale::Ru, "sidebar.books") => "Библиотека книг",
        (Locale::En, "sidebar.books") => "Book Library",
        (Locale::Ru, "sidebar.trash") => "Корзина",
        (Locale::En, "sidebar.trash") => "Trash",
        (Locale::Ru, "inspector.backlinks") => "Обратные ссылки",
        (Locale::En, "inspector.backlinks") => "Backlinks",
        (Locale::Ru, "inspector.history") => "История версий",
        (Locale::En, "inspector.history") => "Version History",
        (Locale::Ru, "inspector.wikixiv") => "Рекомендации Wikixiv",
        (Locale::En, "inspector.wikixiv") => "Wikixiv Sources",
        (Locale::Ru, "tab.new") => "Новая вкладка",
        (Locale::En, "tab.new") => "New Tab",
        (Locale::Ru, "tab.graph") => "Граф знаний",
        (Locale::En, "tab.graph") => "Knowledge Graph",
        (Locale::Ru, "editor.pageSearch") => "Поиск по заметке",
        (Locale::En, "editor.pageSearch") => "Find in Note",
        _ => "Aquilum",
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScaleState {
    pub scale: f32,
    pub focus_mode: bool,
}

impl Default for UiScaleState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            focus_mode: false,
        }
    }
}

impl UiScaleState {
    pub fn zoom_in(&mut self) {
        self.scale = ((self.scale + 0.1) * 10.0).round() / 10.0;
        self.scale = self.scale.clamp(0.5, 2.0);
    }

    pub fn zoom_out(&mut self) {
        self.scale = ((self.scale - 0.1) * 10.0).round() / 10.0;
        self.scale = self.scale.clamp(0.5, 2.0);
    }

    pub fn zoom_reset(&mut self) {
        self.scale = 1.0;
    }

    pub fn toggle_focus_mode(&mut self) -> bool {
        self.focus_mode = !self.focus_mode;
        self.focus_mode
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub bg_primary: (u8, u8, u8),
    pub bg_secondary: (u8, u8, u8),
    pub bg_elevated: (u8, u8, u8),
    pub text_primary: (u8, u8, u8),
    pub text_muted: (u8, u8, u8),
    pub border_subtle: (u8, u8, u8),
    pub accent_rgb: (u8, u8, u8),
}

impl ThemePalette {
    pub fn resolve(theme_mode: &str, primary_hex: &str) -> Self {
        let is_dark = theme_mode == "dark";
        let accent_rgb = parse_hex_rgb(primary_hex).unwrap_or((20, 113, 235));
        if is_dark {
            Self {
                is_dark: true,
                bg_primary: (20, 22, 26),
                bg_secondary: (26, 29, 35),
                bg_elevated: (34, 38, 46),
                text_primary: (232, 235, 242),
                text_muted: (142, 150, 165),
                border_subtle: (44, 49, 60),
                accent_rgb,
            }
        } else {
            Self {
                is_dark: false,
                bg_primary: (255, 255, 255),
                bg_secondary: (246, 248, 250),
                bg_elevated: (255, 255, 255),
                text_primary: (27, 31, 39),
                text_muted: (102, 112, 133),
                border_subtle: (220, 223, 230),
                accent_rgb,
            }
        }
    }
}

pub fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let clean = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if clean.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
    let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
    let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
    Some((r, g, b))
}
