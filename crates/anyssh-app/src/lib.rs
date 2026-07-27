#![forbid(unsafe_code)]

use std::{fmt, path::PathBuf};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    DEFAULT_CONNECTION_TIMEOUT, HostKeyPolicy, SessionAuthentication, SpawnedSession,
    SshConnectionConfig, SshSessionConfig, spawn_session,
};
use anyssh_storage::{
    CredentialSecret, DatabaseActorError, DatabaseActorHandle, ResolvedCredential,
};
use thiserror::Error;
use zeroize::Zeroizing;

pub use anyssh_storage::{
    CredentialKind, CredentialSummary, DatabaseActorConfig, DatabaseActorStartError, VaultState,
    VaultStatus,
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
            jump_host,
            terminal_size: request.terminal_size,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
        }))
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
    #[error(transparent)]
    Database(#[from] DatabaseActorError),
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
}
