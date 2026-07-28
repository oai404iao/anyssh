#![forbid(unsafe_code)]

use std::{
    fmt,
    fs::{File, OpenOptions},
    future::Future,
    io::Read,
    path::{Path, PathBuf},
};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    DEFAULT_CONNECTION_TIMEOUT, HostKeyPolicy, MAX_PRIVATE_KEY_BYTES, PrivateKeyTextEncryption,
    SessionAuthentication, SpawnedSession, SshConnectionConfig, SshSessionConfig, SystemAgentError,
    SystemAgentIdentitySummary, inspect_openssh_private_key_text, list_system_agent_identities,
    spawn_session, validate_private_key_text,
};
use anyssh_storage::{
    CredentialSecret, DatabaseActorError, DatabaseActorHandle, ResolvedCredential,
    ResolvedHostConnection,
};
use thiserror::Error;
use zeroize::Zeroizing;

const _: () = assert!(anyssh_storage::MAX_JUMP_ROUTE_STEPS == anyssh_ssh::MAX_JUMP_HOSTS);
pub const PRIVATE_KEY_PASSPHRASE_MAX_ATTEMPTS: u8 = 3;
const MAX_PRIVATE_KEY_PROMPT_LABEL_CHARS: usize = 128;
const DEFAULT_PRIVATE_KEY_PROMPT_LABEL: &str = "Imported private key";

pub use anyssh_storage::{
    CredentialKind, CredentialSummary, DatabaseActorConfig, DatabaseActorStartError, GroupSummary,
    HostSummary, JumpRouteSummary, MAX_GROUP_DEPTH, Override, VaultState, VaultStatus,
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

#[derive(Clone)]
pub struct ApplicationCore {
    database: DatabaseActorHandle,
}

impl fmt::Debug for ApplicationCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCore")
            .field("database", &self.database)
            .finish_non_exhaustive()
    }
}

impl ApplicationCore {
    pub fn spawn(
        vault_root: PathBuf,
        config: DatabaseActorConfig,
    ) -> Result<Self, DatabaseActorStartError> {
        DatabaseActorHandle::spawn(vault_root, config).map(Self::new)
    }

    pub const fn new(database: DatabaseActorHandle) -> Self {
        Self { database }
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
            AuthenticationSource::Credential { credential_id } => resolved_authentication(
                self.database
                    .resolve_credential(credential_id)
                    .await
                    .map_err(ApplicationError::from)?,
            ),
        };

        Ok(SshConnectionConfig {
            endpoint,
            username,
            authentication,
            host_key_policy: HostKeyPolicy::Prompt,
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
    SystemAgent(#[from] SystemAgentError),
    #[error(transparent)]
    Database(#[from] DatabaseActorError),
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
    };
    (username, authentication)
}

fn resolved_host_connection(
    connection: ResolvedHostConnection,
) -> Result<SshConnectionConfig, ApplicationError> {
    let (_, host, port, credential) = connection.into_parts();
    let endpoint = SshEndpoint::new(host, port).map_err(|_| ApplicationError::InvalidStoredHost)?;
    let (username, authentication) = resolved_authentication(credential);
    Ok(SshConnectionConfig {
        endpoint,
        username,
        authentication,
        host_key_policy: HostKeyPolicy::Prompt,
    })
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
