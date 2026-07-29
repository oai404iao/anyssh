use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock},
};

use anyssh_app::{
    FontAssetError, FontAssetFormat, FontAssetSummary, is_valid_font_asset_id,
    read_managed_font_asset,
};
use tauri::http::{
    Method, Request, Response, StatusCode,
    header::{
        ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE, HeaderValue,
        X_CONTENT_TYPE_OPTIONS,
    },
};

#[derive(Clone, Default)]
pub(crate) struct NativeFontProtocol {
    root: Arc<OnceLock<PathBuf>>,
    registered: Arc<RwLock<BTreeSet<RegisteredFontAsset>>>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RegisteredFontAsset {
    id: String,
    extension: String,
    sha256_hex: String,
}

impl NativeFontProtocol {
    pub(crate) fn initialize(&self, root: PathBuf) -> Result<(), std::io::Error> {
        self.root.set(root).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Font asset protocol root is already initialized",
            )
        })
    }

    pub(crate) fn replace_registered(&self, fonts: &[FontAssetSummary]) {
        let mut registered = self
            .registered
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registered.clear();
        registered.extend(fonts.iter().map(RegisteredFontAsset::from));
    }

    pub(crate) fn register(&self, font: &FontAssetSummary) {
        self.registered
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(RegisteredFontAsset::from(font));
    }

    pub(crate) fn unregister(&self, id: &str) {
        self.registered
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|font| font.id != id);
    }

    pub(crate) fn clear(&self) {
        self.registered
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    pub(crate) fn respond(&self, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
        if request.method() != Method::GET || request.uri().query().is_some() {
            return empty_response(StatusCode::BAD_REQUEST);
        }
        let Some((id, format, digest)) = parse_font_request_path(request.uri().path()) else {
            return empty_response(StatusCode::BAD_REQUEST);
        };
        let Some(root) = self.root.get() else {
            return empty_response(StatusCode::SERVICE_UNAVAILABLE);
        };
        if !self.is_registered(id, format, digest) {
            return empty_response(StatusCode::NOT_FOUND);
        }
        match read_managed_font_asset(root, id, format, digest) {
            Ok(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, format.mime_type())
                .header(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
                .header(
                    CACHE_CONTROL,
                    HeaderValue::from_static("private, max-age=31536000, immutable"),
                )
                .header(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"))
                .body(bytes)
                .unwrap_or_else(|_| empty_response(StatusCode::INTERNAL_SERVER_ERROR)),
            Err(FontAssetError::Integrity | FontAssetError::UnsupportedFileType) => {
                empty_response(StatusCode::NOT_FOUND)
            }
            Err(_) => empty_response(StatusCode::NOT_FOUND),
        }
    }

    fn is_registered(&self, id: &str, format: FontAssetFormat, digest: &str) -> bool {
        let Ok(registered) = self.registered.read() else {
            return false;
        };
        registered.contains(&RegisteredFontAsset {
            id: id.to_owned(),
            extension: format.extension().to_owned(),
            sha256_hex: digest.to_owned(),
        })
    }
}

impl From<&FontAssetSummary> for RegisteredFontAsset {
    fn from(font: &FontAssetSummary) -> Self {
        Self {
            id: font.id().to_owned(),
            extension: font.format().extension().to_owned(),
            sha256_hex: font.sha256_hex().to_owned(),
        }
    }
}

fn parse_font_request_path(path: &str) -> Option<(&str, FontAssetFormat, &str)> {
    let mut segments = path.strip_prefix('/')?.split('/');
    let id = segments.next()?;
    let digest_and_extension = segments.next()?;
    if segments.next().is_some() {
        return None;
    }
    let (digest, extension) = digest_and_extension.rsplit_once('.')?;
    if !is_valid_font_asset_id(id)
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let format = match extension {
        "ttf" => FontAssetFormat::Ttf,
        "otf" => FontAssetFormat::Otf,
        "ttc" => FontAssetFormat::Ttc,
        "woff2" => FontAssetFormat::Woff2,
        _ => return None,
    };
    Some((id, format, digest))
}

fn empty_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"))
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_path_rejects_query_traversal_and_unknown_formats() {
        let digest = "a".repeat(64);
        assert!(parse_font_request_path(&format!("/font-test/{digest}.ttf")).is_some());
        assert!(parse_font_request_path("/../abcd.ttf").is_none());
        assert!(parse_font_request_path("/font-test/abcd.exe").is_none());
        assert!(parse_font_request_path("/font-test/nested/abcd.ttf").is_none());
    }

    #[test]
    fn protocol_requires_a_live_registered_font_asset() {
        let protocol = NativeFontProtocol::default();
        let digest = "a".repeat(64);
        let font = FontAssetSummary::new(
            "font-test".to_owned(),
            "Test Mono".to_owned(),
            "Regular".to_owned(),
            FontAssetFormat::Ttf,
            digest.clone(),
            10,
            1,
        )
        .expect("valid Font summary");

        assert!(!protocol.is_registered(font.id(), font.format(), &digest));
        protocol.register(&font);
        assert!(protocol.is_registered(font.id(), font.format(), &digest));
        protocol.unregister(font.id());
        assert!(!protocol.is_registered(font.id(), font.format(), &digest));
    }
}
