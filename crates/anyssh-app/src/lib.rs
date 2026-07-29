#![deny(unsafe_code)]

mod font_assets;
mod theme_import;

use std::{
    collections::BTreeMap,
    fmt,
    fs::{File, OpenOptions},
    future::Future,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use anyssh_domain::{SshEndpoint, SshEndpointIdentity, TerminalSize};
use anyssh_ssh::{
    DEFAULT_CONNECTION_TIMEOUT, HostKeyPolicy, MAX_PRIVATE_KEY_BYTES, PrivateKeyTextEncryption,
    SessionAuthentication, SessionControl, SessionControlError, SpawnedSession,
    SshConnectionConfig, SshSessionConfig, SystemAgentError, SystemAgentIdentitySummary,
    inspect_openssh_private_key_text, list_system_agent_identities, spawn_session,
    validate_private_key_text,
};
use anyssh_storage::{
    CredentialSecret, DatabaseActorError, DatabaseActorHandle, ResolvedCredential,
    ResolvedHostConnection, ResolvedKnownHostPolicy,
};
use ssh_key::{Algorithm as SshKeyAlgorithm, HashAlg, LineEnding, PrivateKey};
use thiserror::Error;
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

pub use font_assets::{
    FONT_ASSET_DIRECTORY_NAME, FontAssetError, MAX_SYSTEM_FONT_SUMMARIES, SystemFontSummary,
    read_managed_font_asset,
};
use font_assets::{
    cleanup_orphaned_font_assets, cleanup_stale_font_staging, commit_staged_font_asset,
    current_unix_millis, ensure_font_asset_store, enumerate_system_fonts, read_font_import,
    remove_managed_font_asset, remove_staged_font_asset, stage_font_asset,
    verify_managed_font_asset,
};
use theme_import::read_terminal_theme_import;
pub use theme_import::{MAX_TERMINAL_THEME_FILE_BYTES, TerminalThemeImportError};

const _: () = assert!(anyssh_storage::MAX_JUMP_ROUTE_STEPS == anyssh_ssh::MAX_JUMP_HOSTS);
const _: () = assert!(anyssh_storage::MAX_KNOWN_HOST_KEYS == anyssh_ssh::MAX_TRUSTED_HOST_KEYS);
const _: () = assert!(
    anyssh_storage::MAX_KNOWN_HOST_PUBLIC_KEY_BYTES == anyssh_ssh::MAX_HOST_PUBLIC_KEY_BYTES
);
pub const PRIVATE_KEY_PASSPHRASE_MAX_ATTEMPTS: u8 = 3;
pub const PRIVATE_KEY_EXPORT_MAX_ATTEMPTS: u8 = 3;
const PRIVATE_KEY_OPERATION_CONCURRENCY: usize = 1;
const FONT_ASSET_OPERATION_CONCURRENCY: usize = 1;
const MAX_PRIVATE_KEY_PROMPT_LABEL_CHARS: usize = 128;
const DEFAULT_PRIVATE_KEY_PROMPT_LABEL: &str = "Imported private key";
const MAX_GENERATED_PRIVATE_KEY_COMMENT_CHARS: usize = 128;
const MAX_OPENSSH_PUBLIC_KEY_BYTES: usize = 16 * 1024;
const MAX_PRIVATE_KEY_EXPORT_PASSPHRASE_BYTES: usize = 1024;
const MAX_PRIVATE_KEY_EXPORT_FILE_NAME_CHARS: usize = 128;
const DEFAULT_PRIVATE_KEY_EXPORT_FILE_NAME: &str = "anyssh-private-key";
const PRIVATE_KEY_EXPORT_OPERATION_LABEL: &str = "Export SSH private key";
const MAX_PREPARED_SNIPPET_INPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateKeyGenerationAlgorithm {
    Ed25519,
    Rsa4096,
}

impl PrivateKeyGenerationAlgorithm {
    const fn ssh_algorithm(self) -> SshKeyAlgorithm {
        match self {
            Self::Ed25519 => SshKeyAlgorithm::Ed25519,
            Self::Rsa4096 => SshKeyAlgorithm::Rsa { hash: None },
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PrivateKeyPublicSummary {
    credential_id: String,
    algorithm: String,
    fingerprint_sha256: String,
    openssh_public_key: String,
}

impl PrivateKeyPublicSummary {
    pub fn credential_id(&self) -> &str {
        &self.credential_id
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }

    pub fn openssh_public_key(&self) -> &str {
        &self.openssh_public_key
    }
}

impl fmt::Debug for PrivateKeyPublicSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateKeyPublicSummary")
            .field("credential_id", &self.credential_id)
            .field("algorithm", &self.algorithm)
            .field("fingerprint_sha256", &self.fingerprint_sha256)
            .field("openssh_public_key", &"<public>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultStepUpContext {
    operation_label: String,
    attempt: u8,
    max_attempts: u8,
    previous_pin_incorrect: bool,
}

impl VaultStepUpContext {
    pub fn operation_label(&self) -> &str {
        &self.operation_label
    }

    pub const fn attempt(&self) -> u8 {
        self.attempt
    }

    pub const fn max_attempts(&self) -> u8 {
        self.max_attempts
    }

    pub const fn previous_pin_incorrect(&self) -> bool {
        self.previous_pin_incorrect
    }
}

pub trait VaultStepUpPrompt: Send + Sync {
    fn request(
        &self,
        context: VaultStepUpContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, VaultStepUpPromptError>> + Send;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateKeyExportPassphraseContext {
    attempt: u8,
    max_attempts: u8,
    previous_confirmation_mismatch: bool,
}

impl PrivateKeyExportPassphraseContext {
    pub const fn attempt(&self) -> u8 {
        self.attempt
    }

    pub const fn max_attempts(&self) -> u8 {
        self.max_attempts
    }

    pub const fn previous_confirmation_mismatch(&self) -> bool {
        self.previous_confirmation_mismatch
    }
}

pub struct PrivateKeyExportPassphraseCandidate {
    passphrase: Zeroizing<String>,
    confirmation: Zeroizing<String>,
}

impl PrivateKeyExportPassphraseCandidate {
    pub const fn new(passphrase: Zeroizing<String>, confirmation: Zeroizing<String>) -> Self {
        Self {
            passphrase,
            confirmation,
        }
    }

    fn into_parts(self) -> (Zeroizing<String>, Zeroizing<String>) {
        (self.passphrase, self.confirmation)
    }
}

impl fmt::Debug for PrivateKeyExportPassphraseCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateKeyExportPassphraseCandidate")
            .field("passphrase", &"<redacted>")
            .field("confirmation", &"<redacted>")
            .finish()
    }
}

pub trait PrivateKeyExportPassphrasePrompt: Send + Sync {
    fn request(
        &self,
        context: PrivateKeyExportPassphraseContext,
    ) -> impl Future<
        Output = Result<
            Option<PrivateKeyExportPassphraseCandidate>,
            PrivateKeyExportPassphrasePromptError,
        >,
    > + Send;
}

#[derive(Clone, Eq, PartialEq)]
pub struct PrivateKeyExportSummary {
    file_name: String,
    algorithm: String,
    fingerprint_sha256: String,
}

impl PrivateKeyExportSummary {
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }

    pub const fn encrypted(&self) -> bool {
        true
    }
}

impl fmt::Debug for PrivateKeyExportSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateKeyExportSummary")
            .field("file_name", &self.file_name)
            .field("algorithm", &self.algorithm)
            .field("fingerprint_sha256", &self.fingerprint_sha256)
            .field("encrypted", &true)
            .finish()
    }
}

pub struct PreparedSnippetInput {
    input: Zeroizing<String>,
    multiline: bool,
}

impl PreparedSnippetInput {
    pub fn input(&self) -> &str {
        self.input.as_str()
    }

    pub const fn multiline(&self) -> bool {
        self.multiline
    }

    pub fn into_input(self) -> Zeroizing<String> {
        self.input
    }
}

impl fmt::Debug for PreparedSnippetInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedSnippetInput")
            .field("input", &"<redacted>")
            .field("multiline", &self.multiline)
            .finish()
    }
}

pub use anyssh_storage::{
    AmbiguousWidth, AppTheme, AppearanceSettings, CredentialKind, CredentialSummary,
    DatabaseActorConfig, DatabaseActorStartError, FontAssetFormat, FontAssetSummary,
    FontSourceKind, GroupSummary, HostSummary, JumpRouteSummary, KnownHostKeySummary,
    KnownHostSummary, MAX_GROUP_DEPTH, Override, SnippetDraft, SnippetSummary, TerminalPalette,
    TerminalThemeSummary, VaultState, VaultStatus, is_valid_font_asset_id,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateKeyPromptContext {
    label: String,
    attempt: u8,
    max_attempts: u8,
    previous_passphrase_incorrect: bool,
}

impl PrivateKeyPromptContext {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn attempt(&self) -> u8 {
        self.attempt
    }

    pub const fn max_attempts(&self) -> u8 {
        self.max_attempts
    }

    pub const fn previous_passphrase_incorrect(&self) -> bool {
        self.previous_passphrase_incorrect
    }
}

pub trait PrivateKeyPassphrasePrompt: Send + Sync {
    fn request(
        &self,
        context: PrivateKeyPromptContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, PrivateKeyPromptError>> + Send;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownHostForgetPromptContext {
    host: String,
    port: u16,
    fingerprints_sha256: Vec<String>,
}

impl KnownHostForgetPromptContext {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn fingerprints_sha256(&self) -> &[String] {
        &self.fingerprints_sha256
    }
}

pub trait KnownHostForgetPrompt: Send + Sync {
    fn confirm(
        &self,
        context: KnownHostForgetPromptContext,
    ) -> impl Future<Output = Result<bool, KnownHostForgetPromptError>> + Send;
}

#[derive(Clone)]
pub struct ApplicationCore {
    database: DatabaseActorHandle,
    private_key_operations: Arc<Semaphore>,
    font_asset_root: Option<Arc<PathBuf>>,
    font_asset_operations: Arc<Semaphore>,
    system_fonts: Arc<OnceLock<Vec<SystemFontSummary>>>,
}

impl fmt::Debug for ApplicationCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCore")
            .field("database", &self.database)
            .field("private_key_operations", &"<bounded>")
            .field("font_asset_root", &self.font_asset_root.is_some())
            .field("font_asset_operations", &"<bounded>")
            .field("system_fonts", &"<bounded metadata>")
            .finish_non_exhaustive()
    }
}

impl ApplicationCore {
    pub fn spawn(
        vault_root: PathBuf,
        config: DatabaseActorConfig,
    ) -> Result<Self, DatabaseActorStartError> {
        let font_asset_root = vault_root.join(FONT_ASSET_DIRECTORY_NAME);
        DatabaseActorHandle::spawn(vault_root, config)
            .map(|database| Self::new_with_font_asset_root(database, font_asset_root))
    }

    pub fn new(database: DatabaseActorHandle) -> Self {
        Self::new_inner(database, None)
    }

    fn new_with_font_asset_root(database: DatabaseActorHandle, font_asset_root: PathBuf) -> Self {
        Self::new_inner(database, Some(Arc::new(font_asset_root)))
    }

    fn new_inner(database: DatabaseActorHandle, font_asset_root: Option<Arc<PathBuf>>) -> Self {
        Self {
            database,
            private_key_operations: Arc::new(Semaphore::new(PRIVATE_KEY_OPERATION_CONCURRENCY)),
            font_asset_root,
            font_asset_operations: Arc::new(Semaphore::new(FONT_ASSET_OPERATION_CONCURRENCY)),
            system_fonts: Arc::new(OnceLock::new()),
        }
    }

    pub async fn vault_status(&self) -> Result<VaultStatus, ApplicationError> {
        self.database.status().await.map_err(ApplicationError::from)
    }

    pub async fn create_vault(
        &self,
        pin: Zeroizing<String>,
    ) -> Result<VaultStatus, ApplicationError> {
        self.database
            .create(pin)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn unlock_vault(
        &self,
        pin: Zeroizing<String>,
    ) -> Result<VaultStatus, ApplicationError> {
        self.database
            .unlock(pin)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn lock_vault(&self) -> Result<VaultStatus, ApplicationError> {
        self.database.lock().await.map_err(ApplicationError::from)
    }

    pub async fn get_appearance_settings(&self) -> Result<AppearanceSettings, ApplicationError> {
        self.database
            .get_appearance_settings()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_appearance_settings(
        &self,
        settings: AppearanceSettings,
    ) -> Result<AppearanceSettings, ApplicationError> {
        self.database
            .update_appearance_settings(settings)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_terminal_theme(
        &self,
        label: String,
        palette: TerminalPalette,
    ) -> Result<TerminalThemeSummary, ApplicationError> {
        self.database
            .create_terminal_theme(label, palette)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn import_terminal_theme_from_path(
        &self,
        path: PathBuf,
    ) -> Result<TerminalThemeSummary, ApplicationError> {
        let permit = self
            .font_asset_operations
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| TerminalThemeImportError::Unavailable)?;
        let (theme, permit) = tokio::task::spawn_blocking(move || {
            read_terminal_theme_import(&path).map(|theme| (theme, permit))
        })
        .await
        .map_err(|_| TerminalThemeImportError::TaskFailed)??;
        let result = self
            .database
            .create_terminal_theme(theme.label, theme.palette)
            .await
            .map_err(ApplicationError::from);
        drop(permit);
        result
    }

    pub async fn list_terminal_themes(
        &self,
    ) -> Result<Vec<TerminalThemeSummary>, ApplicationError> {
        self.database
            .list_terminal_themes()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_terminal_theme(&self, id: String) -> Result<bool, ApplicationError> {
        self.database
            .delete_terminal_theme(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn import_font_asset_from_path(
        &self,
        path: PathBuf,
    ) -> Result<FontAssetSummary, ApplicationError> {
        let root = self.font_asset_root()?;
        let permit = self
            .font_asset_operations
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| FontAssetError::Unavailable)?;
        let (prepared, permit) = tokio::task::spawn_blocking(move || {
            read_font_import(&path).map(|prepared| (prepared, permit))
        })
        .await
        .map_err(|_| FontAssetError::TaskFailed)??;
        let summary = FontAssetSummary::generate(
            prepared.family,
            prepared.style,
            prepared.format,
            prepared.sha256_hex,
            prepared.bytes.len() as u64,
            current_unix_millis()?,
        )
        .map_err(|_| FontAssetError::InvalidFont)?;

        let stage_root = root.clone();
        let stage_summary = summary.clone();
        let (staged, permit) = tokio::task::spawn_blocking(move || {
            stage_font_asset(&stage_root, &stage_summary, &prepared.bytes)
                .map(|staged| (staged, permit))
        })
        .await
        .map_err(|_| FontAssetError::TaskFailed)??;

        let registered = match self.database.register_font_asset(summary).await {
            Ok(summary) => summary,
            Err(error) => {
                let _ = tokio::task::spawn_blocking(move || {
                    remove_staged_font_asset(&staged);
                    drop(permit);
                })
                .await;
                return Err(ApplicationError::Database(error));
            }
        };

        let (commit_result, staged, permit) = tokio::task::spawn_blocking(move || {
            let result = commit_staged_font_asset(&staged);
            (result, staged, permit)
        })
        .await
        .map_err(|_| FontAssetError::TaskFailed)?;
        if let Err(error) = commit_result {
            let _ = self
                .database
                .delete_font_asset(registered.id().to_owned())
                .await;
            let _ = tokio::task::spawn_blocking(move || {
                remove_staged_font_asset(&staged);
                drop(permit);
            })
            .await;
            return Err(error.into());
        }
        drop(permit);
        Ok(registered)
    }

    pub async fn list_font_assets(&self) -> Result<Vec<FontAssetSummary>, ApplicationError> {
        let root = self.font_asset_root()?;
        let permit = self
            .font_asset_operations
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| FontAssetError::Unavailable)?;
        let stored = self
            .database
            .list_font_assets()
            .await
            .map_err(ApplicationError::from)?;
        let verify_root = root.clone();
        let (valid, invalid, permit) = tokio::task::spawn_blocking(move || {
            ensure_font_asset_store(&verify_root)?;
            cleanup_stale_font_staging(&verify_root)?;
            cleanup_orphaned_font_assets(&verify_root, &stored)?;
            let mut valid = Vec::new();
            let mut invalid = Vec::new();
            for font in stored {
                match verify_managed_font_asset(&verify_root, &font) {
                    Ok(true) => valid.push(font),
                    Ok(false) | Err(_) => invalid.push(font),
                }
            }
            Ok::<_, FontAssetError>((valid, invalid, permit))
        })
        .await
        .map_err(|_| FontAssetError::TaskFailed)??;

        for font in &invalid {
            self.database
                .delete_font_asset(font.id().to_owned())
                .await
                .map_err(ApplicationError::from)?;
        }
        if !invalid.is_empty() {
            let cleanup_root = root;
            let _ = tokio::task::spawn_blocking(move || {
                for font in invalid {
                    let _ = remove_managed_font_asset(&cleanup_root, &font);
                }
                drop(permit);
            })
            .await;
        } else {
            drop(permit);
        }
        Ok(valid)
    }

    pub async fn delete_font_asset(&self, id: String) -> Result<bool, ApplicationError> {
        let root = self.font_asset_root()?;
        let permit = self
            .font_asset_operations
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| FontAssetError::Unavailable)?;
        let stored = self
            .database
            .list_font_assets()
            .await
            .map_err(ApplicationError::from)?;
        let summary = stored.into_iter().find(|font| font.id() == id);
        let deleted = self
            .database
            .delete_font_asset(id)
            .await
            .map_err(ApplicationError::from)?;
        if let Some(summary) = summary {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = remove_managed_font_asset(&root, &summary);
                drop(permit);
            })
            .await;
        } else {
            drop(permit);
        }
        Ok(deleted)
    }

    pub async fn list_system_fonts(&self) -> Result<Vec<SystemFontSummary>, ApplicationError> {
        if let Some(fonts) = self.system_fonts.get() {
            return Ok(fonts.clone());
        }
        let fonts = tokio::task::spawn_blocking(enumerate_system_fonts)
            .await
            .map_err(|_| FontAssetError::TaskFailed)?;
        let _ = self.system_fonts.set(fonts.clone());
        Ok(self.system_fonts.get().cloned().unwrap_or(fonts))
    }

    fn font_asset_root(&self) -> Result<Arc<PathBuf>, FontAssetError> {
        self.font_asset_root
            .clone()
            .ok_or(FontAssetError::Unavailable)
    }

    pub async fn create_snippet(
        &self,
        label: String,
        body: Zeroizing<String>,
    ) -> Result<SnippetSummary, ApplicationError> {
        self.database
            .create_snippet(label, body)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_snippet(
        &self,
        id: String,
        label: String,
        body: Zeroizing<String>,
    ) -> Result<SnippetSummary, ApplicationError> {
        self.database
            .update_snippet(id, label, body)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_snippets(&self) -> Result<Vec<SnippetSummary>, ApplicationError> {
        self.database
            .list_snippets()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn get_snippet(&self, id: String) -> Result<SnippetDraft, ApplicationError> {
        self.database
            .get_snippet(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_snippet(&self, id: String) -> Result<bool, ApplicationError> {
        self.database
            .delete_snippet(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn prepare_snippet_input(
        &self,
        id: String,
        variables: BTreeMap<String, String>,
        append_enter: bool,
        confirmed_multiline: bool,
    ) -> Result<PreparedSnippetInput, ApplicationError> {
        let draft = self
            .database
            .get_snippet(id)
            .await
            .map_err(ApplicationError::from)?;
        prepare_snippet_input(draft, variables, append_enter, confirmed_multiline)
            .map_err(ApplicationError::from)
    }

    pub async fn create_password_credential(
        &self,
        label: String,
        username: String,
        password: Zeroizing<String>,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .create_credential(label, username, CredentialSecret::Password { password })
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_password_credential(
        &self,
        id: String,
        label: String,
        username: String,
        password: Zeroizing<String>,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .update_credential(id, label, username, CredentialSecret::Password { password })
            .await
            .map_err(ApplicationError::from)
    }

    /// Stores a Private Key received from a trusted Rust/native import path.
    ///
    /// Tauri commands must not accept raw Private Key text or arbitrary paths
    /// from the WebView. Future platform import code should read the selected
    /// file in Rust and call this method directly.
    pub async fn store_private_key_credential(
        &self,
        label: String,
        username: String,
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .create_credential(
                label,
                username,
                CredentialSecret::PrivateKey {
                    private_key,
                    passphrase,
                },
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn generate_private_key_credential(
        &self,
        label: String,
        username: String,
        algorithm: PrivateKeyGenerationAlgorithm,
    ) -> Result<CredentialSummary, ApplicationError> {
        let operation_permit = self
            .private_key_operations
            .clone()
            .try_acquire_owned()
            .map_err(|_| PrivateKeyManagementError::OperationBusy)?;
        if self
            .database
            .status()
            .await
            .map_err(ApplicationError::from)?
            .state()
            != VaultState::Unlocked
        {
            return Err(DatabaseActorError::VaultLocked.into());
        }
        let comment = sanitize_generated_private_key_comment(&label);
        let (private_key, _operation_permit) = tokio::task::spawn_blocking(move || {
            let mut private_key = PrivateKey::random(&mut rand::rng(), algorithm.ssh_algorithm())
                .map_err(|_| PrivateKeyManagementError::GenerationFailed)?;
            private_key.set_comment(comment);
            let private_key = private_key
                .to_openssh(LineEnding::LF)
                .map_err(|_| PrivateKeyManagementError::GenerationFailed)?;
            Ok::<_, PrivateKeyManagementError>((private_key, operation_permit))
        })
        .await
        .map_err(|_| PrivateKeyManagementError::TaskFailed)??;

        self.store_private_key_credential(label, username, private_key, None)
            .await
    }

    pub async fn private_key_public_summary(
        &self,
        credential_id: String,
    ) -> Result<PrivateKeyPublicSummary, ApplicationError> {
        let operation_permit = self
            .private_key_operations
            .clone()
            .try_acquire_owned()
            .map_err(|_| PrivateKeyManagementError::OperationBusy)?;
        let resolved = self
            .database
            .resolve_credential(credential_id.clone())
            .await
            .map_err(ApplicationError::from)?;
        let (_, secret) = resolved.into_parts();
        let CredentialSecret::PrivateKey {
            private_key,
            passphrase,
        } = secret
        else {
            return Err(PrivateKeyManagementError::CredentialKindMismatch.into());
        };

        tokio::task::spawn_blocking(move || {
            let _operation_permit = operation_permit;
            private_key_public_summary_from_text(credential_id, private_key, passphrase)
        })
        .await
        .map_err(|_| PrivateKeyManagementError::TaskFailed)?
        .map_err(ApplicationError::from)
    }

    pub async fn export_private_key_credential_to_path_with_prompts<S, P>(
        &self,
        credential_id: String,
        path: PathBuf,
        step_up_prompt: &S,
        passphrase_prompt: &P,
    ) -> Result<Option<PrivateKeyExportSummary>, ApplicationError>
    where
        S: VaultStepUpPrompt,
        P: PrivateKeyExportPassphrasePrompt,
    {
        let operation_permit = self
            .private_key_operations
            .clone()
            .try_acquire_owned()
            .map_err(|_| PrivateKeyManagementError::OperationBusy)?;
        let mut step_up_approved = false;
        for attempt in 1..=PRIVATE_KEY_EXPORT_MAX_ATTEMPTS {
            let context = VaultStepUpContext {
                operation_label: PRIVATE_KEY_EXPORT_OPERATION_LABEL.to_owned(),
                attempt,
                max_attempts: PRIVATE_KEY_EXPORT_MAX_ATTEMPTS,
                previous_pin_incorrect: attempt > 1,
            };
            let Some(pin) = step_up_prompt.request(context).await? else {
                return Ok(None);
            };
            if self
                .database
                .verify_pin(pin)
                .await
                .map_err(ApplicationError::from)?
            {
                step_up_approved = true;
                break;
            }
        }
        if !step_up_approved {
            return Err(PrivateKeyManagementError::StepUpRejected.into());
        }

        let mut export_passphrase = None;
        for attempt in 1..=PRIVATE_KEY_EXPORT_MAX_ATTEMPTS {
            let context = PrivateKeyExportPassphraseContext {
                attempt,
                max_attempts: PRIVATE_KEY_EXPORT_MAX_ATTEMPTS,
                previous_confirmation_mismatch: attempt > 1,
            };
            let Some(candidate) = passphrase_prompt.request(context).await? else {
                return Ok(None);
            };
            let (passphrase, confirmation) = candidate.into_parts();
            if valid_private_key_export_passphrase(passphrase.as_str())
                && passphrase.as_str() == confirmation.as_str()
            {
                export_passphrase = Some(passphrase);
                break;
            }
        }
        let Some(export_passphrase) = export_passphrase else {
            return Err(PrivateKeyManagementError::PassphraseRejected.into());
        };

        let resolved = self
            .database
            .resolve_credential(credential_id)
            .await
            .map_err(ApplicationError::from)?;
        let (_, secret) = resolved.into_parts();
        let CredentialSecret::PrivateKey {
            private_key,
            passphrase,
        } = secret
        else {
            return Err(PrivateKeyManagementError::CredentialKindMismatch.into());
        };

        tokio::task::spawn_blocking(move || {
            let _operation_permit = operation_permit;
            export_private_key_to_new_file(path, private_key, passphrase, export_passphrase)
        })
        .await
        .map_err(|_| PrivateKeyManagementError::TaskFailed)?
        .map(Some)
        .map_err(ApplicationError::from)
    }

    pub async fn import_private_key_credential_from_path(
        &self,
        label: String,
        username: String,
        path: PathBuf,
    ) -> Result<CredentialSummary, ApplicationError> {
        let candidate = tokio::task::spawn_blocking(move || read_private_key_file(&path))
            .await
            .map_err(|_| PrivateKeyImportError::TaskFailed)??;
        match candidate.encryption {
            PrivateKeyTextEncryption::Unencrypted => {
                self.store_private_key_credential(label, username, candidate.private_key, None)
                    .await
            }
            PrivateKeyTextEncryption::Encrypted => {
                Err(PrivateKeyImportError::PassphraseRequired.into())
            }
        }
    }

    pub async fn import_private_key_credential_from_path_with_prompt<P>(
        &self,
        label: String,
        username: String,
        path: PathBuf,
        prompt: &P,
    ) -> Result<Option<CredentialSummary>, ApplicationError>
    where
        P: PrivateKeyPassphrasePrompt,
    {
        let candidate = tokio::task::spawn_blocking(move || read_private_key_file(&path))
            .await
            .map_err(|_| PrivateKeyImportError::TaskFailed)??;
        if candidate.encryption == PrivateKeyTextEncryption::Unencrypted {
            return self
                .store_private_key_credential(label, username, candidate.private_key, None)
                .await
                .map(Some);
        }

        let prompt_label = sanitize_private_key_prompt_label(&label);
        let mut private_key = candidate.private_key;
        for attempt in 1..=PRIVATE_KEY_PASSPHRASE_MAX_ATTEMPTS {
            let context = PrivateKeyPromptContext {
                label: prompt_label.clone(),
                attempt,
                max_attempts: PRIVATE_KEY_PASSPHRASE_MAX_ATTEMPTS,
                previous_passphrase_incorrect: attempt > 1,
            };
            let Some(passphrase) = prompt.request(context).await? else {
                return Ok(None);
            };

            let validation = tokio::task::spawn_blocking(move || {
                let accepted =
                    validate_private_key_text(private_key.as_str(), Some(passphrase.as_str()))
                        .is_ok();
                (private_key, passphrase, accepted)
            })
            .await
            .map_err(|_| PrivateKeyImportError::TaskFailed)?;
            private_key = validation.0;
            if validation.2 {
                return self
                    .store_private_key_credential(label, username, private_key, Some(validation.1))
                    .await
                    .map(Some);
            }
        }

        Err(PrivateKeyImportError::PassphraseRejected.into())
    }

    pub async fn update_private_key_credential(
        &self,
        id: String,
        label: String,
        username: String,
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .update_credential(
                id,
                label,
                username,
                CredentialSecret::PrivateKey {
                    private_key,
                    passphrase,
                },
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_system_agent_identities(
        &self,
    ) -> Result<Vec<SystemAgentIdentitySummary>, ApplicationError> {
        list_system_agent_identities()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_system_agent_credential(
        &self,
        label: String,
        username: String,
        identity_fingerprint_sha256: String,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .create_credential(
                label,
                username,
                CredentialSecret::SystemAgent {
                    identity_fingerprint_sha256: Zeroizing::new(identity_fingerprint_sha256),
                },
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_keyboard_interactive_credential(
        &self,
        label: String,
        username: String,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .create_credential(label, username, CredentialSecret::KeyboardInteractive)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_keyboard_interactive_credential(
        &self,
        id: String,
        label: String,
        username: String,
    ) -> Result<CredentialSummary, ApplicationError> {
        self.database
            .update_credential(id, label, username, CredentialSecret::KeyboardInteractive)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_credentials(&self) -> Result<Vec<CredentialSummary>, ApplicationError> {
        self.database
            .list_credentials()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_credential(&self, id: String) -> Result<bool, ApplicationError> {
        self.database
            .delete_credential(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_group(
        &self,
        label: String,
        parent_group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> Result<GroupSummary, ApplicationError> {
        self.database
            .create_group(
                label,
                parent_group_id,
                credential_override,
                jump_route_override,
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_group(
        &self,
        id: String,
        label: String,
        parent_group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> Result<GroupSummary, ApplicationError> {
        self.database
            .update_group(
                id,
                label,
                parent_group_id,
                credential_override,
                jump_route_override,
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_groups(&self) -> Result<Vec<GroupSummary>, ApplicationError> {
        self.database
            .list_groups()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_group(&self, id: String) -> Result<bool, ApplicationError> {
        self.database
            .delete_group(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_host(
        &self,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<HostSummary, ApplicationError> {
        self.database
            .create_host(display_name, host, port, credential_id, jump_route_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_host(
        &self,
        id: String,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<HostSummary, ApplicationError> {
        self.database
            .update_host(id, display_name, host, port, credential_id, jump_route_id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_host_with_overrides(
        &self,
        display_name: String,
        host: String,
        port: u16,
        group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> Result<HostSummary, ApplicationError> {
        self.database
            .create_host_with_overrides(
                display_name,
                host,
                port,
                group_id,
                credential_override,
                jump_route_override,
            )
            .await
            .map_err(ApplicationError::from)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_host_with_overrides(
        &self,
        id: String,
        display_name: String,
        host: String,
        port: u16,
        group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> Result<HostSummary, ApplicationError> {
        self.database
            .update_host_with_overrides(
                id,
                display_name,
                host,
                port,
                group_id,
                credential_override,
                jump_route_override,
            )
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_hosts(&self) -> Result<Vec<HostSummary>, ApplicationError> {
        self.database
            .list_hosts()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_host(&self, id: String) -> Result<bool, ApplicationError> {
        self.database
            .delete_host(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_known_hosts(&self) -> Result<Vec<KnownHostSummary>, ApplicationError> {
        self.database
            .list_known_hosts()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn forget_known_host_with_prompt(
        &self,
        id: String,
        prompt: &impl KnownHostForgetPrompt,
    ) -> Result<bool, ApplicationError> {
        let summary = self
            .database
            .get_known_host(id.clone())
            .await
            .map_err(ApplicationError::from)?;
        let context = KnownHostForgetPromptContext {
            host: summary.host().to_owned(),
            port: summary.port(),
            fingerprints_sha256: summary
                .keys()
                .iter()
                .map(|key| key.fingerprint_sha256().to_owned())
                .collect(),
        };
        if !prompt.confirm(context).await? {
            return Ok(false);
        }
        self.database
            .delete_known_host(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn create_jump_route(
        &self,
        label: String,
        host_ids: Vec<String>,
    ) -> Result<JumpRouteSummary, ApplicationError> {
        self.database
            .create_jump_route(label, host_ids)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn update_jump_route(
        &self,
        id: String,
        label: String,
        host_ids: Vec<String>,
    ) -> Result<JumpRouteSummary, ApplicationError> {
        self.database
            .update_jump_route(id, label, host_ids)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn list_jump_routes(&self) -> Result<Vec<JumpRouteSummary>, ApplicationError> {
        self.database
            .list_jump_routes()
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn delete_jump_route(&self, id: String) -> Result<bool, ApplicationError> {
        self.database
            .delete_jump_route(id)
            .await
            .map_err(ApplicationError::from)
    }

    pub async fn spawn_ssh_session(
        &self,
        request: SshSessionRequest,
    ) -> Result<SpawnedSession, ApplicationError> {
        let target = self
            .resolve_connection(request.target.endpoint, request.target.authentication)
            .await?;
        let jump_host = match request.jump_host {
            Some(jump_host) => Some(
                self.resolve_connection(jump_host.endpoint, jump_host.authentication)
                    .await?,
            ),
            None => None,
        };

        Ok(spawn_session(SshSessionConfig {
            target,
            jump_hosts: jump_host.into_iter().collect(),
            terminal_size: request.terminal_size,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
        }))
    }

    pub async fn spawn_saved_host_session(
        &self,
        host_id: String,
        terminal_size: TerminalSize,
    ) -> Result<SpawnedSession, ApplicationError> {
        self.resolve_saved_session_config(host_id, terminal_size)
            .await
            .map(spawn_session)
    }

    pub async fn decide_host_key(
        &self,
        control: &SessionControl,
        request_id: u64,
        accepted: bool,
    ) -> Result<(), ApplicationError> {
        if !accepted {
            return control
                .confirm_host_key(request_id, false)
                .await
                .map_err(ApplicationError::from);
        }

        let observed = control.observed_host_key(request_id).await?;
        let (endpoint, algorithm, fingerprint_sha256, public_key) = observed.into_parts();
        let identity = SshEndpointIdentity::from_endpoint(&endpoint)
            .map_err(|_| ApplicationError::InvalidStoredHost)?;
        if let Err(error) = self
            .database
            .trust_observed_host_key(identity, algorithm, fingerprint_sha256, public_key)
            .await
        {
            let _ = control.confirm_host_key(request_id, false).await;
            return Err(ApplicationError::from(error));
        }

        control
            .confirm_host_key(request_id, true)
            .await
            .map_err(ApplicationError::from)
    }

    async fn resolve_saved_session_config(
        &self,
        host_id: String,
        terminal_size: TerminalSize,
    ) -> Result<SshSessionConfig, ApplicationError> {
        let plan = self
            .database
            .resolve_host_connection_plan(host_id)
            .await
            .map_err(ApplicationError::from)?;
        let (target, jump_hosts) = plan.into_parts();
        let target = resolved_host_connection(target)?;
        let jump_hosts = jump_hosts
            .into_iter()
            .map(resolved_host_connection)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SshSessionConfig {
            target,
            jump_hosts,
            terminal_size,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
        })
    }

    async fn resolve_connection(
        &self,
        endpoint: SshEndpoint,
        authentication: AuthenticationSource,
    ) -> Result<SshConnectionConfig, ApplicationError> {
        let (username, authentication) = match authentication {
            AuthenticationSource::TemporaryPassword { username, password } => {
                let username = normalize_username(username)?;
                (username, SessionAuthentication::Password { password })
            }
            AuthenticationSource::KeyboardInteractive { username } => {
                let username = normalize_username(username)?;
                (username, SessionAuthentication::KeyboardInteractive)
            }
            AuthenticationSource::Credential { credential_id } => resolved_authentication(
                self.database
                    .resolve_credential(credential_id)
                    .await
                    .map_err(ApplicationError::from)?,
            ),
        };

        let identity = SshEndpointIdentity::from_endpoint(&endpoint)
            .map_err(|_| ApplicationError::InvalidStoredHost)?;
        let known_host_policy = self
            .database
            .resolve_known_host_policy(identity)
            .await
            .map_err(ApplicationError::from)?;

        Ok(SshConnectionConfig {
            endpoint,
            username,
            authentication,
            host_key_policy: ssh_host_key_policy(known_host_policy),
        })
    }
}

pub struct SshSessionRequest {
    pub target: SshHopRequest,
    pub jump_host: Option<SshHopRequest>,
    pub terminal_size: TerminalSize,
}

impl fmt::Debug for SshSessionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SshSessionRequest")
            .field("target", &self.target)
            .field("jump_host", &self.jump_host)
            .field("terminal_size", &self.terminal_size)
            .finish()
    }
}

pub struct SshHopRequest {
    pub endpoint: SshEndpoint,
    pub authentication: AuthenticationSource,
}

impl fmt::Debug for SshHopRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SshHopRequest")
            .field("endpoint", &self.endpoint)
            .field("authentication", &self.authentication)
            .finish()
    }
}

pub enum AuthenticationSource {
    TemporaryPassword {
        username: String,
        password: Zeroizing<String>,
    },
    KeyboardInteractive {
        username: String,
    },
    Credential {
        credential_id: String,
    },
}

impl fmt::Debug for AuthenticationSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemporaryPassword { username, .. } => formatter
                .debug_struct("TemporaryPassword")
                .field("username", username)
                .field("password", &"<redacted>")
                .finish(),
            Self::KeyboardInteractive { username } => formatter
                .debug_struct("KeyboardInteractive")
                .field("username", username)
                .finish(),
            Self::Credential { credential_id } => formatter
                .debug_struct("Credential")
                .field("credential_id", credential_id)
                .finish(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("SSH username must not be empty")]
    EmptyUsername,
    #[error("saved Host endpoint is invalid")]
    InvalidStoredHost,
    #[error(transparent)]
    PrivateKeyImport(#[from] PrivateKeyImportError),
    #[error(transparent)]
    PrivateKeyPrompt(#[from] PrivateKeyPromptError),
    #[error(transparent)]
    PrivateKeyManagement(#[from] PrivateKeyManagementError),
    #[error(transparent)]
    VaultStepUpPrompt(#[from] VaultStepUpPromptError),
    #[error(transparent)]
    PrivateKeyExportPassphrasePrompt(#[from] PrivateKeyExportPassphrasePromptError),
    #[error(transparent)]
    KnownHostForgetPrompt(#[from] KnownHostForgetPromptError),
    #[error(transparent)]
    SnippetExecution(#[from] SnippetExecutionError),
    #[error(transparent)]
    FontAsset(#[from] FontAssetError),
    #[error(transparent)]
    TerminalThemeImport(#[from] TerminalThemeImportError),
    #[error(transparent)]
    SystemAgent(#[from] SystemAgentError),
    #[error(transparent)]
    SessionControl(#[from] SessionControlError),
    #[error(transparent)]
    Database(#[from] DatabaseActorError),
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SnippetExecutionError {
    #[error("Snippet variables are invalid")]
    InvalidVariables,
    #[error("multi-line Snippet requires explicit confirmation")]
    MultilineConfirmationRequired,
    #[error("rendered Snippet exceeds the supported size")]
    RenderedInputTooLarge,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PrivateKeyImportError {
    #[error("selected private key file is unavailable")]
    Unavailable,
    #[error("selected private key must be a regular file")]
    UnsupportedFileType,
    #[error("selected private key file must be between 1 byte and 1 MiB")]
    InvalidSize,
    #[error("selected private key file must be UTF-8 text")]
    InvalidEncoding,
    #[error("selected private key is invalid")]
    InvalidKey,
    #[error("selected private key requires a native passphrase prompt")]
    PassphraseRequired,
    #[error("private key passphrase was not accepted")]
    PassphraseRejected,
    #[error("private key validation task failed")]
    TaskFailed,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PrivateKeyPromptError {
    #[error("private key passphrase prompt is unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PrivateKeyManagementError {
    #[error("another private key operation is already in progress")]
    OperationBusy,
    #[error("credential is not a private key")]
    CredentialKindMismatch,
    #[error("stored private key is invalid")]
    InvalidKey,
    #[error("private key generation failed")]
    GenerationFailed,
    #[error("private key operation task failed")]
    TaskFailed,
    #[error("OpenSSH public key exceeds the supported size")]
    PublicKeyTooLarge,
    #[error("Vault step-up PIN was not accepted")]
    StepUpRejected,
    #[error("private key export passphrase was not accepted")]
    PassphraseRejected,
    #[error("private key export destination already exists")]
    DestinationExists,
    #[error("private key export destination is not a regular file")]
    UnsupportedDestination,
    #[error("private key export failed")]
    ExportFailed,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum VaultStepUpPromptError {
    #[error("Vault step-up prompt is unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PrivateKeyExportPassphrasePromptError {
    #[error("private key export passphrase prompt is unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum KnownHostForgetPromptError {
    #[error("known Host forget confirmation is unavailable")]
    Unavailable,
}

struct PrivateKeyImportCandidate {
    private_key: Zeroizing<String>,
    encryption: PrivateKeyTextEncryption,
}

impl fmt::Debug for PrivateKeyImportCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateKeyImportCandidate")
            .field("private_key", &"<redacted>")
            .field("encryption", &self.encryption)
            .finish()
    }
}

fn read_private_key_file(path: &Path) -> Result<PrivateKeyImportCandidate, PrivateKeyImportError> {
    let link_metadata =
        std::fs::symlink_metadata(path).map_err(|_| PrivateKeyImportError::Unavailable)?;
    if link_metadata.file_type().is_symlink() {
        return Err(PrivateKeyImportError::UnsupportedFileType);
    }
    if !link_metadata.is_file() {
        return Err(PrivateKeyImportError::UnsupportedFileType);
    }

    let file = open_private_key_file(path)?;
    let metadata = file
        .metadata()
        .map_err(|_| PrivateKeyImportError::Unavailable)?;
    if !metadata.is_file() {
        return Err(PrivateKeyImportError::UnsupportedFileType);
    }
    if metadata.len() == 0 || metadata.len() > MAX_PRIVATE_KEY_BYTES as u64 {
        return Err(PrivateKeyImportError::InvalidSize);
    }

    let mut private_key = Zeroizing::new(String::new());
    file.take(MAX_PRIVATE_KEY_BYTES as u64 + 1)
        .read_to_string(&mut private_key)
        .map_err(|_| PrivateKeyImportError::InvalidEncoding)?;
    if private_key.is_empty() || private_key.len() > MAX_PRIVATE_KEY_BYTES {
        return Err(PrivateKeyImportError::InvalidSize);
    }
    let encryption = inspect_openssh_private_key_text(private_key.as_str())
        .map_err(|_| PrivateKeyImportError::InvalidKey)?;
    Ok(PrivateKeyImportCandidate {
        private_key,
        encryption,
    })
}

fn sanitize_private_key_prompt_label(label: &str) -> String {
    let label = label.trim();
    let label: String = label
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_PRIVATE_KEY_PROMPT_LABEL_CHARS)
        .collect();
    let label = label.trim();
    if label.is_empty() {
        DEFAULT_PRIVATE_KEY_PROMPT_LABEL.to_owned()
    } else {
        label.to_owned()
    }
}

fn sanitize_generated_private_key_comment(label: &str) -> String {
    label
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_GENERATED_PRIVATE_KEY_COMMENT_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn private_key_public_summary_from_text(
    credential_id: String,
    private_key: Zeroizing<String>,
    passphrase: Option<Zeroizing<String>>,
) -> Result<PrivateKeyPublicSummary, PrivateKeyManagementError> {
    let private_key = decode_stored_private_key(private_key, passphrase)?;
    let public_key = private_key.public_key();
    let openssh_public_key = public_key
        .to_openssh()
        .map_err(|_| PrivateKeyManagementError::InvalidKey)?;
    if openssh_public_key.len() > MAX_OPENSSH_PUBLIC_KEY_BYTES {
        return Err(PrivateKeyManagementError::PublicKeyTooLarge);
    }
    if openssh_public_key.chars().any(char::is_control) {
        return Err(PrivateKeyManagementError::InvalidKey);
    }

    Ok(PrivateKeyPublicSummary {
        credential_id,
        algorithm: public_key.algorithm().to_string(),
        fingerprint_sha256: public_key.fingerprint(HashAlg::Sha256).to_string(),
        openssh_public_key,
    })
}

fn decode_stored_private_key(
    private_key: Zeroizing<String>,
    passphrase: Option<Zeroizing<String>>,
) -> Result<PrivateKey, PrivateKeyManagementError> {
    let private_key = PrivateKey::from_openssh(private_key.as_bytes())
        .map_err(|_| PrivateKeyManagementError::InvalidKey)?;
    if private_key.is_encrypted() {
        let passphrase = passphrase
            .as_ref()
            .ok_or(PrivateKeyManagementError::InvalidKey)?;
        private_key
            .decrypt(passphrase.as_bytes())
            .map_err(|_| PrivateKeyManagementError::InvalidKey)
    } else {
        Ok(private_key)
    }
}

fn valid_private_key_export_passphrase(passphrase: &str) -> bool {
    !passphrase.is_empty() && passphrase.len() <= MAX_PRIVATE_KEY_EXPORT_PASSPHRASE_BYTES
}

fn prepare_snippet_input(
    draft: SnippetDraft,
    variables: BTreeMap<String, String>,
    append_enter: bool,
    confirmed_multiline: bool,
) -> Result<PreparedSnippetInput, SnippetExecutionError> {
    let mut input = draft
        .render(&variables)
        .map_err(|_| SnippetExecutionError::InvalidVariables)?;
    let multiline = input.contains('\n') || input.contains('\r');
    if multiline && !confirmed_multiline {
        return Err(SnippetExecutionError::MultilineConfirmationRequired);
    }
    if append_enter {
        if input.len() >= MAX_PREPARED_SNIPPET_INPUT_BYTES {
            return Err(SnippetExecutionError::RenderedInputTooLarge);
        }
        input.push('\r');
    }
    Ok(PreparedSnippetInput { input, multiline })
}

fn export_private_key_to_new_file(
    path: PathBuf,
    private_key: Zeroizing<String>,
    stored_passphrase: Option<Zeroizing<String>>,
    export_passphrase: Zeroizing<String>,
) -> Result<PrivateKeyExportSummary, PrivateKeyManagementError> {
    let private_key = decode_stored_private_key(private_key, stored_passphrase)?;
    let public_key = private_key.public_key();
    let algorithm = public_key.algorithm().to_string();
    let fingerprint_sha256 = public_key.fingerprint(HashAlg::Sha256).to_string();
    let encrypted = private_key
        .encrypt(&mut rand::rng(), export_passphrase.as_bytes())
        .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
    let serialized = encrypted
        .to_openssh(LineEnding::LF)
        .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
    if serialized.is_empty() || serialized.len() > MAX_PRIVATE_KEY_BYTES {
        return Err(PrivateKeyManagementError::ExportFailed);
    }

    write_private_key_export_new(&path, serialized.as_bytes())?;
    Ok(PrivateKeyExportSummary {
        file_name: sanitized_export_file_name(&path),
        algorithm,
        fingerprint_sha256,
    })
}

fn write_private_key_export_new(
    path: &Path,
    serialized: &[u8],
) -> Result<(), PrivateKeyManagementError> {
    write_private_key_export_new_with(path, |file| {
        file.write_all(serialized)
            .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
        file.sync_all()
            .map_err(|_| PrivateKeyManagementError::ExportFailed)
    })
}

fn write_private_key_export_new_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), PrivateKeyManagementError>,
) -> Result<(), PrivateKeyManagementError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => return Err(PrivateKeyManagementError::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(PrivateKeyManagementError::ExportFailed),
    }

    let mut file = open_private_key_export_file(path)?;
    let result = (|| {
        if !file
            .metadata()
            .map_err(|_| PrivateKeyManagementError::ExportFailed)?
            .is_file()
        {
            return Err(PrivateKeyManagementError::UnsupportedDestination);
        }
        write(&mut file)
    })();
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

#[cfg(unix)]
fn open_private_key_export_file(path: &Path) -> Result<File, PrivateKeyManagementError> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(map_private_key_export_open_error)
}

#[cfg(windows)]
fn open_private_key_export_file(path: &Path) -> Result<File, PrivateKeyManagementError> {
    windows_private_key_export::open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_private_key_export_file(path: &Path) -> Result<File, PrivateKeyManagementError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(map_private_key_export_open_error)
}

#[cfg(not(windows))]
fn map_private_key_export_open_error(error: std::io::Error) -> PrivateKeyManagementError {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        PrivateKeyManagementError::DestinationExists
    } else {
        PrivateKeyManagementError::ExportFailed
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_private_key_export {
    use std::{
        ffi::OsStr,
        fs::File,
        mem::size_of,
        os::windows::{ffi::OsStrExt, fs::MetadataExt, io::FromRawHandle},
        path::Path,
    };

    use windows::{
        Win32::{
            Foundation::{
                CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, FALSE, GENERIC_WRITE, HANDLE,
                HLOCAL, LocalFree,
            },
            Security::{
                Authorization::{
                    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                    SDDL_REVISION_1,
                },
                GetTokenInformation, IsValidSid, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
                TOKEN_QUERY, TOKEN_USER, TokenUser,
            },
            Storage::FileSystem::{
                CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
                FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_MODE,
                FileAttributeTagInfo, GetFileInformationByHandleEx,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
        core::{PCWSTR, PWSTR},
    };

    use super::PrivateKeyManagementError;

    struct LocalSecurityDescriptor(PSECURITY_DESCRIPTOR);

    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            if !self.0.0.is_null() {
                // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW
                // allocated this descriptor with LocalAlloc and ownership has
                // not been transferred.
                unsafe {
                    let _ = LocalFree(Some(HLOCAL(self.0.0)));
                }
            }
        }
    }

    struct LocalWideString(PWSTR);

    impl Drop for LocalWideString {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: ConvertSidToStringSidW allocated this string with
                // LocalAlloc and ownership has not been transferred.
                unsafe {
                    let _ = LocalFree(Some(HLOCAL(self.0.0.cast())));
                }
            }
        }
    }

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                // SAFETY: this wrapper owns the token handle returned by
                // OpenProcessToken and closes it exactly once.
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    pub(super) fn open(path: &Path) -> Result<File, PrivateKeyManagementError> {
        if !path.is_absolute()
            || path
                .file_name()
                .is_none_or(|name| name.to_string_lossy().contains(':'))
        {
            return Err(PrivateKeyManagementError::UnsupportedDestination);
        }
        reject_reparse_point_ancestors(path)?;

        let current_user_sid = current_user_sid_string()?;
        let owner_only_dacl = format!("O:{current_user_sid}D:P(A;;FA;;;{current_user_sid})");
        let security_descriptor_text = wide_null_terminated(&owner_only_dacl);
        let mut security_descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the SDDL input is NUL-terminated and the output pointer is
        // valid for the duration of the call.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(security_descriptor_text.as_ptr()),
                SDDL_REVISION_1,
                &mut security_descriptor,
                None,
            )
        }
        .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
        if security_descriptor.0.is_null() {
            return Err(PrivateKeyManagementError::ExportFailed);
        }
        let security_descriptor = LocalSecurityDescriptor(security_descriptor);
        let security_attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security_descriptor.0.0,
            bInheritHandle: FALSE,
        };
        let wide_path = wide_path(path);

        // SAFETY: the path and security descriptor remain live and
        // NUL-terminated for the synchronous call. CREATE_NEW prevents
        // overwrite, FILE_FLAG_OPEN_REPARSE_POINT prevents following a final
        // reparse point, and the protected DACL grants full access only to the
        // current process-token user SID, which is also set as the file owner.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                Some(&security_attributes as *const SECURITY_ATTRIBUTES),
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|error| {
            if error.code() == ERROR_FILE_EXISTS.to_hresult()
                || error.code() == ERROR_ALREADY_EXISTS.to_hresult()
            {
                PrivateKeyManagementError::DestinationExists
            } else {
                PrivateKeyManagementError::ExportFailed
            }
        })?;

        // SAFETY: CreateFileW returned a unique owned handle and ownership is
        // transferred exactly once to File.
        let file = unsafe { File::from_raw_handle(handle.0) };
        let mut attributes = FILE_ATTRIBUTE_TAG_INFO::default();
        // SAFETY: attributes points to a correctly sized writable structure,
        // and File keeps the handle valid for the duration of the call.
        let inspection = unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileAttributeTagInfo,
                (&mut attributes as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        };
        if inspection.is_err() {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(PrivateKeyManagementError::ExportFailed);
        }
        if attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(PrivateKeyManagementError::UnsupportedDestination);
        }
        Ok(file)
    }

    fn current_user_sid_string() -> Result<String, PrivateKeyManagementError> {
        let mut token = HANDLE::default();
        // SAFETY: GetCurrentProcess returns a process pseudo-handle and token
        // points to writable storage for the returned owned token handle.
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
        let token = OwnedHandle(token);

        let mut required_length = 0;
        // SAFETY: the first call intentionally supplies no buffer so Windows
        // reports the required TOKEN_USER size.
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut required_length) };
        if required_length < size_of::<TOKEN_USER>() as u32 {
            return Err(PrivateKeyManagementError::ExportFailed);
        }

        let token_information_words = (required_length as usize).div_ceil(size_of::<usize>());
        let mut token_information = vec![0_usize; token_information_words];
        let token_information_bytes = (token_information.len() * size_of::<usize>()) as u32;
        // SAFETY: token_information is writable and at least the size Windows
        // requested for TOKEN_USER, with alignment suitable for TOKEN_USER.
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(token_information.as_mut_ptr().cast()),
                token_information_bytes,
                &mut required_length,
            )
        }
        .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
        // SAFETY: GetTokenInformation initialized the buffer as TOKEN_USER.
        let token_user = unsafe { &*(token_information.as_ptr().cast::<TOKEN_USER>()) };
        // SAFETY: the SID pointer belongs to the live TOKEN_USER buffer.
        if !unsafe { IsValidSid(token_user.User.Sid) }.as_bool() {
            return Err(PrivateKeyManagementError::ExportFailed);
        }

        let mut string_sid = PWSTR::null();
        // SAFETY: the SID is valid and string_sid points to writable output
        // storage. Windows allocates the returned NUL-terminated string.
        unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut string_sid) }
            .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
        if string_sid.is_null() {
            return Err(PrivateKeyManagementError::ExportFailed);
        }
        let string_sid = LocalWideString(string_sid);
        // SAFETY: ConvertSidToStringSidW returned a live NUL-terminated UTF-16
        // string owned by string_sid.
        unsafe { string_sid.0.to_string() }.map_err(|_| PrivateKeyManagementError::ExportFailed)
    }

    fn reject_reparse_point_ancestors(path: &Path) -> Result<(), PrivateKeyManagementError> {
        let Some(parent) = path.parent() else {
            return Err(PrivateKeyManagementError::UnsupportedDestination);
        };
        for ancestor in parent.ancestors() {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            let metadata = std::fs::symlink_metadata(ancestor)
                .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
                return Err(PrivateKeyManagementError::UnsupportedDestination);
            }
        }
        Ok(())
    }

    fn wide_null_terminated(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

fn sanitized_export_file_name(path: &Path) -> String {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let file_name: String = file_name
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_PRIVATE_KEY_EXPORT_FILE_NAME_CHARS)
        .collect();
    let file_name = file_name.trim();
    if file_name.is_empty() {
        DEFAULT_PRIVATE_KEY_EXPORT_FILE_NAME.to_owned()
    } else {
        file_name.to_owned()
    }
}

#[cfg(unix)]
fn open_private_key_file(path: &Path) -> Result<File, PrivateKeyImportError> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| PrivateKeyImportError::Unavailable)
}

#[cfg(not(unix))]
fn open_private_key_file(path: &Path) -> Result<File, PrivateKeyImportError> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|_| PrivateKeyImportError::Unavailable)
}

fn resolved_authentication(resolved: ResolvedCredential) -> (String, SessionAuthentication) {
    let (username, secret) = resolved.into_parts();
    let authentication = match secret {
        CredentialSecret::Password { password } => SessionAuthentication::Password { password },
        CredentialSecret::PrivateKey {
            private_key,
            passphrase,
        } => SessionAuthentication::PrivateKey {
            private_key,
            passphrase,
        },
        CredentialSecret::SystemAgent {
            identity_fingerprint_sha256,
        } => SessionAuthentication::SystemAgent {
            identity_fingerprint_sha256: identity_fingerprint_sha256.to_string(),
        },
        CredentialSecret::KeyboardInteractive => SessionAuthentication::KeyboardInteractive,
    };
    (username, authentication)
}

fn resolved_host_connection(
    connection: ResolvedHostConnection,
) -> Result<SshConnectionConfig, ApplicationError> {
    let (_, host, port, credential, known_host_policy) = connection.into_parts();
    let endpoint = SshEndpoint::new(host, port).map_err(|_| ApplicationError::InvalidStoredHost)?;
    let (username, authentication) = resolved_authentication(credential);
    Ok(SshConnectionConfig {
        endpoint,
        username,
        authentication,
        host_key_policy: ssh_host_key_policy(known_host_policy),
    })
}

fn ssh_host_key_policy(policy: ResolvedKnownHostPolicy) -> HostKeyPolicy {
    match policy {
        ResolvedKnownHostPolicy::Prompt => HostKeyPolicy::Prompt,
        ResolvedKnownHostPolicy::RequireSha256Set(fingerprints) => {
            HostKeyPolicy::RequireSha256Set { fingerprints }
        }
    }
}

fn normalize_username(username: String) -> Result<String, ApplicationError> {
    let username = username.trim();
    if username.is_empty() {
        return Err(ApplicationError::EmptyUsername);
    }
    Ok(username.to_owned())
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex as StdMutex};

    use anyssh_vault::PinKdfParameters;
    use russh::keys::{
        PrivateKey,
        ssh_key::{Algorithm, LineEnding},
    };
    use tempfile::tempdir;

    use super::*;

    fn test_core() -> (ApplicationCore, tempfile::TempDir) {
        let directory = tempdir().expect("tempdir");
        let core = ApplicationCore::spawn(
            directory.path().join("vault"),
            DatabaseActorConfig {
                command_queue_capacity: 8,
                pin_kdf_parameters: PinKdfParameters::new(8 * 1024, 1, 1)
                    .expect("test KDF parameters"),
            },
        )
        .expect("application core");
        (core, directory)
    }

    fn fixture_private_key() -> PrivateKey {
        PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)
            .expect("generate fixture Private Key")
    }

    fn bundled_terminal_font_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/client/src/assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf")
    }

    enum TestPromptReply {
        Passphrase(&'static str),
        Cancel,
        Unavailable,
    }

    struct TestPrompt {
        replies: StdMutex<VecDeque<TestPromptReply>>,
        contexts: StdMutex<Vec<PrivateKeyPromptContext>>,
    }

    impl TestPrompt {
        fn new(replies: impl IntoIterator<Item = TestPromptReply>) -> Self {
            Self {
                replies: StdMutex::new(replies.into_iter().collect()),
                contexts: StdMutex::new(Vec::new()),
            }
        }

        fn contexts(&self) -> Vec<PrivateKeyPromptContext> {
            self.contexts.lock().expect("prompt contexts").clone()
        }
    }

    enum TestStepUpReply {
        Pin(&'static str),
        Cancel,
        Unavailable,
    }

    struct TestStepUpPrompt {
        replies: StdMutex<VecDeque<TestStepUpReply>>,
        contexts: StdMutex<Vec<VaultStepUpContext>>,
    }

    impl TestStepUpPrompt {
        fn new(replies: impl IntoIterator<Item = TestStepUpReply>) -> Self {
            Self {
                replies: StdMutex::new(replies.into_iter().collect()),
                contexts: StdMutex::new(Vec::new()),
            }
        }

        fn contexts(&self) -> Vec<VaultStepUpContext> {
            self.contexts.lock().expect("step-up contexts").clone()
        }
    }

    impl VaultStepUpPrompt for TestStepUpPrompt {
        fn request(
            &self,
            context: VaultStepUpContext,
        ) -> impl Future<Output = Result<Option<Zeroizing<String>>, VaultStepUpPromptError>> + Send
        {
            self.contexts
                .lock()
                .expect("step-up contexts")
                .push(context);
            let reply = self
                .replies
                .lock()
                .expect("step-up replies")
                .pop_front()
                .unwrap_or(TestStepUpReply::Cancel);
            std::future::ready(match reply {
                TestStepUpReply::Pin(pin) => Ok(Some(Zeroizing::new(pin.to_owned()))),
                TestStepUpReply::Cancel => Ok(None),
                TestStepUpReply::Unavailable => Err(VaultStepUpPromptError::Unavailable),
            })
        }
    }

    enum TestExportPassphraseReply {
        Pair(&'static str, &'static str),
        Cancel,
        Unavailable,
    }

    struct TestExportPassphrasePrompt {
        replies: StdMutex<VecDeque<TestExportPassphraseReply>>,
        contexts: StdMutex<Vec<PrivateKeyExportPassphraseContext>>,
    }

    impl TestExportPassphrasePrompt {
        fn new(replies: impl IntoIterator<Item = TestExportPassphraseReply>) -> Self {
            Self {
                replies: StdMutex::new(replies.into_iter().collect()),
                contexts: StdMutex::new(Vec::new()),
            }
        }

        fn contexts(&self) -> Vec<PrivateKeyExportPassphraseContext> {
            self.contexts
                .lock()
                .expect("export passphrase contexts")
                .clone()
        }
    }

    impl PrivateKeyExportPassphrasePrompt for TestExportPassphrasePrompt {
        fn request(
            &self,
            context: PrivateKeyExportPassphraseContext,
        ) -> impl Future<
            Output = Result<
                Option<PrivateKeyExportPassphraseCandidate>,
                PrivateKeyExportPassphrasePromptError,
            >,
        > + Send {
            self.contexts
                .lock()
                .expect("export passphrase contexts")
                .push(context);
            let reply = self
                .replies
                .lock()
                .expect("export passphrase replies")
                .pop_front()
                .unwrap_or(TestExportPassphraseReply::Cancel);
            std::future::ready(match reply {
                TestExportPassphraseReply::Pair(passphrase, confirmation) => {
                    Ok(Some(PrivateKeyExportPassphraseCandidate::new(
                        Zeroizing::new(passphrase.to_owned()),
                        Zeroizing::new(confirmation.to_owned()),
                    )))
                }
                TestExportPassphraseReply::Cancel => Ok(None),
                TestExportPassphraseReply::Unavailable => {
                    Err(PrivateKeyExportPassphrasePromptError::Unavailable)
                }
            })
        }
    }

    struct TestKnownHostForgetPrompt {
        approved: bool,
        contexts: StdMutex<Vec<KnownHostForgetPromptContext>>,
    }

    impl TestKnownHostForgetPrompt {
        fn new(approved: bool) -> Self {
            Self {
                approved,
                contexts: StdMutex::new(Vec::new()),
            }
        }
    }

    impl KnownHostForgetPrompt for TestKnownHostForgetPrompt {
        fn confirm(
            &self,
            context: KnownHostForgetPromptContext,
        ) -> impl Future<Output = Result<bool, KnownHostForgetPromptError>> + Send {
            self.contexts
                .lock()
                .expect("Known Host contexts")
                .push(context);
            std::future::ready(Ok(self.approved))
        }
    }

    struct UnavailableKnownHostForgetPrompt;

    impl KnownHostForgetPrompt for UnavailableKnownHostForgetPrompt {
        fn confirm(
            &self,
            _context: KnownHostForgetPromptContext,
        ) -> impl Future<Output = Result<bool, KnownHostForgetPromptError>> + Send {
            std::future::ready(Err(KnownHostForgetPromptError::Unavailable))
        }
    }

    impl PrivateKeyPassphrasePrompt for TestPrompt {
        fn request(
            &self,
            context: PrivateKeyPromptContext,
        ) -> impl Future<Output = Result<Option<Zeroizing<String>>, PrivateKeyPromptError>> + Send
        {
            self.contexts.lock().expect("prompt contexts").push(context);
            let reply = self
                .replies
                .lock()
                .expect("prompt replies")
                .pop_front()
                .unwrap_or(TestPromptReply::Unavailable);
            async move {
                match reply {
                    TestPromptReply::Passphrase(passphrase) => {
                        Ok(Some(Zeroizing::new(passphrase.to_owned())))
                    }
                    TestPromptReply::Cancel => Ok(None),
                    TestPromptReply::Unavailable => Err(PrivateKeyPromptError::Unavailable),
                }
            }
        }
    }

    #[test]
    fn temporary_password_debug_output_is_redacted() {
        let request = SshSessionRequest {
            target: SshHopRequest {
                endpoint: SshEndpoint::new("example.com", 22).expect("endpoint"),
                authentication: AuthenticationSource::TemporaryPassword {
                    username: "alice".to_owned(),
                    password: Zeroizing::new("temporary-secret".to_owned()),
                },
            },
            jump_host: None,
            terminal_size: TerminalSize::default(),
        };

        let debug = format!("{request:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("temporary-secret"));
    }

    #[tokio::test]
    async fn quick_keyboard_interactive_resolves_without_a_saved_secret() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");

        let resolved = core
            .resolve_connection(
                SshEndpoint::new("example.com", 22).expect("endpoint"),
                AuthenticationSource::KeyboardInteractive {
                    username: " interactive-user ".to_owned(),
                },
            )
            .await
            .expect("resolve keyboard-interactive request");
        assert_eq!(resolved.username, "interactive-user");
        assert!(matches!(
            resolved.authentication,
            SessionAuthentication::KeyboardInteractive
        ));
    }

    #[tokio::test]
    async fn snippet_input_is_rendered_in_rust_and_requires_multiline_confirmation() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");

        let single = core
            .create_snippet(
                "Single line".to_owned(),
                Zeroizing::new("echo {{target}}".to_owned()),
            )
            .await
            .expect("create single-line Snippet");
        let missing = core
            .prepare_snippet_input(single.id().to_owned(), BTreeMap::new(), true, false)
            .await
            .expect_err("missing variable must fail");
        assert!(matches!(
            missing,
            ApplicationError::SnippetExecution(SnippetExecutionError::InvalidVariables)
        ));

        let prepared = core
            .prepare_snippet_input(
                single.id().to_owned(),
                BTreeMap::from([("target".to_owned(), "server".to_owned())]),
                true,
                false,
            )
            .await
            .expect("prepare single-line Snippet");
        assert_eq!(prepared.input(), "echo server\r");
        assert!(!prepared.multiline());
        assert!(!format!("{prepared:?}").contains("echo server"));

        let multi = core
            .create_snippet(
                "Multi line".to_owned(),
                Zeroizing::new("printf first\nprintf {{value}}".to_owned()),
            )
            .await
            .expect("create multi-line Snippet");
        let confirmation = core
            .prepare_snippet_input(
                multi.id().to_owned(),
                BTreeMap::from([("value".to_owned(), "second".to_owned())]),
                false,
                false,
            )
            .await
            .expect_err("multi-line Snippet needs confirmation");
        assert!(matches!(
            confirmation,
            ApplicationError::SnippetExecution(
                SnippetExecutionError::MultilineConfirmationRequired
            )
        ));
        let confirmed = core
            .prepare_snippet_input(
                multi.id().to_owned(),
                BTreeMap::from([("value".to_owned(), "second".to_owned())]),
                false,
                true,
            )
            .await
            .expect("confirmed multi-line Snippet");
        assert!(confirmed.multiline());
        assert_eq!(confirmed.input(), "printf first\nprintf second");
    }

    #[tokio::test]
    async fn imported_font_asset_is_integrity_checked_and_falls_back_after_tampering() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");

        let font = core
            .import_font_asset_from_path(bundled_terminal_font_path())
            .await
            .expect("import bundled Font through native boundary");
        assert_eq!(font.format(), FontAssetFormat::Ttf);
        assert!(!font.family().is_empty());
        assert_eq!(
            core.list_font_assets()
                .await
                .expect("list Font assets")
                .as_slice(),
            std::slice::from_ref(&font)
        );

        let asset_root = directory
            .path()
            .join("vault")
            .join(FONT_ASSET_DIRECTORY_NAME);
        let bytes =
            read_managed_font_asset(&asset_root, font.id(), font.format(), font.sha256_hex())
                .expect("read verified Font protocol payload");
        assert_eq!(bytes.len() as u64, font.size_bytes());
        let asset_path = asset_root.join(format!("{}.{}", font.id(), font.format().extension()));
        let orphan_path = asset_root.join("font-orphan.ttf");
        std::fs::copy(&asset_path, &orphan_path).expect("create orphaned managed Font");
        assert_eq!(
            core.list_font_assets()
                .await
                .expect("clean orphaned Font assets")
                .as_slice(),
            std::slice::from_ref(&font)
        );
        assert!(!orphan_path.exists());

        let current = core
            .get_appearance_settings()
            .await
            .expect("Appearance settings");
        core.update_appearance_settings(
            AppearanceSettings::new(
                current.app_theme(),
                current.terminal_theme_id().to_owned(),
                FontSourceKind::Imported,
                Some(font.id().to_owned()),
                font.family().to_owned(),
                current.font_size(),
                current.line_height_millis(),
                current.ligatures_enabled(),
                current.ambiguous_width(),
            )
            .expect("Imported Font Appearance"),
        )
        .await
        .expect("select imported Font");

        std::fs::write(&asset_path, b"tampered-font").expect("tamper managed Font");
        assert!(
            core.list_font_assets()
                .await
                .expect("reconcile tampered Font")
                .is_empty()
        );
        let fallback = core
            .get_appearance_settings()
            .await
            .expect("Appearance fallback");
        assert_eq!(fallback.font_source_kind(), FontSourceKind::Bundled);
        assert_eq!(fallback.font_id(), Some(anyssh_storage::DEFAULT_FONT_ID));
        assert!(!asset_path.exists());
    }

    #[tokio::test]
    async fn stored_private_key_moves_from_credential_id_into_ssh_authentication() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let summary = core
            .store_private_key_credential(
                "Fixture key".to_owned(),
                "fixture-user".to_owned(),
                Zeroizing::new("private-key-material".to_owned()),
                Some(Zeroizing::new("private-key-passphrase".to_owned())),
            )
            .await
            .expect("store private key");

        let source = AuthenticationSource::Credential {
            credential_id: summary.id().to_owned(),
        };
        let source_debug = format!("{source:?}");
        assert!(source_debug.contains(summary.id()));
        assert!(!source_debug.contains("private-key-material"));
        assert!(!source_debug.contains("private-key-passphrase"));

        let resolved = core
            .resolve_connection(
                SshEndpoint::new("example.com", 22).expect("endpoint"),
                source,
            )
            .await
            .expect("resolve stored credential");
        let resolved_debug = format!("{resolved:?}");
        assert!(resolved_debug.contains("<redacted>"));
        assert!(!resolved_debug.contains("private-key-material"));
        assert!(!resolved_debug.contains("private-key-passphrase"));

        assert_eq!(resolved.username, "fixture-user");
        let SessionAuthentication::PrivateKey {
            private_key,
            passphrase,
        } = resolved.authentication
        else {
            panic!("expected private-key authentication");
        };
        assert_eq!(private_key.as_str(), "private-key-material");
        assert_eq!(
            passphrase.as_deref().map(String::as_str),
            Some("private-key-passphrase")
        );
    }

    #[tokio::test]
    async fn generated_private_keys_are_stored_and_project_public_metadata_only() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");

        for (algorithm, expected_algorithm) in [
            (PrivateKeyGenerationAlgorithm::Ed25519, "ssh-ed25519"),
            (PrivateKeyGenerationAlgorithm::Rsa4096, "ssh-rsa"),
        ] {
            let summary = core
                .generate_private_key_credential(
                    format!("Generated {expected_algorithm}"),
                    "generated-user".to_owned(),
                    algorithm,
                )
                .await
                .expect("generate private key");
            assert_eq!(summary.kind(), CredentialKind::PrivateKey);

            let public = core
                .private_key_public_summary(summary.id().to_owned())
                .await
                .expect("public key summary");
            assert_eq!(public.credential_id(), summary.id());
            assert_eq!(public.algorithm(), expected_algorithm);
            assert!(public.fingerprint_sha256().starts_with("SHA256:"));
            assert!(public.openssh_public_key().starts_with(expected_algorithm));
            assert!(!public.openssh_public_key().contains('\n'));
            assert!(public.openssh_public_key().len() <= MAX_OPENSSH_PUBLIC_KEY_BYTES);

            let debug = format!("{public:?}");
            assert!(debug.contains("<public>"));
            assert!(!debug.contains(public.openssh_public_key()));

            let resolved = core
                .resolve_connection(
                    SshEndpoint::new("example.com", 22).expect("endpoint"),
                    AuthenticationSource::Credential {
                        credential_id: summary.id().to_owned(),
                    },
                )
                .await
                .expect("resolve generated key");
            let SessionAuthentication::PrivateKey {
                private_key,
                passphrase,
            } = resolved.authentication
            else {
                panic!("expected generated private key");
            };
            assert!(passphrase.is_none());
            validate_private_key_text(private_key.as_str(), None)
                .expect("generated OpenSSH private key");
            if algorithm == PrivateKeyGenerationAlgorithm::Rsa4096 {
                let parsed = PrivateKey::from_openssh(private_key.as_bytes())
                    .expect("parse generated RSA private key");
                assert_eq!(
                    parsed
                        .public_key()
                        .key_data()
                        .rsa()
                        .expect("generated RSA public key")
                        .key_size(),
                    4096
                );
            }
        }

        let credential_count = core
            .list_credentials()
            .await
            .expect("list generated Credentials")
            .len();
        let invalid = core
            .generate_private_key_credential(
                String::new(),
                "generated-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect_err("invalid generated-key metadata must not create a Credential");
        assert!(matches!(invalid, ApplicationError::Database(_)));
        assert_eq!(
            core.list_credentials()
                .await
                .expect("list after failed generation")
                .len(),
            credential_count
        );
    }

    #[tokio::test]
    async fn private_key_operations_fail_closed_when_the_bounded_slot_is_busy() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let permit = core
            .private_key_operations
            .clone()
            .try_acquire_owned()
            .expect("reserve private-key operation slot");

        let error = core
            .generate_private_key_credential(
                "Busy fixture".to_owned(),
                "busy-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect_err("concurrent private-key operation must fail closed");
        assert!(matches!(
            error,
            ApplicationError::PrivateKeyManagement(PrivateKeyManagementError::OperationBusy)
        ));
        assert!(
            core.list_credentials()
                .await
                .expect("list credentials")
                .is_empty()
        );
        drop(permit);
    }

    #[tokio::test]
    async fn encrypted_imported_key_projects_the_same_public_key_without_exposing_passphrase() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let key = fixture_private_key();
        let expected_public = key.public_key().to_openssh().expect("public key");
        let encrypted = key
            .encrypt(&mut rand::rng(), "projection-passphrase")
            .expect("encrypt fixture")
            .to_openssh(LineEnding::LF)
            .expect("serialize fixture");
        let summary = core
            .store_private_key_credential(
                "Encrypted projection".to_owned(),
                "projection-user".to_owned(),
                encrypted,
                Some(Zeroizing::new("projection-passphrase".to_owned())),
            )
            .await
            .expect("store encrypted key");

        let public = core
            .private_key_public_summary(summary.id().to_owned())
            .await
            .expect("project encrypted key");
        assert_eq!(public.openssh_public_key(), expected_public);
        let debug = format!("{public:?}");
        assert!(!debug.contains("projection-passphrase"));
        assert!(!debug.contains(expected_public.as_str()));
    }

    #[tokio::test]
    async fn private_key_public_projection_rejects_wrong_kinds_invalid_keys_and_lock() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let password = core
            .create_password_credential(
                "Password".to_owned(),
                "password-user".to_owned(),
                Zeroizing::new("secret".to_owned()),
            )
            .await
            .expect("password credential");
        let wrong_kind = core
            .private_key_public_summary(password.id().to_owned())
            .await
            .expect_err("password cannot project a public key");
        assert!(matches!(
            wrong_kind,
            ApplicationError::PrivateKeyManagement(
                PrivateKeyManagementError::CredentialKindMismatch
            )
        ));

        let invalid = core
            .store_private_key_credential(
                "Invalid key".to_owned(),
                "invalid-user".to_owned(),
                Zeroizing::new("not-an-openssh-key".to_owned()),
                None,
            )
            .await
            .expect("trusted storage accepts opaque fixture");
        let invalid_error = core
            .private_key_public_summary(invalid.id().to_owned())
            .await
            .expect_err("invalid stored key must fail");
        assert!(matches!(
            invalid_error,
            ApplicationError::PrivateKeyManagement(PrivateKeyManagementError::InvalidKey)
        ));

        let generated = core
            .generate_private_key_credential(
                "Lock fixture".to_owned(),
                "lock-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect("generated key");
        core.lock_vault().await.expect("lock vault");
        let locked = core
            .private_key_public_summary(generated.id().to_owned())
            .await
            .expect_err("locked Vault must reject projection");
        assert!(matches!(
            locked,
            ApplicationError::Database(DatabaseActorError::VaultLocked)
        ));
        let locked_generation = core
            .generate_private_key_credential(
                "Locked generation".to_owned(),
                "locked-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect_err("locked Vault must reject generation before CSPRNG work");
        assert!(matches!(
            locked_generation,
            ApplicationError::Database(DatabaseActorError::VaultLocked)
        ));
    }

    #[tokio::test]
    async fn encrypted_private_key_export_requires_step_up_and_new_matching_passphrase() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let generated = core
            .generate_private_key_credential(
                "Export fixture".to_owned(),
                "export-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect("generated key");
        let expected_public = core
            .private_key_public_summary(generated.id().to_owned())
            .await
            .expect("public summary");
        let step_up = TestStepUpPrompt::new([
            TestStepUpReply::Pin("654321"),
            TestStepUpReply::Pin("123456"),
        ]);
        let passphrase = TestExportPassphrasePrompt::new([
            TestExportPassphraseReply::Pair("first-export-passphrase", "mismatch"),
            TestExportPassphraseReply::Pair(
                "accepted-export-passphrase",
                "accepted-export-passphrase",
            ),
        ]);
        let path = directory.path().join("generated-export.key");

        let exported = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                path.clone(),
                &step_up,
                &passphrase,
            )
            .await
            .expect("export operation")
            .expect("export summary");
        assert_eq!(exported.file_name(), "generated-export.key");
        assert_eq!(exported.algorithm(), expected_public.algorithm());
        assert_eq!(
            exported.fingerprint_sha256(),
            expected_public.fingerprint_sha256()
        );
        assert!(exported.encrypted());

        let step_up_contexts = step_up.contexts();
        assert_eq!(step_up_contexts.len(), 2);
        assert!(!step_up_contexts[0].previous_pin_incorrect());
        assert!(step_up_contexts[1].previous_pin_incorrect());
        assert_eq!(
            step_up_contexts[0].operation_label(),
            PRIVATE_KEY_EXPORT_OPERATION_LABEL
        );
        let passphrase_contexts = passphrase.contexts();
        assert_eq!(passphrase_contexts.len(), 2);
        assert!(!passphrase_contexts[0].previous_confirmation_mismatch());
        assert!(passphrase_contexts[1].previous_confirmation_mismatch());

        let serialized = std::fs::read_to_string(&path).expect("read exported key");
        let encrypted =
            PrivateKey::from_openssh(serialized.as_bytes()).expect("parse exported key");
        assert!(encrypted.is_encrypted());
        assert!(encrypted.decrypt("first-export-passphrase").is_err());
        let decrypted = encrypted
            .decrypt("accepted-export-passphrase")
            .expect("new export passphrase");
        assert_eq!(
            decrypted
                .public_key()
                .to_openssh()
                .expect("export public key"),
            expected_public.openssh_public_key()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(&path)
                    .expect("export metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn private_key_export_cancellation_and_attempt_limits_leave_no_file() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let generated = core
            .generate_private_key_credential(
                "Cancellation fixture".to_owned(),
                "cancel-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect("generated key");

        let step_up_cancel = TestStepUpPrompt::new([TestStepUpReply::Cancel]);
        let unused_passphrase = TestExportPassphrasePrompt::new([]);
        let cancelled_step_up_path = directory.path().join("step-up-cancelled.key");
        assert!(
            core.export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                cancelled_step_up_path.clone(),
                &step_up_cancel,
                &unused_passphrase,
            )
            .await
            .expect("cancel step-up")
            .is_none()
        );
        assert!(!cancelled_step_up_path.exists());
        assert!(unused_passphrase.contexts().is_empty());

        let approved = TestStepUpPrompt::new([TestStepUpReply::Pin("123456")]);
        let passphrase_cancel =
            TestExportPassphrasePrompt::new([TestExportPassphraseReply::Cancel]);
        let cancelled_passphrase_path = directory.path().join("passphrase-cancelled.key");
        assert!(
            core.export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                cancelled_passphrase_path.clone(),
                &approved,
                &passphrase_cancel,
            )
            .await
            .expect("cancel passphrase")
            .is_none()
        );
        assert!(!cancelled_passphrase_path.exists());

        let rejected_step_up = TestStepUpPrompt::new([
            TestStepUpReply::Pin("000000"),
            TestStepUpReply::Pin("111111"),
            TestStepUpReply::Pin("222222"),
        ]);
        let unused_passphrase = TestExportPassphrasePrompt::new([]);
        let rejected_step_up_path = directory.path().join("step-up-rejected.key");
        let rejected = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                rejected_step_up_path.clone(),
                &rejected_step_up,
                &unused_passphrase,
            )
            .await
            .expect_err("three wrong PINs must fail");
        assert!(matches!(
            rejected,
            ApplicationError::PrivateKeyManagement(PrivateKeyManagementError::StepUpRejected)
        ));
        assert!(!rejected_step_up_path.exists());

        let approved = TestStepUpPrompt::new([TestStepUpReply::Pin("123456")]);
        let rejected_passphrase = TestExportPassphrasePrompt::new([
            TestExportPassphraseReply::Pair("one", "mismatch"),
            TestExportPassphraseReply::Pair("", ""),
            TestExportPassphraseReply::Pair("three", "different"),
        ]);
        let rejected_passphrase_path = directory.path().join("passphrase-rejected.key");
        let rejected = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                rejected_passphrase_path.clone(),
                &approved,
                &rejected_passphrase,
            )
            .await
            .expect_err("three invalid confirmations must fail");
        assert!(matches!(
            rejected,
            ApplicationError::PrivateKeyManagement(PrivateKeyManagementError::PassphraseRejected)
        ));
        assert!(!rejected_passphrase_path.exists());

        core.lock_vault().await.expect("lock Vault");
        let locked_step_up = TestStepUpPrompt::new([TestStepUpReply::Pin("123456")]);
        let unused_passphrase = TestExportPassphrasePrompt::new([]);
        let locked_path = directory.path().join("locked-vault.key");
        let locked = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                locked_path.clone(),
                &locked_step_up,
                &unused_passphrase,
            )
            .await
            .expect_err("locked Vault must reject export");
        assert!(matches!(
            locked,
            ApplicationError::Database(DatabaseActorError::VaultLocked)
        ));
        assert!(!locked_path.exists());
    }

    #[tokio::test]
    async fn private_key_export_rejects_existing_destinations_and_unavailable_prompts() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let generated = core
            .generate_private_key_credential(
                "Existing destination fixture".to_owned(),
                "existing-user".to_owned(),
                PrivateKeyGenerationAlgorithm::Ed25519,
            )
            .await
            .expect("generated key");

        let unavailable_step_up = TestStepUpPrompt::new([TestStepUpReply::Unavailable]);
        let unused_passphrase = TestExportPassphrasePrompt::new([]);
        let unavailable_path = directory.path().join("unavailable-step-up.key");
        let unavailable = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                unavailable_path.clone(),
                &unavailable_step_up,
                &unused_passphrase,
            )
            .await
            .expect_err("unavailable step-up must fail closed");
        assert!(matches!(
            unavailable,
            ApplicationError::VaultStepUpPrompt(VaultStepUpPromptError::Unavailable)
        ));
        assert!(!unavailable_path.exists());

        let approved = TestStepUpPrompt::new([TestStepUpReply::Pin("123456")]);
        let unavailable_passphrase =
            TestExportPassphrasePrompt::new([TestExportPassphraseReply::Unavailable]);
        let unavailable_path = directory.path().join("unavailable-passphrase.key");
        let unavailable = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                unavailable_path.clone(),
                &approved,
                &unavailable_passphrase,
            )
            .await
            .expect_err("unavailable passphrase prompt must fail closed");
        assert!(matches!(
            unavailable,
            ApplicationError::PrivateKeyExportPassphrasePrompt(
                PrivateKeyExportPassphrasePromptError::Unavailable
            )
        ));
        assert!(!unavailable_path.exists());

        let existing_path = directory.path().join("existing.key");
        std::fs::write(&existing_path, b"preserve-existing-content")
            .expect("write existing fixture");
        let approved = TestStepUpPrompt::new([TestStepUpReply::Pin("123456")]);
        let passphrase = TestExportPassphrasePrompt::new([TestExportPassphraseReply::Pair(
            "export-passphrase",
            "export-passphrase",
        )]);
        let existing = core
            .export_private_key_credential_to_path_with_prompts(
                generated.id().to_owned(),
                existing_path.clone(),
                &approved,
                &passphrase,
            )
            .await
            .expect_err("existing destination must not be overwritten");
        assert!(matches!(
            existing,
            ApplicationError::PrivateKeyManagement(PrivateKeyManagementError::DestinationExists)
        ));
        assert_eq!(
            std::fs::read(&existing_path).expect("existing content"),
            b"preserve-existing-content"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_key_export_does_not_follow_symlink_destinations() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("tempdir");
        let target = directory.path().join("existing-target");
        let link = directory.path().join("private-key-export");
        std::fs::write(&target, b"preserve-target").expect("write symlink target");
        symlink(&target, &link).expect("create export symlink");

        let error = write_private_key_export_new(&link, b"private-key-fixture")
            .expect_err("export symlink must fail closed");
        assert_eq!(error, PrivateKeyManagementError::DestinationExists);
        assert_eq!(
            std::fs::read(&target).expect("read symlink target"),
            b"preserve-target"
        );
    }

    #[test]
    fn private_key_export_removes_partial_files_after_write_failure() {
        let directory = tempdir().expect("tempdir");
        let destination = directory.path().join("private-key-export");

        let error = write_private_key_export_new_with(&destination, |file| {
            file.write_all(b"partial-private-key")
                .map_err(|_| PrivateKeyManagementError::ExportFailed)?;
            Err(PrivateKeyManagementError::ExportFailed)
        })
        .expect_err("failed export must remove its partial file");

        assert_eq!(error, PrivateKeyManagementError::ExportFailed);
        assert!(!destination.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_private_key_export_rejects_reparse_points_and_alternate_streams() {
        let directory = tempdir().expect("tempdir");
        let real_parent = directory.path().join("real-parent");
        let junction_parent = directory.path().join("junction-parent");
        std::fs::create_dir(&real_parent).expect("create real parent");
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction_parent)
            .arg(&real_parent)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("create Windows junction fixture");
        assert!(status.success(), "Windows junction fixture failed");

        let destination = junction_parent.join("private-key-export");
        let error = write_private_key_export_new(&destination, b"private-key-fixture")
            .expect_err("export through a reparse-point parent must fail closed");
        assert_eq!(error, PrivateKeyManagementError::UnsupportedDestination);
        assert!(!destination.exists());
        assert!(!real_parent.join("private-key-export").exists());

        std::fs::remove_dir(&junction_parent).expect("remove junction fixture");

        let alternate_stream = real_parent.join("private-key-export:stream");
        let error = write_private_key_export_new(&alternate_stream, b"private-key-fixture")
            .expect_err("alternate data stream destinations must fail closed");
        assert_eq!(error, PrivateKeyManagementError::UnsupportedDestination);
        assert!(!real_parent.join("private-key-export").exists());
    }

    #[tokio::test]
    async fn stored_system_agent_selector_moves_into_ssh_authentication_without_key_material() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let summary = core
            .create_system_agent_credential(
                "Workstation agent".to_owned(),
                "agent-user".to_owned(),
                "SHA256:application-agent-selector".to_owned(),
            )
            .await
            .expect("store system agent");

        let resolved = core
            .resolve_connection(
                SshEndpoint::new("example.com", 22).expect("endpoint"),
                AuthenticationSource::Credential {
                    credential_id: summary.id().to_owned(),
                },
            )
            .await
            .expect("resolve stored credential");
        let debug = format!("{resolved:?}");
        assert!(debug.contains("<selected>"));
        assert!(!debug.contains("application-agent-selector"));
        assert_eq!(resolved.username, "agent-user");
        let SessionAuthentication::SystemAgent {
            identity_fingerprint_sha256,
        } = resolved.authentication
        else {
            panic!("expected system-agent authentication");
        };
        assert_eq!(
            identity_fingerprint_sha256,
            "SHA256:application-agent-selector"
        );
    }

    #[tokio::test]
    async fn stored_keyboard_interactive_credential_resolves_without_a_secret() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let summary = core
            .create_keyboard_interactive_credential(
                "Production OTP".to_owned(),
                "interactive-user".to_owned(),
            )
            .await
            .expect("create keyboard-interactive credential");
        assert_eq!(summary.kind(), CredentialKind::KeyboardInteractive);

        let updated = core
            .update_keyboard_interactive_credential(
                summary.id().to_owned(),
                "Updated OTP".to_owned(),
                "updated-interactive-user".to_owned(),
            )
            .await
            .expect("update keyboard-interactive credential");
        assert_eq!(updated.label(), "Updated OTP");

        let resolved = core
            .resolve_connection(
                SshEndpoint::new("example.com", 22).expect("endpoint"),
                AuthenticationSource::Credential {
                    credential_id: summary.id().to_owned(),
                },
            )
            .await
            .expect("resolve stored keyboard-interactive credential");
        assert_eq!(resolved.username, "updated-interactive-user");
        assert!(matches!(
            resolved.authentication,
            SessionAuthentication::KeyboardInteractive
        ));
    }

    #[tokio::test]
    async fn known_host_forget_requires_external_confirmation_and_uses_only_metadata() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let key = fixture_private_key();
        let public_key = key.public_key();
        let summary = core
            .database
            .trust_observed_host_key(
                SshEndpointIdentity::new("KNOWN.EXAMPLE.", 2222).expect("Known Host endpoint"),
                public_key.algorithm().to_string(),
                public_key
                    .fingerprint(russh::keys::ssh_key::HashAlg::Sha256)
                    .to_string(),
                public_key.to_bytes().expect("Known Host key bytes"),
            )
            .await
            .expect("create Known Host Trust");

        let cancel = TestKnownHostForgetPrompt::new(false);
        assert!(
            !core
                .forget_known_host_with_prompt(summary.id().to_owned(), &cancel)
                .await
                .expect("cancel Forget Trust")
        );
        assert_eq!(
            core.list_known_hosts()
                .await
                .expect("Trust survives cancellation")
                .len(),
            1
        );

        let unavailable = core
            .forget_known_host_with_prompt(
                summary.id().to_owned(),
                &UnavailableKnownHostForgetPrompt,
            )
            .await
            .expect_err("unavailable native confirmation must fail closed");
        assert!(matches!(
            unavailable,
            ApplicationError::KnownHostForgetPrompt(KnownHostForgetPromptError::Unavailable)
        ));
        assert_eq!(
            core.list_known_hosts()
                .await
                .expect("Trust survives unavailable confirmation")
                .len(),
            1
        );

        let approve = TestKnownHostForgetPrompt::new(true);
        assert!(
            core.forget_known_host_with_prompt(summary.id().to_owned(), &approve)
                .await
                .expect("approve Forget Trust")
        );
        assert!(
            core.list_known_hosts()
                .await
                .expect("list after Forget Trust")
                .is_empty()
        );
        let contexts = approve
            .contexts
            .lock()
            .expect("Known Host contexts after confirmation");
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].host(), "known.example");
        assert_eq!(contexts[0].port(), 2222);
        assert_eq!(contexts[0].fingerprints_sha256().len(), 1);
        let debug = format!("{:?}", contexts[0]);
        assert!(!debug.contains("public_key"));
        assert!(!debug.contains("publicKey"));
    }

    #[tokio::test]
    async fn native_private_key_file_import_validates_before_storing() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let private_key = fixture_private_key()
            .to_openssh(LineEnding::LF)
            .expect("encode fixture Private Key");
        let path = directory.path().join("id_ed25519");
        std::fs::write(&path, private_key.as_bytes()).expect("write fixture Private Key");

        let summary = core
            .import_private_key_credential_from_path(
                "Imported fixture".to_owned(),
                "fixture-user".to_owned(),
                path,
            )
            .await
            .expect("import Private Key");

        assert_eq!(summary.kind(), CredentialKind::PrivateKey);
        assert_eq!(summary.label(), "Imported fixture");
        assert_eq!(
            core.list_credentials()
                .await
                .expect("list imported credentials"),
            vec![summary]
        );
        let debug = format!(
            "{:?}",
            core.list_credentials()
                .await
                .expect("list imported credentials")
        );
        assert!(!debug.contains("BEGIN OPENSSH PRIVATE KEY"));
    }

    #[tokio::test]
    async fn native_private_key_file_import_rejects_invalid_and_requires_a_prompt_for_encrypted_keys()
     {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");

        let invalid_path = directory.path().join("invalid-key");
        std::fs::write(&invalid_path, "not-a-private-key").expect("write invalid fixture");
        let invalid_error = core
            .import_private_key_credential_from_path(
                "Invalid fixture".to_owned(),
                "fixture-user".to_owned(),
                invalid_path,
            )
            .await
            .expect_err("invalid Private Key must fail");
        assert!(matches!(
            invalid_error,
            ApplicationError::PrivateKeyImport(PrivateKeyImportError::InvalidKey)
        ));

        let encrypted_key = fixture_private_key()
            .encrypt(&mut rand::rng(), "fixture-passphrase")
            .expect("encrypt fixture Private Key")
            .to_openssh(LineEnding::LF)
            .expect("encode encrypted fixture Private Key");
        let encrypted_path = directory.path().join("encrypted-key");
        std::fs::write(&encrypted_path, encrypted_key.as_bytes()).expect("write encrypted fixture");
        let encrypted_error = core
            .import_private_key_credential_from_path(
                "Encrypted fixture".to_owned(),
                "fixture-user".to_owned(),
                encrypted_path,
            )
            .await
            .expect_err("encrypted Private Key must fail without native passphrase");
        assert!(matches!(
            encrypted_error,
            ApplicationError::PrivateKeyImport(PrivateKeyImportError::PassphraseRequired)
        ));
        assert!(
            core.list_credentials()
                .await
                .expect("list credentials after rejected imports")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn native_encrypted_private_key_import_retries_and_stores_the_original_key() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let encrypted_key = fixture_private_key()
            .encrypt(&mut rand::rng(), "correct-passphrase")
            .expect("encrypt fixture Private Key")
            .to_openssh(LineEnding::LF)
            .expect("encode encrypted fixture Private Key");
        let path = directory.path().join("encrypted-key");
        std::fs::write(&path, encrypted_key.as_bytes()).expect("write encrypted fixture");
        let prompt = TestPrompt::new([
            TestPromptReply::Passphrase(""),
            TestPromptReply::Passphrase("wrong-passphrase"),
            TestPromptReply::Passphrase("correct-passphrase"),
        ]);

        let summary = core
            .import_private_key_credential_from_path_with_prompt(
                "  Encrypted fixture  ".to_owned(),
                "fixture-user".to_owned(),
                path,
                &prompt,
            )
            .await
            .expect("import encrypted Private Key")
            .expect("prompt was accepted");
        assert_eq!(summary.kind(), CredentialKind::PrivateKey);

        let contexts = prompt.contexts();
        assert_eq!(contexts.len(), 3);
        assert_eq!(contexts[0].label(), "Encrypted fixture");
        assert_eq!(contexts[0].attempt(), 1);
        assert_eq!(contexts[0].max_attempts(), 3);
        assert!(!contexts[0].previous_passphrase_incorrect());
        assert_eq!(contexts[1].attempt(), 2);
        assert!(contexts[1].previous_passphrase_incorrect());
        assert_eq!(contexts[2].attempt(), 3);
        assert!(contexts[2].previous_passphrase_incorrect());

        let resolved = core
            .database
            .resolve_credential(summary.id().to_owned())
            .await
            .expect("resolve imported Credential");
        let (_, secret) = resolved.into_parts();
        let CredentialSecret::PrivateKey {
            private_key,
            passphrase,
        } = secret
        else {
            panic!("expected Private Key Credential");
        };
        assert_eq!(private_key.as_str(), encrypted_key.as_str());
        assert_eq!(
            passphrase.as_deref().map(String::as_str),
            Some("correct-passphrase")
        );
        let debug = format!(
            "{:?}",
            CredentialSecret::PrivateKey {
                private_key,
                passphrase,
            }
        );
        assert!(!debug.contains("correct-passphrase"));
    }

    #[tokio::test]
    async fn native_encrypted_private_key_import_cancellation_and_attempt_limit_do_not_store() {
        let (core, directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let encrypted_key = fixture_private_key()
            .encrypt(&mut rand::rng(), "correct-passphrase")
            .expect("encrypt fixture Private Key")
            .to_openssh(LineEnding::LF)
            .expect("encode encrypted fixture Private Key");
        let cancelled_path = directory.path().join("cancelled-key");
        std::fs::write(&cancelled_path, encrypted_key.as_bytes()).expect("write cancelled fixture");
        let cancelled = TestPrompt::new([TestPromptReply::Cancel]);

        assert!(
            core.import_private_key_credential_from_path_with_prompt(
                "\n".to_owned(),
                "fixture-user".to_owned(),
                cancelled_path,
                &cancelled,
            )
            .await
            .expect("cancel import")
            .is_none()
        );
        assert_eq!(
            cancelled.contexts()[0].label(),
            DEFAULT_PRIVATE_KEY_PROMPT_LABEL
        );

        let unavailable_path = directory.path().join("unavailable-key");
        std::fs::write(&unavailable_path, encrypted_key.as_bytes())
            .expect("write unavailable fixture");
        let unavailable = TestPrompt::new([TestPromptReply::Unavailable]);
        let error = core
            .import_private_key_credential_from_path_with_prompt(
                "Unavailable fixture".to_owned(),
                "fixture-user".to_owned(),
                unavailable_path,
                &unavailable,
            )
            .await
            .expect_err("unavailable prompt must fail");
        assert!(matches!(
            error,
            ApplicationError::PrivateKeyPrompt(PrivateKeyPromptError::Unavailable)
        ));

        let rejected_path = directory.path().join("rejected-key");
        std::fs::write(&rejected_path, encrypted_key.as_bytes()).expect("write rejected fixture");
        let rejected = TestPrompt::new([
            TestPromptReply::Passphrase("wrong-one"),
            TestPromptReply::Passphrase("wrong-two"),
            TestPromptReply::Passphrase("wrong-three"),
        ]);
        let error = core
            .import_private_key_credential_from_path_with_prompt(
                "Rejected fixture".to_owned(),
                "fixture-user".to_owned(),
                rejected_path,
                &rejected,
            )
            .await
            .expect_err("three incorrect passphrases must fail");
        assert!(matches!(
            error,
            ApplicationError::PrivateKeyImport(PrivateKeyImportError::PassphraseRejected)
        ));
        assert_eq!(rejected.contexts().len(), 3);
        assert!(
            core.list_credentials()
                .await
                .expect("list credentials after cancelled and rejected imports")
                .is_empty()
        );
        let error_text = error.to_string();
        for secret in [
            "wrong-one",
            "wrong-two",
            "wrong-three",
            "correct-passphrase",
        ] {
            assert!(!error_text.contains(secret));
        }
    }

    #[test]
    fn private_key_prompt_label_is_bounded_and_contains_no_control_characters() {
        assert_eq!(
            sanitize_private_key_prompt_label("  Encrypted\nfixture\t  "),
            "Encryptedfixture"
        );
        assert_eq!(
            sanitize_private_key_prompt_label("\n\t"),
            DEFAULT_PRIVATE_KEY_PROMPT_LABEL
        );
        assert_eq!(
            sanitize_private_key_prompt_label(&"x".repeat(MAX_PRIVATE_KEY_PROMPT_LABEL_CHARS + 5))
                .chars()
                .count(),
            MAX_PRIVATE_KEY_PROMPT_LABEL_CHARS
        );
    }

    #[test]
    fn generated_private_key_comment_is_bounded_and_contains_no_control_characters() {
        assert_eq!(
            sanitize_generated_private_key_comment("  Generated\nfixture\t  "),
            "Generatedfixture"
        );
        assert_eq!(sanitize_generated_private_key_comment("\n\t"), "");
        assert_eq!(
            sanitize_generated_private_key_comment(
                &"x".repeat(MAX_GENERATED_PRIVATE_KEY_COMMENT_CHARS + 5)
            )
            .len(),
            MAX_GENERATED_PRIVATE_KEY_COMMENT_CHARS
        );
    }

    #[test]
    fn native_private_key_reader_rejects_unsafe_file_shapes_without_leaking_paths() {
        let directory = tempdir().expect("tempdir");
        let empty_path = directory.path().join("empty-private-key");
        std::fs::write(&empty_path, "").expect("write empty fixture");
        assert!(matches!(
            read_private_key_file(&empty_path),
            Err(PrivateKeyImportError::InvalidSize)
        ));

        let invalid_utf8_path = directory.path().join("invalid-utf8-private-key");
        std::fs::write(&invalid_utf8_path, [0xff, 0xfe]).expect("write invalid UTF-8 fixture");
        assert!(matches!(
            read_private_key_file(&invalid_utf8_path),
            Err(PrivateKeyImportError::InvalidEncoding)
        ));

        let oversized_path = directory.path().join("oversized-private-key");
        let oversized = File::create(&oversized_path).expect("create oversized fixture");
        oversized
            .set_len(MAX_PRIVATE_KEY_BYTES as u64 + 1)
            .expect("extend oversized fixture");
        assert!(matches!(
            read_private_key_file(&oversized_path),
            Err(PrivateKeyImportError::InvalidSize)
        ));

        assert!(matches!(
            read_private_key_file(directory.path()),
            Err(PrivateKeyImportError::UnsupportedFileType)
        ));

        let missing_path = directory.path().join("secret-path-must-not-leak");
        let error = read_private_key_file(&missing_path).expect_err("missing file must fail");
        assert_eq!(error, PrivateKeyImportError::Unavailable);
        assert!(!error.to_string().contains("secret-path-must-not-leak"));
    }

    #[cfg(unix)]
    #[test]
    fn native_private_key_reader_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let directory = tempdir().expect("tempdir");
        let target = directory.path().join("target-key");
        std::fs::write(&target, "not-reached").expect("write symlink target");
        let link = directory.path().join("linked-key");
        symlink(target, &link).expect("create symlink fixture");

        assert!(matches!(
            read_private_key_file(&link),
            Err(PrivateKeyImportError::UnsupportedFileType)
        ));

        let socket_path = directory.path().join("socket-key");
        let _listener = UnixListener::bind(&socket_path).expect("create socket fixture");
        assert!(matches!(
            read_private_key_file(&socket_path),
            Err(PrivateKeyImportError::UnsupportedFileType)
        ));
    }

    #[tokio::test]
    async fn host_and_jump_route_keep_only_credential_references() {
        let (core, _directory) = test_core();
        core.create_vault(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let credential = core
            .create_password_credential(
                "Shared password".to_owned(),
                "shared-user".to_owned(),
                Zeroizing::new("shared-password-secret".to_owned()),
            )
            .await
            .expect("create credential");
        let jump_one = core
            .create_host(
                "Jump one".to_owned(),
                "jump-one.internal".to_owned(),
                22,
                Some(credential.id().to_owned()),
                None,
            )
            .await
            .expect("create first jump Host");
        let route_to_jump_two = core
            .create_jump_route(
                "Route to Jump two".to_owned(),
                vec![jump_one.id().to_owned()],
            )
            .await
            .expect("create route to second Jump");
        let jump_two = core
            .create_host(
                "Jump two".to_owned(),
                "jump-two.internal".to_owned(),
                22,
                Some(credential.id().to_owned()),
                Some(route_to_jump_two.id().to_owned()),
            )
            .await
            .expect("create second jump Host");
        let route_to_target = core
            .create_jump_route("Route to Target".to_owned(), vec![jump_two.id().to_owned()])
            .await
            .expect("create route to Target");
        let target = core
            .create_host(
                "Target".to_owned(),
                "target.internal".to_owned(),
                2222,
                Some(credential.id().to_owned()),
                Some(route_to_target.id().to_owned()),
            )
            .await
            .expect("create target Host");

        assert_eq!(target.credential_id(), Some(credential.id()));
        assert_eq!(target.jump_route_id(), Some(route_to_target.id()));
        assert_eq!(
            core.list_jump_routes().await.expect("list Jump Routes"),
            vec![route_to_jump_two, route_to_target]
        );
        let debug = format!(
            "{:?}",
            core.list_hosts().await.expect("list persisted Hosts")
        );
        assert!(!debug.contains("shared-user"));
        assert!(!debug.contains("shared-password-secret"));

        let config = core
            .resolve_saved_session_config(target.id().to_owned(), TerminalSize::default())
            .await
            .expect("resolve saved Host session");
        assert_eq!(config.target.endpoint.host, "target.internal");
        assert_eq!(
            config
                .jump_hosts
                .iter()
                .map(|host| host.endpoint.host.as_str())
                .collect::<Vec<_>>(),
            ["jump-one.internal", "jump-two.internal"]
        );
        let config_debug = format!("{config:?}");
        assert!(config_debug.contains("<redacted>"));
        assert!(!config_debug.contains("shared-password-secret"));
    }
}
