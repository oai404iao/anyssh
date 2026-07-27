#![forbid(unsafe_code)]

use std::{
    fmt,
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    DEFAULT_CONNECTION_TIMEOUT, HostKeyPolicy, MAX_PRIVATE_KEY_BYTES, SessionAuthentication,
    SpawnedSession, SshConnectionConfig, SshSessionConfig, spawn_session,
    validate_private_key_text,
};
use anyssh_storage::{
    CredentialSecret, DatabaseActorError, DatabaseActorHandle, ResolvedCredential,
    ResolvedHostConnection,
};
use thiserror::Error;
use zeroize::Zeroizing;

const _: () = assert!(anyssh_storage::MAX_JUMP_ROUTE_STEPS == anyssh_ssh::MAX_JUMP_HOSTS);

pub use anyssh_storage::{
    CredentialKind, CredentialSummary, DatabaseActorConfig, DatabaseActorStartError, GroupSummary,
    HostSummary, JumpRouteSummary, MAX_GROUP_DEPTH, Override, VaultState, VaultStatus,
};

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
        let private_key = tokio::task::spawn_blocking(move || read_private_key_file(&path))
            .await
            .map_err(|_| PrivateKeyImportError::TaskFailed)??;
        self.store_private_key_credential(label, username, private_key, None)
            .await
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
    #[error("selected private key is invalid or encrypted")]
    InvalidKey,
    #[error("private key validation task failed")]
    TaskFailed,
}

fn read_private_key_file(path: &Path) -> Result<Zeroizing<String>, PrivateKeyImportError> {
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
    validate_private_key_text(private_key.as_str(), None)
        .map_err(|_| PrivateKeyImportError::InvalidKey)?;
    Ok(private_key)
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
    async fn native_private_key_file_import_rejects_invalid_and_encrypted_keys() {
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
            ApplicationError::PrivateKeyImport(PrivateKeyImportError::InvalidKey)
        ));
        assert!(
            core.list_credentials()
                .await
                .expect("list credentials after rejected imports")
                .is_empty()
        );
    }

    #[test]
    fn native_private_key_reader_rejects_unsafe_file_shapes_without_leaking_paths() {
        let directory = tempdir().expect("tempdir");
        let empty_path = directory.path().join("empty-private-key");
        std::fs::write(&empty_path, "").expect("write empty fixture");
        assert_eq!(
            read_private_key_file(&empty_path),
            Err(PrivateKeyImportError::InvalidSize)
        );

        let invalid_utf8_path = directory.path().join("invalid-utf8-private-key");
        std::fs::write(&invalid_utf8_path, [0xff, 0xfe]).expect("write invalid UTF-8 fixture");
        assert_eq!(
            read_private_key_file(&invalid_utf8_path),
            Err(PrivateKeyImportError::InvalidEncoding)
        );

        let oversized_path = directory.path().join("oversized-private-key");
        let oversized = File::create(&oversized_path).expect("create oversized fixture");
        oversized
            .set_len(MAX_PRIVATE_KEY_BYTES as u64 + 1)
            .expect("extend oversized fixture");
        assert_eq!(
            read_private_key_file(&oversized_path),
            Err(PrivateKeyImportError::InvalidSize)
        );

        assert_eq!(
            read_private_key_file(directory.path()),
            Err(PrivateKeyImportError::UnsupportedFileType)
        );

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

        assert_eq!(
            read_private_key_file(&link),
            Err(PrivateKeyImportError::UnsupportedFileType)
        );

        let socket_path = directory.path().join("socket-key");
        let _listener = UnixListener::bind(&socket_path).expect("create socket fixture");
        assert_eq!(
            read_private_key_file(&socket_path),
            Err(PrivateKeyImportError::UnsupportedFileType)
        );
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
