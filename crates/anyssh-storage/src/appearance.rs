use serde::{Deserialize, Serialize};

use crate::{
    StorageError,
    entity_id::{generate_opaque_id, is_valid_opaque_id},
};

pub const DEFAULT_TERMINAL_THEME_ID: &str = "builtin:obsidian";
pub const DEFAULT_FONT_ID: &str = "builtin:anyssh-nerd-mono";
pub const DEFAULT_FONT_FAMILY: &str = "AnySSH Nerd Mono";
pub const MIN_TERMINAL_FONT_SIZE: u16 = 10;
pub const MAX_TERMINAL_FONT_SIZE: u16 = 32;
pub const MIN_TERMINAL_LINE_HEIGHT_MILLIS: u16 = 1_000;
pub const MAX_TERMINAL_LINE_HEIGHT_MILLIS: u16 = 2_000;
pub const MAX_TERMINAL_THEME_LABEL_BYTES: usize = 128;
pub const MAX_TERMINAL_THEMES: usize = 32;
pub const MAX_FONT_FAMILY_BYTES: usize = 128;
pub const MAX_FONT_STYLE_BYTES: usize = 128;
pub const MAX_IMPORTED_FONTS: usize = 32;
pub const MAX_IMPORTED_FONT_BYTES: u64 = 16 * 1024 * 1024;

const TERMINAL_THEME_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppTheme {
    System,
    Dark,
    Light,
}

impl AppTheme {
    pub(crate) const fn storage_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Result<Self, StorageError> {
        match value {
            "system" => Ok(Self::System),
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            _ => Err(StorageError::RecordIntegrity),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontSourceKind {
    Bundled,
    System,
    Imported,
}

impl FontSourceKind {
    pub(crate) const fn storage_value(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::System => "system",
            Self::Imported => "imported",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Result<Self, StorageError> {
        match value {
            "bundled" => Ok(Self::Bundled),
            "system" => Ok(Self::System),
            "imported" => Ok(Self::Imported),
            _ => Err(StorageError::RecordIntegrity),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmbiguousWidth {
    Narrow,
    Wide,
}

impl AmbiguousWidth {
    pub(crate) const fn storage_value(self) -> &'static str {
        match self {
            Self::Narrow => "narrow",
            Self::Wide => "wide",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Result<Self, StorageError> {
        match value {
            "narrow" => Ok(Self::Narrow),
            "wide" => Ok(Self::Wide),
            _ => Err(StorageError::RecordIntegrity),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppearanceSettings {
    app_theme: AppTheme,
    terminal_theme_id: String,
    font_source_kind: FontSourceKind,
    font_id: Option<String>,
    font_family: String,
    font_size: u16,
    line_height_millis: u16,
    ligatures_enabled: bool,
    ambiguous_width: AmbiguousWidth,
}

impl AppearanceSettings {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_theme: AppTheme,
        terminal_theme_id: String,
        font_source_kind: FontSourceKind,
        font_id: Option<String>,
        font_family: String,
        font_size: u16,
        line_height_millis: u16,
        ligatures_enabled: bool,
        ambiguous_width: AmbiguousWidth,
    ) -> Result<Self, StorageError> {
        if !valid_terminal_theme_reference(&terminal_theme_id)
            || !valid_font_family(&font_family)
            || !(MIN_TERMINAL_FONT_SIZE..=MAX_TERMINAL_FONT_SIZE).contains(&font_size)
            || !(MIN_TERMINAL_LINE_HEIGHT_MILLIS..=MAX_TERMINAL_LINE_HEIGHT_MILLIS)
                .contains(&line_height_millis)
            || !valid_font_reference(font_source_kind, font_id.as_deref())
        {
            return Err(StorageError::InvalidAppearance);
        }
        Ok(Self {
            app_theme,
            terminal_theme_id,
            font_source_kind,
            font_id,
            font_family,
            font_size,
            line_height_millis,
            ligatures_enabled,
            ambiguous_width,
        })
    }

    pub fn defaults() -> Self {
        Self {
            app_theme: AppTheme::Dark,
            terminal_theme_id: DEFAULT_TERMINAL_THEME_ID.to_owned(),
            font_source_kind: FontSourceKind::Bundled,
            font_id: Some(DEFAULT_FONT_ID.to_owned()),
            font_family: DEFAULT_FONT_FAMILY.to_owned(),
            font_size: 13,
            line_height_millis: 1_420,
            ligatures_enabled: false,
            ambiguous_width: AmbiguousWidth::Narrow,
        }
    }

    pub const fn app_theme(&self) -> AppTheme {
        self.app_theme
    }

    pub fn terminal_theme_id(&self) -> &str {
        &self.terminal_theme_id
    }

    pub const fn font_source_kind(&self) -> FontSourceKind {
        self.font_source_kind
    }

    pub fn font_id(&self) -> Option<&str> {
        self.font_id.as_deref()
    }

    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    pub const fn font_size(&self) -> u16 {
        self.font_size
    }

    pub const fn line_height_millis(&self) -> u16 {
        self.line_height_millis
    }

    pub const fn ligatures_enabled(&self) -> bool {
        self.ligatures_enabled
    }

    pub const fn ambiguous_width(&self) -> AmbiguousWidth {
        self.ambiguous_width
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TerminalPalette {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub cursor_accent: String,
    pub selection_background: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

impl TerminalPalette {
    pub fn validate(&self) -> Result<(), StorageError> {
        for color in [
            &self.background,
            &self.foreground,
            &self.cursor,
            &self.cursor_accent,
            &self.selection_background,
            &self.black,
            &self.red,
            &self.green,
            &self.yellow,
            &self.blue,
            &self.magenta,
            &self.cyan,
            &self.white,
            &self.bright_black,
            &self.bright_red,
            &self.bright_green,
            &self.bright_yellow,
            &self.bright_blue,
            &self.bright_magenta,
            &self.bright_cyan,
            &self.bright_white,
        ] {
            if !valid_hex_color(color) {
                return Err(StorageError::InvalidTerminalTheme);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalThemeSummary {
    id: String,
    label: String,
    schema_version: u16,
    palette: TerminalPalette,
}

impl TerminalThemeSummary {
    pub fn new(
        id: String,
        label: String,
        schema_version: u16,
        palette: TerminalPalette,
    ) -> Result<Self, StorageError> {
        if !valid_custom_terminal_theme_id(&id)
            || !valid_label(&label, MAX_TERMINAL_THEME_LABEL_BYTES)
            || schema_version != TERMINAL_THEME_SCHEMA_VERSION
        {
            return Err(StorageError::InvalidTerminalTheme);
        }
        palette.validate()?;
        Ok(Self {
            id,
            label,
            schema_version,
            palette,
        })
    }

    pub fn generate(label: String, palette: TerminalPalette) -> Result<Self, StorageError> {
        Self::new(
            generate_opaque_id("theme-")?,
            label,
            TERMINAL_THEME_SCHEMA_VERSION,
            palette,
        )
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub const fn palette(&self) -> &TerminalPalette {
        &self.palette
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontAssetFormat {
    Ttf,
    Otf,
    Ttc,
    Woff2,
}

impl FontAssetFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Ttf => "ttf",
            Self::Otf => "otf",
            Self::Ttc => "ttc",
            Self::Woff2 => "woff2",
        }
    }

    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
            Self::Ttc => "font/collection",
            Self::Woff2 => "font/woff2",
        }
    }

    pub(crate) const fn storage_value(self) -> &'static str {
        match self {
            Self::Ttf => "ttf",
            Self::Otf => "otf",
            Self::Ttc => "ttc",
            Self::Woff2 => "woff2",
        }
    }

    pub(crate) fn from_storage(value: &str) -> Result<Self, StorageError> {
        match value {
            "ttf" => Ok(Self::Ttf),
            "otf" => Ok(Self::Otf),
            "ttc" => Ok(Self::Ttc),
            "woff2" => Ok(Self::Woff2),
            _ => Err(StorageError::RecordIntegrity),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontAssetSummary {
    id: String,
    family: String,
    style: String,
    format: FontAssetFormat,
    sha256_hex: String,
    size_bytes: u64,
    created_at: i64,
}

impl FontAssetSummary {
    pub fn new(
        id: String,
        family: String,
        style: String,
        format: FontAssetFormat,
        sha256_hex: String,
        size_bytes: u64,
        created_at: i64,
    ) -> Result<Self, StorageError> {
        if !is_valid_font_asset_id(&id)
            || !valid_font_family(&family)
            || !valid_text(&style, MAX_FONT_STYLE_BYTES)
            || !valid_sha256_hex(&sha256_hex)
            || size_bytes == 0
            || size_bytes > MAX_IMPORTED_FONT_BYTES
            || created_at < 0
        {
            return Err(StorageError::InvalidFontAsset);
        }
        Ok(Self {
            id,
            family,
            style,
            format,
            sha256_hex,
            size_bytes,
            created_at,
        })
    }

    pub fn generate(
        family: String,
        style: String,
        format: FontAssetFormat,
        sha256_hex: String,
        size_bytes: u64,
        created_at: i64,
    ) -> Result<Self, StorageError> {
        Self::new(
            generate_opaque_id("font-")?,
            family,
            style,
            format,
            sha256_hex,
            size_bytes,
            created_at,
        )
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn style(&self) -> &str {
        &self.style
    }

    pub const fn format(&self) -> FontAssetFormat {
        self.format
    }

    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub const fn created_at(&self) -> i64 {
        self.created_at
    }
}

pub(crate) fn valid_custom_terminal_theme_id(id: &str) -> bool {
    id.starts_with("theme-") && is_valid_opaque_id(id)
}

pub fn is_valid_font_asset_id(id: &str) -> bool {
    id.starts_with("font-") && is_valid_opaque_id(id)
}

fn valid_terminal_theme_reference(id: &str) -> bool {
    matches!(
        id,
        "builtin:obsidian" | "builtin:aurora" | "builtin:solarized-light"
    ) || valid_custom_terminal_theme_id(id)
}

fn valid_font_reference(kind: FontSourceKind, id: Option<&str>) -> bool {
    match kind {
        FontSourceKind::Bundled => id == Some(DEFAULT_FONT_ID),
        FontSourceKind::System => id.is_none(),
        FontSourceKind::Imported => id.is_some_and(is_valid_font_asset_id),
    }
}

fn valid_font_family(value: &str) -> bool {
    valid_text(value, MAX_FONT_FAMILY_BYTES)
}

fn valid_label(value: &str, max_bytes: usize) -> bool {
    valid_text(value, max_bytes) && value.trim() == value
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_hex_color(value: &str) -> bool {
    matches!(value.len(), 7 | 9)
        && value.starts_with('#')
        && value.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> TerminalPalette {
        TerminalPalette {
            background: "#090d16".to_owned(),
            foreground: "#c8d0df".to_owned(),
            cursor: "#6be6d2".to_owned(),
            cursor_accent: "#090d16".to_owned(),
            selection_background: "#294a50".to_owned(),
            black: "#11151f".to_owned(),
            red: "#ff7888".to_owned(),
            green: "#6be6d2".to_owned(),
            yellow: "#ffc66d".to_owned(),
            blue: "#7aa2f7".to_owned(),
            magenta: "#b29cff".to_owned(),
            cyan: "#6be6d2".to_owned(),
            white: "#c8d0df".to_owned(),
            bright_black: "#667188".to_owned(),
            bright_red: "#ff9aa6".to_owned(),
            bright_green: "#93f2e2".to_owned(),
            bright_yellow: "#ffdb9e".to_owned(),
            bright_blue: "#a5c2ff".to_owned(),
            bright_magenta: "#c9bdff".to_owned(),
            bright_cyan: "#9af4e5".to_owned(),
            bright_white: "#f1f5ff".to_owned(),
        }
    }

    #[test]
    fn appearance_defaults_match_the_existing_terminal() {
        let defaults = AppearanceSettings::defaults();
        assert_eq!(defaults.app_theme(), AppTheme::Dark);
        assert_eq!(defaults.terminal_theme_id(), DEFAULT_TERMINAL_THEME_ID);
        assert_eq!(defaults.font_id(), Some(DEFAULT_FONT_ID));
        assert_eq!(defaults.font_size(), 13);
        assert_eq!(defaults.line_height_millis(), 1_420);
    }

    #[test]
    fn terminal_theme_rejects_css_and_remote_values() {
        let mut invalid = palette();
        invalid.background = "url(https://example.invalid/theme)".to_owned();
        assert!(matches!(
            TerminalThemeSummary::generate("Remote".to_owned(), invalid),
            Err(StorageError::InvalidTerminalTheme)
        ));
    }
}
