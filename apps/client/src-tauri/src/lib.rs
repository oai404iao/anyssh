#![deny(unsafe_code)]

mod native_font;
mod native_key_export;
mod native_known_host;
mod native_passphrase;

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyssh_app::{
    AmbiguousWidth as AppAmbiguousWidth, AppTheme as CoreAppTheme,
    AppearanceSettings as AppAppearanceSettings, ApplicationCore, AuthenticationSource,
    CredentialKind as StorageCredentialKind, CredentialSummary as StorageCredentialSummary,
    DatabaseActorConfig, FONT_ASSET_DIRECTORY_NAME, FontAssetFormat as AppFontAssetFormat,
    FontAssetSummary as AppFontAssetSummary, FontSourceKind as AppFontSourceKind,
    GroupSummary as StorageGroupSummary, HostSummary as StorageHostSummary,
    JumpRouteSummary as StorageJumpRouteSummary, KnownHostSummary as StorageKnownHostSummary,
    Override as StorageOverride, PrivateKeyExportSummary as AppPrivateKeyExportSummary,
    PrivateKeyGenerationAlgorithm as AppPrivateKeyGenerationAlgorithm,
    PrivateKeyPublicSummary as AppPrivateKeyPublicSummary, SnippetDraft as AppSnippetDraft,
    SnippetSummary as AppSnippetSummary, SshHopRequest, SshSessionRequest,
    SystemFontSummary as AppSystemFontSummary, TerminalPalette as AppTerminalPalette,
    TerminalThemeSummary as AppTerminalThemeSummary, VaultState as StorageVaultState,
    VaultStatus as StorageVaultStatus,
};
use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    PortForwardKind as SshPortForwardKind, PortForwardRequest as SshPortForwardRequest,
    PortForwardSummary as SshPortForwardSummary, SessionControl, SessionEvent, SessionHop,
    SpawnedSession, SystemAgentIdentitySummary as SshSystemAgentIdentitySummary,
};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, State,
    ipc::{Channel, InvokeResponseBody},
};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::RwLock;
use zeroize::Zeroizing;

use crate::native_font::NativeFontProtocol;
use crate::native_key_export::{NativePrivateKeyExportPassphrasePrompt, NativeVaultStepUpPrompt};
use crate::native_known_host::NativeKnownHostForgetPrompt;
use crate::native_passphrase::NativePrivateKeyPassphrasePrompt;

const OUTPUT_ACK_BUFFER: usize = 64;
const MAX_IN_FLIGHT_OUTPUT_CHUNKS: usize = 8;
const QA_VAULT_ROOT_ENVIRONMENT_VARIABLE: &str = "ANYSSH_QA_VAULT_ROOT";

#[derive(Clone, Default)]
struct SessionRegistry {
    sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
    next_id: Arc<AtomicU64>,
}

struct SessionEntry {
    control: SessionControl,
    output_acknowledgements: tokio::sync::mpsc::Sender<()>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultStatus {
    state: VaultStatusKind,
    vault_id: Option<String>,
    cipher_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum VaultStatusKind {
    Uninitialized,
    Locked,
    Unlocked,
    Damaged,
}

impl From<StorageVaultStatus> for VaultStatus {
    fn from(status: StorageVaultStatus) -> Self {
        let state = match status.state() {
            StorageVaultState::Uninitialized => VaultStatusKind::Uninitialized,
            StorageVaultState::Locked => VaultStatusKind::Locked,
            StorageVaultState::Unlocked => VaultStatusKind::Unlocked,
            StorageVaultState::Damaged => VaultStatusKind::Damaged,
        };
        Self {
            state,
            vault_id: status.vault_id().map(str::to_owned),
            cipher_version: status.cipher_version().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VaultPinRequest {
    pin: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum AppTheme {
    System,
    Dark,
    Light,
}

impl From<AppTheme> for CoreAppTheme {
    fn from(value: AppTheme) -> Self {
        match value {
            AppTheme::System => Self::System,
            AppTheme::Dark => Self::Dark,
            AppTheme::Light => Self::Light,
        }
    }
}

impl From<CoreAppTheme> for AppTheme {
    fn from(value: CoreAppTheme) -> Self {
        match value {
            CoreAppTheme::System => Self::System,
            CoreAppTheme::Dark => Self::Dark,
            CoreAppTheme::Light => Self::Light,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum FontSourceKind {
    Bundled,
    System,
    Imported,
}

impl From<FontSourceKind> for AppFontSourceKind {
    fn from(value: FontSourceKind) -> Self {
        match value {
            FontSourceKind::Bundled => Self::Bundled,
            FontSourceKind::System => Self::System,
            FontSourceKind::Imported => Self::Imported,
        }
    }
}

impl From<AppFontSourceKind> for FontSourceKind {
    fn from(value: AppFontSourceKind) -> Self {
        match value {
            AppFontSourceKind::Bundled => Self::Bundled,
            AppFontSourceKind::System => Self::System,
            AppFontSourceKind::Imported => Self::Imported,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum AmbiguousWidth {
    Narrow,
    Wide,
}

impl From<AmbiguousWidth> for AppAmbiguousWidth {
    fn from(value: AmbiguousWidth) -> Self {
        match value {
            AmbiguousWidth::Narrow => Self::Narrow,
            AmbiguousWidth::Wide => Self::Wide,
        }
    }
}

impl From<AppAmbiguousWidth> for AmbiguousWidth {
    fn from(value: AppAmbiguousWidth) -> Self {
        match value {
            AppAmbiguousWidth::Narrow => Self::Narrow,
            AppAmbiguousWidth::Wide => Self::Wide,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppearanceSettings {
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

impl TryFrom<AppearanceSettings> for AppAppearanceSettings {
    type Error = String;

    fn try_from(settings: AppearanceSettings) -> Result<Self, Self::Error> {
        AppAppearanceSettings::new(
            settings.app_theme.into(),
            settings.terminal_theme_id,
            settings.font_source_kind.into(),
            settings.font_id,
            settings.font_family,
            settings.font_size,
            settings.line_height_millis,
            settings.ligatures_enabled,
            settings.ambiguous_width.into(),
        )
        .map_err(|error| error.to_string())
    }
}

impl From<AppAppearanceSettings> for AppearanceSettings {
    fn from(settings: AppAppearanceSettings) -> Self {
        Self {
            app_theme: settings.app_theme().into(),
            terminal_theme_id: settings.terminal_theme_id().to_owned(),
            font_source_kind: settings.font_source_kind().into(),
            font_id: settings.font_id().map(str::to_owned),
            font_family: settings.font_family().to_owned(),
            font_size: settings.font_size(),
            line_height_millis: settings.line_height_millis(),
            ligatures_enabled: settings.ligatures_enabled(),
            ambiguous_width: settings.ambiguous_width().into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateTerminalThemeRequest {
    label: String,
    palette: AppTerminalPalette,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalThemeSummary {
    id: String,
    label: String,
    schema_version: u16,
    palette: AppTerminalPalette,
}

impl From<AppTerminalThemeSummary> for TerminalThemeSummary {
    fn from(theme: AppTerminalThemeSummary) -> Self {
        Self {
            id: theme.id().to_owned(),
            label: theme.label().to_owned(),
            schema_version: theme.schema_version(),
            palette: theme.palette().clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteTerminalThemeRequest {
    theme_id: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum FontAssetFormat {
    Ttf,
    Otf,
    Ttc,
    Woff2,
}

impl From<AppFontAssetFormat> for FontAssetFormat {
    fn from(value: AppFontAssetFormat) -> Self {
        match value {
            AppFontAssetFormat::Ttf => Self::Ttf,
            AppFontAssetFormat::Otf => Self::Otf,
            AppFontAssetFormat::Ttc => Self::Ttc,
            AppFontAssetFormat::Woff2 => Self::Woff2,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontAssetSummary {
    id: String,
    family: String,
    style: String,
    format: FontAssetFormat,
    sha256_hex: String,
    size_bytes: u64,
}

impl From<AppFontAssetSummary> for FontAssetSummary {
    fn from(font: AppFontAssetSummary) -> Self {
        Self {
            id: font.id().to_owned(),
            family: font.family().to_owned(),
            style: font.style().to_owned(),
            format: font.format().into(),
            sha256_hex: font.sha256_hex().to_owned(),
            size_bytes: font.size_bytes(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemFontSummary {
    family: String,
    style: String,
    monospaced: bool,
}

impl From<AppSystemFontSummary> for SystemFontSummary {
    fn from(font: AppSystemFontSummary) -> Self {
        Self {
            family: font.family().to_owned(),
            style: font.style().to_owned(),
            monospaced: font.monospaced(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteFontAssetRequest {
    font_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnippetSummary {
    id: String,
    label: String,
    variables: Vec<String>,
    line_count: u32,
    updated_at: i64,
}

impl From<AppSnippetSummary> for SnippetSummary {
    fn from(snippet: AppSnippetSummary) -> Self {
        Self {
            id: snippet.id().to_owned(),
            label: snippet.label().to_owned(),
            variables: snippet.variables().to_vec(),
            line_count: snippet.line_count(),
            updated_at: snippet.updated_at(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnippetDraft {
    summary: SnippetSummary,
    body: String,
}

impl From<AppSnippetDraft> for SnippetDraft {
    fn from(snippet: AppSnippetDraft) -> Self {
        let summary = SnippetSummary {
            id: snippet.summary().id().to_owned(),
            label: snippet.summary().label().to_owned(),
            variables: snippet.summary().variables().to_vec(),
            line_count: snippet.summary().line_count(),
            updated_at: snippet.summary().updated_at(),
        };
        Self {
            summary,
            body: snippet.into_body().to_string(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateSnippetRequest {
    label: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateSnippetRequest {
    snippet_id: String,
    label: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnippetIdRequest {
    snippet_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunSnippetRequest {
    session_id: String,
    snippet_id: String,
    variables: BTreeMap<String, String>,
    append_enter: bool,
    confirmed_multiline: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialSummary {
    id: String,
    label: String,
    username: String,
    kind: CredentialKind,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum CredentialKind {
    Password,
    PrivateKey,
    SystemAgent,
    KeyboardInteractive,
}

impl From<StorageCredentialSummary> for CredentialSummary {
    fn from(summary: StorageCredentialSummary) -> Self {
        let kind = match summary.kind() {
            StorageCredentialKind::Password => CredentialKind::Password,
            StorageCredentialKind::PrivateKey => CredentialKind::PrivateKey,
            StorageCredentialKind::SystemAgent => CredentialKind::SystemAgent,
            StorageCredentialKind::KeyboardInteractive => CredentialKind::KeyboardInteractive,
        };
        Self {
            id: summary.id().to_owned(),
            label: summary.label().to_owned(),
            username: summary.username().to_owned(),
            kind,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatePasswordCredentialRequest {
    label: String,
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdatePasswordCredentialRequest {
    credential_id: String,
    label: String,
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportPrivateKeyCredentialRequest {
    label: String,
    username: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum PrivateKeyGenerationAlgorithm {
    Ed25519,
    Rsa4096,
}

impl From<PrivateKeyGenerationAlgorithm> for AppPrivateKeyGenerationAlgorithm {
    fn from(value: PrivateKeyGenerationAlgorithm) -> Self {
        match value {
            PrivateKeyGenerationAlgorithm::Ed25519 => Self::Ed25519,
            PrivateKeyGenerationAlgorithm::Rsa4096 => Self::Rsa4096,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratePrivateKeyCredentialRequest {
    label: String,
    username: String,
    algorithm: PrivateKeyGenerationAlgorithm,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GetPrivateKeyPublicSummaryRequest {
    credential_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportPrivateKeyCredentialRequest {
    credential_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateKeyPublicSummary {
    credential_id: String,
    algorithm: String,
    fingerprint_sha256: String,
    openssh_public_key: String,
}

impl From<AppPrivateKeyPublicSummary> for PrivateKeyPublicSummary {
    fn from(summary: AppPrivateKeyPublicSummary) -> Self {
        Self {
            credential_id: summary.credential_id().to_owned(),
            algorithm: summary.algorithm().to_owned(),
            fingerprint_sha256: summary.fingerprint_sha256().to_owned(),
            openssh_public_key: summary.openssh_public_key().to_owned(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateKeyExportSummary {
    file_name: String,
    algorithm: String,
    fingerprint_sha256: String,
    encrypted: bool,
}

impl From<AppPrivateKeyExportSummary> for PrivateKeyExportSummary {
    fn from(summary: AppPrivateKeyExportSummary) -> Self {
        Self {
            file_name: summary.file_name().to_owned(),
            algorithm: summary.algorithm().to_owned(),
            fingerprint_sha256: summary.fingerprint_sha256().to_owned(),
            encrypted: summary.encrypted(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateSystemAgentCredentialRequest {
    label: String,
    username: String,
    identity_fingerprint_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateKeyboardInteractiveCredentialRequest {
    label: String,
    username: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateKeyboardInteractiveCredentialRequest {
    credential_id: String,
    label: String,
    username: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemAgentIdentitySummary {
    algorithm: String,
    fingerprint_sha256: String,
    comment: String,
}

impl From<SshSystemAgentIdentitySummary> for SystemAgentIdentitySummary {
    fn from(identity: SshSystemAgentIdentitySummary) -> Self {
        Self {
            algorithm: identity.algorithm().to_owned(),
            fingerprint_sha256: identity.fingerprint_sha256().to_owned(),
            comment: identity.comment().to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum ReferenceOverride {
    Inherit,
    Set { value: String },
    Clear,
}

impl From<ReferenceOverride> for StorageOverride<String> {
    fn from(value: ReferenceOverride) -> Self {
        match value {
            ReferenceOverride::Inherit => Self::Inherit,
            ReferenceOverride::Set { value } => Self::Set(value),
            ReferenceOverride::Clear => Self::Clear,
        }
    }
}

impl From<&StorageOverride<String>> for ReferenceOverride {
    fn from(value: &StorageOverride<String>) -> Self {
        match value {
            StorageOverride::Inherit => Self::Inherit,
            StorageOverride::Set(value) => Self::Set {
                value: value.clone(),
            },
            StorageOverride::Clear => Self::Clear,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupSummary {
    id: String,
    label: String,
    parent_group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
    effective_credential_id: Option<String>,
    effective_jump_route_id: Option<String>,
}

impl From<StorageGroupSummary> for GroupSummary {
    fn from(summary: StorageGroupSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            label: summary.label().to_owned(),
            parent_group_id: summary.parent_group_id().map(str::to_owned),
            credential_override: summary.credential_override().into(),
            jump_route_override: summary.jump_route_override().into(),
            effective_credential_id: summary.effective_credential_id().map(str::to_owned),
            effective_jump_route_id: summary.effective_jump_route_id().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateGroupRequest {
    label: String,
    parent_group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateGroupRequest {
    group_id: String,
    label: String,
    parent_group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostSummary {
    id: String,
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
    effective_credential_id: Option<String>,
    effective_jump_route_id: Option<String>,
}

impl From<StorageHostSummary> for HostSummary {
    fn from(summary: StorageHostSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            display_name: summary.display_name().to_owned(),
            host: summary.host().to_owned(),
            port: summary.port(),
            group_id: summary.group_id().map(str::to_owned),
            credential_override: summary.credential_override().into(),
            jump_route_override: summary.jump_route_override().into(),
            effective_credential_id: summary.effective_credential_id().map(str::to_owned),
            effective_jump_route_id: summary.effective_jump_route_id().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateHostRequest {
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateHostRequest {
    host_id: String,
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnownHostKeySummary {
    algorithm: String,
    fingerprint_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnownHostSummary {
    id: String,
    host: String,
    port: u16,
    keys: Vec<KnownHostKeySummary>,
}

impl From<StorageKnownHostSummary> for KnownHostSummary {
    fn from(summary: StorageKnownHostSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            host: summary.host().to_owned(),
            port: summary.port(),
            keys: summary
                .keys()
                .iter()
                .map(|key| KnownHostKeySummary {
                    algorithm: key.algorithm().to_owned(),
                    fingerprint_sha256: key.fingerprint_sha256().to_owned(),
                })
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ForgetKnownHostRequest {
    known_host_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JumpRouteSummary {
    id: String,
    label: String,
    host_ids: Vec<String>,
}

impl From<StorageJumpRouteSummary> for JumpRouteSummary {
    fn from(summary: StorageJumpRouteSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            label: summary.label().to_owned(),
            host_ids: summary.host_ids().to_vec(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateJumpRouteRequest {
    label: String,
    host_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateJumpRouteRequest {
    jump_route_id: String,
    label: String,
    host_ids: Vec<String>,
}

impl SessionRegistry {
    fn new_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("ssh-{id}")
    }

    async fn insert(
        &self,
        id: String,
        control: SessionControl,
        output_acknowledgements: tokio::sync::mpsc::Sender<()>,
    ) {
        self.sessions.write().await.insert(
            id,
            SessionEntry {
                control,
                output_acknowledgements,
            },
        );
    }

    async fn get(&self, id: &str) -> Result<SessionControl, String> {
        self.sessions
            .read()
            .await
            .get(id)
            .map(|entry| entry.control.clone())
            .ok_or_else(|| format!("SSH session `{id}` was not found"))
    }

    async fn acknowledge_output(&self, id: &str) -> Result<(), String> {
        let acknowledgements = self
            .sessions
            .read()
            .await
            .get(id)
            .map(|entry| entry.output_acknowledgements.clone())
            .ok_or_else(|| format!("SSH session `{id}` was not found"))?;

        acknowledgements
            .send(())
            .await
            .map_err(|_| format!("SSH session `{id}` is no longer receiving terminal output"))
    }

    async fn remove(&self, id: &str) {
        self.sessions.write().await.remove(id);
    }

    async fn remove_and_disconnect(&self, id: &str) {
        let entry = { self.sessions.write().await.remove(id) };
        if let Some(entry) = entry {
            let _ = entry.control.disconnect().await;
        }
    }

    async fn drain(&self) -> Vec<SessionControl> {
        self.sessions
            .write()
            .await
            .drain()
            .map(|(_, entry)| entry.control)
            .collect()
    }

    async fn disconnect_all(&self) {
        for control in self.drain().await {
            let _ = control.disconnect().await;
        }
    }
}

async fn await_output_capacity(
    acknowledgements: &mut tokio::sync::mpsc::Receiver<()>,
    in_flight_chunks: &mut usize,
) -> bool {
    while acknowledgements.try_recv().is_ok() {
        *in_flight_chunks = in_flight_chunks.saturating_sub(1);
    }

    while *in_flight_chunks >= MAX_IN_FLIGHT_OUTPUT_CHUNKS {
        match acknowledgements.recv().await {
            Some(()) => *in_flight_chunks = in_flight_chunks.saturating_sub(1),
            None => return false,
        }
    }

    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectRequest {
    host: String,
    port: u16,
    authentication: AuthenticationRequest,
    columns: u32,
    rows: u32,
    jump_host: Option<JumpHostRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectSavedHostRequest {
    host_id: String,
    columns: u32,
    rows: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum PortForwardKind {
    Local,
    Remote,
    Dynamic,
}

impl From<PortForwardKind> for SshPortForwardKind {
    fn from(value: PortForwardKind) -> Self {
        match value {
            PortForwardKind::Local => Self::Local,
            PortForwardKind::Remote => Self::Remote,
            PortForwardKind::Dynamic => Self::Dynamic,
        }
    }
}

impl From<SshPortForwardKind> for PortForwardKind {
    fn from(value: SshPortForwardKind) -> Self {
        match value {
            SshPortForwardKind::Local => Self::Local,
            SshPortForwardKind::Remote => Self::Remote,
            SshPortForwardKind::Dynamic => Self::Dynamic,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartPortForwardRequest {
    session_id: String,
    kind: PortForwardKind,
    bind_host: String,
    bind_port: u16,
    destination_host: Option<String>,
    destination_port: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StopPortForwardRequest {
    session_id: String,
    forward_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortForwardSummary {
    id: String,
    kind: PortForwardKind,
    bind_host: String,
    bound_port: u16,
    destination_host: Option<String>,
    destination_port: Option<u16>,
}

impl From<SshPortForwardSummary> for PortForwardSummary {
    fn from(summary: SshPortForwardSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            kind: summary.kind().into(),
            bind_host: summary.bind_host().to_owned(),
            bound_port: summary.bound_port(),
            destination_host: summary.destination_host().map(str::to_owned),
            destination_port: summary.destination_port(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthenticationResponseRequest {
    session_id: String,
    request_id: u64,
    responses: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JumpHostRequest {
    host: String,
    port: u16,
    authentication: AuthenticationRequest,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum AuthenticationRequest {
    TemporaryPassword {
        username: String,
        password: String,
    },
    KeyboardInteractive {
        username: String,
    },
    Credential {
        #[serde(rename = "credentialId")]
        credential_id: String,
    },
}

impl From<AuthenticationRequest> for AuthenticationSource {
    fn from(authentication: AuthenticationRequest) -> Self {
        match authentication {
            AuthenticationRequest::TemporaryPassword { username, password } => {
                Self::TemporaryPassword {
                    username,
                    password: Zeroizing::new(password),
                }
            }
            AuthenticationRequest::KeyboardInteractive { username } => {
                Self::KeyboardInteractive { username }
            }
            AuthenticationRequest::Credential { credential_id } => {
                Self::Credential { credential_id }
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClientEvent {
    Connecting,
    HostKey {
        #[serde(rename = "requestId")]
        request_id: u64,
        hop: ClientSessionHop,
        host: String,
        port: u16,
        algorithm: String,
        #[serde(rename = "fingerprintSha256")]
        fingerprint_sha256: String,
    },
    HostKeyChanged {
        hop: ClientSessionHop,
        host: String,
        port: u16,
        algorithm: String,
        #[serde(rename = "receivedFingerprintSha256")]
        received_fingerprint_sha256: String,
        #[serde(rename = "trustedFingerprintsSha256")]
        trusted_fingerprints_sha256: Vec<String>,
    },
    AuthenticationChallenge {
        #[serde(rename = "requestId")]
        request_id: u64,
        hop: ClientSessionHop,
        host: String,
        port: u16,
        name: String,
        instructions: String,
        prompts: Vec<ClientAuthenticationPrompt>,
    },
    Authenticated,
    Connected,
    ExitStatus {
        code: u32,
    },
    Error {
        message: String,
    },
    Closed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientAuthenticationPrompt {
    text: String,
    echo: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ClientSessionHop {
    JumpHost { index: usize },
    Target,
}

impl From<SessionHop> for ClientSessionHop {
    fn from(hop: SessionHop) -> Self {
        match hop {
            SessionHop::JumpHost { index } => Self::JumpHost { index },
            SessionHop::Target => Self::Target,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
    name: &'static str,
    version: &'static str,
}

#[tauri::command]
fn runtime_info() -> RuntimeInfo {
    RuntimeInfo {
        name: "AnySSH Native",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
async fn vault_status(core: State<'_, ApplicationCore>) -> Result<VaultStatus, String> {
    core.vault_status()
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn vault_create(
    request: VaultPinRequest,
    core: State<'_, ApplicationCore>,
) -> Result<VaultStatus, String> {
    let pin = Zeroizing::new(request.pin);
    core.create_vault(pin)
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn vault_unlock(
    request: VaultPinRequest,
    core: State<'_, ApplicationCore>,
) -> Result<VaultStatus, String> {
    let pin = Zeroizing::new(request.pin);
    core.unlock_vault(pin)
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn vault_lock(
    core: State<'_, ApplicationCore>,
    sessions: State<'_, SessionRegistry>,
    font_protocol: State<'_, NativeFontProtocol>,
) -> Result<VaultStatus, String> {
    sessions.disconnect_all().await;
    font_protocol.clear();
    core.lock_vault()
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn appearance_get(core: State<'_, ApplicationCore>) -> Result<AppearanceSettings, String> {
    core.get_appearance_settings()
        .await
        .map(AppearanceSettings::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn appearance_update(
    request: AppearanceSettings,
    core: State<'_, ApplicationCore>,
) -> Result<AppearanceSettings, String> {
    core.update_appearance_settings(request.try_into()?)
        .await
        .map(AppearanceSettings::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn terminal_theme_create(
    request: CreateTerminalThemeRequest,
    core: State<'_, ApplicationCore>,
) -> Result<TerminalThemeSummary, String> {
    core.create_terminal_theme(request.label, request.palette)
        .await
        .map(TerminalThemeSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn terminal_theme_import(
    app: AppHandle,
    core: State<'_, ApplicationCore>,
) -> Result<Option<TerminalThemeSummary>, String> {
    if !cfg!(any(target_os = "linux", windows)) {
        return Err("Terminal Theme import is not supported on this platform yet".to_owned());
    }
    let selected = app
        .dialog()
        .file()
        .set_title("Import AnySSH Terminal Theme")
        .add_filter("AnySSH Terminal Theme", &["json"])
        .blocking_pick_file();
    let Some(path) = selected_native_file_path(selected, "Terminal Theme")? else {
        return Ok(None);
    };
    core.import_terminal_theme_from_path(path)
        .await
        .map(|theme| Some(TerminalThemeSummary::from(theme)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn terminal_theme_list(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<TerminalThemeSummary>, String> {
    core.list_terminal_themes()
        .await
        .map(|themes| themes.into_iter().map(TerminalThemeSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn terminal_theme_delete(
    request: DeleteTerminalThemeRequest,
    core: State<'_, ApplicationCore>,
) -> Result<bool, String> {
    core.delete_terminal_theme(request.theme_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn font_asset_list(
    core: State<'_, ApplicationCore>,
    font_protocol: State<'_, NativeFontProtocol>,
) -> Result<Vec<FontAssetSummary>, String> {
    match core.list_font_assets().await {
        Ok(fonts) => {
            font_protocol.replace_registered(&fonts);
            Ok(fonts.into_iter().map(FontAssetSummary::from).collect())
        }
        Err(error) => {
            font_protocol.clear();
            Err(error.to_string())
        }
    }
}

#[tauri::command]
async fn font_asset_import(
    app: AppHandle,
    core: State<'_, ApplicationCore>,
    font_protocol: State<'_, NativeFontProtocol>,
) -> Result<Option<FontAssetSummary>, String> {
    if !cfg!(any(target_os = "linux", windows)) {
        return Err("custom Font import is not supported on this platform yet".to_owned());
    }
    let selected = app
        .dialog()
        .file()
        .set_title("Import Terminal Font")
        .add_filter("Terminal Font", &["ttf", "otf", "ttc", "woff2"])
        .blocking_pick_file();
    let Some(path) = selected_native_file_path(selected, "Font")? else {
        return Ok(None);
    };
    core.import_font_asset_from_path(path)
        .await
        .map(|font| {
            font_protocol.register(&font);
            Some(FontAssetSummary::from(font))
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn font_asset_delete(
    request: DeleteFontAssetRequest,
    core: State<'_, ApplicationCore>,
    font_protocol: State<'_, NativeFontProtocol>,
) -> Result<bool, String> {
    let font_id = request.font_id;
    let deleted = core
        .delete_font_asset(font_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    font_protocol.unregister(&font_id);
    Ok(deleted)
}

#[tauri::command]
async fn font_system_list(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<SystemFontSummary>, String> {
    core.list_system_fonts()
        .await
        .map(|fonts| fonts.into_iter().map(SystemFontSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn snippet_create(
    request: CreateSnippetRequest,
    core: State<'_, ApplicationCore>,
) -> Result<SnippetSummary, String> {
    core.create_snippet(request.label, Zeroizing::new(request.body))
        .await
        .map(SnippetSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn snippet_update(
    request: UpdateSnippetRequest,
    core: State<'_, ApplicationCore>,
) -> Result<SnippetSummary, String> {
    core.update_snippet(
        request.snippet_id,
        request.label,
        Zeroizing::new(request.body),
    )
    .await
    .map(SnippetSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn snippet_list(core: State<'_, ApplicationCore>) -> Result<Vec<SnippetSummary>, String> {
    core.list_snippets()
        .await
        .map(|snippets| snippets.into_iter().map(SnippetSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn snippet_get(
    request: SnippetIdRequest,
    core: State<'_, ApplicationCore>,
) -> Result<SnippetDraft, String> {
    core.get_snippet(request.snippet_id)
        .await
        .map(SnippetDraft::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn snippet_delete(
    request: SnippetIdRequest,
    core: State<'_, ApplicationCore>,
) -> Result<bool, String> {
    core.delete_snippet(request.snippet_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn snippet_run(
    request: RunSnippetRequest,
    core: State<'_, ApplicationCore>,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    let control = registry.get(&request.session_id).await?;
    let input = core
        .prepare_snippet_input(
            request.snippet_id,
            request.variables,
            request.append_enter,
            request.confirmed_multiline,
        )
        .await
        .map_err(|error| error.to_string())?
        .into_input();
    control
        .send_input(input.as_bytes().to_vec())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_create_password(
    request: CreatePasswordCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.create_password_credential(
        request.label,
        request.username,
        Zeroizing::new(request.password),
    )
    .await
    .map(CredentialSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_update_password(
    request: UpdatePasswordCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.update_password_credential(
        request.credential_id,
        request.label,
        request.username,
        Zeroizing::new(request.password),
    )
    .await
    .map(CredentialSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_import_private_key(
    request: ImportPrivateKeyCredentialRequest,
    app: AppHandle,
    core: State<'_, ApplicationCore>,
) -> Result<Option<CredentialSummary>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Import SSH private key")
        .blocking_pick_file();
    let Some(path) = selected_private_key_path(selected)? else {
        return Ok(None);
    };

    let prompt = NativePrivateKeyPassphrasePrompt::new(app);
    core.import_private_key_credential_from_path_with_prompt(
        request.label,
        request.username,
        path,
        &prompt,
    )
    .await
    .map(|summary| summary.map(CredentialSummary::from))
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_generate_private_key(
    request: GeneratePrivateKeyCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.generate_private_key_credential(request.label, request.username, request.algorithm.into())
        .await
        .map(CredentialSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_get_private_key_public(
    request: GetPrivateKeyPublicSummaryRequest,
    core: State<'_, ApplicationCore>,
) -> Result<PrivateKeyPublicSummary, String> {
    core.private_key_public_summary(request.credential_id)
        .await
        .map(PrivateKeyPublicSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_export_private_key(
    request: ExportPrivateKeyCredentialRequest,
    app: AppHandle,
    core: State<'_, ApplicationCore>,
) -> Result<Option<PrivateKeyExportSummary>, String> {
    if !cfg!(any(target_os = "linux", windows)) {
        return Err(
            "encrypted Private Key export is not supported on this platform yet".to_owned(),
        );
    }

    let selected = app
        .dialog()
        .file()
        .set_title("Export encrypted SSH private key")
        .set_file_name("anyssh-private-key")
        .add_filter("OpenSSH private key", &["key"])
        .blocking_save_file();
    let Some(path) = selected_private_key_export_path(selected)? else {
        return Ok(None);
    };

    let step_up = NativeVaultStepUpPrompt::new(app.clone());
    let passphrase = NativePrivateKeyExportPassphrasePrompt::new(app);
    core.export_private_key_credential_to_path_with_prompts(
        request.credential_id,
        path,
        &step_up,
        &passphrase,
    )
    .await
    .map(|summary| summary.map(PrivateKeyExportSummary::from))
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_list_system_agent_identities(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<SystemAgentIdentitySummary>, String> {
    core.list_system_agent_identities()
        .await
        .map(|identities| {
            identities
                .into_iter()
                .map(SystemAgentIdentitySummary::from)
                .collect()
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_create_system_agent(
    request: CreateSystemAgentCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.create_system_agent_credential(
        request.label,
        request.username,
        request.identity_fingerprint_sha256,
    )
    .await
    .map(CredentialSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_create_keyboard_interactive(
    request: CreateKeyboardInteractiveCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.create_keyboard_interactive_credential(request.label, request.username)
        .await
        .map(CredentialSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_update_keyboard_interactive(
    request: UpdateKeyboardInteractiveCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.update_keyboard_interactive_credential(
        request.credential_id,
        request.label,
        request.username,
    )
    .await
    .map(CredentialSummary::from)
    .map_err(|error| error.to_string())
}

fn selected_private_key_path(
    selected: Option<FilePath>,
) -> Result<Option<std::path::PathBuf>, String> {
    selected
        .map(|selected| {
            selected
                .into_path()
                .map_err(|_| "selected private key cannot be read on this platform".to_owned())
        })
        .transpose()
}

fn selected_native_file_path(
    selected: Option<FilePath>,
    kind: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    selected
        .map(|selected| {
            selected
                .into_path()
                .map_err(|_| format!("selected {kind} cannot be read on this platform"))
        })
        .transpose()
}

fn selected_private_key_export_path(
    selected: Option<FilePath>,
) -> Result<Option<std::path::PathBuf>, String> {
    selected
        .map(|selected| {
            selected
                .into_path()
                .map_err(|_| "selected export destination is not supported".to_owned())
        })
        .transpose()
}

fn configured_vault_root(default: PathBuf) -> std::io::Result<PathBuf> {
    #[cfg(debug_assertions)]
    let qa_override = std::env::var_os(QA_VAULT_ROOT_ENVIRONMENT_VARIABLE).map(PathBuf::from);
    #[cfg(not(debug_assertions))]
    let qa_override = None;

    resolve_vault_root(default, qa_override)
}

fn resolve_vault_root(default: PathBuf, qa_override: Option<PathBuf>) -> std::io::Result<PathBuf> {
    let Some(qa_override) = qa_override else {
        return Ok(default);
    };
    if !qa_override.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{QA_VAULT_ROOT_ENVIRONMENT_VARIABLE} must be an absolute path"),
        ));
    }
    Ok(qa_override)
}

#[tauri::command]
async fn credential_list(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<CredentialSummary>, String> {
    core.list_credentials()
        .await
        .map(|summaries| summaries.into_iter().map(CredentialSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_delete(
    credential_id: String,
    core: State<'_, ApplicationCore>,
) -> Result<bool, String> {
    core.delete_credential(credential_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_create(
    request: CreateGroupRequest,
    core: State<'_, ApplicationCore>,
) -> Result<GroupSummary, String> {
    core.create_group(
        request.label,
        request.parent_group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
    )
    .await
    .map(GroupSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_update(
    request: UpdateGroupRequest,
    core: State<'_, ApplicationCore>,
) -> Result<GroupSummary, String> {
    core.update_group(
        request.group_id,
        request.label,
        request.parent_group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
    )
    .await
    .map(GroupSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_list(core: State<'_, ApplicationCore>) -> Result<Vec<GroupSummary>, String> {
    core.list_groups()
        .await
        .map(|groups| groups.into_iter().map(GroupSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_delete(group_id: String, core: State<'_, ApplicationCore>) -> Result<bool, String> {
    core.delete_group(group_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn host_create(
    request: CreateHostRequest,
    core: State<'_, ApplicationCore>,
) -> Result<HostSummary, String> {
    core.create_host_with_overrides(
        request.display_name,
        request.host,
        request.port,
        request.group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
    )
    .await
    .map(HostSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn host_update(
    request: UpdateHostRequest,
    core: State<'_, ApplicationCore>,
) -> Result<HostSummary, String> {
    core.update_host_with_overrides(
        request.host_id,
        request.display_name,
        request.host,
        request.port,
        request.group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
    )
    .await
    .map(HostSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn host_list(core: State<'_, ApplicationCore>) -> Result<Vec<HostSummary>, String> {
    core.list_hosts()
        .await
        .map(|hosts| hosts.into_iter().map(HostSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn host_delete(host_id: String, core: State<'_, ApplicationCore>) -> Result<bool, String> {
    core.delete_host(host_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn known_host_list(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<KnownHostSummary>, String> {
    core.list_known_hosts()
        .await
        .map(|known_hosts| {
            known_hosts
                .into_iter()
                .map(KnownHostSummary::from)
                .collect()
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn known_host_forget(
    request: ForgetKnownHostRequest,
    app: AppHandle,
    core: State<'_, ApplicationCore>,
) -> Result<bool, String> {
    let prompt = NativeKnownHostForgetPrompt::new(app);
    core.forget_known_host_with_prompt(request.known_host_id, &prompt)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn jump_route_create(
    request: CreateJumpRouteRequest,
    core: State<'_, ApplicationCore>,
) -> Result<JumpRouteSummary, String> {
    core.create_jump_route(request.label, request.host_ids)
        .await
        .map(JumpRouteSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn jump_route_update(
    request: UpdateJumpRouteRequest,
    core: State<'_, ApplicationCore>,
) -> Result<JumpRouteSummary, String> {
    core.update_jump_route(request.jump_route_id, request.label, request.host_ids)
        .await
        .map(JumpRouteSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn jump_route_list(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<JumpRouteSummary>, String> {
    core.list_jump_routes()
        .await
        .map(|routes| routes.into_iter().map(JumpRouteSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn jump_route_delete(
    jump_route_id: String,
    core: State<'_, ApplicationCore>,
) -> Result<bool, String> {
    core.delete_jump_route(jump_route_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_connect(
    request: ConnectRequest,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: State<'_, SessionRegistry>,
    core: State<'_, ApplicationCore>,
) -> Result<String, String> {
    let ConnectRequest {
        host,
        port,
        authentication,
        columns,
        rows,
        jump_host,
    } = request;

    let endpoint = SshEndpoint::new(host, port).map_err(|error| error.to_string())?;
    let terminal_size = TerminalSize::new(columns, rows).map_err(|error| error.to_string())?;
    let jump_host = match jump_host {
        Some(request) => Some(SshHopRequest {
            endpoint: SshEndpoint::new(request.host, request.port)
                .map_err(|error| error.to_string())?,
            authentication: request.authentication.into(),
        }),
        None => None,
    };
    let spawned = core
        .spawn_ssh_session(SshSessionRequest {
            target: SshHopRequest {
                endpoint,
                authentication: authentication.into(),
            },
            jump_host,
            terminal_size,
        })
        .await
        .map_err(|error| error.to_string())?;

    register_spawned_session(spawned, events, data, registry.inner()).await
}

#[tauri::command]
async fn ssh_connect_saved_host(
    request: ConnectSavedHostRequest,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: State<'_, SessionRegistry>,
    core: State<'_, ApplicationCore>,
) -> Result<String, String> {
    let terminal_size =
        TerminalSize::new(request.columns, request.rows).map_err(|error| error.to_string())?;
    let spawned = core
        .spawn_saved_host_session(request.host_id, terminal_size)
        .await
        .map_err(|error| error.to_string())?;

    register_spawned_session(spawned, events, data, registry.inner()).await
}

async fn register_spawned_session(
    spawned: SpawnedSession,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: &SessionRegistry,
) -> Result<String, String> {
    let session_id = registry.new_id();
    let (output_acknowledgements, mut output_acknowledgement_receiver) =
        tokio::sync::mpsc::channel(OUTPUT_ACK_BUFFER);
    registry
        .insert(session_id.clone(), spawned.control, output_acknowledgements)
        .await;

    let registry = registry.clone();
    let event_session_id = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let mut session_events = spawned.events;
        let mut in_flight_output_chunks = 0usize;

        while let Some(event) = session_events.recv().await {
            let result = match event {
                SessionEvent::Connecting => events.send(ClientEvent::Connecting),
                SessionEvent::HostKey(info) => events.send(ClientEvent::HostKey {
                    request_id: info.request_id,
                    hop: info.hop.into(),
                    host: info.endpoint.host,
                    port: info.endpoint.port,
                    algorithm: info.algorithm,
                    fingerprint_sha256: info.fingerprint_sha256,
                }),
                SessionEvent::HostKeyChanged(info) => events.send(ClientEvent::HostKeyChanged {
                    hop: info.hop.into(),
                    host: info.endpoint.host,
                    port: info.endpoint.port,
                    algorithm: info.algorithm,
                    received_fingerprint_sha256: info.received_fingerprint_sha256,
                    trusted_fingerprints_sha256: info.trusted_fingerprints_sha256,
                }),
                SessionEvent::AuthenticationChallenge(info) => {
                    events.send(ClientEvent::AuthenticationChallenge {
                        request_id: info.request_id,
                        hop: info.hop.into(),
                        host: info.endpoint.host,
                        port: info.endpoint.port,
                        name: info.name,
                        instructions: info.instructions,
                        prompts: info
                            .prompts
                            .into_iter()
                            .map(|prompt| ClientAuthenticationPrompt {
                                text: prompt.text,
                                echo: prompt.echo,
                            })
                            .collect(),
                    })
                }
                SessionEvent::Authenticated => events.send(ClientEvent::Authenticated),
                SessionEvent::Connected => events.send(ClientEvent::Connected),
                SessionEvent::Data(bytes) => {
                    if !await_output_capacity(
                        &mut output_acknowledgement_receiver,
                        &mut in_flight_output_chunks,
                    )
                    .await
                    {
                        break;
                    }

                    in_flight_output_chunks += 1;
                    let sent = data.send(InvokeResponseBody::Raw(bytes.to_vec()));
                    if sent.is_err() {
                        in_flight_output_chunks = in_flight_output_chunks.saturating_sub(1);
                    }
                    sent
                }
                SessionEvent::ExitStatus(code) => events.send(ClientEvent::ExitStatus { code }),
                SessionEvent::Error(message) => events.send(ClientEvent::Error { message }),
                SessionEvent::Closed => {
                    let sent = events.send(ClientEvent::Closed);
                    registry.remove(&event_session_id).await;
                    sent
                }
            };

            if result.is_err() {
                break;
            }
        }

        registry.remove_and_disconnect(&event_session_id).await;
    });

    Ok(session_id)
}

#[tauri::command]
async fn ssh_confirm_host_key(
    session_id: String,
    request_id: u64,
    accepted: bool,
    registry: State<'_, SessionRegistry>,
    core: State<'_, ApplicationCore>,
) -> Result<(), String> {
    let control = registry.get(&session_id).await?;
    core.decide_host_key(&control, request_id, accepted)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_respond_authentication(
    request: AuthenticationResponseRequest,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    let responses = request.responses.map(|responses| {
        responses
            .into_iter()
            .map(Zeroizing::new)
            .collect::<Vec<_>>()
    });
    registry
        .get(&request.session_id)
        .await?
        .respond_authentication(request.request_id, responses)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_ack_output(
    session_id: String,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    registry.acknowledge_output(&session_id).await
}

#[tauri::command]
async fn ssh_send(
    session_id: String,
    input: String,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    registry
        .get(&session_id)
        .await?
        .send_input(input.into_bytes())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_resize(
    session_id: String,
    columns: u32,
    rows: u32,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    let size = TerminalSize::new(columns, rows).map_err(|error| error.to_string())?;

    registry
        .get(&session_id)
        .await?
        .resize(size)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_forward_start(
    request: StartPortForwardRequest,
    registry: State<'_, SessionRegistry>,
) -> Result<PortForwardSummary, String> {
    let forward = SshPortForwardRequest::new(
        request.kind.into(),
        request.bind_host,
        request.bind_port,
        request.destination_host,
        request.destination_port,
    )
    .map_err(|error| error.to_string())?;

    registry
        .get(&request.session_id)
        .await?
        .start_port_forward(forward)
        .await
        .map(PortForwardSummary::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_forward_stop(
    request: StopPortForwardRequest,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    registry
        .get(&request.session_id)
        .await?
        .stop_port_forward(request.forward_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_disconnect(
    session_id: String,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    registry
        .get(&session_id)
        .await?
        .disconnect()
        .await
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let font_protocol = NativeFontProtocol::default();
    let protocol_handler = font_protocol.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("anyssh-font", move |_context, request| {
            protocol_handler.respond(request)
        })
        .manage(SessionRegistry::default())
        .manage(font_protocol)
        .setup(|app| {
            let vault_root = configured_vault_root(app.path().app_data_dir()?.join("vault"))?;
            app.state::<NativeFontProtocol>()
                .initialize(vault_root.join(FONT_ASSET_DIRECTORY_NAME))?;
            let core = ApplicationCore::spawn(vault_root, DatabaseActorConfig::phase0_default())?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runtime_info,
            vault_status,
            vault_create,
            vault_unlock,
            vault_lock,
            appearance_get,
            appearance_update,
            terminal_theme_create,
            terminal_theme_import,
            terminal_theme_list,
            terminal_theme_delete,
            font_asset_list,
            font_asset_import,
            font_asset_delete,
            font_system_list,
            snippet_create,
            snippet_update,
            snippet_list,
            snippet_get,
            snippet_delete,
            snippet_run,
            credential_create_password,
            credential_update_password,
            credential_import_private_key,
            credential_generate_private_key,
            credential_get_private_key_public,
            credential_export_private_key,
            credential_list_system_agent_identities,
            credential_create_system_agent,
            credential_create_keyboard_interactive,
            credential_update_keyboard_interactive,
            credential_list,
            credential_delete,
            group_create,
            group_update,
            group_list,
            group_delete,
            host_create,
            host_update,
            host_list,
            host_delete,
            known_host_list,
            known_host_forget,
            jump_route_create,
            jump_route_update,
            jump_route_list,
            jump_route_delete,
            ssh_connect,
            ssh_connect_saved_host,
            ssh_confirm_host_key,
            ssh_respond_authentication,
            ssh_ack_output,
            ssh_send,
            ssh_resize,
            ssh_forward_start,
            ssh_forward_stop,
            ssh_disconnect
        ])
        .build(tauri::generate_context!())
        .expect("error while building AnySSH");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            let registry = app_handle.state::<SessionRegistry>().inner().clone();
            tauri::async_runtime::block_on(registry.disconnect_all());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use anyssh_ssh::{
        HostKeyPolicy, SessionAuthentication, SshConnectionConfig, SshSessionConfig, spawn_session,
    };

    fn spawn_registry_test_session() -> SpawnedSession {
        spawn_session(SshSessionConfig {
            target: SshConnectionConfig {
                endpoint: SshEndpoint::new("192.0.2.1", 65_000)
                    .expect("documentation-only test endpoint"),
                username: "registry-test".to_owned(),
                authentication: SessionAuthentication::KeyboardInteractive,
                host_key_policy: HostKeyPolicy::Prompt,
            },
            jump_hosts: Vec::new(),
            terminal_size: TerminalSize::new(80, 24).expect("test terminal size"),
            connection_timeout: Duration::from_secs(60),
        })
    }

    #[test]
    fn host_key_event_uses_the_typed_bridge_field_names() {
        let value = serde_json::to_value(ClientEvent::HostKey {
            request_id: 7,
            hop: ClientSessionHop::JumpHost { index: 1 },
            host: "jump.example".to_owned(),
            port: 22,
            algorithm: "ssh-ed25519".to_owned(),
            fingerprint_sha256: "SHA256:test".to_owned(),
        })
        .expect("host-key event should serialize");

        assert_eq!(value["type"], "hostKey");
        assert_eq!(value["requestId"], 7);
        assert_eq!(value["fingerprintSha256"], "SHA256:test");
        assert_eq!(value["hop"]["kind"], "jumpHost");
        assert_eq!(value["hop"]["index"], 1);
        assert!(value.get("request_id").is_none());
        assert!(value.get("fingerprint_sha256").is_none());
    }

    #[test]
    fn changed_host_key_event_is_typed_and_has_no_accept_field() {
        let value = serde_json::to_value(ClientEvent::HostKeyChanged {
            hop: ClientSessionHop::Target,
            host: "target.internal".to_owned(),
            port: 22,
            algorithm: "ssh-ed25519".to_owned(),
            received_fingerprint_sha256: "SHA256:received".to_owned(),
            trusted_fingerprints_sha256: vec!["SHA256:trusted".to_owned()],
        })
        .expect("changed-key event should serialize");

        assert_eq!(value["type"], "hostKeyChanged");
        assert_eq!(value["receivedFingerprintSha256"], "SHA256:received");
        assert_eq!(
            value["trustedFingerprintsSha256"],
            serde_json::json!(["SHA256:trusted"])
        );
        assert!(value.get("accepted").is_none());
        assert!(value.get("publicKey").is_none());
    }

    #[test]
    fn authentication_challenge_event_contains_prompts_but_no_responses() {
        let value = serde_json::to_value(ClientEvent::AuthenticationChallenge {
            request_id: 11,
            hop: ClientSessionHop::Target,
            host: "target.internal".to_owned(),
            port: 22,
            name: "One-time code".to_owned(),
            instructions: "Enter the value from your authenticator.".to_owned(),
            prompts: vec![ClientAuthenticationPrompt {
                text: "Verification code:".to_owned(),
                echo: false,
            }],
        })
        .expect("authentication challenge should serialize");

        assert_eq!(value["type"], "authenticationChallenge");
        assert_eq!(value["requestId"], 11);
        assert_eq!(value["prompts"][0]["text"], "Verification code:");
        assert_eq!(value["prompts"][0]["echo"], false);
        assert!(value.get("responses").is_none());
        assert!(value.get("password").is_none());
        assert!(value.get("credentialId").is_none());
    }

    #[test]
    fn authentication_response_request_rejects_saved_secret_fields() {
        let request: AuthenticationResponseRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "ssh-1",
            "requestId": 11,
            "responses": ["123456"]
        }))
        .expect("typed authentication response");
        assert_eq!(request.session_id, "ssh-1");
        assert_eq!(request.request_id, 11);
        assert_eq!(
            request
                .responses
                .as_ref()
                .and_then(|responses| responses.first())
                .map(String::as_str),
            Some("123456")
        );

        for forbidden in ["password", "credentialId", "privateKey", "otpSeed"] {
            let mut value = serde_json::json!({
                "sessionId": "ssh-1",
                "requestId": 11,
                "responses": ["123456"]
            });
            value[forbidden] = serde_json::json!("forbidden");
            assert!(
                serde_json::from_value::<AuthenticationResponseRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }
    }

    #[test]
    fn connect_request_accepts_an_optional_jump_host() {
        let request: ConnectRequest = serde_json::from_value(serde_json::json!({
            "host": "target.internal",
            "port": 22,
            "authentication": {
                "kind": "temporaryPassword",
                "username": "target-user",
                "password": "target-password"
            },
            "columns": 100,
            "rows": 30,
            "jumpHost": {
                "host": "jump.example",
                "port": 2222,
                "authentication": {
                    "kind": "credential",
                    "credentialId": "cred-jump"
                }
            }
        }))
        .expect("typed Jump Host request should deserialize");

        let jump_host = request.jump_host.expect("Jump Host should be present");
        assert_eq!(jump_host.host, "jump.example");
        assert_eq!(jump_host.port, 2222);
        assert!(matches!(
            jump_host.authentication,
            AuthenticationRequest::Credential { credential_id }
                if credential_id == "cred-jump"
        ));
    }

    #[test]
    fn credential_authentication_rejects_raw_private_key_fields() {
        let request = serde_json::from_value::<AuthenticationRequest>(serde_json::json!({
            "kind": "credential",
            "credentialId": "cred-private-key",
            "privateKey": "must-not-enter-ipc"
        }));

        assert!(request.is_err());
    }

    #[test]
    fn keyboard_interactive_ipc_carries_username_but_no_saved_response_rules() {
        let request: AuthenticationRequest = serde_json::from_value(serde_json::json!({
            "kind": "keyboardInteractive",
            "username": "interactive-user"
        }))
        .expect("keyboard-interactive request should deserialize");
        assert!(matches!(
            request,
            AuthenticationRequest::KeyboardInteractive { username }
                if username == "interactive-user"
        ));

        for forbidden in ["password", "response", "otpSeed", "promptRule"] {
            let mut value = serde_json::json!({
                "kind": "keyboardInteractive",
                "username": "interactive-user"
            });
            value[forbidden] = serde_json::json!("forbidden");
            assert!(
                serde_json::from_value::<AuthenticationRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }

        let create: CreateKeyboardInteractiveCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "label": "Production OTP",
                "username": "interactive-user"
            }))
            .expect("metadata-only interactive Credential request");
        assert_eq!(create.label, "Production OTP");
        assert_eq!(create.username, "interactive-user");

        let rejected = serde_json::from_value::<CreateKeyboardInteractiveCredentialRequest>(
            serde_json::json!({
                "label": "Production OTP",
                "username": "interactive-user",
                "otpSeed": "must-not-enter-ipc"
            }),
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn credential_summary_serializes_metadata_only() {
        let value = serde_json::to_value(CredentialSummary {
            id: "cred-test".to_owned(),
            label: "Test key".to_owned(),
            username: "alice".to_owned(),
            kind: CredentialKind::PrivateKey,
        })
        .expect("credential summary should serialize");

        assert_eq!(value["id"], "cred-test");
        assert_eq!(value["kind"], "privateKey");
        assert!(value.get("password").is_none());
        assert!(value.get("privateKey").is_none());
        assert!(value.get("passphrase").is_none());

        let interactive = serde_json::to_value(CredentialSummary {
            id: "cred-interactive".to_owned(),
            label: "Production OTP".to_owned(),
            username: "interactive-user".to_owned(),
            kind: CredentialKind::KeyboardInteractive,
        })
        .expect("interactive Credential summary should serialize");
        assert_eq!(interactive["kind"], "keyboardInteractive");
        assert!(interactive.get("response").is_none());
        assert!(interactive.get("otpSeed").is_none());
        assert!(interactive.get("promptRule").is_none());
    }

    #[test]
    fn system_agent_ipc_contains_public_metadata_and_rejects_agent_endpoints() {
        let identity = serde_json::to_value(SystemAgentIdentitySummary {
            algorithm: "ssh-ed25519".to_owned(),
            fingerprint_sha256: "SHA256:agent-selector".to_owned(),
            comment: "workstation key".to_owned(),
        })
        .expect("agent identity should serialize");
        assert_eq!(identity["algorithm"], "ssh-ed25519");
        assert_eq!(identity["fingerprintSha256"], "SHA256:agent-selector");
        assert_eq!(identity["comment"], "workstation key");
        assert!(identity.get("publicKey").is_none());
        assert!(identity.get("signature").is_none());
        assert!(identity.get("socketPath").is_none());

        let request: CreateSystemAgentCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector"
            }))
            .expect("metadata-only agent request should deserialize");
        assert_eq!(request.label, "Workstation agent");
        assert_eq!(request.username, "alice");
        assert_eq!(request.identity_fingerprint_sha256, "SHA256:agent-selector");

        for extra in [
            serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector",
                "socketPath": "/tmp/ssh-agent.sock"
            }),
            serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector",
                "namedPipe": "\\\\.\\pipe\\openssh-ssh-agent"
            }),
            serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector",
                "publicKey": "must-not-enter-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<CreateSystemAgentCredentialRequest>(extra).is_err());
        }
    }

    #[test]
    fn private_key_import_request_rejects_paths_and_secrets() {
        let request: ImportPrivateKeyCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "label": "Imported key",
                "username": "alice"
            }))
            .expect("metadata-only import request should deserialize");
        assert_eq!(request.label, "Imported key");
        assert_eq!(request.username, "alice");

        for extra in [
            serde_json::json!({
                "label": "Imported key",
                "username": "alice",
                "path": "/tmp/id_ed25519"
            }),
            serde_json::json!({
                "label": "Imported key",
                "username": "alice",
                "privateKey": "must-not-enter-ipc"
            }),
            serde_json::json!({
                "label": "Imported key",
                "username": "alice",
                "passphrase": "must-not-enter-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<ImportPrivateKeyCredentialRequest>(extra).is_err());
        }
    }

    #[test]
    fn private_key_generation_and_public_projection_ipc_are_metadata_only() {
        let request: GeneratePrivateKeyCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "label": "Generated key",
                "username": "alice",
                "algorithm": "ed25519"
            }))
            .expect("metadata-only generation request");
        assert_eq!(request.label, "Generated key");
        assert_eq!(request.username, "alice");
        assert!(matches!(
            request.algorithm,
            PrivateKeyGenerationAlgorithm::Ed25519
        ));

        for forbidden in [
            "privateKey",
            "passphrase",
            "pin",
            "path",
            "seed",
            "payload",
            "command",
        ] {
            let mut value = serde_json::json!({
                "label": "Generated key",
                "username": "alice",
                "algorithm": "rsa4096"
            });
            value[forbidden] = serde_json::json!("must-not-enter-ipc");
            assert!(
                serde_json::from_value::<GeneratePrivateKeyCredentialRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }

        let projection_request: GetPrivateKeyPublicSummaryRequest =
            serde_json::from_value(serde_json::json!({
                "credentialId": "cred-generated"
            }))
            .expect("ID-only public projection request");
        assert_eq!(projection_request.credential_id, "cred-generated");
        for forbidden in [
            "privateKey",
            "passphrase",
            "pin",
            "path",
            "seed",
            "payload",
            "command",
        ] {
            let mut value = serde_json::json!({
                "credentialId": "cred-generated"
            });
            value[forbidden] = serde_json::json!("must-not-enter-ipc");
            assert!(
                serde_json::from_value::<GetPrivateKeyPublicSummaryRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }

        let projection = serde_json::to_value(PrivateKeyPublicSummary {
            credential_id: "cred-generated".to_owned(),
            algorithm: "ssh-ed25519".to_owned(),
            fingerprint_sha256: "SHA256:public-fingerprint".to_owned(),
            openssh_public_key: "ssh-ed25519 AAAAC3NzaPublic generated".to_owned(),
        })
        .expect("public projection response");
        assert_eq!(projection["credentialId"], "cred-generated");
        assert_eq!(projection["algorithm"], "ssh-ed25519");
        assert_eq!(projection["fingerprintSha256"], "SHA256:public-fingerprint");
        assert!(projection["opensshPublicKey"].as_str().is_some());
        assert!(projection.get("privateKey").is_none());
        assert!(projection.get("passphrase").is_none());
        assert!(projection.get("pin").is_none());
        assert!(projection.get("path").is_none());

        let export_request: ExportPrivateKeyCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "credentialId": "cred-generated"
            }))
            .expect("ID-only export request");
        assert_eq!(export_request.credential_id, "cred-generated");
        for forbidden in [
            "privateKey",
            "passphrase",
            "pin",
            "path",
            "seed",
            "payload",
            "command",
        ] {
            let mut value = serde_json::json!({
                "credentialId": "cred-generated"
            });
            value[forbidden] = serde_json::json!("must-not-enter-ipc");
            assert!(
                serde_json::from_value::<ExportPrivateKeyCredentialRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }

        let exported = serde_json::to_value(PrivateKeyExportSummary {
            file_name: "anyssh-private-key".to_owned(),
            algorithm: "ssh-ed25519".to_owned(),
            fingerprint_sha256: "SHA256:public-fingerprint".to_owned(),
            encrypted: true,
        })
        .expect("export metadata");
        assert_eq!(exported["fileName"], "anyssh-private-key");
        assert_eq!(exported["encrypted"], true);
        assert!(exported.get("path").is_none());
        assert!(exported.get("privateKey").is_none());
        assert!(exported.get("passphrase").is_none());
        assert!(exported.get("pin").is_none());
    }

    #[test]
    fn appearance_theme_and_snippet_ipc_reject_executable_or_path_fields() {
        let appearance: AppearanceSettings = serde_json::from_value(serde_json::json!({
            "appTheme": "dark",
            "terminalThemeId": "builtin:obsidian",
            "fontSourceKind": "bundled",
            "fontId": "builtin:anyssh-nerd-mono",
            "fontFamily": "AnySSH Nerd Mono",
            "fontSize": 13,
            "lineHeightMillis": 1420,
            "ligaturesEnabled": false,
            "ambiguousWidth": "narrow"
        }))
        .expect("typed Appearance settings");
        assert_eq!(appearance.font_family, "AnySSH Nerd Mono");

        for forbidden in ["css", "script", "url", "path", "fontBytes"] {
            let mut value = serde_json::json!({
                "appTheme": "dark",
                "terminalThemeId": "builtin:obsidian",
                "fontSourceKind": "bundled",
                "fontId": "builtin:anyssh-nerd-mono",
                "fontFamily": "AnySSH Nerd Mono",
                "fontSize": 13,
                "lineHeightMillis": 1420,
                "ligaturesEnabled": false,
                "ambiguousWidth": "narrow"
            });
            value[forbidden] = serde_json::json!("forbidden");
            assert!(
                serde_json::from_value::<AppearanceSettings>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }

        let font = serde_json::to_value(FontAssetSummary {
            id: "font-test".to_owned(),
            family: "QA Mono".to_owned(),
            style: "Regular".to_owned(),
            format: FontAssetFormat::Ttf,
            sha256_hex: "a".repeat(64),
            size_bytes: 4096,
        })
        .expect("Font metadata");
        assert_eq!(font["family"], "QA Mono");
        assert!(font.get("sha256Hex").is_some());
        assert!(font.get("path").is_none());
        assert!(font.get("fontBytes").is_none());

        let run: RunSnippetRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "ssh-1",
            "snippetId": "snippet-1",
            "variables": {"host": "server.example"},
            "appendEnter": true,
            "confirmedMultiline": false
        }))
        .expect("ID-only Snippet run");
        assert_eq!(run.session_id, "ssh-1");
        assert_eq!(run.snippet_id, "snippet-1");

        for forbidden in [
            "body",
            "command",
            "shell",
            "workingDirectory",
            "environment",
            "credentialId",
            "path",
        ] {
            let mut value = serde_json::json!({
                "sessionId": "ssh-1",
                "snippetId": "snippet-1",
                "variables": {"host": "server.example"},
                "appendEnter": true,
                "confirmedMultiline": false
            });
            value[forbidden] = serde_json::json!("forbidden");
            assert!(
                serde_json::from_value::<RunSnippetRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }

        let create: CreateSnippetRequest = serde_json::from_value(serde_json::json!({
            "label": "Deploy",
            "body": "echo {{target}}"
        }))
        .expect("explicit Snippet editor body");
        assert_eq!(create.body, "echo {{target}}");
        for forbidden in ["path", "credentialId", "secret", "localShell"] {
            let mut value = serde_json::json!({
                "label": "Deploy",
                "body": "echo {{target}}"
            });
            value[forbidden] = serde_json::json!("forbidden");
            assert!(
                serde_json::from_value::<CreateSnippetRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }
    }

    #[test]
    fn private_key_picker_cancellation_returns_no_path() {
        assert_eq!(
            selected_private_key_path(None).expect("cancelled selection"),
            None
        );
        assert_eq!(
            selected_private_key_export_path(None).expect("cancelled export selection"),
            None
        );
        assert_eq!(
            selected_native_file_path(None, "Font").expect("cancelled Font selection"),
            None
        );
        assert_eq!(
            selected_private_key_path(Some(FilePath::Path(std::path::PathBuf::from(
                "/tmp/id_ed25519"
            ),)))
            .expect("desktop selection"),
            Some(std::path::PathBuf::from("/tmp/id_ed25519"))
        );
    }

    #[test]
    fn qa_vault_root_override_requires_an_absolute_path() {
        let default = std::env::temp_dir().join("anyssh-default-vault");
        let absolute = std::env::temp_dir().join("anyssh-qa-vault");

        assert_eq!(
            resolve_vault_root(default.clone(), None).expect("default Vault root"),
            default
        );
        assert_eq!(
            resolve_vault_root(default.clone(), Some(absolute.clone()))
                .expect("absolute QA Vault root"),
            absolute
        );
        assert!(resolve_vault_root(default, Some(PathBuf::from("relative-vault"))).is_err());
    }

    #[test]
    fn host_and_route_ipc_contain_references_only() {
        let host = serde_json::to_value(HostSummary {
            id: "host-target".to_owned(),
            display_name: "Target".to_owned(),
            host: "target.internal".to_owned(),
            port: 22,
            group_id: Some("group-production".to_owned()),
            credential_override: ReferenceOverride::Inherit,
            jump_route_override: ReferenceOverride::Set {
                value: "route-production".to_owned(),
            },
            effective_credential_id: Some("cred-target".to_owned()),
            effective_jump_route_id: Some("route-production".to_owned()),
        })
        .expect("host summary should serialize");
        let group = serde_json::to_value(GroupSummary {
            id: "group-production".to_owned(),
            label: "Production".to_owned(),
            parent_group_id: None,
            credential_override: ReferenceOverride::Set {
                value: "cred-target".to_owned(),
            },
            jump_route_override: ReferenceOverride::Clear,
            effective_credential_id: Some("cred-target".to_owned()),
            effective_jump_route_id: None,
        })
        .expect("Group summary should serialize");
        let route = serde_json::to_value(JumpRouteSummary {
            id: "route-production".to_owned(),
            label: "Production".to_owned(),
            host_ids: vec!["host-jump-one".to_owned(), "host-jump-two".to_owned()],
        })
        .expect("route summary should serialize");

        assert_eq!(host["groupId"], "group-production");
        assert_eq!(host["credentialOverride"]["kind"], "inherit");
        assert_eq!(host["jumpRouteOverride"]["kind"], "set");
        assert_eq!(host["jumpRouteOverride"]["value"], "route-production");
        assert_eq!(host["effectiveCredentialId"], "cred-target");
        assert_eq!(host["effectiveJumpRouteId"], "route-production");
        assert!(host.get("username").is_none());
        assert!(host.get("password").is_none());
        assert!(host.get("privateKey").is_none());
        assert_eq!(group["credentialOverride"]["kind"], "set");
        assert_eq!(group["credentialOverride"]["value"], "cred-target");
        assert_eq!(group["jumpRouteOverride"]["kind"], "clear");
        assert_eq!(group["effectiveCredentialId"], "cred-target");
        assert!(group.get("password").is_none());
        assert_eq!(
            route["hostIds"],
            serde_json::json!(["host-jump-one", "host-jump-two"])
        );
        assert!(route.get("credentialId").is_none());
        assert!(route.get("password").is_none());
    }

    #[test]
    fn known_host_ipc_is_metadata_only_and_forget_accepts_only_an_id() {
        let summary = serde_json::to_value(KnownHostSummary {
            id: "known-fixture".to_owned(),
            host: "target.internal".to_owned(),
            port: 22,
            keys: vec![KnownHostKeySummary {
                algorithm: "ssh-ed25519".to_owned(),
                fingerprint_sha256: "SHA256:trusted".to_owned(),
            }],
        })
        .expect("Known Host summary should serialize");
        assert_eq!(summary["host"], "target.internal");
        assert_eq!(summary["keys"][0]["fingerprintSha256"], "SHA256:trusted");
        assert!(summary.get("publicKey").is_none());

        let request: ForgetKnownHostRequest = serde_json::from_value(serde_json::json!({
            "knownHostId": "known-fixture"
        }))
        .expect("ID-only forget request");
        assert_eq!(request.known_host_id, "known-fixture");

        for extra in [
            serde_json::json!({
                "knownHostId": "known-fixture",
                "host": "target.internal"
            }),
            serde_json::json!({
                "knownHostId": "known-fixture",
                "fingerprintSha256": "SHA256:replacement"
            }),
            serde_json::json!({
                "knownHostId": "known-fixture",
                "publicKey": "must-not-enter-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<ForgetKnownHostRequest>(extra).is_err());
        }
    }

    #[test]
    fn host_request_rejects_embedded_credentials() {
        let valid: CreateHostRequest = serde_json::from_value(serde_json::json!({
            "displayName": "Target",
            "host": "target.internal",
            "port": 22,
            "groupId": "group-production",
            "credentialOverride": {
                "kind": "inherit"
            },
            "jumpRouteOverride": {
                "kind": "clear"
            }
        }))
        .expect("three-state Host request should deserialize");
        assert_eq!(valid.group_id.as_deref(), Some("group-production"));
        assert!(matches!(
            valid.credential_override,
            ReferenceOverride::Inherit
        ));
        assert!(matches!(
            valid.jump_route_override,
            ReferenceOverride::Clear
        ));

        let request = serde_json::from_value::<CreateHostRequest>(serde_json::json!({
            "displayName": "Target",
            "host": "target.internal",
            "port": 22,
            "groupId": "group-production",
            "credentialOverride": {
                "kind": "set",
                "value": "cred-target"
            },
            "jumpRouteOverride": {
                "kind": "inherit"
            },
            "password": "must-not-enter-host-ipc"
        }));

        assert!(request.is_err());

        let route = serde_json::from_value::<CreateJumpRouteRequest>(serde_json::json!({
            "label": "Production",
            "hostIds": ["host-jump"],
            "credentialId": "cred-must-not-be-copied"
        }));
        assert!(route.is_err());

        let group = serde_json::from_value::<CreateGroupRequest>(serde_json::json!({
            "label": "Production",
            "parentGroupId": null,
            "credentialOverride": {
                "kind": "set",
                "value": "cred-target",
                "password": "must-not-enter-group-ipc"
            },
            "jumpRouteOverride": {
                "kind": "inherit"
            }
        }));
        assert!(group.is_err());
    }

    #[test]
    fn saved_host_connect_request_accepts_only_host_id_and_terminal_size() {
        let request: ConnectSavedHostRequest = serde_json::from_value(serde_json::json!({
            "hostId": "host-target",
            "columns": 120,
            "rows": 32
        }))
        .expect("saved Host request should deserialize");
        assert_eq!(request.host_id, "host-target");

        for extra in [
            serde_json::json!({
                "hostId": "host-target",
                "columns": 120,
                "rows": 32,
                "credentialId": "cred-target"
            }),
            serde_json::json!({
                "hostId": "host-target",
                "columns": 120,
                "rows": 32,
                "password": "must-not-enter-saved-host-ipc"
            }),
            serde_json::json!({
                "hostId": "host-target",
                "columns": 120,
                "rows": 32,
                "host": "must-not-enter-saved-host-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<ConnectSavedHostRequest>(extra).is_err());
        }
    }

    #[test]
    fn port_forward_ipc_is_metadata_only_and_rejects_payload_fields() {
        let request: StartPortForwardRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "ssh-1",
            "kind": "local",
            "bindHost": "127.0.0.1",
            "bindPort": 0,
            "destinationHost": "target.internal",
            "destinationPort": 8080
        }))
        .expect("typed Local forward request");
        assert_eq!(request.session_id, "ssh-1");
        assert!(matches!(request.kind, PortForwardKind::Local));
        assert_eq!(request.destination_host.as_deref(), Some("target.internal"));

        let summary = serde_json::to_value(PortForwardSummary {
            id: "forward-1".to_owned(),
            kind: PortForwardKind::Dynamic,
            bind_host: "127.0.0.1".to_owned(),
            bound_port: 41080,
            destination_host: None,
            destination_port: None,
        })
        .expect("forward summary");
        assert_eq!(summary["kind"], "dynamic");
        assert_eq!(summary["boundPort"], 41080);
        assert!(summary.get("payload").is_none());
        assert!(summary.get("socket").is_none());
        assert!(summary.get("password").is_none());

        for forbidden in ["payload", "socket", "channel", "socksPassword", "command"] {
            let mut value = serde_json::json!({
                "sessionId": "ssh-1",
                "kind": "local",
                "bindHost": "127.0.0.1",
                "bindPort": 0,
                "destinationHost": "target.internal",
                "destinationPort": 8080
            });
            value[forbidden] = serde_json::json!("forbidden");
            assert!(
                serde_json::from_value::<StartPortForwardRequest>(value).is_err(),
                "{forbidden} must be rejected"
            );
        }
    }

    #[tokio::test]
    async fn session_registry_routes_acknowledgements_and_removal_by_id() {
        let registry = SessionRegistry::default();
        let first_id = registry.new_id();
        let second_id = registry.new_id();
        assert_ne!(first_id, second_id);

        let first = spawn_registry_test_session();
        let first_control = first.control.clone();
        let _first_events = first.events;
        let second = spawn_registry_test_session();
        let _second_events = second.events;
        let (first_acknowledgements, mut first_acks) = tokio::sync::mpsc::channel(2);
        let (second_acknowledgements, mut second_acks) = tokio::sync::mpsc::channel(2);

        registry
            .insert(first_id.clone(), first.control, first_acknowledgements)
            .await;
        registry
            .insert(second_id.clone(), second.control, second_acknowledgements)
            .await;

        registry
            .acknowledge_output(&first_id)
            .await
            .expect("first Session acknowledgement");
        assert_eq!(first_acks.recv().await, Some(()));
        assert!(second_acks.try_recv().is_err());

        registry
            .acknowledge_output(&second_id)
            .await
            .expect("second Session acknowledgement");
        assert_eq!(second_acks.recv().await, Some(()));
        assert!(first_acks.try_recv().is_err());

        registry.remove_and_disconnect(&first_id).await;
        assert!(registry.get(&first_id).await.is_err());
        assert!(first_control.send_input("ignored").await.is_err());
        assert!(registry.get(&second_id).await.is_ok());
        registry
            .acknowledge_output(&second_id)
            .await
            .expect("remaining Session acknowledgement");
        assert_eq!(second_acks.recv().await, Some(()));

        registry.disconnect_all().await;
    }

    #[tokio::test]
    async fn session_registry_disconnect_all_drains_every_session() {
        let registry = SessionRegistry::default();
        let first = spawn_registry_test_session();
        let first_control = first.control.clone();
        let _first_events = first.events;
        let second = spawn_registry_test_session();
        let second_control = second.control.clone();
        let _second_events = second.events;
        let (first_acknowledgements, _first_acks) = tokio::sync::mpsc::channel(1);
        let (second_acknowledgements, _second_acks) = tokio::sync::mpsc::channel(1);

        registry
            .insert(
                "ssh-first".to_owned(),
                first.control,
                first_acknowledgements,
            )
            .await;
        registry
            .insert(
                "ssh-second".to_owned(),
                second.control,
                second_acknowledgements,
            )
            .await;

        registry.disconnect_all().await;

        assert!(registry.get("ssh-first").await.is_err());
        assert!(registry.get("ssh-second").await.is_err());
        assert!(first_control.send_input("ignored").await.is_err());
        assert!(second_control.send_input("ignored").await.is_err());
        registry.disconnect_all().await;
    }

    #[tokio::test]
    async fn output_window_waits_for_webview_acknowledgement() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let mut in_flight = MAX_IN_FLIGHT_OUTPUT_CHUNKS;
        sender
            .send(())
            .await
            .expect("acknowledgement should enter the test channel");

        assert!(await_output_capacity(&mut receiver, &mut in_flight).await);
        assert_eq!(in_flight, MAX_IN_FLIGHT_OUTPUT_CHUNKS - 1);
    }

    #[tokio::test]
    async fn output_window_closes_when_the_ack_channel_is_dropped() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        drop(sender);
        let mut in_flight = MAX_IN_FLIGHT_OUTPUT_CHUNKS;

        assert!(!await_output_capacity(&mut receiver, &mut in_flight).await);
    }
}
