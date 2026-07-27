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

use crate::{LocalVault, StorageError, VaultPresence};

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
