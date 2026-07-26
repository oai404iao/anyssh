#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{PasswordSessionConfig, SessionControl, SessionEvent, spawn_password_session};
use anyssh_storage::{LocalVault, VaultPresence};
use anyssh_vault::PinKdfParameters;
use serde::{Deserialize, Serialize};
use tauri::{
    Manager, State,
    ipc::{Channel, InvokeResponseBody},
};
use tokio::sync::RwLock;
use zeroize::Zeroizing;

#[derive(Clone, Default)]
struct SessionRegistry {
    sessions: Arc<RwLock<HashMap<String, SessionControl>>>,
    next_id: Arc<AtomicU64>,
}

#[derive(Clone)]
struct VaultManager {
    root: Arc<PathBuf>,
    unlocked: Arc<Mutex<Option<LocalVault>>>,
}

impl VaultManager {
    fn new(root: PathBuf) -> Self {
        Self {
            root: Arc::new(root),
            unlocked: Arc::new(Mutex::new(None)),
        }
    }

    fn status(&self) -> Result<VaultStatus, String> {
        let unlocked = self
            .unlocked
            .lock()
            .map_err(|_| "Vault state lock is unavailable".to_owned())?;
        if let Some(vault) = unlocked.as_ref() {
            return Ok(VaultStatus {
                state: VaultStatusKind::Unlocked,
                vault_id: Some(vault.vault_id().to_owned()),
                cipher_version: Some(vault.cipher_version().to_owned()),
            });
        }

        let state = match LocalVault::presence(&self.root) {
            VaultPresence::Uninitialized => VaultStatusKind::Uninitialized,
            VaultPresence::Locked => VaultStatusKind::Locked,
            VaultPresence::Damaged => VaultStatusKind::Damaged,
        };
        Ok(VaultStatus {
            state,
            vault_id: None,
            cipher_version: None,
        })
    }

    fn create(&self, pin: &str) -> Result<VaultStatus, String> {
        let mut unlocked = self
            .unlocked
            .lock()
            .map_err(|_| "Vault state lock is unavailable".to_owned())?;
        if unlocked.is_some() {
            return Err("Vault is already unlocked".to_owned());
        }

        let vault = LocalVault::create(&self.root, pin, PinKdfParameters::phase0_default())
            .map_err(|error| error.to_string())?;
        let status = VaultStatus {
            state: VaultStatusKind::Unlocked,
            vault_id: Some(vault.vault_id().to_owned()),
            cipher_version: Some(vault.cipher_version().to_owned()),
        };
        *unlocked = Some(vault);
        Ok(status)
    }

    fn unlock(&self, pin: &str) -> Result<VaultStatus, String> {
        let mut unlocked = self
            .unlocked
            .lock()
            .map_err(|_| "Vault state lock is unavailable".to_owned())?;
        if unlocked.is_some() {
            return Err("Vault is already unlocked".to_owned());
        }

        let vault = LocalVault::unlock(&self.root, pin).map_err(|error| error.to_string())?;
        let status = VaultStatus {
            state: VaultStatusKind::Unlocked,
            vault_id: Some(vault.vault_id().to_owned()),
            cipher_version: Some(vault.cipher_version().to_owned()),
        };
        *unlocked = Some(vault);
        Ok(status)
    }

    fn lock(&self) -> Result<VaultStatus, String> {
        let mut unlocked = self
            .unlocked
            .lock()
            .map_err(|_| "Vault state lock is unavailable".to_owned())?;
        *unlocked = None;
        drop(unlocked);
        self.status()
    }
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultPinRequest {
    pin: String,
}

impl SessionRegistry {
    fn new_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("ssh-{id}")
    }

    async fn insert(&self, id: String, control: SessionControl) {
        self.sessions.write().await.insert(id, control);
    }

    async fn get(&self, id: &str) -> Result<SessionControl, String> {
        self.sessions
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| format!("SSH session `{id}` was not found"))
    }

    async fn remove(&self, id: &str) {
        self.sessions.write().await.remove(id);
    }

    async fn drain(&self) -> Vec<SessionControl> {
        self.sessions
            .write()
            .await
            .drain()
            .map(|(_, control)| control)
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectRequest {
    host: String,
    port: u16,
    username: String,
    password: String,
    columns: u32,
    rows: u32,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClientEvent {
    Connecting,
    HostKey {
        algorithm: String,
        fingerprint_sha256: String,
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
fn vault_status(manager: State<'_, VaultManager>) -> Result<VaultStatus, String> {
    manager.status()
}

#[tauri::command]
async fn vault_create(
    request: VaultPinRequest,
    manager: State<'_, VaultManager>,
) -> Result<VaultStatus, String> {
    let manager = manager.inner().clone();
    let pin = Zeroizing::new(request.pin);
    tauri::async_runtime::spawn_blocking(move || manager.create(pin.as_str()))
        .await
        .map_err(|_| "Vault initialization task failed".to_owned())?
}

#[tauri::command]
async fn vault_unlock(
    request: VaultPinRequest,
    manager: State<'_, VaultManager>,
) -> Result<VaultStatus, String> {
    let manager = manager.inner().clone();
    let pin = Zeroizing::new(request.pin);
    tauri::async_runtime::spawn_blocking(move || manager.unlock(pin.as_str()))
        .await
        .map_err(|_| "Vault unlock task failed".to_owned())?
}

#[tauri::command]
async fn vault_lock(
    manager: State<'_, VaultManager>,
    sessions: State<'_, SessionRegistry>,
) -> Result<VaultStatus, String> {
    for session in sessions.drain().await {
        let _ = session.disconnect().await;
    }
    manager.lock()
}

#[tauri::command]
async fn ssh_connect(
    request: ConnectRequest,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: State<'_, SessionRegistry>,
) -> Result<String, String> {
    let endpoint =
        SshEndpoint::new(request.host, request.port).map_err(|error| error.to_string())?;
    let terminal_size =
        TerminalSize::new(request.columns, request.rows).map_err(|error| error.to_string())?;
    let username = request.username.trim().to_owned();

    if username.is_empty() {
        return Err("SSH username must not be empty".to_owned());
    }

    let spawned = spawn_password_session(PasswordSessionConfig {
        endpoint,
        username,
        password: Zeroizing::new(request.password),
        terminal_size,
    });

    let session_id = registry.new_id();
    registry.insert(session_id.clone(), spawned.control).await;

    let registry = registry.inner().clone();
    let event_session_id = session_id.clone();

    tauri::async_runtime::spawn(async move {
        let mut session_events = spawned.events;

        while let Some(event) = session_events.recv().await {
            let result = match event {
                SessionEvent::Connecting => events.send(ClientEvent::Connecting),
                SessionEvent::HostKey(info) => events.send(ClientEvent::HostKey {
                    algorithm: info.algorithm,
                    fingerprint_sha256: info.fingerprint_sha256,
                }),
                SessionEvent::Authenticated => events.send(ClientEvent::Authenticated),
                SessionEvent::Connected => events.send(ClientEvent::Connected),
                SessionEvent::Data(bytes) => data.send(InvokeResponseBody::Raw(bytes.to_vec())),
                SessionEvent::ExitStatus(code) => events.send(ClientEvent::ExitStatus { code }),
                SessionEvent::Error(message) => events.send(ClientEvent::Error { message }),
                SessionEvent::Closed => {
                    let sent = events.send(ClientEvent::Closed);
                    registry.remove(&event_session_id).await;
                    sent
                }
            };

            if result.is_err() {
                registry.remove(&event_session_id).await;
                break;
            }
        }
    });

    Ok(session_id)
}

#[tauri::command]
async fn ssh_confirm_host_key(
    session_id: String,
    accepted: bool,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    registry
        .get(&session_id)
        .await?
        .confirm_host_key(accepted)
        .await
        .map_err(|error| error.to_string())
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
    tauri::Builder::default()
        .manage(SessionRegistry::default())
        .setup(|app| {
            let vault_root = app.path().app_data_dir()?.join("vault");
            app.manage(VaultManager::new(vault_root));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runtime_info,
            vault_status,
            vault_create,
            vault_unlock,
            vault_lock,
            ssh_connect,
            ssh_confirm_host_key,
            ssh_send,
            ssh_resize,
            ssh_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("error while running AnySSH");
}
