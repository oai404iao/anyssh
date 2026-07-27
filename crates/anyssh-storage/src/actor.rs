use std::{
    fmt, io,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use anyssh_vault::PinKdfParameters;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

use crate::{
    CredentialSecret, CredentialSummary, HostSummary, JumpRouteSummary, LocalVault,
    ResolvedCredential, StorageError, VaultPresence,
    credential::{CredentialRecord, generate_credential_id},
    host::generate_host_id,
    jump_route::generate_jump_route_id,
};

pub const DEFAULT_DATABASE_COMMAND_QUEUE_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct DatabaseActorConfig {
    pub command_queue_capacity: usize,
    pub pin_kdf_parameters: PinKdfParameters,
}

impl DatabaseActorConfig {
    pub const fn phase0_default() -> Self {
        Self {
            command_queue_capacity: DEFAULT_DATABASE_COMMAND_QUEUE_CAPACITY,
            pin_kdf_parameters: PinKdfParameters::phase0_default(),
        }
    }
}

impl Default for DatabaseActorConfig {
    fn default() -> Self {
        Self::phase0_default()
    }
}

#[derive(Debug, Error)]
pub enum DatabaseActorStartError {
    #[error("database actor command queue capacity must be greater than zero")]
    InvalidCommandQueueCapacity,
    #[error("database actor thread could not be started")]
    Thread(#[source] io::Error),
}

#[derive(Debug, Error)]
pub enum DatabaseActorError {
    #[error("vault is already unlocked")]
    AlreadyUnlocked,
    #[error("vault is locked")]
    VaultLocked,
    #[error("database actor is unavailable")]
    Unavailable,
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultState {
    Uninitialized,
    Locked,
    Unlocked,
    Damaged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultStatus {
    state: VaultState,
    vault_id: Option<String>,
    cipher_version: Option<String>,
}

impl VaultStatus {
    pub const fn state(&self) -> VaultState {
        self.state
    }

    pub fn vault_id(&self) -> Option<&str> {
        self.vault_id.as_deref()
    }

    pub fn cipher_version(&self) -> Option<&str> {
        self.cipher_version.as_deref()
    }
}

#[derive(Clone)]
pub struct DatabaseActorHandle {
    inner: Arc<DatabaseActorInner>,
}

impl fmt::Debug for DatabaseActorHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let remaining_capacity = self
            .command_sender()
            .map(|commands| commands.capacity())
            .ok();
        formatter
            .debug_struct("DatabaseActorHandle")
            .field("remaining_capacity", &remaining_capacity)
            .finish_non_exhaustive()
    }
}

impl DatabaseActorHandle {
    pub fn spawn(
        root: PathBuf,
        config: DatabaseActorConfig,
    ) -> Result<Self, DatabaseActorStartError> {
        if config.command_queue_capacity == 0 {
            return Err(DatabaseActorStartError::InvalidCommandQueueCapacity);
        }

        let (commands, receiver) = mpsc::channel(config.command_queue_capacity);
        let thread = thread::Builder::new()
            .name("anyssh-database".to_owned())
            .spawn(move || run_database_actor(root, config.pin_kdf_parameters, receiver))
            .map_err(DatabaseActorStartError::Thread)?;

        Ok(Self {
            inner: Arc::new(DatabaseActorInner {
                commands: Mutex::new(Some(commands)),
                thread: Mutex::new(Some(thread)),
            }),
        })
    }

    pub async fn status(&self) -> Result<VaultStatus, DatabaseActorError> {
        self.request(|response| DatabaseCommand::Status { response })
            .await
    }

    pub async fn create(&self, pin: Zeroizing<String>) -> Result<VaultStatus, DatabaseActorError> {
        self.request(|response| DatabaseCommand::Create { pin, response })
            .await
    }

    pub async fn unlock(&self, pin: Zeroizing<String>) -> Result<VaultStatus, DatabaseActorError> {
        self.request(|response| DatabaseCommand::Unlock { pin, response })
            .await
    }

    pub async fn lock(&self) -> Result<VaultStatus, DatabaseActorError> {
        self.request(|response| DatabaseCommand::Lock { response })
            .await
    }

    pub async fn create_credential(
        &self,
        label: String,
        username: String,
        secret: CredentialSecret,
    ) -> Result<CredentialSummary, DatabaseActorError> {
        self.request(|response| DatabaseCommand::CreateCredential {
            label,
            username,
            secret,
            response,
        })
        .await
    }

    pub async fn update_credential(
        &self,
        id: String,
        label: String,
        username: String,
        secret: CredentialSecret,
    ) -> Result<CredentialSummary, DatabaseActorError> {
        self.request(|response| DatabaseCommand::UpdateCredential {
            id,
            label,
            username,
            secret,
            response,
        })
        .await
    }

    pub async fn list_credentials(&self) -> Result<Vec<CredentialSummary>, DatabaseActorError> {
        self.request(|response| DatabaseCommand::ListCredentials { response })
            .await
    }

    pub async fn delete_credential(&self, id: String) -> Result<bool, DatabaseActorError> {
        self.request(|response| DatabaseCommand::DeleteCredential { id, response })
            .await
    }

    pub async fn resolve_credential(
        &self,
        id: String,
    ) -> Result<ResolvedCredential, DatabaseActorError> {
        self.request(|response| DatabaseCommand::ResolveCredential { id, response })
            .await
    }

    pub async fn create_host(
        &self,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<HostSummary, DatabaseActorError> {
        self.request(|response| DatabaseCommand::CreateHost {
            display_name,
            host,
            port,
            credential_id,
            jump_route_id,
            response,
        })
        .await
    }

    pub async fn update_host(
        &self,
        id: String,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<HostSummary, DatabaseActorError> {
        self.request(|response| DatabaseCommand::UpdateHost {
            id,
            display_name,
            host,
            port,
            credential_id,
            jump_route_id,
            response,
        })
        .await
    }

    pub async fn list_hosts(&self) -> Result<Vec<HostSummary>, DatabaseActorError> {
        self.request(|response| DatabaseCommand::ListHosts { response })
            .await
    }

    pub async fn delete_host(&self, id: String) -> Result<bool, DatabaseActorError> {
        self.request(|response| DatabaseCommand::DeleteHost { id, response })
            .await
    }

    pub async fn create_jump_route(
        &self,
        label: String,
        host_ids: Vec<String>,
    ) -> Result<JumpRouteSummary, DatabaseActorError> {
        self.request(|response| DatabaseCommand::CreateJumpRoute {
            label,
            host_ids,
            response,
        })
        .await
    }

    pub async fn update_jump_route(
        &self,
        id: String,
        label: String,
        host_ids: Vec<String>,
    ) -> Result<JumpRouteSummary, DatabaseActorError> {
        self.request(|response| DatabaseCommand::UpdateJumpRoute {
            id,
            label,
            host_ids,
            response,
        })
        .await
    }

    pub async fn list_jump_routes(&self) -> Result<Vec<JumpRouteSummary>, DatabaseActorError> {
        self.request(|response| DatabaseCommand::ListJumpRoutes { response })
            .await
    }

    pub async fn delete_jump_route(&self, id: String) -> Result<bool, DatabaseActorError> {
        self.request(|response| DatabaseCommand::DeleteJumpRoute { id, response })
            .await
    }

    pub async fn shutdown(&self) -> Result<(), DatabaseActorError> {
        self.request(|response| DatabaseCommand::Shutdown { response })
            .await
    }

    async fn request<Response>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<Response, DatabaseActorError>>) -> DatabaseCommand,
    ) -> Result<Response, DatabaseActorError> {
        let (response, receiver) = oneshot::channel();
        self.command_sender()?
            .send(command(response))
            .await
            .map_err(|_| DatabaseActorError::Unavailable)?;
        receiver
            .await
            .map_err(|_| DatabaseActorError::Unavailable)?
    }

    fn command_sender(&self) -> Result<mpsc::Sender<DatabaseCommand>, DatabaseActorError> {
        self.inner
            .commands
            .lock()
            .map_err(|_| DatabaseActorError::Unavailable)?
            .as_ref()
            .cloned()
            .ok_or(DatabaseActorError::Unavailable)
    }
}

struct DatabaseActorInner {
    commands: Mutex<Option<mpsc::Sender<DatabaseCommand>>>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

impl Drop for DatabaseActorInner {
    fn drop(&mut self) {
        let commands = match self.commands.get_mut() {
            Ok(commands) => commands,
            Err(poisoned) => poisoned.into_inner(),
        };
        commands.take();

        let thread = match self.thread.get_mut() {
            Ok(thread) => thread,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(thread) = thread.take() {
            let _ = thread.join();
        }
    }
}

type VaultStatusResponse = oneshot::Sender<Result<VaultStatus, DatabaseActorError>>;
type CredentialSummaryResponse = oneshot::Sender<Result<CredentialSummary, DatabaseActorError>>;
type CredentialListResponse = oneshot::Sender<Result<Vec<CredentialSummary>, DatabaseActorError>>;
type CredentialDeleteResponse = oneshot::Sender<Result<bool, DatabaseActorError>>;
type ResolvedCredentialResponse = oneshot::Sender<Result<ResolvedCredential, DatabaseActorError>>;
type HostSummaryResponse = oneshot::Sender<Result<HostSummary, DatabaseActorError>>;
type HostListResponse = oneshot::Sender<Result<Vec<HostSummary>, DatabaseActorError>>;
type JumpRouteSummaryResponse = oneshot::Sender<Result<JumpRouteSummary, DatabaseActorError>>;
type JumpRouteListResponse = oneshot::Sender<Result<Vec<JumpRouteSummary>, DatabaseActorError>>;
type DeleteResponse = oneshot::Sender<Result<bool, DatabaseActorError>>;
type EmptyResponse = oneshot::Sender<Result<(), DatabaseActorError>>;

enum DatabaseCommand {
    Status {
        response: VaultStatusResponse,
    },
    Create {
        pin: Zeroizing<String>,
        response: VaultStatusResponse,
    },
    Unlock {
        pin: Zeroizing<String>,
        response: VaultStatusResponse,
    },
    Lock {
        response: VaultStatusResponse,
    },
    CreateCredential {
        label: String,
        username: String,
        secret: CredentialSecret,
        response: CredentialSummaryResponse,
    },
    UpdateCredential {
        id: String,
        label: String,
        username: String,
        secret: CredentialSecret,
        response: CredentialSummaryResponse,
    },
    ListCredentials {
        response: CredentialListResponse,
    },
    DeleteCredential {
        id: String,
        response: CredentialDeleteResponse,
    },
    ResolveCredential {
        id: String,
        response: ResolvedCredentialResponse,
    },
    CreateHost {
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
        response: HostSummaryResponse,
    },
    UpdateHost {
        id: String,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
        response: HostSummaryResponse,
    },
    ListHosts {
        response: HostListResponse,
    },
    DeleteHost {
        id: String,
        response: DeleteResponse,
    },
    CreateJumpRoute {
        label: String,
        host_ids: Vec<String>,
        response: JumpRouteSummaryResponse,
    },
    UpdateJumpRoute {
        id: String,
        label: String,
        host_ids: Vec<String>,
        response: JumpRouteSummaryResponse,
    },
    ListJumpRoutes {
        response: JumpRouteListResponse,
    },
    DeleteJumpRoute {
        id: String,
        response: DeleteResponse,
    },
    Shutdown {
        response: EmptyResponse,
    },
}

struct DatabaseActorState {
    root: PathBuf,
    pin_kdf_parameters: PinKdfParameters,
    unlocked: Option<LocalVault>,
}

impl DatabaseActorState {
    fn status(&self) -> VaultStatus {
        if let Some(vault) = self.unlocked.as_ref() {
            return VaultStatus {
                state: VaultState::Unlocked,
                vault_id: Some(vault.vault_id().to_owned()),
                cipher_version: Some(vault.cipher_version().to_owned()),
            };
        }

        let state = match LocalVault::presence(&self.root) {
            VaultPresence::Uninitialized => VaultState::Uninitialized,
            VaultPresence::Locked => VaultState::Locked,
            VaultPresence::Damaged => VaultState::Damaged,
        };
        VaultStatus {
            state,
            vault_id: None,
            cipher_version: None,
        }
    }

    fn create(&mut self, pin: Zeroizing<String>) -> Result<VaultStatus, DatabaseActorError> {
        if self.unlocked.is_some() {
            return Err(DatabaseActorError::AlreadyUnlocked);
        }

        let vault = LocalVault::create(&self.root, pin.as_str(), self.pin_kdf_parameters)?;
        let status = unlocked_status(&vault);
        self.unlocked = Some(vault);
        Ok(status)
    }

    fn unlock(&mut self, pin: Zeroizing<String>) -> Result<VaultStatus, DatabaseActorError> {
        if self.unlocked.is_some() {
            return Err(DatabaseActorError::AlreadyUnlocked);
        }

        let vault = LocalVault::unlock(&self.root, pin.as_str())?;
        let status = unlocked_status(&vault);
        self.unlocked = Some(vault);
        Ok(status)
    }

    fn lock(&mut self) -> VaultStatus {
        self.unlocked = None;
        self.status()
    }

    fn create_credential(
        &mut self,
        label: String,
        username: String,
        secret: CredentialSecret,
    ) -> Result<CredentialSummary, DatabaseActorError> {
        let id = generate_credential_id()?;
        let record = CredentialRecord::new(id, label, username, secret)?;
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .create_credential(&record)
            .map_err(DatabaseActorError::from)
    }

    fn update_credential(
        &mut self,
        id: String,
        label: String,
        username: String,
        secret: CredentialSecret,
    ) -> Result<CredentialSummary, DatabaseActorError> {
        let record = CredentialRecord::new(id, label, username, secret)?;
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .update_credential(&record)
            .map_err(DatabaseActorError::from)
    }

    fn list_credentials(&self) -> Result<Vec<CredentialSummary>, DatabaseActorError> {
        self.unlocked
            .as_ref()
            .ok_or(DatabaseActorError::VaultLocked)?
            .list_credentials()
            .map_err(DatabaseActorError::from)
    }

    fn delete_credential(&mut self, id: &str) -> Result<bool, DatabaseActorError> {
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .delete_credential(id)
            .map_err(DatabaseActorError::from)
    }

    fn resolve_credential(&self, id: &str) -> Result<ResolvedCredential, DatabaseActorError> {
        self.unlocked
            .as_ref()
            .ok_or(DatabaseActorError::VaultLocked)?
            .resolve_credential(id)
            .map_err(DatabaseActorError::from)
    }

    fn create_host(
        &mut self,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<HostSummary, DatabaseActorError> {
        let record = HostSummary::new(
            generate_host_id()?,
            display_name,
            host,
            port,
            credential_id,
            jump_route_id,
        )?;
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .create_host(&record)
            .map_err(DatabaseActorError::from)
    }

    fn update_host(
        &mut self,
        id: String,
        display_name: String,
        host: String,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<HostSummary, DatabaseActorError> {
        let record = HostSummary::new(id, display_name, host, port, credential_id, jump_route_id)?;
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .update_host(&record)
            .map_err(DatabaseActorError::from)
    }

    fn list_hosts(&self) -> Result<Vec<HostSummary>, DatabaseActorError> {
        self.unlocked
            .as_ref()
            .ok_or(DatabaseActorError::VaultLocked)?
            .list_hosts()
            .map_err(DatabaseActorError::from)
    }

    fn delete_host(&mut self, id: &str) -> Result<bool, DatabaseActorError> {
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .delete_host(id)
            .map_err(DatabaseActorError::from)
    }

    fn create_jump_route(
        &mut self,
        label: String,
        host_ids: Vec<String>,
    ) -> Result<JumpRouteSummary, DatabaseActorError> {
        let route = JumpRouteSummary::new(generate_jump_route_id()?, label, host_ids)?;
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .create_jump_route(&route)
            .map_err(DatabaseActorError::from)
    }

    fn update_jump_route(
        &mut self,
        id: String,
        label: String,
        host_ids: Vec<String>,
    ) -> Result<JumpRouteSummary, DatabaseActorError> {
        let route = JumpRouteSummary::new(id, label, host_ids)?;
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .update_jump_route(&route)
            .map_err(DatabaseActorError::from)
    }

    fn list_jump_routes(&self) -> Result<Vec<JumpRouteSummary>, DatabaseActorError> {
        self.unlocked
            .as_ref()
            .ok_or(DatabaseActorError::VaultLocked)?
            .list_jump_routes()
            .map_err(DatabaseActorError::from)
    }

    fn delete_jump_route(&mut self, id: &str) -> Result<bool, DatabaseActorError> {
        self.unlocked
            .as_mut()
            .ok_or(DatabaseActorError::VaultLocked)?
            .delete_jump_route(id)
            .map_err(DatabaseActorError::from)
    }
}

fn unlocked_status(vault: &LocalVault) -> VaultStatus {
    VaultStatus {
        state: VaultState::Unlocked,
        vault_id: Some(vault.vault_id().to_owned()),
        cipher_version: Some(vault.cipher_version().to_owned()),
    }
}

fn run_database_actor(
    root: PathBuf,
    pin_kdf_parameters: PinKdfParameters,
    mut commands: mpsc::Receiver<DatabaseCommand>,
) {
    let mut state = DatabaseActorState {
        root,
        pin_kdf_parameters,
        unlocked: None,
    };

    while let Some(command) = commands.blocking_recv() {
        match command {
            DatabaseCommand::Status { response } => {
                let _ = response.send(Ok(state.status()));
            }
            DatabaseCommand::Create { pin, response } => {
                let _ = response.send(state.create(pin));
            }
            DatabaseCommand::Unlock { pin, response } => {
                let _ = response.send(state.unlock(pin));
            }
            DatabaseCommand::Lock { response } => {
                let _ = response.send(Ok(state.lock()));
            }
            DatabaseCommand::CreateCredential {
                label,
                username,
                secret,
                response,
            } => {
                let _ = response.send(state.create_credential(label, username, secret));
            }
            DatabaseCommand::UpdateCredential {
                id,
                label,
                username,
                secret,
                response,
            } => {
                let _ = response.send(state.update_credential(id, label, username, secret));
            }
            DatabaseCommand::ListCredentials { response } => {
                let _ = response.send(state.list_credentials());
            }
            DatabaseCommand::DeleteCredential { id, response } => {
                let _ = response.send(state.delete_credential(&id));
            }
            DatabaseCommand::ResolveCredential { id, response } => {
                let _ = response.send(state.resolve_credential(&id));
            }
            DatabaseCommand::CreateHost {
                display_name,
                host,
                port,
                credential_id,
                jump_route_id,
                response,
            } => {
                let _ = response.send(state.create_host(
                    display_name,
                    host,
                    port,
                    credential_id,
                    jump_route_id,
                ));
            }
            DatabaseCommand::UpdateHost {
                id,
                display_name,
                host,
                port,
                credential_id,
                jump_route_id,
                response,
            } => {
                let _ = response.send(state.update_host(
                    id,
                    display_name,
                    host,
                    port,
                    credential_id,
                    jump_route_id,
                ));
            }
            DatabaseCommand::ListHosts { response } => {
                let _ = response.send(state.list_hosts());
            }
            DatabaseCommand::DeleteHost { id, response } => {
                let _ = response.send(state.delete_host(&id));
            }
            DatabaseCommand::CreateJumpRoute {
                label,
                host_ids,
                response,
            } => {
                let _ = response.send(state.create_jump_route(label, host_ids));
            }
            DatabaseCommand::UpdateJumpRoute {
                id,
                label,
                host_ids,
                response,
            } => {
                let _ = response.send(state.update_jump_route(id, label, host_ids));
            }
            DatabaseCommand::ListJumpRoutes { response } => {
                let _ = response.send(state.list_jump_routes());
            }
            DatabaseCommand::DeleteJumpRoute { id, response } => {
                let _ = response.send(state.delete_jump_route(&id));
            }
            DatabaseCommand::Shutdown { response } => {
                state.unlocked = None;
                let _ = response.send(Ok(()));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc as std_mpsc, time::Duration};

    use tempfile::tempdir;
    use tokio::time::timeout;

    use super::*;

    fn test_config(command_queue_capacity: usize) -> DatabaseActorConfig {
        DatabaseActorConfig {
            command_queue_capacity,
            pin_kdf_parameters: PinKdfParameters::new(8 * 1024, 1, 1).expect("test KDF parameters"),
        }
    }

    #[tokio::test]
    async fn actor_owns_and_serializes_the_vault_lifecycle() {
        let directory = tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let actor = DatabaseActorHandle::spawn(root.clone(), test_config(4)).expect("start actor");

        assert_eq!(
            actor.status().await.expect("initial status").state(),
            VaultState::Uninitialized
        );

        let created = actor
            .create(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        assert_eq!(created.state(), VaultState::Unlocked);
        assert!(created.vault_id().is_some());
        assert!(
            created
                .cipher_version()
                .expect("cipher version")
                .starts_with("4.")
        );

        assert!(matches!(
            actor.create(Zeroizing::new("123456".to_owned())).await,
            Err(DatabaseActorError::AlreadyUnlocked)
        ));

        assert_eq!(
            actor.lock().await.expect("lock vault").state(),
            VaultState::Locked
        );

        let wrong_pin = actor
            .unlock(Zeroizing::new("654321".to_owned()))
            .await
            .expect_err("wrong PIN must fail");
        assert!(!wrong_pin.to_string().contains("654321"));
        assert_eq!(
            actor.status().await.expect("locked status").state(),
            VaultState::Locked
        );

        assert_eq!(
            actor
                .unlock(Zeroizing::new("123456".to_owned()))
                .await
                .expect("unlock vault")
                .state(),
            VaultState::Unlocked
        );

        actor.shutdown().await.expect("shutdown actor");
        assert!(matches!(
            actor.status().await,
            Err(DatabaseActorError::Unavailable)
        ));
        assert_eq!(LocalVault::presence(&root), VaultPresence::Locked);
    }

    #[tokio::test]
    async fn actor_serializes_credential_repository_commands() {
        let directory = tempdir().expect("tempdir");
        let actor = DatabaseActorHandle::spawn(directory.path().join("vault"), test_config(8))
            .expect("start actor");

        assert!(matches!(
            actor.list_credentials().await,
            Err(DatabaseActorError::VaultLocked)
        ));

        actor
            .create(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let password_summary = actor
            .create_credential(
                "Password credential".to_owned(),
                "password-user".to_owned(),
                CredentialSecret::Password {
                    password: Zeroizing::new("password-secret".to_owned()),
                },
            )
            .await
            .expect("create password credential");
        let private_key_summary = actor
            .create_credential(
                "Private-key credential".to_owned(),
                "key-user".to_owned(),
                CredentialSecret::PrivateKey {
                    private_key: Zeroizing::new("private-key-secret".to_owned()),
                    passphrase: Some(Zeroizing::new("key-passphrase".to_owned())),
                },
            )
            .await
            .expect("create private-key credential");

        let summaries = actor.list_credentials().await.expect("list credentials");
        assert_eq!(summaries.len(), 2);
        let debug = format!("{summaries:?}");
        assert!(!debug.contains("password-secret"));
        assert!(!debug.contains("private-key-secret"));
        assert!(!debug.contains("key-passphrase"));

        let resolved = actor
            .resolve_credential(private_key_summary.id().to_owned())
            .await
            .expect("resolve private-key credential");
        let debug = format!("{resolved:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("private-key-secret"));
        assert!(!debug.contains("key-passphrase"));

        actor
            .update_credential(
                password_summary.id().to_owned(),
                "Updated password credential".to_owned(),
                "updated-user".to_owned(),
                CredentialSecret::Password {
                    password: Zeroizing::new("updated-password-secret".to_owned()),
                },
            )
            .await
            .expect("update password credential");
        let (username, secret) = actor
            .resolve_credential(password_summary.id().to_owned())
            .await
            .expect("resolve updated credential")
            .into_parts();
        assert_eq!(username, "updated-user");
        let CredentialSecret::Password { password } = secret else {
            panic!("expected password credential");
        };
        assert_eq!(password.as_str(), "updated-password-secret");

        assert!(
            actor
                .delete_credential(private_key_summary.id().to_owned())
                .await
                .expect("delete private-key credential")
        );
        assert!(matches!(
            actor
                .resolve_credential(private_key_summary.id().to_owned())
                .await,
            Err(DatabaseActorError::Storage(
                StorageError::CredentialNotFound
            ))
        ));

        actor.lock().await.expect("lock vault");
        assert!(matches!(
            actor
                .resolve_credential(password_summary.id().to_owned())
                .await,
            Err(DatabaseActorError::VaultLocked)
        ));
    }

    #[tokio::test]
    async fn actor_serializes_host_and_jump_route_repository_commands() {
        let directory = tempdir().expect("tempdir");
        let actor = DatabaseActorHandle::spawn(directory.path().join("vault"), test_config(8))
            .expect("start actor");

        assert!(matches!(
            actor.list_hosts().await,
            Err(DatabaseActorError::VaultLocked)
        ));
        assert!(matches!(
            actor.list_jump_routes().await,
            Err(DatabaseActorError::VaultLocked)
        ));

        actor
            .create(Zeroizing::new("123456".to_owned()))
            .await
            .expect("create vault");
        let credential = actor
            .create_credential(
                "Shared credential".to_owned(),
                "shared-user".to_owned(),
                CredentialSecret::Password {
                    password: Zeroizing::new("shared-secret".to_owned()),
                },
            )
            .await
            .expect("create shared credential");
        let jump = actor
            .create_host(
                "Jump".to_owned(),
                "jump.internal".to_owned(),
                22,
                Some(credential.id().to_owned()),
                None,
            )
            .await
            .expect("create jump Host");
        let target = actor
            .create_host(
                "Target".to_owned(),
                "target.internal".to_owned(),
                2222,
                Some(credential.id().to_owned()),
                None,
            )
            .await
            .expect("create target Host");
        let route = actor
            .create_jump_route("Production".to_owned(), vec![jump.id().to_owned()])
            .await
            .expect("create Jump Route");
        let target = actor
            .update_host(
                target.id().to_owned(),
                target.display_name().to_owned(),
                target.host().to_owned(),
                target.port(),
                target.credential_id().map(str::to_owned),
                Some(route.id().to_owned()),
            )
            .await
            .expect("attach Jump Route");

        let hosts = actor.list_hosts().await.expect("list Hosts");
        assert_eq!(hosts.len(), 2);
        assert_eq!(target.jump_route_id(), Some(route.id()));
        let routes = actor.list_jump_routes().await.expect("list Jump Routes");
        assert_eq!(routes, vec![route.clone()]);
        let debug = format!("{hosts:?} {routes:?}");
        assert!(!debug.contains("shared-secret"));
        assert!(!debug.contains("shared-user"));

        assert!(matches!(
            actor.delete_credential(credential.id().to_owned()).await,
            Err(DatabaseActorError::Storage(StorageError::CredentialInUse))
        ));
        assert!(matches!(
            actor.delete_host(jump.id().to_owned()).await,
            Err(DatabaseActorError::Storage(StorageError::HostInUse))
        ));
        assert!(matches!(
            actor.delete_jump_route(route.id().to_owned()).await,
            Err(DatabaseActorError::Storage(StorageError::JumpRouteInUse))
        ));

        actor.lock().await.expect("lock vault");
        assert!(matches!(
            actor.list_hosts().await,
            Err(DatabaseActorError::VaultLocked)
        ));
    }

    #[tokio::test]
    async fn bounded_command_queue_backpressures_callers() {
        let (commands, mut receiver) = mpsc::channel(1);
        let actor = DatabaseActorHandle {
            inner: Arc::new(DatabaseActorInner {
                commands: Mutex::new(Some(commands)),
                thread: Mutex::new(None),
            }),
        };
        let first_actor = actor.clone();
        let first = tokio::spawn(async move { first_actor.status().await });

        timeout(Duration::from_secs(1), async {
            while actor.command_sender().expect("command sender").capacity() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("first command should fill the queue");

        let second_actor = actor.clone();
        let second = tokio::spawn(async move { second_actor.status().await });
        tokio::task::yield_now().await;
        assert!(!second.is_finished());

        respond_with_uninitialized_status(receiver.recv().await.expect("first command"));
        respond_with_uninitialized_status(receiver.recv().await.expect("second command"));

        assert_eq!(
            first
                .await
                .expect("first task")
                .expect("first response")
                .state(),
            VaultState::Uninitialized
        );
        assert_eq!(
            second
                .await
                .expect("second task")
                .expect("second response")
                .state(),
            VaultState::Uninitialized
        );
    }

    #[test]
    fn zero_capacity_is_rejected_without_starting_a_thread() {
        let directory = tempdir().expect("tempdir");
        assert!(matches!(
            DatabaseActorHandle::spawn(directory.path().join("vault"), test_config(0)),
            Err(DatabaseActorStartError::InvalidCommandQueueCapacity)
        ));
    }

    #[test]
    fn dropping_the_last_handle_closes_the_queue_before_joining() {
        let directory = tempdir().expect("tempdir");
        let actor = DatabaseActorHandle::spawn(directory.path().join("vault"), test_config(1))
            .expect("start actor");
        let (completed, receiver) = std_mpsc::channel();

        thread::spawn(move || {
            drop(actor);
            let _ = completed.send(());
        });

        receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("dropping the last handle must not deadlock");
    }

    fn respond_with_uninitialized_status(command: DatabaseCommand) {
        let DatabaseCommand::Status { response } = command else {
            panic!("expected status command");
        };
        response
            .send(Ok(VaultStatus {
                state: VaultState::Uninitialized,
                vault_id: None,
                cipher_version: None,
            }))
            .expect("response receiver");
    }
}
