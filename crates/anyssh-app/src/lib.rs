#![forbid(unsafe_code)]

use std::{fmt, path::PathBuf};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    DEFAULT_CONNECTION_TIMEOUT, HostKeyPolicy, SessionAuthentication, SpawnedSession,
    SshConnectionConfig, SshSessionConfig, spawn_session,
};
use anyssh_storage::{
    CredentialSecret, DatabaseActorError, DatabaseActorHandle, ResolvedCredential,
    ResolvedHostConnection,
};
use thiserror::Error;
use zeroize::Zeroizing;

const _: () = assert!(anyssh_storage::MAX_JUMP_ROUTE_STEPS == anyssh_ssh::MAX_JUMP_HOSTS);

pub use anyssh_storage::{
    CredentialKind, CredentialSummary, DatabaseActorConfig, DatabaseActorStartError, HostSummary,
    JumpRouteSummary, VaultState, VaultStatus,
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
