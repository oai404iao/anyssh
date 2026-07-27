#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyssh_app::{
    ApplicationCore, AuthenticationSource, CredentialKind as StorageCredentialKind,
    CredentialSummary as StorageCredentialSummary, DatabaseActorConfig,
    HostSummary as StorageHostSummary, JumpRouteSummary as StorageJumpRouteSummary, SshHopRequest,
    SshSessionRequest, VaultState as StorageVaultState, VaultStatus as StorageVaultStatus,
};
use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{SessionControl, SessionEvent, SessionHop};
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VaultPinRequest {
    pin: String,
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
}

impl From<StorageCredentialSummary> for CredentialSummary {
    fn from(summary: StorageCredentialSummary) -> Self {
        let kind = match summary.kind() {
            StorageCredentialKind::Password => CredentialKind::Password,
            StorageCredentialKind::PrivateKey => CredentialKind::PrivateKey,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostSummary {
    id: String,
    display_name: String,
    host: String,
    port: u16,
    credential_id: Option<String>,
    jump_route_id: Option<String>,
}

impl From<StorageHostSummary> for HostSummary {
    fn from(summary: StorageHostSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            display_name: summary.display_name().to_owned(),
            host: summary.host().to_owned(),
            port: summary.port(),
            credential_id: summary.credential_id().map(str::to_owned),
            jump_route_id: summary.jump_route_id().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateHostRequest {
    display_name: String,
    host: String,
    port: u16,
    credential_id: Option<String>,
    jump_route_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateHostRequest {
    host_id: String,
    display_name: String,
    host: String,
    port: u16,
    credential_id: Option<String>,
    jump_route_id: Option<String>,
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
) -> Result<VaultStatus, String> {
    for session in sessions.drain().await {
        let _ = session.disconnect().await;
    }
    core.lock_vault()
        .await
        .map(VaultStatus::from)
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
async fn host_create(
    request: CreateHostRequest,
    core: State<'_, ApplicationCore>,
) -> Result<HostSummary, String> {
    core.create_host(
        request.display_name,
        request.host,
        request.port,
        request.credential_id,
        request.jump_route_id,
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
    core.update_host(
        request.host_id,
        request.display_name,
        request.host,
        request.port,
        request.credential_id,
        request.jump_route_id,
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
            credential_create_password,
            credential_update_password,
            credential_list,
            credential_delete,
            host_create,
            host_update,
            host_list,
            host_delete,
            jump_route_create,
            jump_route_update,
            jump_route_list,
            jump_route_delete,
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
    }

    #[test]
    fn host_and_route_ipc_contain_references_only() {
        let host = serde_json::to_value(HostSummary {
            id: "host-target".to_owned(),
            display_name: "Target".to_owned(),
            host: "target.internal".to_owned(),
            port: 22,
            credential_id: Some("cred-target".to_owned()),
            jump_route_id: Some("route-production".to_owned()),
        })
        .expect("host summary should serialize");
        let route = serde_json::to_value(JumpRouteSummary {
            id: "route-production".to_owned(),
            label: "Production".to_owned(),
            host_ids: vec!["host-jump-one".to_owned(), "host-jump-two".to_owned()],
        })
        .expect("route summary should serialize");

        assert_eq!(host["credentialId"], "cred-target");
        assert_eq!(host["jumpRouteId"], "route-production");
        assert!(host.get("username").is_none());
        assert!(host.get("password").is_none());
        assert!(host.get("privateKey").is_none());
        assert_eq!(
            route["hostIds"],
            serde_json::json!(["host-jump-one", "host-jump-two"])
        );
        assert!(route.get("credentialId").is_none());
        assert!(route.get("password").is_none());
    }

    #[test]
    fn host_request_rejects_embedded_credentials() {
        let request = serde_json::from_value::<CreateHostRequest>(serde_json::json!({
            "displayName": "Target",
            "host": "target.internal",
            "port": 22,
            "credentialId": "cred-target",
            "password": "must-not-enter-host-ipc"
        }));

        assert!(request.is_err());

        let route = serde_json::from_value::<CreateJumpRouteRequest>(serde_json::json!({
            "label": "Production",
            "hostIds": ["host-jump"],
            "credentialId": "cred-must-not-be-copied"
        }));
        assert!(route.is_err());
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
