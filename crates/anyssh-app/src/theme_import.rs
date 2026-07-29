use std::path::Path;

use anyssh_storage::TerminalPalette;
use serde::Deserialize;
use thiserror::Error;

use crate::font_assets::{FontAssetError, read_bounded_regular_file};

pub const MAX_TERMINAL_THEME_FILE_BYTES: u64 = 32 * 1024;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TerminalThemeImportError {
    #[error("selected Terminal Theme file is unavailable")]
    Unavailable,
    #[error("selected Terminal Theme must be a regular JSON file")]
    UnsupportedFileType,
    #[error("selected Terminal Theme must be between 1 byte and 32 KiB")]
    InvalidSize,
    #[error("selected Terminal Theme is invalid")]
    InvalidTheme,
    #[error("Terminal Theme import task failed")]
    TaskFailed,
}

pub(crate) struct ImportedTerminalTheme {
    pub(crate) label: String,
    pub(crate) palette: TerminalPalette,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalThemeFile {
    schema_version: u16,
    label: String,
    palette: TerminalPalette,
}

pub(crate) fn read_terminal_theme_import(
    path: &Path,
) -> Result<ImportedTerminalTheme, TerminalThemeImportError> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err(TerminalThemeImportError::UnsupportedFileType);
    }
    let bytes =
        read_bounded_regular_file(path, MAX_TERMINAL_THEME_FILE_BYTES).map_err(map_file_error)?;
    let theme = serde_json::from_slice::<TerminalThemeFile>(&bytes)
        .map_err(|_| TerminalThemeImportError::InvalidTheme)?;
    if theme.schema_version != 1 {
        return Err(TerminalThemeImportError::InvalidTheme);
    }
    theme
        .palette
        .validate()
        .map_err(|_| TerminalThemeImportError::InvalidTheme)?;
    Ok(ImportedTerminalTheme {
        label: theme.label,
        palette: theme.palette,
    })
}

fn map_file_error(error: FontAssetError) -> TerminalThemeImportError {
    match error {
        FontAssetError::Unavailable => TerminalThemeImportError::Unavailable,
        FontAssetError::UnsupportedFileType => TerminalThemeImportError::UnsupportedFileType,
        FontAssetError::InvalidSize => TerminalThemeImportError::InvalidSize,
        _ => TerminalThemeImportError::InvalidTheme,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn theme_import_rejects_unknown_executable_fields() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("theme.json");
        std::fs::write(
            &path,
            r##"{
              "schemaVersion": 1,
              "label": "Unsafe",
              "script": "alert(1)",
              "palette": {
                "background": "#090d16",
                "foreground": "#c8d0df",
                "cursor": "#6be6d2",
                "cursorAccent": "#090d16",
                "selectionBackground": "#294a50",
                "black": "#11151f",
                "red": "#ff7888",
                "green": "#6be6d2",
                "yellow": "#ffc66d",
                "blue": "#7aa2f7",
                "magenta": "#b29cff",
                "cyan": "#6be6d2",
                "white": "#c8d0df",
                "brightBlack": "#667188",
                "brightRed": "#ff9aa6",
                "brightGreen": "#93f2e2",
                "brightYellow": "#ffdb9e",
                "brightBlue": "#a5c2ff",
                "brightMagenta": "#c9bdff",
                "brightCyan": "#9af4e5",
                "brightWhite": "#f1f5ff"
              }
            }"##,
        )
        .expect("write Theme");
        assert_eq!(
            read_terminal_theme_import(&path).err(),
            Some(TerminalThemeImportError::InvalidTheme)
        );
    }
}
