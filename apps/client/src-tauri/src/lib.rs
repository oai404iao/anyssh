#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    DEFAULT_CONNECTION_TIMEOUT, JumpPasswordSessionConfig, PasswordJumpHostConfig,
    PasswordSessionConfig, SessionControl, SessionEvent, SessionHop, spawn_jump_password_session,
    spawn_password_session,
};
use anyssh_storage::{
    DatabaseActorConfig, DatabaseActorHandle, VaultState as StorageVaultState,
    VaultStatus as StorageVaultStatus,
};
use serde::{Deserialize, Serialize};
use tauri::{
    Manager, State,
    ipc::{Channel, InvokeResponseBody},
};
use tokio::sync::RwLock;
use zeroize::Zeroizing;

const OUTPUT_ACK_BUFFER: usize = 64;
const MAX_IN_FLIGHT_OUTPUT_CHUNKS: usize = 8;

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
#[serde(rename_all = "camelCase")]
struct VaultPinRequest {
    pin: String,
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

    async fn drain(&self) -> Vec<SessionControl> {
        self.sessions
            .write()
            .await
            .drain()
            .map(|(_, entry)| entry.control)
            .collect()
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
#[serde(rename_all = "camelCase")]
struct ConnectRequest {
    host: String,
    port: u16,
    username: String,
    password: String,
    columns: u32,
    rows: u32,
    jump_host: Option<JumpHostRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JumpHostRequest {
    host: String,
    port: u16,
    username: String,
    password: String,
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
async fn vault_status(actor: State<'_, DatabaseActorHandle>) -> Result<VaultStatus, String> {
    actor
        .status()
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn vault_create(
    request: VaultPinRequest,
    actor: State<'_, DatabaseActorHandle>,
) -> Result<VaultStatus, String> {
    let pin = Zeroizing::new(request.pin);
    actor
        .create(pin)
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn vault_unlock(
    request: VaultPinRequest,
    actor: State<'_, DatabaseActorHandle>,
) -> Result<VaultStatus, String> {
    let pin = Zeroizing::new(request.pin);
    actor
        .unlock(pin)
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn vault_lock(
    actor: State<'_, DatabaseActorHandle>,
    sessions: State<'_, SessionRegistry>,
) -> Result<VaultStatus, String> {
    for session in sessions.drain().await {
        let _ = session.disconnect().await;
    }
    actor
        .lock()
        .await
        .map(VaultStatus::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn ssh_connect(
    request: ConnectRequest,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: State<'_, SessionRegistry>,
) -> Result<String, String> {
    let ConnectRequest {
        host,
        port,
        username,
        password,
        columns,
        rows,
        jump_host,
    } = request;
    let password = Zeroizing::new(password);
    let jump_host = jump_host.map(|request| {
        let JumpHostRequest {
            host,
            port,
            username,
            password,
        } = request;
        (host, port, username, Zeroizing::new(password))
    });

    let endpoint = SshEndpoint::new(host, port).map_err(|error| error.to_string())?;
    let terminal_size = TerminalSize::new(columns, rows).map_err(|error| error.to_string())?;
    let username = username.trim().to_owned();

    if username.is_empty() {
        return Err("SSH username must not be empty".to_owned());
    }

    let target = PasswordSessionConfig {
        endpoint,
        username,
        password,
        terminal_size,
    };

    let spawned = if let Some((host, port, username, password)) = jump_host {
        let username = username.trim().to_owned();
        if username.is_empty() {
            return Err("Jump Host SSH username must not be empty".to_owned());
        }

        let endpoint = SshEndpoint::new(host, port).map_err(|error| error.to_string())?;
        spawn_jump_password_session(JumpPasswordSessionConfig {
            jump_host: PasswordJumpHostConfig {
                endpoint,
                username,
                password,
            },
            target,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
        })
    } else {
        spawn_password_session(target)
    };

    let session_id = registry.new_id();
    let (output_acknowledgements, mut output_acknowledgement_receiver) =
        tokio::sync::mpsc::channel(OUTPUT_ACK_BUFFER);
    registry
        .insert(session_id.clone(), spawned.control, output_acknowledgements)
        .await;

    let registry = registry.inner().clone();
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
                SessionEvent::Authenticated => events.send(ClientEvent::Authenticated),
                SessionEvent::Connected => events.send(ClientEvent::Connected),
                SessionEvent::Data(bytes) => {
                    if !await_output_capacity(
                        &mut output_acknowledgement_receiver,
                        &mut in_flight_output_chunks,
                    )
                    .await
                    {
                        registry.remove(&event_session_id).await;
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
    request_id: u64,
    accepted: bool,
    registry: State<'_, SessionRegistry>,
) -> Result<(), String> {
    registry
        .get(&session_id)
        .await?
        .confirm_host_key(request_id, accepted)
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
            let actor =
                DatabaseActorHandle::spawn(vault_root, DatabaseActorConfig::phase0_default())?;
            app.manage(actor);
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
            ssh_ack_output,
            ssh_send,
            ssh_resize,
            ssh_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("error while running AnySSH");
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn connect_request_accepts_an_optional_jump_host() {
        let request: ConnectRequest = serde_json::from_value(serde_json::json!({
            "host": "target.internal",
            "port": 22,
            "username": "target-user",
            "password": "target-password",
            "columns": 100,
            "rows": 30,
            "jumpHost": {
                "host": "jump.example",
                "port": 2222,
                "username": "jump-user",
                "password": "jump-password"
            }
        }))
        .expect("typed Jump Host request should deserialize");

        let jump_host = request.jump_host.expect("Jump Host should be present");
        assert_eq!(jump_host.host, "jump.example");
        assert_eq!(jump_host.port, 2222);
        assert_eq!(jump_host.username, "jump-user");
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
