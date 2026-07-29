use std::{
    collections::BTreeSet,
    error::Error as StdError,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyssh_storage::{
    FontAssetFormat, FontAssetSummary, MAX_IMPORTED_FONT_BYTES, is_valid_font_asset_id,
};
use fontdb::{Database, FaceInfo, Source, Style};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const FONT_ASSET_DIRECTORY_NAME: &str = "font-assets";
pub const MAX_SYSTEM_FONT_SUMMARIES: usize = 512;

const MAX_SYSTEM_FONT_FILES: usize = 4_096;
const MAX_SYSTEM_FONT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECOMPRESSED_WOFF2_BYTES: u32 = 64 * 1024 * 1024;
const MAX_FONT_ASSET_STORE_ENTRIES: usize = 1_024;
const STAGING_MAX_AGE: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SystemFontSummary {
    family: String,
    style: String,
    monospaced: bool,
}

impl SystemFontSummary {
    fn new(family: String, style: String, monospaced: bool) -> Option<Self> {
        if !valid_font_text(&family) || !valid_font_text(&style) {
            return None;
        }
        Some(Self {
            family,
            style,
            monospaced,
        })
    }

    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn style(&self) -> &str {
        &self.style
    }

    pub const fn monospaced(&self) -> bool {
        self.monospaced
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FontAssetError {
    #[error("Font assets are unavailable")]
    Unavailable,
    #[error("selected Font must be a regular file")]
    UnsupportedFileType,
    #[error("selected Font must be between 1 byte and 16 MiB")]
    InvalidSize,
    #[error("selected Font format is unsupported")]
    UnsupportedFormat,
    #[error("selected Font is invalid")]
    InvalidFont,
    #[error("Font asset integrity verification failed")]
    Integrity,
    #[error("Font asset operation task failed")]
    TaskFailed,
}

pub(crate) struct PreparedFontAsset {
    pub(crate) bytes: Vec<u8>,
    pub(crate) family: String,
    pub(crate) style: String,
    pub(crate) format: FontAssetFormat,
    pub(crate) sha256_hex: String,
}

pub(crate) struct StagedFontAsset {
    staging_path: PathBuf,
    final_path: PathBuf,
}

pub(crate) fn read_font_import(path: &Path) -> Result<PreparedFontAsset, FontAssetError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or(FontAssetError::UnsupportedFormat)?;
    let expected_format = match extension.as_str() {
        "ttf" => FontAssetFormat::Ttf,
        "otf" => FontAssetFormat::Otf,
        "ttc" => FontAssetFormat::Ttc,
        "woff2" => FontAssetFormat::Woff2,
        _ => return Err(FontAssetError::UnsupportedFormat),
    };

    let bytes = read_bounded_regular_file(path, MAX_IMPORTED_FONT_BYTES)?;
    let detected_format = detect_font_format(&bytes).ok_or(FontAssetError::UnsupportedFormat)?;
    if detected_format != expected_format {
        return Err(FontAssetError::UnsupportedFormat);
    }

    let parse_bytes = if detected_format == FontAssetFormat::Woff2 {
        validate_woff2_declared_size(&bytes)?;
        decompress_woff2_bounded(&bytes)?
    } else {
        bytes.clone()
    };
    let (family, style) = first_font_metadata(&parse_bytes)?;
    let sha256_hex = sha256_hex(&bytes);

    Ok(PreparedFontAsset {
        bytes,
        family,
        style,
        format: detected_format,
        sha256_hex,
    })
}

pub(crate) fn stage_font_asset(
    root: &Path,
    summary: &FontAssetSummary,
    bytes: &[u8],
) -> Result<StagedFontAsset, FontAssetError> {
    ensure_font_asset_directory(root)?;
    let staging_path = root.join(format!(".{}.staging", summary.id()));
    let final_path = managed_font_asset_path(root, summary.id(), summary.format())?;

    match std::fs::symlink_metadata(&final_path) {
        Ok(_) => return Err(FontAssetError::Integrity),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(FontAssetError::Unavailable),
    }
    let mut file = open_new_managed_file(&staging_path)?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|_| FontAssetError::Unavailable)?;
        file.sync_all().map_err(|_| FontAssetError::Unavailable)?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&staging_path);
        return Err(error);
    }
    Ok(StagedFontAsset {
        staging_path,
        final_path,
    })
}

pub(crate) fn ensure_font_asset_store(root: &Path) -> Result<(), FontAssetError> {
    ensure_font_asset_directory(root)
}

pub(crate) fn current_unix_millis() -> Result<i64, FontAssetError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| FontAssetError::Unavailable)?
        .as_millis();
    i64::try_from(millis).map_err(|_| FontAssetError::Unavailable)
}

pub(crate) fn commit_staged_font_asset(staged: &StagedFontAsset) -> Result<(), FontAssetError> {
    std::fs::rename(&staged.staging_path, &staged.final_path)
        .map_err(|_| FontAssetError::Unavailable)
}

pub(crate) fn remove_staged_font_asset(staged: &StagedFontAsset) {
    let _ = std::fs::remove_file(&staged.staging_path);
    let _ = std::fs::remove_file(&staged.final_path);
}

pub(crate) fn remove_managed_font_asset(
    root: &Path,
    summary: &FontAssetSummary,
) -> Result<(), FontAssetError> {
    let path = managed_font_asset_path(root, summary.id(), summary.format())?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(FontAssetError::Unavailable),
    }
}

pub(crate) fn verify_managed_font_asset(
    root: &Path,
    summary: &FontAssetSummary,
) -> Result<bool, FontAssetError> {
    ensure_font_asset_directory(root)?;
    let path = managed_font_asset_path(root, summary.id(), summary.format())?;
    let bytes = match read_bounded_regular_file(&path, MAX_IMPORTED_FONT_BYTES) {
        Ok(bytes) => bytes,
        Err(FontAssetError::Unavailable) => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(bytes.len() as u64 == summary.size_bytes()
        && detect_font_format(&bytes) == Some(summary.format())
        && sha256_hex(&bytes) == summary.sha256_hex())
}

pub(crate) fn cleanup_stale_font_staging(root: &Path) -> Result<(), FontAssetError> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(FontAssetError::Unavailable),
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(".font-") || !name.ends_with(".staging") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= STAGING_MAX_AGE);
        if stale && metadata.is_file() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

pub(crate) fn cleanup_orphaned_font_assets(
    root: &Path,
    registered: &[FontAssetSummary],
) -> Result<(), FontAssetError> {
    let expected: BTreeSet<_> = registered
        .iter()
        .map(|font| format!("{}.{}", font.id(), font.format().extension()))
        .collect();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(FontAssetError::Unavailable),
    };
    for (index, entry) in entries.enumerate() {
        if index >= MAX_FONT_ASSET_STORE_ENTRIES {
            return Err(FontAssetError::Unavailable);
        }
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if expected.contains(name) || !is_managed_font_asset_name(name) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.is_file() || metadata.file_type().is_symlink() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

pub fn read_managed_font_asset(
    root: &Path,
    id: &str,
    format: FontAssetFormat,
    expected_sha256_hex: &str,
) -> Result<Vec<u8>, FontAssetError> {
    if !valid_sha256_hex(expected_sha256_hex) {
        return Err(FontAssetError::Integrity);
    }
    ensure_font_asset_directory(root)?;
    let path = managed_font_asset_path(root, id, format)?;
    let bytes = read_bounded_regular_file(&path, MAX_IMPORTED_FONT_BYTES)?;
    if detect_font_format(&bytes) != Some(format) || sha256_hex(&bytes) != expected_sha256_hex {
        return Err(FontAssetError::Integrity);
    }
    Ok(bytes)
}

pub(crate) fn enumerate_system_fonts() -> Vec<SystemFontSummary> {
    let mut fonts = BTreeSet::new();
    let mut scanned_files = 0usize;
    for directory in system_font_directories() {
        scan_system_font_directory(&directory, 0, &mut scanned_files, &mut fonts);
        if scanned_files >= MAX_SYSTEM_FONT_FILES || fonts.len() >= MAX_SYSTEM_FONT_SUMMARIES {
            break;
        }
    }
    let mut fonts: Vec<_> = fonts.into_iter().collect();
    fonts.sort_by(|left, right| {
        right
            .monospaced
            .cmp(&left.monospaced)
            .then_with(|| {
                left.family
                    .to_ascii_lowercase()
                    .cmp(&right.family.to_ascii_lowercase())
            })
            .then_with(|| left.style.cmp(&right.style))
    });
    fonts.truncate(MAX_SYSTEM_FONT_SUMMARIES);
    fonts
}

fn scan_system_font_directory(
    directory: &Path,
    depth: usize,
    scanned_files: &mut usize,
    fonts: &mut BTreeSet<SystemFontSummary>,
) {
    if depth > 8
        || *scanned_files >= MAX_SYSTEM_FONT_FILES
        || fonts.len() >= MAX_SYSTEM_FONT_SUMMARIES
    {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if *scanned_files >= MAX_SYSTEM_FONT_FILES || fonts.len() >= MAX_SYSTEM_FONT_SUMMARIES {
            return;
        }
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            scan_system_font_directory(&path, depth + 1, scanned_files, fonts);
            continue;
        }
        if !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > MAX_SYSTEM_FONT_FILE_BYTES
            || !is_system_font_extension(&path)
        {
            continue;
        }
        *scanned_files += 1;
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        collect_font_summaries(&bytes, fonts);
    }
}

fn collect_font_summaries(bytes: &[u8], fonts: &mut BTreeSet<SystemFontSummary>) {
    let mut database = Database::new();
    let ids = database.load_font_source(Source::Binary(Arc::new(bytes.to_vec())));
    for id in ids {
        let Some(face) = database.face(id) else {
            continue;
        };
        let Some(family) = face
            .families
            .first()
            .and_then(|(family, _)| normalized_font_text(family))
        else {
            continue;
        };
        let Some(style) = normalized_font_text(&font_face_style(face)) else {
            continue;
        };
        if let Some(summary) = SystemFontSummary::new(family, style, face.monospaced) {
            fonts.insert(summary);
        }
        if fonts.len() >= MAX_SYSTEM_FONT_SUMMARIES {
            return;
        }
    }
}

fn first_font_metadata(bytes: &[u8]) -> Result<(String, String), FontAssetError> {
    let mut database = Database::new();
    let ids = database.load_font_source(Source::Binary(Arc::new(bytes.to_vec())));
    let id = ids.first().copied().ok_or(FontAssetError::InvalidFont)?;
    let face = database.face(id).ok_or(FontAssetError::InvalidFont)?;
    let family = face
        .families
        .first()
        .and_then(|(family, _)| normalized_font_text(family))
        .ok_or(FontAssetError::InvalidFont)?;
    let style = normalized_font_text(&font_face_style(face)).ok_or(FontAssetError::InvalidFont)?;
    Ok((family, style))
}

fn font_face_style(face: &FaceInfo) -> String {
    let weight = match face.weight.0 {
        0..=150 => "Thin",
        151..=250 => "Extra Light",
        251..=350 => "Light",
        351..=450 => "Regular",
        451..=550 => "Medium",
        551..=650 => "Semi Bold",
        651..=750 => "Bold",
        751..=850 => "Extra Bold",
        _ => "Black",
    };
    match (weight, face.style) {
        ("Regular", Style::Normal) => "Regular".to_owned(),
        ("Regular", Style::Italic) => "Italic".to_owned(),
        ("Regular", Style::Oblique) => "Oblique".to_owned(),
        (_, Style::Normal) => weight.to_owned(),
        (_, Style::Italic) => format!("{weight} Italic"),
        (_, Style::Oblique) => format!("{weight} Oblique"),
    }
}

fn detect_font_format(bytes: &[u8]) -> Option<FontAssetFormat> {
    match bytes.get(..4)? {
        b"\0\x01\0\0" | b"true" | b"typ1" => Some(FontAssetFormat::Ttf),
        b"OTTO" => Some(FontAssetFormat::Otf),
        b"ttcf" => Some(FontAssetFormat::Ttc),
        b"wOF2" => Some(FontAssetFormat::Woff2),
        _ => None,
    }
}

fn validate_woff2_declared_size(bytes: &[u8]) -> Result<(), FontAssetError> {
    let declared = bytes
        .get(16..20)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map(u32::from_be_bytes)
        .ok_or(FontAssetError::InvalidFont)?;
    if declared == 0 || declared > MAX_DECOMPRESSED_WOFF2_BYTES {
        return Err(FontAssetError::InvalidSize);
    }
    Ok(())
}

fn decompress_woff2_bounded(bytes: &[u8]) -> Result<Vec<u8>, FontAssetError> {
    let mut decompressor =
        |compressed: &[u8], expected_size: usize| -> Result<Vec<u8>, Box<dyn StdError>> {
            if expected_size == 0 || expected_size > MAX_DECOMPRESSED_WOFF2_BYTES as usize {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOFF2 output exceeds the supported size",
                )));
            }
            bounded_brotli_decompress(compressed, expected_size)
        };
    let output = wuff::decompress_woff2_with_custom_brotli(bytes, &mut decompressor)
        .map_err(|_| FontAssetError::InvalidFont)?;
    if output.is_empty() || output.len() > MAX_DECOMPRESSED_WOFF2_BYTES as usize {
        return Err(FontAssetError::InvalidSize);
    }
    Ok(output)
}

#[derive(Default)]
struct BrotliBuffer<T>(Box<[T]>);

impl<T> brotli_decompressor::SliceWrapper<T> for BrotliBuffer<T> {
    fn slice(&self) -> &[T] {
        &self.0
    }
}

impl<T> brotli_decompressor::SliceWrapperMut<T> for BrotliBuffer<T> {
    fn slice_mut(&mut self) -> &mut [T] {
        &mut self.0
    }
}

#[derive(Clone, Copy)]
struct BrotliAllocator;

impl<T: Clone + Default> brotli_decompressor::Allocator<T> for BrotliAllocator {
    type AllocatedMemory = BrotliBuffer<T>;

    fn alloc_cell(&mut self, len: usize) -> Self::AllocatedMemory {
        BrotliBuffer(vec![T::default(); len].into_boxed_slice())
    }

    fn free_cell(&mut self, _data: Self::AllocatedMemory) {}
}

fn bounded_brotli_decompress(
    compressed: &[u8],
    expected_size: usize,
) -> Result<Vec<u8>, Box<dyn StdError>> {
    use brotli_decompressor::{BrotliDecompressStream, BrotliResult, BrotliState};

    let mut output = vec![0_u8; expected_size];
    let mut available_in = compressed.len();
    let mut input_offset = 0usize;
    let mut available_out = output.len();
    let mut output_offset = 0usize;
    let mut total_out = 0usize;
    let mut state = BrotliState::new(BrotliAllocator, BrotliAllocator, BrotliAllocator);
    let result = BrotliDecompressStream(
        &mut available_in,
        &mut input_offset,
        compressed,
        &mut available_out,
        &mut output_offset,
        &mut output,
        &mut total_out,
        &mut state,
    );
    if !matches!(result, BrotliResult::ResultSuccess) || output_offset != expected_size {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid WOFF2 Brotli stream",
        )));
    }
    Ok(output)
}

fn managed_font_asset_path(
    root: &Path,
    id: &str,
    format: FontAssetFormat,
) -> Result<PathBuf, FontAssetError> {
    if !is_valid_font_asset_id(id) {
        return Err(FontAssetError::Integrity);
    }
    Ok(root.join(format!("{id}.{}", format.extension())))
}

fn is_managed_font_asset_name(name: &str) -> bool {
    let Some((id, extension)) = name.rsplit_once('.') else {
        return false;
    };
    is_valid_font_asset_id(id) && matches!(extension, "ttf" | "otf" | "ttc" | "woff2")
}

fn ensure_font_asset_directory(root: &Path) -> Result<(), FontAssetError> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(FontAssetError::UnsupportedFileType);
            }
            reject_windows_reparse_metadata(&metadata)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(root).map_err(|_| FontAssetError::Unavailable)?;
        }
        Err(_) => return Err(FontAssetError::Unavailable),
    }
    set_private_directory_permissions(root)?;
    Ok(())
}

pub(crate) fn read_bounded_regular_file(
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, FontAssetError> {
    let link_metadata = std::fs::symlink_metadata(path).map_err(|_| FontAssetError::Unavailable)?;
    if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
        return Err(FontAssetError::UnsupportedFileType);
    }
    reject_windows_reparse_metadata(&link_metadata)?;
    reject_windows_reparse_ancestors(path)?;
    if link_metadata.len() == 0 || link_metadata.len() > max_bytes {
        return Err(FontAssetError::InvalidSize);
    }

    let file = open_font_file(path)?;
    let metadata = file.metadata().map_err(|_| FontAssetError::Unavailable)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(FontAssetError::InvalidSize);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| FontAssetError::Unavailable)?;
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(FontAssetError::InvalidSize);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_font_file(path: &Path) -> Result<File, FontAssetError> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| FontAssetError::Unavailable)
}

#[cfg(windows)]
fn open_font_file(path: &Path) -> Result<File, FontAssetError> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| FontAssetError::Unavailable)
}

#[cfg(not(any(unix, windows)))]
fn open_font_file(path: &Path) -> Result<File, FontAssetError> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|_| FontAssetError::Unavailable)
}

#[cfg(unix)]
fn open_new_managed_file(path: &Path) -> Result<File, FontAssetError> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| FontAssetError::Unavailable)
}

#[cfg(not(unix))]
fn open_new_managed_file(path: &Path) -> Result<File, FontAssetError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| FontAssetError::Unavailable)
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), FontAssetError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| FontAssetError::Unavailable)
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), FontAssetError> {
    Ok(())
}

#[cfg(windows)]
fn reject_windows_reparse_metadata(metadata: &std::fs::Metadata) -> Result<(), FontAssetError> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(FontAssetError::UnsupportedFileType);
    }
    Ok(())
}

#[cfg(not(windows))]
fn reject_windows_reparse_metadata(_metadata: &std::fs::Metadata) -> Result<(), FontAssetError> {
    Ok(())
}

#[cfg(windows)]
fn reject_windows_reparse_ancestors(path: &Path) -> Result<(), FontAssetError> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let Some(parent) = path.parent() else {
        return Err(FontAssetError::UnsupportedFileType);
    };
    for ancestor in parent.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let metadata =
            std::fs::symlink_metadata(ancestor).map_err(|_| FontAssetError::Unavailable)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(FontAssetError::UnsupportedFileType);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn reject_windows_reparse_ancestors(_path: &Path) -> Result<(), FontAssetError> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn system_font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(root) = std::env::var_os("SYSTEMROOT") {
        directories.push(PathBuf::from(root).join("Fonts"));
    } else {
        directories.push(PathBuf::from(r"C:\Windows\Fonts"));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let profile = PathBuf::from(profile);
        directories.push(
            profile
                .join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
        directories.push(
            profile
                .join("AppData")
                .join("Roaming")
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    directories
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn system_font_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.push(home.join(".fonts"));
        directories.push(home.join(".local").join("share").join("fonts"));
    }
    directories
}

#[cfg(target_os = "macos")]
fn system_font_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join("Library").join("Fonts"));
    }
    directories
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn system_font_directories() -> Vec<PathBuf> {
    Vec::new()
}

fn is_system_font_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        })
}

fn normalized_font_text(value: &str) -> Option<String> {
    let value = value.trim();
    if !valid_font_text(value) {
        return None;
    }
    Some(value.to_owned())
}

fn valid_font_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_asset_path_rejects_non_opaque_ids() {
        let root = Path::new("/tmp/font-assets");
        assert!(managed_font_asset_path(root, "../font", FontAssetFormat::Ttf).is_err());
        assert!(managed_font_asset_path(root, "font-test", FontAssetFormat::Ttf).is_ok());
    }

    #[test]
    fn system_font_summaries_are_bounded_and_path_free() {
        let fonts = enumerate_system_fonts();
        assert!(fonts.len() <= MAX_SYSTEM_FONT_SUMMARIES);
        assert!(
            fonts
                .iter()
                .all(|font| { valid_font_text(font.family()) && valid_font_text(font.style()) })
        );
    }
}
