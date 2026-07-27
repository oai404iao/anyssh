#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyssh_app::{
    ApplicationCore, AuthenticationSource, CredentialKind as StorageCredentialKind,
    CredentialSummary as StorageCredentialSummary, DatabaseActorConfig,
    GroupSummary as StorageGroupSummary, HostSummary as StorageHostSummary,
    JumpRouteSummary as StorageJumpRouteSummary, Override as StorageOverride, SshHopRequest,
    SshSessionRequest, VaultState as StorageVaultState, VaultStatus as StorageVaultStatus,
};
use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    SessionControl, SessionEvent, SessionHop, SpawnedSession,
    SystemAgentIdentitySummary as SshSystemAgentIdentitySummary,
};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, State,
    ipc::{Channel, InvokeResponseBody},
};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::RwLock;
use zeroize::Zeroizing;

const OUTPUT_ACK_BUFFER: usize = 64;
const MAX_IN_FLIGHT_OUTPUT_CHUNKS: usize = 8;
const QA_VAULT_ROOT_ENVIRONMENT_VARIABLE: &str = "ANYSSH_QA_VAULT_ROOT";

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
    SystemAgent,
}

impl From<StorageCredentialSummary> for CredentialSummary {
    fn from(summary: StorageCredentialSummary) -> Self {
        let kind = match summary.kind() {
            StorageCredentialKind::Password => CredentialKind::Password,
            StorageCredentialKind::PrivateKey => CredentialKind::PrivateKey,
            StorageCredentialKind::SystemAgent => CredentialKind::SystemAgent,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportPrivateKeyCredentialRequest {
    label: String,
    username: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateSystemAgentCredentialRequest {
    label: String,
    username: String,
    identity_fingerprint_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemAgentIdentitySummary {
    algorithm: String,
    fingerprint_sha256: String,
    comment: String,
}

impl From<SshSystemAgentIdentitySummary> for SystemAgentIdentitySummary {
    fn from(identity: SshSystemAgentIdentitySummary) -> Self {
        Self {
            algorithm: identity.algorithm().to_owned(),
            fingerprint_sha256: identity.fingerprint_sha256().to_owned(),
            comment: identity.comment().to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum ReferenceOverride {
    Inherit,
    Set { value: String },
    Clear,
}

impl From<ReferenceOverride> for StorageOverride<String> {
    fn from(value: ReferenceOverride) -> Self {
        match value {
            ReferenceOverride::Inherit => Self::Inherit,
            ReferenceOverride::Set { value } => Self::Set(value),
            ReferenceOverride::Clear => Self::Clear,
        }
    }
}

impl From<&StorageOverride<String>> for ReferenceOverride {
    fn from(value: &StorageOverride<String>) -> Self {
        match value {
            StorageOverride::Inherit => Self::Inherit,
            StorageOverride::Set(value) => Self::Set {
                value: value.clone(),
            },
            StorageOverride::Clear => Self::Clear,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupSummary {
    id: String,
    label: String,
    parent_group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
    effective_credential_id: Option<String>,
    effective_jump_route_id: Option<String>,
}

impl From<StorageGroupSummary> for GroupSummary {
    fn from(summary: StorageGroupSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            label: summary.label().to_owned(),
            parent_group_id: summary.parent_group_id().map(str::to_owned),
            credential_override: summary.credential_override().into(),
            jump_route_override: summary.jump_route_override().into(),
            effective_credential_id: summary.effective_credential_id().map(str::to_owned),
            effective_jump_route_id: summary.effective_jump_route_id().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateGroupRequest {
    label: String,
    parent_group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateGroupRequest {
    group_id: String,
    label: String,
    parent_group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostSummary {
    id: String,
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
    effective_credential_id: Option<String>,
    effective_jump_route_id: Option<String>,
}

impl From<StorageHostSummary> for HostSummary {
    fn from(summary: StorageHostSummary) -> Self {
        Self {
            id: summary.id().to_owned(),
            display_name: summary.display_name().to_owned(),
            host: summary.host().to_owned(),
            port: summary.port(),
            group_id: summary.group_id().map(str::to_owned),
            credential_override: summary.credential_override().into(),
            jump_route_override: summary.jump_route_override().into(),
            effective_credential_id: summary.effective_credential_id().map(str::to_owned),
            effective_jump_route_id: summary.effective_jump_route_id().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateHostRequest {
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateHostRequest {
    host_id: String,
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: ReferenceOverride,
    jump_route_override: ReferenceOverride,
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
struct ConnectSavedHostRequest {
    host_id: String,
    columns: u32,
    rows: u32,
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
async fn credential_import_private_key(
    request: ImportPrivateKeyCredentialRequest,
    app: AppHandle,
    core: State<'_, ApplicationCore>,
) -> Result<Option<CredentialSummary>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Import SSH private key")
        .blocking_pick_file();
    let Some(path) = selected_private_key_path(selected)? else {
        return Ok(None);
    };

    core.import_private_key_credential_from_path(request.label, request.username, path)
        .await
        .map(CredentialSummary::from)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_list_system_agent_identities(
    core: State<'_, ApplicationCore>,
) -> Result<Vec<SystemAgentIdentitySummary>, String> {
    core.list_system_agent_identities()
        .await
        .map(|identities| {
            identities
                .into_iter()
                .map(SystemAgentIdentitySummary::from)
                .collect()
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn credential_create_system_agent(
    request: CreateSystemAgentCredentialRequest,
    core: State<'_, ApplicationCore>,
) -> Result<CredentialSummary, String> {
    core.create_system_agent_credential(
        request.label,
        request.username,
        request.identity_fingerprint_sha256,
    )
    .await
    .map(CredentialSummary::from)
    .map_err(|error| error.to_string())
}

fn selected_private_key_path(
    selected: Option<FilePath>,
) -> Result<Option<std::path::PathBuf>, String> {
    selected
        .map(|selected| {
            selected
                .into_path()
                .map_err(|_| "selected private key cannot be read on this platform".to_owned())
        })
        .transpose()
}

fn configured_vault_root(default: PathBuf) -> std::io::Result<PathBuf> {
    #[cfg(debug_assertions)]
    let qa_override = std::env::var_os(QA_VAULT_ROOT_ENVIRONMENT_VARIABLE).map(PathBuf::from);
    #[cfg(not(debug_assertions))]
    let qa_override = None;

    resolve_vault_root(default, qa_override)
}

fn resolve_vault_root(default: PathBuf, qa_override: Option<PathBuf>) -> std::io::Result<PathBuf> {
    let Some(qa_override) = qa_override else {
        return Ok(default);
    };
    if !qa_override.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{QA_VAULT_ROOT_ENVIRONMENT_VARIABLE} must be an absolute path"),
        ));
    }
    Ok(qa_override)
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
async fn group_create(
    request: CreateGroupRequest,
    core: State<'_, ApplicationCore>,
) -> Result<GroupSummary, String> {
    core.create_group(
        request.label,
        request.parent_group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
    )
    .await
    .map(GroupSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_update(
    request: UpdateGroupRequest,
    core: State<'_, ApplicationCore>,
) -> Result<GroupSummary, String> {
    core.update_group(
        request.group_id,
        request.label,
        request.parent_group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
    )
    .await
    .map(GroupSummary::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_list(core: State<'_, ApplicationCore>) -> Result<Vec<GroupSummary>, String> {
    core.list_groups()
        .await
        .map(|groups| groups.into_iter().map(GroupSummary::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn group_delete(group_id: String, core: State<'_, ApplicationCore>) -> Result<bool, String> {
    core.delete_group(group_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn host_create(
    request: CreateHostRequest,
    core: State<'_, ApplicationCore>,
) -> Result<HostSummary, String> {
    core.create_host_with_overrides(
        request.display_name,
        request.host,
        request.port,
        request.group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
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
    core.update_host_with_overrides(
        request.host_id,
        request.display_name,
        request.host,
        request.port,
        request.group_id,
        request.credential_override.into(),
        request.jump_route_override.into(),
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

    register_spawned_session(spawned, events, data, registry.inner()).await
}

#[tauri::command]
async fn ssh_connect_saved_host(
    request: ConnectSavedHostRequest,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: State<'_, SessionRegistry>,
    core: State<'_, ApplicationCore>,
) -> Result<String, String> {
    let terminal_size =
        TerminalSize::new(request.columns, request.rows).map_err(|error| error.to_string())?;
    let spawned = core
        .spawn_saved_host_session(request.host_id, terminal_size)
        .await
        .map_err(|error| error.to_string())?;

    register_spawned_session(spawned, events, data, registry.inner()).await
}

async fn register_spawned_session(
    spawned: SpawnedSession,
    events: Channel<ClientEvent>,
    data: Channel<InvokeResponseBody>,
    registry: &SessionRegistry,
) -> Result<String, String> {
    let session_id = registry.new_id();
    let (output_acknowledgements, mut output_acknowledgement_receiver) =
        tokio::sync::mpsc::channel(OUTPUT_ACK_BUFFER);
    registry
        .insert(session_id.clone(), spawned.control, output_acknowledgements)
        .await;

    let registry = registry.clone();
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
        .plugin(tauri_plugin_dialog::init())
        .manage(SessionRegistry::default())
        .setup(|app| {
            let vault_root = configured_vault_root(app.path().app_data_dir()?.join("vault"))?;
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
            credential_import_private_key,
            credential_list_system_agent_identities,
            credential_create_system_agent,
            credential_list,
            credential_delete,
            group_create,
            group_update,
            group_list,
            group_delete,
            host_create,
            host_update,
            host_list,
            host_delete,
            jump_route_create,
            jump_route_update,
            jump_route_list,
            jump_route_delete,
            ssh_connect,
            ssh_connect_saved_host,
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
    fn system_agent_ipc_contains_public_metadata_and_rejects_agent_endpoints() {
        let identity = serde_json::to_value(SystemAgentIdentitySummary {
            algorithm: "ssh-ed25519".to_owned(),
            fingerprint_sha256: "SHA256:agent-selector".to_owned(),
            comment: "workstation key".to_owned(),
        })
        .expect("agent identity should serialize");
        assert_eq!(identity["algorithm"], "ssh-ed25519");
        assert_eq!(identity["fingerprintSha256"], "SHA256:agent-selector");
        assert_eq!(identity["comment"], "workstation key");
        assert!(identity.get("publicKey").is_none());
        assert!(identity.get("signature").is_none());
        assert!(identity.get("socketPath").is_none());

        let request: CreateSystemAgentCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector"
            }))
            .expect("metadata-only agent request should deserialize");
        assert_eq!(request.label, "Workstation agent");
        assert_eq!(request.username, "alice");
        assert_eq!(request.identity_fingerprint_sha256, "SHA256:agent-selector");

        for extra in [
            serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector",
                "socketPath": "/tmp/ssh-agent.sock"
            }),
            serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector",
                "namedPipe": "\\\\.\\pipe\\openssh-ssh-agent"
            }),
            serde_json::json!({
                "label": "Workstation agent",
                "username": "alice",
                "identityFingerprintSha256": "SHA256:agent-selector",
                "publicKey": "must-not-enter-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<CreateSystemAgentCredentialRequest>(extra).is_err());
        }
    }

    #[test]
    fn private_key_import_request_rejects_paths_and_secrets() {
        let request: ImportPrivateKeyCredentialRequest =
            serde_json::from_value(serde_json::json!({
                "label": "Imported key",
                "username": "alice"
            }))
            .expect("metadata-only import request should deserialize");
        assert_eq!(request.label, "Imported key");
        assert_eq!(request.username, "alice");

        for extra in [
            serde_json::json!({
                "label": "Imported key",
                "username": "alice",
                "path": "/tmp/id_ed25519"
            }),
            serde_json::json!({
                "label": "Imported key",
                "username": "alice",
                "privateKey": "must-not-enter-ipc"
            }),
            serde_json::json!({
                "label": "Imported key",
                "username": "alice",
                "passphrase": "must-not-enter-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<ImportPrivateKeyCredentialRequest>(extra).is_err());
        }
    }

    #[test]
    fn private_key_picker_cancellation_returns_no_path() {
        assert_eq!(
            selected_private_key_path(None).expect("cancelled selection"),
            None
        );
        assert_eq!(
            selected_private_key_path(Some(FilePath::Path(std::path::PathBuf::from(
                "/tmp/id_ed25519"
            ),)))
            .expect("desktop selection"),
            Some(std::path::PathBuf::from("/tmp/id_ed25519"))
        );
    }

    #[test]
    fn qa_vault_root_override_requires_an_absolute_path() {
        let default = std::env::temp_dir().join("anyssh-default-vault");
        let absolute = std::env::temp_dir().join("anyssh-qa-vault");

        assert_eq!(
            resolve_vault_root(default.clone(), None).expect("default Vault root"),
            default
        );
        assert_eq!(
            resolve_vault_root(default.clone(), Some(absolute.clone()))
                .expect("absolute QA Vault root"),
            absolute
        );
        assert!(resolve_vault_root(default, Some(PathBuf::from("relative-vault"))).is_err());
    }

    #[test]
    fn host_and_route_ipc_contain_references_only() {
        let host = serde_json::to_value(HostSummary {
            id: "host-target".to_owned(),
            display_name: "Target".to_owned(),
            host: "target.internal".to_owned(),
            port: 22,
            group_id: Some("group-production".to_owned()),
            credential_override: ReferenceOverride::Inherit,
            jump_route_override: ReferenceOverride::Set {
                value: "route-production".to_owned(),
            },
            effective_credential_id: Some("cred-target".to_owned()),
            effective_jump_route_id: Some("route-production".to_owned()),
        })
        .expect("host summary should serialize");
        let group = serde_json::to_value(GroupSummary {
            id: "group-production".to_owned(),
            label: "Production".to_owned(),
            parent_group_id: None,
            credential_override: ReferenceOverride::Set {
                value: "cred-target".to_owned(),
            },
            jump_route_override: ReferenceOverride::Clear,
            effective_credential_id: Some("cred-target".to_owned()),
            effective_jump_route_id: None,
        })
        .expect("Group summary should serialize");
        let route = serde_json::to_value(JumpRouteSummary {
            id: "route-production".to_owned(),
            label: "Production".to_owned(),
            host_ids: vec!["host-jump-one".to_owned(), "host-jump-two".to_owned()],
        })
        .expect("route summary should serialize");

        assert_eq!(host["groupId"], "group-production");
        assert_eq!(host["credentialOverride"]["kind"], "inherit");
        assert_eq!(host["jumpRouteOverride"]["kind"], "set");
        assert_eq!(host["jumpRouteOverride"]["value"], "route-production");
        assert_eq!(host["effectiveCredentialId"], "cred-target");
        assert_eq!(host["effectiveJumpRouteId"], "route-production");
        assert!(host.get("username").is_none());
        assert!(host.get("password").is_none());
        assert!(host.get("privateKey").is_none());
        assert_eq!(group["credentialOverride"]["kind"], "set");
        assert_eq!(group["credentialOverride"]["value"], "cred-target");
        assert_eq!(group["jumpRouteOverride"]["kind"], "clear");
        assert_eq!(group["effectiveCredentialId"], "cred-target");
        assert!(group.get("password").is_none());
        assert_eq!(
            route["hostIds"],
            serde_json::json!(["host-jump-one", "host-jump-two"])
        );
        assert!(route.get("credentialId").is_none());
        assert!(route.get("password").is_none());
    }

    #[test]
    fn host_request_rejects_embedded_credentials() {
        let valid: CreateHostRequest = serde_json::from_value(serde_json::json!({
            "displayName": "Target",
            "host": "target.internal",
            "port": 22,
            "groupId": "group-production",
            "credentialOverride": {
                "kind": "inherit"
            },
            "jumpRouteOverride": {
                "kind": "clear"
            }
        }))
        .expect("three-state Host request should deserialize");
        assert_eq!(valid.group_id.as_deref(), Some("group-production"));
        assert!(matches!(
            valid.credential_override,
            ReferenceOverride::Inherit
        ));
        assert!(matches!(
            valid.jump_route_override,
            ReferenceOverride::Clear
        ));

        let request = serde_json::from_value::<CreateHostRequest>(serde_json::json!({
            "displayName": "Target",
            "host": "target.internal",
            "port": 22,
            "groupId": "group-production",
            "credentialOverride": {
                "kind": "set",
                "value": "cred-target"
            },
            "jumpRouteOverride": {
                "kind": "inherit"
            },
            "password": "must-not-enter-host-ipc"
        }));

        assert!(request.is_err());

        let route = serde_json::from_value::<CreateJumpRouteRequest>(serde_json::json!({
            "label": "Production",
            "hostIds": ["host-jump"],
            "credentialId": "cred-must-not-be-copied"
        }));
        assert!(route.is_err());

        let group = serde_json::from_value::<CreateGroupRequest>(serde_json::json!({
            "label": "Production",
            "parentGroupId": null,
            "credentialOverride": {
                "kind": "set",
                "value": "cred-target",
                "password": "must-not-enter-group-ipc"
            },
            "jumpRouteOverride": {
                "kind": "inherit"
            }
        }));
        assert!(group.is_err());
    }

    #[test]
    fn saved_host_connect_request_accepts_only_host_id_and_terminal_size() {
        let request: ConnectSavedHostRequest = serde_json::from_value(serde_json::json!({
            "hostId": "host-target",
            "columns": 120,
            "rows": 32
        }))
        .expect("saved Host request should deserialize");
        assert_eq!(request.host_id, "host-target");

        for extra in [
            serde_json::json!({
                "hostId": "host-target",
                "columns": 120,
                "rows": 32,
                "credentialId": "cred-target"
            }),
            serde_json::json!({
                "hostId": "host-target",
                "columns": 120,
                "rows": 32,
                "password": "must-not-enter-saved-host-ipc"
            }),
            serde_json::json!({
                "hostId": "host-target",
                "columns": 120,
                "rows": 32,
                "host": "must-not-enter-saved-host-ipc"
            }),
        ] {
            assert!(serde_json::from_value::<ConnectSavedHostRequest>(extra).is_err());
        }
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
