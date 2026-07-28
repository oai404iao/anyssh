#![forbid(unsafe_code)]

use std::{
    fmt,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyssh_domain::{SshEndpoint, TerminalSize};
use bytes::Bytes;
use russh::{
    ChannelMsg, Disconnect, MethodKind,
    client::{self, AuthResult, Config, Handle, Handler, KeyboardInteractiveAuthResponse},
    keys::{
        PrivateKeyWithHashAlg,
        agent::{
            AgentIdentity,
            client::{AgentClient, AgentStream},
        },
        decode_secret_key,
        ssh_key::{self, HashAlg},
    },
};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::{Mutex, Notify, mpsc},
};
use tracing::{debug, warn};
use zeroize::Zeroizing;

pub const SESSION_EVENT_BUFFER_CAPACITY: usize = 64;
pub const SESSION_COMMAND_BUFFER_CAPACITY: usize = 64;
const HOST_KEY_DECISION_BUFFER: usize = 4;
const HOST_KEY_DECISION_TIMEOUT: Duration = Duration::from_secs(60);
const AUTHENTICATION_RESPONSE_BUFFER: usize = 4;
const AUTHENTICATION_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);
pub const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_JUMP_HOSTS: usize = 32;
pub const MAX_SYSTEM_AGENT_IDENTITIES: usize = 64;
pub const MAX_TRUSTED_HOST_KEYS: usize = 16;
pub const MAX_HOST_PUBLIC_KEY_BYTES: usize = 64 * 1024;
const CHANNEL_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
const SYSTEM_AGENT_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_PRIVATE_KEY_BYTES: usize = 1024 * 1024;
pub const MAX_PRIVATE_KEY_PASSPHRASE_BYTES: usize = 64 * 1024;
pub const MAX_KEYBOARD_INTERACTIVE_ROUNDS: usize = 8;
pub const MAX_KEYBOARD_INTERACTIVE_PROMPTS: usize = 16;
pub const MAX_KEYBOARD_INTERACTIVE_NAME_BYTES: usize = 1024;
pub const MAX_KEYBOARD_INTERACTIVE_INSTRUCTIONS_BYTES: usize = 4 * 1024;
pub const MAX_KEYBOARD_INTERACTIVE_PROMPT_BYTES: usize = 1024;
pub const MAX_KEYBOARD_INTERACTIVE_RESPONSE_BYTES: usize = 16 * 1024;
pub const MAX_KEYBOARD_INTERACTIVE_RESPONSES_BYTES: usize = 64 * 1024;
const MAX_SYSTEM_AGENT_COMMENT_BYTES: usize = 512;
#[cfg(windows)]
const WINDOWS_OPENSSH_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

pub struct PasswordSessionConfig {
    pub endpoint: SshEndpoint,
    pub username: String,
    pub password: Zeroizing<String>,
    pub terminal_size: TerminalSize,
}

impl fmt::Debug for PasswordSessionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PasswordSessionConfig")
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("terminal_size", &self.terminal_size)
            .finish()
    }
}

pub struct PasswordJumpHostConfig {
    pub endpoint: SshEndpoint,
    pub username: String,
    pub password: Zeroizing<String>,
}

impl fmt::Debug for PasswordJumpHostConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PasswordJumpHostConfig")
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

pub struct PrivateKeySessionConfig {
    pub endpoint: SshEndpoint,
    pub username: String,
    pub private_key: Zeroizing<String>,
    pub passphrase: Option<Zeroizing<String>>,
    pub terminal_size: TerminalSize,
}

impl fmt::Debug for PrivateKeySessionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateKeySessionConfig")
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("private_key", &"<redacted>")
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "<redacted>"),
            )
            .field("terminal_size", &self.terminal_size)
            .finish()
    }
}

pub struct JumpPasswordSessionConfig {
    pub jump_host: PasswordJumpHostConfig,
    pub target: PasswordSessionConfig,
    pub connection_timeout: Duration,
}

impl fmt::Debug for JumpPasswordSessionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JumpPasswordSessionConfig")
            .field("jump_host", &self.jump_host)
            .field("target", &self.target)
            .field("connection_timeout", &self.connection_timeout)
            .finish()
    }
}

pub enum SessionAuthentication {
    Password {
        password: Zeroizing<String>,
    },
    PrivateKey {
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    },
    SystemAgent {
        identity_fingerprint_sha256: String,
    },
    KeyboardInteractive,
}

impl fmt::Debug for SessionAuthentication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password { .. } => formatter
                .debug_struct("Password")
                .field("password", &"<redacted>")
                .finish(),
            Self::PrivateKey { passphrase, .. } => formatter
                .debug_struct("PrivateKey")
                .field("private_key", &"<redacted>")
                .field("passphrase", &passphrase.as_ref().map(|_| "<redacted>"))
                .finish(),
            Self::SystemAgent { .. } => formatter
                .debug_struct("SystemAgent")
                .field("identity_fingerprint_sha256", &"<selected>")
                .finish(),
            Self::KeyboardInteractive => formatter
                .debug_struct("KeyboardInteractive")
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemAgentIdentitySummary {
    algorithm: String,
    fingerprint_sha256: String,
    comment: String,
}

impl SystemAgentIdentitySummary {
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SystemAgentError {
    #[error("System SSH Agent is unavailable")]
    Unavailable,
    #[error("System SSH Agent returned too many identities")]
    TooManyIdentities,
    #[error("System SSH Agent is unsupported on this platform")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyPolicy {
    Prompt,
    RequireSha256Set { fingerprints: Vec<String> },
}

pub struct SshConnectionConfig {
    pub endpoint: SshEndpoint,
    pub username: String,
    pub authentication: SessionAuthentication,
    pub host_key_policy: HostKeyPolicy,
}

impl fmt::Debug for SshConnectionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SshConnectionConfig")
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("authentication", &self.authentication)
            .field("host_key_policy", &self.host_key_policy)
            .finish()
    }
}

pub struct SshSessionConfig {
    pub target: SshConnectionConfig,
    pub jump_hosts: Vec<SshConnectionConfig>,
    pub terminal_size: TerminalSize,
    pub connection_timeout: Duration,
}

pub async fn list_system_agent_identities()
-> Result<Vec<SystemAgentIdentitySummary>, SystemAgentError> {
    let mut agent = tokio::time::timeout(SYSTEM_AGENT_OPERATION_TIMEOUT, connect_system_agent())
        .await
        .map_err(|_| SystemAgentError::Unavailable)??;
    let identities =
        tokio::time::timeout(SYSTEM_AGENT_OPERATION_TIMEOUT, agent.request_identities())
            .await
            .map_err(|_| SystemAgentError::Unavailable)?
            .map_err(|_| SystemAgentError::Unavailable)?;
    summarize_system_agent_identities(identities)
}

impl fmt::Debug for SshSessionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SshSessionConfig")
            .field("target", &self.target)
            .field("jump_hosts", &self.jump_hosts)
            .field("terminal_size", &self.terminal_size)
            .field("connection_timeout", &self.connection_timeout)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionHop {
    JumpHost { index: usize },
    Target,
}

impl fmt::Display for SessionHop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JumpHost { index } => write!(formatter, "jump host {index}"),
            Self::Target => formatter.write_str("target"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyInfo {
    pub request_id: u64,
    pub hop: SessionHop,
    pub endpoint: SshEndpoint,
    pub algorithm: String,
    pub fingerprint_sha256: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ObservedHostKey {
    request_id: u64,
    hop: SessionHop,
    endpoint: SshEndpoint,
    algorithm: String,
    fingerprint_sha256: String,
    public_key: Vec<u8>,
}

impl ObservedHostKey {
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    pub const fn hop(&self) -> &SessionHop {
        &self.hop
    }

    pub const fn endpoint(&self) -> &SshEndpoint {
        &self.endpoint
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }

    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    pub fn into_parts(self) -> (SshEndpoint, String, String, Vec<u8>) {
        (
            self.endpoint,
            self.algorithm,
            self.fingerprint_sha256,
            self.public_key,
        )
    }
}

impl fmt::Debug for ObservedHostKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObservedHostKey")
            .field("request_id", &self.request_id)
            .field("hop", &self.hop)
            .field("endpoint", &self.endpoint)
            .field("algorithm", &self.algorithm)
            .field("fingerprint_sha256", &self.fingerprint_sha256)
            .field("public_key", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyChangedInfo {
    pub hop: SessionHop,
    pub endpoint: SshEndpoint,
    pub algorithm: String,
    pub received_fingerprint_sha256: String,
    pub trusted_fingerprints_sha256: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthenticationPrompt {
    pub text: String,
    pub echo: bool,
}

impl fmt::Debug for AuthenticationPrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticationPrompt")
            .field("text", &"<server-provided>")
            .field("echo", &self.echo)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthenticationChallengeInfo {
    pub request_id: u64,
    pub hop: SessionHop,
    pub endpoint: SshEndpoint,
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<AuthenticationPrompt>,
}

impl fmt::Debug for AuthenticationChallengeInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticationChallengeInfo")
            .field("request_id", &self.request_id)
            .field("hop", &self.hop)
            .field("endpoint", &self.endpoint)
            .field("name", &"<server-provided>")
            .field("instructions", &"<server-provided>")
            .field("prompt_count", &self.prompts.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Connecting,
    HostKey(HostKeyInfo),
    HostKeyChanged(HostKeyChangedInfo),
    AuthenticationChallenge(AuthenticationChallengeInfo),
    Authenticated,
    Connected,
    Data(Bytes),
    ExitStatus(u32),
    Error(String),
    Closed,
}

#[derive(Debug)]
enum SessionCommand {
    Input(Bytes),
    Resize(TerminalSize),
}

#[derive(Debug)]
struct HostKeyDecision {
    request_id: u64,
    accepted: bool,
}

struct AuthenticationResponse {
    request_id: u64,
    responses: Option<Vec<Zeroizing<String>>>,
}

impl fmt::Debug for AuthenticationResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticationResponse")
            .field("request_id", &self.request_id)
            .field(
                "response_count",
                &self.responses.as_ref().map_or(0, Vec::len),
            )
            .field("responses", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Default)]
struct SessionCancellation {
    state: Arc<CancellationState>,
}

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl SessionCancellation {
    fn cancel(&self) {
        if !self.state.cancelled.swap(true, Ordering::AcqRel) {
            self.state.notify.notify_waiters();
        }
    }

    fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    async fn cancelled(&self) {
        loop {
            let notified = self.state.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

struct SessionControlInner {
    commands: mpsc::Sender<SessionCommand>,
    host_key_decisions: mpsc::Sender<HostKeyDecision>,
    authentication_responses: mpsc::Sender<AuthenticationResponse>,
    pending_host_key_request: Arc<AtomicU64>,
    pending_authentication_request: Arc<AtomicU64>,
    pending_observed_host_key: Arc<Mutex<Option<ObservedHostKey>>>,
    cancellation: SessionCancellation,
}

impl Drop for SessionControlInner {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

#[derive(Clone)]
pub struct SessionControl {
    inner: Arc<SessionControlInner>,
}

impl SessionControl {
    pub async fn observed_host_key(
        &self,
        request_id: u64,
    ) -> Result<ObservedHostKey, SessionControlError> {
        if self.inner.cancellation.is_cancelled()
            || self.inner.pending_host_key_request.load(Ordering::Acquire) != request_id
        {
            return Err(SessionControlError::HostKeyRequestExpired);
        }

        self.inner
            .pending_observed_host_key
            .lock()
            .await
            .as_ref()
            .filter(|observed| observed.request_id == request_id)
            .cloned()
            .ok_or(SessionControlError::HostKeyRequestExpired)
    }

    pub async fn confirm_host_key(
        &self,
        request_id: u64,
        accepted: bool,
    ) -> Result<(), SessionControlError> {
        if self.inner.cancellation.is_cancelled() {
            return Err(SessionControlError::SessionClosed);
        }

        self.inner
            .pending_host_key_request
            .compare_exchange(request_id, 0, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| SessionControlError::HostKeyRequestExpired)?;
        let mut observed = self.inner.pending_observed_host_key.lock().await;
        if observed
            .as_ref()
            .is_some_and(|observed| observed.request_id == request_id)
        {
            observed.take();
        }
        drop(observed);

        self.inner
            .host_key_decisions
            .send(HostKeyDecision {
                request_id,
                accepted,
            })
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn respond_authentication(
        &self,
        request_id: u64,
        responses: Option<Vec<Zeroizing<String>>>,
    ) -> Result<(), SessionControlError> {
        if self.inner.cancellation.is_cancelled() {
            return Err(SessionControlError::SessionClosed);
        }

        self.inner
            .pending_authentication_request
            .compare_exchange(request_id, 0, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| SessionControlError::AuthenticationRequestExpired)?;

        self.inner
            .authentication_responses
            .send(AuthenticationResponse {
                request_id,
                responses,
            })
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn send_input(&self, input: impl Into<Bytes>) -> Result<(), SessionControlError> {
        if self.inner.cancellation.is_cancelled() {
            return Err(SessionControlError::SessionClosed);
        }

        self.inner
            .commands
            .send(SessionCommand::Input(input.into()))
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn resize(&self, size: TerminalSize) -> Result<(), SessionControlError> {
        if self.inner.cancellation.is_cancelled() {
            return Err(SessionControlError::SessionClosed);
        }

        self.inner
            .commands
            .send(SessionCommand::Resize(size))
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn disconnect(&self) -> Result<(), SessionControlError> {
        if self.inner.commands.is_closed() {
            return Err(SessionControlError::SessionClosed);
        }

        self.inner.cancellation.cancel();
        Ok(())
    }
}

pub struct SpawnedSession {
    pub control: SessionControl,
    pub events: mpsc::Receiver<SessionEvent>,
}

#[derive(Clone)]
struct AuthenticationInteraction {
    events: mpsc::Sender<SessionEvent>,
    responses: Arc<Mutex<mpsc::Receiver<AuthenticationResponse>>>,
    pending_request: Arc<AtomicU64>,
    next_request: Arc<AtomicU64>,
    response_timeout: Duration,
    cancellation: SessionCancellation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateKeyTextEncryption {
    Unencrypted,
    Encrypted,
}

pub fn inspect_openssh_private_key_text(
    private_key: &str,
) -> Result<PrivateKeyTextEncryption, PrivateKeyValidationError> {
    if private_key.is_empty() || private_key.len() > MAX_PRIVATE_KEY_BYTES {
        return Err(PrivateKeyValidationError);
    }

    let private_key = ssh_key::PrivateKey::from_openssh(private_key.as_bytes())
        .map_err(|_| PrivateKeyValidationError)?;
    if private_key.is_encrypted() {
        Ok(PrivateKeyTextEncryption::Encrypted)
    } else {
        Ok(PrivateKeyTextEncryption::Unencrypted)
    }
}

pub fn validate_private_key_text(
    private_key: &str,
    passphrase: Option<&str>,
) -> Result<(), PrivateKeyValidationError> {
    if private_key.is_empty()
        || private_key.len() > MAX_PRIVATE_KEY_BYTES
        || passphrase.is_some_and(|value| value.len() > MAX_PRIVATE_KEY_PASSPHRASE_BYTES)
    {
        return Err(PrivateKeyValidationError);
    }

    decode_secret_key(private_key, passphrase)
        .map(|_| ())
        .map_err(|_| PrivateKeyValidationError)
}

pub fn spawn_password_session(config: PasswordSessionConfig) -> SpawnedSession {
    let PasswordSessionConfig {
        endpoint,
        username,
        password,
        terminal_size,
    } = config;
    spawn_session(SshSessionConfig {
        target: SshConnectionConfig {
            endpoint,
            username,
            authentication: SessionAuthentication::Password { password },
            host_key_policy: HostKeyPolicy::Prompt,
        },
        jump_hosts: Vec::new(),
        terminal_size,
        connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
    })
}

pub fn spawn_private_key_session(config: PrivateKeySessionConfig) -> SpawnedSession {
    let PrivateKeySessionConfig {
        endpoint,
        username,
        private_key,
        passphrase,
        terminal_size,
    } = config;
    spawn_session(SshSessionConfig {
        target: SshConnectionConfig {
            endpoint,
            username,
            authentication: SessionAuthentication::PrivateKey {
                private_key,
                passphrase,
            },
            host_key_policy: HostKeyPolicy::Prompt,
        },
        jump_hosts: Vec::new(),
        terminal_size,
        connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
    })
}

pub fn spawn_jump_password_session(config: JumpPasswordSessionConfig) -> SpawnedSession {
    let JumpPasswordSessionConfig {
        jump_host,
        target,
        connection_timeout,
    } = config;
    let PasswordJumpHostConfig {
        endpoint: jump_endpoint,
        username: jump_username,
        password: jump_password,
    } = jump_host;
    let PasswordSessionConfig {
        endpoint: target_endpoint,
        username: target_username,
        password: target_password,
        terminal_size,
    } = target;

    spawn_session(SshSessionConfig {
        target: SshConnectionConfig {
            endpoint: target_endpoint,
            username: target_username,
            authentication: SessionAuthentication::Password {
                password: target_password,
            },
            host_key_policy: HostKeyPolicy::Prompt,
        },
        jump_hosts: vec![SshConnectionConfig {
            endpoint: jump_endpoint,
            username: jump_username,
            authentication: SessionAuthentication::Password {
                password: jump_password,
            },
            host_key_policy: HostKeyPolicy::Prompt,
        }],
        terminal_size,
        connection_timeout,
    })
}

pub fn spawn_session(config: SshSessionConfig) -> SpawnedSession {
    let (event_sender, event_receiver) = mpsc::channel(SESSION_EVENT_BUFFER_CAPACITY);
    let (command_sender, command_receiver) = mpsc::channel(SESSION_COMMAND_BUFFER_CAPACITY);
    let (host_key_sender, host_key_receiver) = mpsc::channel(HOST_KEY_DECISION_BUFFER);
    let (authentication_sender, authentication_receiver) =
        mpsc::channel(AUTHENTICATION_RESPONSE_BUFFER);
    let cancellation = SessionCancellation::default();
    let pending_host_key_request = Arc::new(AtomicU64::new(0));
    let pending_authentication_request = Arc::new(AtomicU64::new(0));
    let pending_observed_host_key = Arc::new(Mutex::new(None));
    let next_host_key_request = Arc::new(AtomicU64::new(1));
    let authentication_interaction = AuthenticationInteraction {
        events: event_sender.clone(),
        responses: Arc::new(Mutex::new(authentication_receiver)),
        pending_request: pending_authentication_request.clone(),
        next_request: Arc::new(AtomicU64::new(1)),
        response_timeout: AUTHENTICATION_RESPONSE_TIMEOUT,
        cancellation: cancellation.clone(),
    };

    let control = SessionControl {
        inner: Arc::new(SessionControlInner {
            commands: command_sender,
            host_key_decisions: host_key_sender,
            authentication_responses: authentication_sender,
            pending_host_key_request: pending_host_key_request.clone(),
            pending_authentication_request,
            pending_observed_host_key: pending_observed_host_key.clone(),
            cancellation: cancellation.clone(),
        }),
    };

    tokio::spawn(run_session(
        config,
        event_sender,
        command_receiver,
        Arc::new(Mutex::new(host_key_receiver)),
        pending_host_key_request,
        pending_observed_host_key,
        next_host_key_request,
        authentication_interaction,
        cancellation,
    ));

    SpawnedSession {
        control,
        events: event_receiver,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_session(
    config: SshSessionConfig,
    events: mpsc::Sender<SessionEvent>,
    mut commands: mpsc::Receiver<SessionCommand>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
    pending_observed_host_key: Arc<Mutex<Option<ObservedHostKey>>>,
    next_host_key_request: Arc<AtomicU64>,
    authentication_interaction: AuthenticationInteraction,
    cancellation: SessionCancellation,
) {
    if emit_event(&events, &cancellation, SessionEvent::Connecting)
        .await
        .is_err()
    {
        return;
    }

    let result = connect_and_run(
        config,
        &events,
        &mut commands,
        host_key_decisions,
        pending_host_key_request,
        pending_observed_host_key,
        next_host_key_request,
        &authentication_interaction,
        &cancellation,
    )
    .await;

    match result {
        Err(SessionError::HostKeyChanged {
            hop,
            host,
            port,
            algorithm,
            trusted_fingerprints_sha256,
            received_fingerprint_sha256,
        }) => {
            let _ = events
                .send(SessionEvent::HostKeyChanged(HostKeyChangedInfo {
                    hop,
                    endpoint: SshEndpoint { host, port },
                    algorithm,
                    received_fingerprint_sha256,
                    trusted_fingerprints_sha256,
                }))
                .await;
        }
        Err(SessionError::Cancelled) | Ok(()) => {}
        Err(error) => {
            let _ = events.send(SessionEvent::Error(error.to_string())).await;
        }
    }

    let _ = events.send(SessionEvent::Closed).await;
}

#[allow(clippy::too_many_arguments)]
async fn connect_and_run(
    config: SshSessionConfig,
    events: &mpsc::Sender<SessionEvent>,
    commands: &mut mpsc::Receiver<SessionCommand>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
    pending_observed_host_key: Arc<Mutex<Option<ObservedHostKey>>>,
    next_host_key_request: Arc<AtomicU64>,
    authentication_interaction: &AuthenticationInteraction,
    cancellation: &SessionCancellation,
) -> Result<(), SessionError> {
    let SshSessionConfig {
        target,
        jump_hosts,
        terminal_size,
        connection_timeout,
    } = config;

    if connection_timeout.is_zero() {
        return Err(SessionError::InvalidConnectionTimeout);
    }

    if jump_hosts.len() > MAX_JUMP_HOSTS {
        return Err(SessionError::TooManyJumpHosts {
            count: jump_hosts.len(),
            maximum: MAX_JUMP_HOSTS,
        });
    }

    let client_config = make_client_config();
    if jump_hosts.is_empty() {
        let target_session = connect_tcp_endpoint(
            &target.endpoint,
            SessionHop::Target,
            target.host_key_policy.clone(),
            client_config,
            events,
            host_key_decisions,
            pending_host_key_request,
            pending_observed_host_key,
            next_host_key_request,
            cancellation,
            connection_timeout,
        )
        .await?;

        return run_target_session(
            target_session,
            target,
            terminal_size,
            events,
            commands,
            authentication_interaction,
            cancellation,
            connection_timeout,
        )
        .await;
    }

    let mut jump_sessions: Vec<Handle<ClientHandler>> = Vec::with_capacity(jump_hosts.len());
    let result = async {
        for (offset, jump_host) in jump_hosts.into_iter().enumerate() {
            let index = offset + 1;
            let jump_hop = SessionHop::JumpHost { index };
            let SshConnectionConfig {
                endpoint,
                username,
                authentication,
                host_key_policy,
            } = jump_host;

            let mut jump_session = if offset == 0 {
                connect_tcp_endpoint(
                    &endpoint,
                    jump_hop.clone(),
                    host_key_policy,
                    client_config.clone(),
                    events,
                    host_key_decisions.clone(),
                    pending_host_key_request.clone(),
                    pending_observed_host_key.clone(),
                    next_host_key_request.clone(),
                    cancellation,
                    connection_timeout,
                )
                .await?
            } else {
                let parent_hop = SessionHop::JumpHost { index: offset };
                let parent_session = jump_sessions
                    .last_mut()
                    .ok_or(SessionError::InvalidJumpRoute)?;
                let channel = await_ssh_operation(
                    parent_session.channel_open_direct_tcpip(
                        endpoint.host.clone(),
                        u32::from(endpoint.port),
                        "127.0.0.1",
                        0,
                    ),
                    cancellation,
                    connection_timeout,
                    parent_hop,
                    "direct-tcpip channel",
                )
                .await?;

                connect_stream_endpoint(
                    channel.into_stream(),
                    &endpoint,
                    jump_hop.clone(),
                    host_key_policy,
                    client_config.clone(),
                    events,
                    host_key_decisions.clone(),
                    pending_host_key_request.clone(),
                    pending_observed_host_key.clone(),
                    next_host_key_request.clone(),
                    cancellation,
                    connection_timeout,
                )
                .await?
            };

            if let Err(error) = authenticate(
                &mut jump_session,
                &jump_hop,
                &username,
                authentication,
                &endpoint,
                authentication_interaction,
                cancellation,
                connection_timeout,
            )
            .await
            {
                let _ = jump_session
                    .disconnect(Disconnect::ByApplication, "", "en")
                    .await;
                return Err(error);
            }

            debug!(
                %jump_hop,
                host = %endpoint.host,
                port = endpoint.port,
                "jump host authenticated"
            );
            jump_sessions.push(jump_session);
        }

        let final_jump_index = jump_sessions.len();
        let final_jump_hop = SessionHop::JumpHost {
            index: final_jump_index,
        };
        let final_jump_session = jump_sessions
            .last_mut()
            .ok_or(SessionError::InvalidJumpRoute)?;
        let target_channel = await_ssh_operation(
            final_jump_session.channel_open_direct_tcpip(
                target.endpoint.host.clone(),
                u32::from(target.endpoint.port),
                "127.0.0.1",
                0,
            ),
            cancellation,
            connection_timeout,
            final_jump_hop,
            "direct-tcpip channel",
        )
        .await?;

        let target_session = connect_stream_endpoint(
            target_channel.into_stream(),
            &target.endpoint,
            SessionHop::Target,
            target.host_key_policy.clone(),
            client_config,
            events,
            host_key_decisions,
            pending_host_key_request,
            pending_observed_host_key,
            next_host_key_request,
            cancellation,
            connection_timeout,
        )
        .await?;

        run_target_session(
            target_session,
            target,
            terminal_size,
            events,
            commands,
            authentication_interaction,
            cancellation,
            connection_timeout,
        )
        .await
    }
    .await;

    for jump_session in jump_sessions.into_iter().rev() {
        let _ = jump_session
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }

    result
}

#[allow(clippy::too_many_arguments)]
async fn connect_tcp_endpoint(
    endpoint: &SshEndpoint,
    hop: SessionHop,
    host_key_policy: HostKeyPolicy,
    client_config: Arc<Config>,
    events: &mpsc::Sender<SessionEvent>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
    pending_observed_host_key: Arc<Mutex<Option<ObservedHostKey>>>,
    next_host_key_request: Arc<AtomicU64>,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<Handle<ClientHandler>, SessionError> {
    debug!(
        %hop,
        host = %endpoint.host,
        port = endpoint.port,
        "connecting SSH transport"
    );

    let socket = await_ssh_operation(
        async {
            TcpStream::connect((endpoint.host.as_str(), endpoint.port))
                .await
                .map_err(russh::Error::from)
        },
        cancellation,
        connection_timeout,
        hop.clone(),
        "TCP connection",
    )
    .await?;

    if client_config.nodelay
        && let Err(error) = socket.set_nodelay(true)
    {
        warn!(%hop, "set_nodelay failed: {error}");
    }

    connect_stream_endpoint(
        socket,
        endpoint,
        hop,
        host_key_policy,
        client_config,
        events,
        host_key_decisions,
        pending_host_key_request,
        pending_observed_host_key,
        next_host_key_request,
        cancellation,
        connection_timeout,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn connect_stream_endpoint<R>(
    stream: R,
    endpoint: &SshEndpoint,
    hop: SessionHop,
    host_key_policy: HostKeyPolicy,
    client_config: Arc<Config>,
    events: &mpsc::Sender<SessionEvent>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
    pending_observed_host_key: Arc<Mutex<Option<ObservedHostKey>>>,
    next_host_key_request: Arc<AtomicU64>,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<Handle<ClientHandler>, SessionError>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let host_key_mismatch = Arc::new(Mutex::new(None));
    let handler = ClientHandler {
        events: events.clone(),
        host_key_decisions,
        pending_host_key_request,
        pending_observed_host_key,
        next_host_key_request,
        cancellation: cancellation.clone(),
        hop: hop.clone(),
        endpoint: endpoint.clone(),
        host_key_policy,
        host_key_mismatch: host_key_mismatch.clone(),
        accepted_fingerprint: None,
    };

    let result = await_ssh_operation(
        client::connect_stream(client_config, stream, handler),
        cancellation,
        connection_timeout,
        hop.clone(),
        "SSH handshake",
    )
    .await;

    match result {
        Ok(session) => Ok(session),
        Err(error) => {
            if let Some(mismatch) = host_key_mismatch.lock().await.take() {
                Err(SessionError::HostKeyChanged {
                    hop,
                    host: endpoint.host.clone(),
                    port: endpoint.port,
                    algorithm: mismatch.algorithm,
                    trusted_fingerprints_sha256: mismatch.trusted_fingerprints_sha256,
                    received_fingerprint_sha256: mismatch.received_fingerprint_sha256,
                })
            } else {
                Err(error)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_target_session(
    mut session: Handle<ClientHandler>,
    config: SshConnectionConfig,
    terminal_size: TerminalSize,
    events: &mpsc::Sender<SessionEvent>,
    commands: &mut mpsc::Receiver<SessionCommand>,
    authentication_interaction: &AuthenticationInteraction,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<(), SessionError> {
    let SshConnectionConfig {
        endpoint,
        username,
        authentication,
        ..
    } = config;

    let result = async {
        authenticate(
            &mut session,
            &SessionHop::Target,
            &username,
            authentication,
            &endpoint,
            authentication_interaction,
            cancellation,
            connection_timeout,
        )
        .await?;

        emit_event(events, cancellation, SessionEvent::Authenticated).await?;

        let mut channel = await_ssh_operation(
            session.channel_open_session(),
            cancellation,
            connection_timeout,
            SessionHop::Target,
            "session channel",
        )
        .await?;

        await_ssh_operation(
            channel.request_pty(
                true,
                "xterm-256color",
                terminal_size.columns,
                terminal_size.rows,
                0,
                0,
                &[],
            ),
            cancellation,
            connection_timeout,
            SessionHop::Target,
            "PTY request",
        )
        .await?;
        await_ssh_operation(
            channel.request_shell(true),
            cancellation,
            connection_timeout,
            SessionHop::Target,
            "shell request",
        )
        .await?;

        emit_event(events, cancellation, SessionEvent::Connected).await?;

        run_shell_loop(
            &mut channel,
            events,
            commands,
            cancellation,
            connection_timeout,
        )
        .await
    }
    .await;

    let _ = session
        .disconnect(Disconnect::ByApplication, "", "en")
        .await;

    result
}

#[allow(clippy::too_many_arguments)]
async fn authenticate(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    username: &str,
    authentication: SessionAuthentication,
    endpoint: &SshEndpoint,
    interaction: &AuthenticationInteraction,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<(), SessionError> {
    if username.trim().is_empty() {
        return Err(SessionError::InvalidUsername { hop: hop.clone() });
    }

    let outcome = match authentication {
        SessionAuthentication::Password { password } => {
            authenticate_password(
                session,
                hop,
                username,
                password,
                cancellation,
                connection_timeout,
            )
            .await
        }
        SessionAuthentication::PrivateKey {
            private_key,
            passphrase,
        } => {
            authenticate_private_key(
                session,
                hop,
                username,
                private_key,
                passphrase,
                cancellation,
                connection_timeout,
            )
            .await
        }
        SessionAuthentication::SystemAgent {
            identity_fingerprint_sha256,
        } => {
            authenticate_system_agent(
                session,
                hop,
                username,
                identity_fingerprint_sha256,
                cancellation,
                connection_timeout,
            )
            .await
        }
        SessionAuthentication::KeyboardInteractive => {
            return authenticate_keyboard_interactive(
                session,
                hop,
                endpoint,
                username,
                interaction,
                cancellation,
                connection_timeout,
            )
            .await;
        }
    }?;

    match outcome {
        PrimaryAuthenticationOutcome::Success => Ok(()),
        PrimaryAuthenticationOutcome::ContinueKeyboardInteractive => {
            authenticate_keyboard_interactive(
                session,
                hop,
                endpoint,
                username,
                interaction,
                cancellation,
                connection_timeout,
            )
            .await
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimaryAuthenticationOutcome {
    Success,
    ContinueKeyboardInteractive,
}

fn classify_primary_authentication(
    authentication: AuthResult,
    hop: &SessionHop,
) -> Result<PrimaryAuthenticationOutcome, SessionError> {
    match authentication {
        AuthResult::Success => Ok(PrimaryAuthenticationOutcome::Success),
        AuthResult::Failure {
            remaining_methods,
            partial_success: true,
        } if remaining_methods.contains(&MethodKind::KeyboardInteractive) => {
            Ok(PrimaryAuthenticationOutcome::ContinueKeyboardInteractive)
        }
        AuthResult::Failure { .. } => Err(SessionError::AuthenticationFailed { hop: hop.clone() }),
    }
}

async fn authenticate_password(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    username: &str,
    password: Zeroizing<String>,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<PrimaryAuthenticationOutcome, SessionError> {
    let authentication = await_ssh_operation(
        session.authenticate_password(username, password.as_str()),
        cancellation,
        connection_timeout,
        hop.clone(),
        "password authentication",
    )
    .await?;

    classify_primary_authentication(authentication, hop)
}

#[allow(clippy::too_many_arguments)]
async fn authenticate_keyboard_interactive(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    endpoint: &SshEndpoint,
    username: &str,
    interaction: &AuthenticationInteraction,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<(), SessionError> {
    let mut response = await_ssh_operation(
        session.authenticate_keyboard_interactive_start(username, None),
        cancellation,
        connection_timeout,
        hop.clone(),
        "keyboard-interactive authentication",
    )
    .await?;
    let mut rounds = 0usize;

    loop {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(()),
            KeyboardInteractiveAuthResponse::Failure { .. } => {
                return Err(SessionError::KeyboardInteractiveAuthenticationFailed {
                    hop: hop.clone(),
                });
            }
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                rounds += 1;
                if rounds > MAX_KEYBOARD_INTERACTIVE_ROUNDS {
                    return Err(SessionError::AuthenticationChallengeRoundsExceeded {
                        hop: hop.clone(),
                        maximum: MAX_KEYBOARD_INTERACTIVE_ROUNDS,
                    });
                }

                let (name, instructions, prompts) =
                    sanitize_authentication_challenge(name, instructions, prompts, hop)?;
                let responses = if prompts.is_empty() {
                    Vec::new()
                } else {
                    interaction
                        .request(hop, endpoint, name, instructions, prompts)
                        .await?
                };

                response = await_ssh_operation(
                    session.authenticate_keyboard_interactive_respond(responses),
                    cancellation,
                    connection_timeout,
                    hop.clone(),
                    "keyboard-interactive response",
                )
                .await?;
            }
        }
    }
}

fn sanitize_authentication_challenge(
    name: String,
    instructions: String,
    prompts: Vec<russh::client::Prompt>,
    hop: &SessionHop,
) -> Result<(String, String, Vec<AuthenticationPrompt>), SessionError> {
    if prompts.len() > MAX_KEYBOARD_INTERACTIVE_PROMPTS {
        return Err(SessionError::AuthenticationChallengeTooLarge { hop: hop.clone() });
    }

    let name =
        sanitize_authentication_text(&name, MAX_KEYBOARD_INTERACTIVE_NAME_BYTES, false, hop)?;
    let instructions = sanitize_authentication_text(
        &instructions,
        MAX_KEYBOARD_INTERACTIVE_INSTRUCTIONS_BYTES,
        true,
        hop,
    )?;
    let prompts = prompts
        .into_iter()
        .map(|prompt| {
            sanitize_authentication_text(
                &prompt.prompt,
                MAX_KEYBOARD_INTERACTIVE_PROMPT_BYTES,
                false,
                hop,
            )
            .map(|text| AuthenticationPrompt {
                text,
                echo: prompt.echo,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((name, instructions, prompts))
}

fn sanitize_authentication_text(
    value: &str,
    maximum_bytes: usize,
    allow_newlines: bool,
    hop: &SessionHop,
) -> Result<String, SessionError> {
    if value.len() > maximum_bytes {
        return Err(SessionError::AuthenticationChallengeTooLarge { hop: hop.clone() });
    }

    Ok(value
        .chars()
        .map(|character| {
            if allow_newlines && character == '\n' {
                '\n'
            } else if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
async fn authenticate_private_key(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    username: &str,
    private_key: Zeroizing<String>,
    passphrase: Option<Zeroizing<String>>,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<PrimaryAuthenticationOutcome, SessionError> {
    if private_key.is_empty()
        || private_key.len() > MAX_PRIVATE_KEY_BYTES
        || passphrase
            .as_ref()
            .is_some_and(|value| value.len() > MAX_PRIVATE_KEY_PASSPHRASE_BYTES)
    {
        return Err(SessionError::InvalidPrivateKey { hop: hop.clone() });
    }

    let decode_task = tokio::task::spawn_blocking(move || {
        decode_secret_key(
            private_key.as_str(),
            passphrase.as_ref().map(|value| value.as_str()),
        )
    });
    let decoded_key = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(SessionError::Cancelled),
        result = decode_task => {
            result
                .map_err(|_| SessionError::PrivateKeyTaskFailed { hop: hop.clone() })?
                .map_err(|source| SessionError::PrivateKeyDecode {
                    hop: hop.clone(),
                    source,
                })?
        }
    };

    let hash_alg = if decoded_key.algorithm().is_rsa() {
        await_ssh_operation(
            session.best_supported_rsa_hash(),
            cancellation,
            connection_timeout,
            hop.clone(),
            "RSA signature negotiation",
        )
        .await?
        .flatten()
    } else {
        None
    };

    let authentication = await_ssh_operation(
        session.authenticate_publickey(
            username,
            PrivateKeyWithHashAlg::new(Arc::new(decoded_key), hash_alg),
        ),
        cancellation,
        connection_timeout,
        hop.clone(),
        "private-key authentication",
    )
    .await?;

    classify_primary_authentication(authentication, hop)
}

async fn authenticate_system_agent(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    username: &str,
    identity_fingerprint_sha256: String,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<PrimaryAuthenticationOutcome, SessionError> {
    if identity_fingerprint_sha256.trim().is_empty() {
        return Err(SessionError::SystemAgentIdentityUnavailable { hop: hop.clone() });
    }

    let mut agent = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(SessionError::Cancelled),
        result = tokio::time::timeout(connection_timeout, connect_system_agent()) => {
            result
                .map_err(|_| SessionError::OperationTimedOut {
                    hop: hop.clone(),
                    operation: "system SSH Agent connection",
                })?
                .map_err(|error| match error {
                    SystemAgentError::Unsupported => SessionError::SystemAgentUnsupported {
                        hop: hop.clone(),
                    },
                    SystemAgentError::Unavailable | SystemAgentError::TooManyIdentities => {
                        SessionError::SystemAgentUnavailable { hop: hop.clone() }
                    }
                })?
        }
    };

    let identities = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(SessionError::Cancelled),
        result = tokio::time::timeout(connection_timeout, agent.request_identities()) => {
            result
                .map_err(|_| SessionError::OperationTimedOut {
                    hop: hop.clone(),
                    operation: "system SSH Agent identity request",
                })?
                .map_err(|_| SessionError::SystemAgentUnavailable { hop: hop.clone() })?
        }
    };
    if identities.len() > MAX_SYSTEM_AGENT_IDENTITIES {
        return Err(SessionError::SystemAgentTooManyIdentities { hop: hop.clone() });
    }

    let selected_key = identities.into_iter().find_map(|identity| match identity {
        AgentIdentity::PublicKey { key, .. }
            if key.fingerprint(HashAlg::Sha256).to_string() == identity_fingerprint_sha256 =>
        {
            Some(key)
        }
        AgentIdentity::PublicKey { .. } | AgentIdentity::Certificate { .. } => None,
    });
    let selected_key = selected_key
        .ok_or_else(|| SessionError::SystemAgentIdentityUnavailable { hop: hop.clone() })?;

    let hash_alg = if selected_key.algorithm().is_rsa() {
        await_ssh_operation(
            session.best_supported_rsa_hash(),
            cancellation,
            connection_timeout,
            hop.clone(),
            "RSA signature negotiation",
        )
        .await?
        .flatten()
    } else {
        None
    };

    let authentication = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(SessionError::Cancelled),
        result = tokio::time::timeout(
            connection_timeout,
            session.authenticate_publickey_with(
                username,
                selected_key,
                hash_alg,
                &mut agent,
            ),
        ) => {
            result
                .map_err(|_| SessionError::OperationTimedOut {
                    hop: hop.clone(),
                    operation: "system SSH Agent authentication",
                })?
                .map_err(|_| SessionError::SystemAgentSigningFailed { hop: hop.clone() })?
        }
    };

    classify_primary_authentication(authentication, hop)
}

type DynamicSystemAgent = AgentClient<Box<dyn AgentStream + Send + Unpin>>;

#[cfg(all(unix, not(any(target_os = "android", target_os = "ios"))))]
async fn connect_system_agent() -> Result<DynamicSystemAgent, SystemAgentError> {
    AgentClient::connect_env()
        .await
        .map(AgentClient::dynamic)
        .map_err(|_| SystemAgentError::Unavailable)
}

#[cfg(windows)]
async fn connect_system_agent() -> Result<DynamicSystemAgent, SystemAgentError> {
    AgentClient::connect_named_pipe(WINDOWS_OPENSSH_AGENT_PIPE)
        .await
        .map(AgentClient::dynamic)
        .map_err(|_| SystemAgentError::Unavailable)
}

#[cfg(not(any(windows, all(unix, not(any(target_os = "android", target_os = "ios"))))))]
async fn connect_system_agent() -> Result<DynamicSystemAgent, SystemAgentError> {
    Err(SystemAgentError::Unsupported)
}

fn summarize_system_agent_identities(
    identities: Vec<AgentIdentity>,
) -> Result<Vec<SystemAgentIdentitySummary>, SystemAgentError> {
    if identities.len() > MAX_SYSTEM_AGENT_IDENTITIES {
        return Err(SystemAgentError::TooManyIdentities);
    }

    Ok(identities
        .into_iter()
        .filter_map(|identity| match identity {
            AgentIdentity::PublicKey { key, comment } => Some(SystemAgentIdentitySummary {
                algorithm: key.algorithm().to_string(),
                fingerprint_sha256: key.fingerprint(HashAlg::Sha256).to_string(),
                comment: sanitize_system_agent_comment(&comment),
            }),
            AgentIdentity::Certificate { .. } => None,
        })
        .collect())
}

fn sanitize_system_agent_comment(comment: &str) -> String {
    let comment = comment.trim();
    if comment.chars().any(char::is_control) {
        return String::new();
    }
    if comment.len() <= MAX_SYSTEM_AGENT_COMMENT_BYTES {
        return comment.to_owned();
    }

    let mut end = MAX_SYSTEM_AGENT_COMMENT_BYTES;
    while !comment.is_char_boundary(end) {
        end -= 1;
    }
    comment[..end].to_owned()
}

async fn run_shell_loop(
    channel: &mut russh::Channel<russh::client::Msg>,
    events: &mpsc::Sender<SessionEvent>,
    commands: &mut mpsc::Receiver<SessionCommand>,
    cancellation: &SessionCancellation,
    operation_timeout: Duration,
) -> Result<(), SessionError> {
    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                close_channel(channel).await;
                break;
            }
            command = commands.recv() => {
                match command {
                    Some(SessionCommand::Input(data)) => {
                        await_ssh_operation(
                            channel.data_bytes(data),
                            cancellation,
                            operation_timeout,
                            SessionHop::Target,
                            "terminal input",
                        ).await?;
                    }
                    Some(SessionCommand::Resize(size)) => {
                        await_ssh_operation(
                            channel.window_change(size.columns, size.rows, 0, 0),
                            cancellation,
                            operation_timeout,
                            SessionHop::Target,
                            "terminal resize",
                        ).await?;
                    }
                    None => {
                        close_channel(channel).await;
                        break;
                    }
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) |
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        emit_event(events, cancellation, SessionEvent::Data(data)).await?;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        emit_event(
                            events,
                            cancellation,
                            SessionEvent::ExitStatus(exit_status),
                        ).await?;
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    Some(ChannelMsg::Failure) => {
                        warn!("SSH channel request failed");
                    }
                    Some(_) => {}
                }
            }
        }
    }

    Ok(())
}

async fn close_channel(channel: &russh::Channel<russh::client::Msg>) {
    let _ = tokio::time::timeout(CHANNEL_CLOSE_TIMEOUT, channel.eof()).await;
    let _ = tokio::time::timeout(CHANNEL_CLOSE_TIMEOUT, channel.close()).await;
}

fn make_client_config() -> Arc<Config> {
    Arc::new(Config {
        inactivity_timeout: Some(DEFAULT_CONNECTION_TIMEOUT),
        nodelay: true,
        ..Default::default()
    })
}

async fn await_ssh_operation<T, F>(
    future: F,
    cancellation: &SessionCancellation,
    operation_timeout: Duration,
    hop: SessionHop,
    operation: &'static str,
) -> Result<T, SessionError>
where
    F: Future<Output = Result<T, russh::Error>>,
{
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(SessionError::Cancelled),
        result = tokio::time::timeout(operation_timeout, future) => {
            match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(source)) => Err(SessionError::SshOperation {
                    hop,
                    operation,
                    source,
                }),
                Err(_) => {
                    cancellation.cancel();
                    Err(SessionError::OperationTimedOut { hop, operation })
                }
            }
        }
    }
}

async fn emit_event(
    events: &mpsc::Sender<SessionEvent>,
    cancellation: &SessionCancellation,
    event: SessionEvent,
) -> Result<(), SessionError> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(SessionError::Cancelled),
        result = events.send(event) => {
            result.map_err(|_| SessionError::EventReceiverClosed)
        }
    }
}

impl AuthenticationInteraction {
    async fn request(
        &self,
        hop: &SessionHop,
        endpoint: &SshEndpoint,
        name: String,
        instructions: String,
        prompts: Vec<AuthenticationPrompt>,
    ) -> Result<Vec<String>, SessionError> {
        let request_id = self.next_request.fetch_add(1, Ordering::Relaxed);
        let Some(_pending_request) =
            PendingAuthenticationRequest::begin(request_id, self.pending_request.clone())
        else {
            return Err(SessionError::AuthenticationChallengeAlreadyPending { hop: hop.clone() });
        };
        let expected_response_count = prompts.len();
        let info = AuthenticationChallengeInfo {
            request_id,
            hop: hop.clone(),
            endpoint: endpoint.clone(),
            name,
            instructions,
            prompts,
        };

        emit_event(
            &self.events,
            &self.cancellation,
            SessionEvent::AuthenticationChallenge(info),
        )
        .await?;

        let receive_response = async {
            let mut responses = self.responses.lock().await;
            loop {
                match responses.recv().await {
                    Some(response) if response.request_id == request_id => return Some(response),
                    Some(response) => {
                        warn!(
                            expected_request_id = request_id,
                            received_request_id = response.request_id,
                            "ignored stale authentication response"
                        );
                    }
                    None => return None,
                }
            }
        };

        let response = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => return Err(SessionError::Cancelled),
            response = tokio::time::timeout(
                self.response_timeout,
                receive_response,
            ) => {
                response
                    .ok()
                    .flatten()
                    .ok_or_else(|| SessionError::AuthenticationChallengeExpired {
                        hop: hop.clone(),
                    })?
            }
        };

        let Some(mut responses) = response.responses else {
            return Err(SessionError::AuthenticationCancelled { hop: hop.clone() });
        };
        if responses.len() != expected_response_count {
            return Err(SessionError::AuthenticationResponseCountMismatch { hop: hop.clone() });
        }

        let mut total_bytes = 0usize;
        for response in &responses {
            if response.len() > MAX_KEYBOARD_INTERACTIVE_RESPONSE_BYTES {
                return Err(SessionError::AuthenticationResponseTooLarge { hop: hop.clone() });
            }
            total_bytes = total_bytes
                .checked_add(response.len())
                .ok_or_else(|| SessionError::AuthenticationResponseTooLarge { hop: hop.clone() })?;
            if total_bytes > MAX_KEYBOARD_INTERACTIVE_RESPONSES_BYTES {
                return Err(SessionError::AuthenticationResponseTooLarge { hop: hop.clone() });
            }
        }

        Ok(responses
            .iter_mut()
            .map(|response| std::mem::take(&mut **response))
            .collect())
    }
}

struct PendingAuthenticationRequest {
    request_id: u64,
    pending: Arc<AtomicU64>,
}

impl PendingAuthenticationRequest {
    fn begin(request_id: u64, pending: Arc<AtomicU64>) -> Option<Self> {
        pending
            .compare_exchange(0, request_id, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        Some(Self {
            request_id,
            pending,
        })
    }
}

impl Drop for PendingAuthenticationRequest {
    fn drop(&mut self) {
        let _ =
            self.pending
                .compare_exchange(self.request_id, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

struct PendingHostKeyRequest {
    request_id: u64,
    pending: Arc<AtomicU64>,
}

impl PendingHostKeyRequest {
    fn begin(request_id: u64, pending: Arc<AtomicU64>) -> Option<Self> {
        pending
            .compare_exchange(0, request_id, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        Some(Self {
            request_id,
            pending,
        })
    }
}

impl Drop for PendingHostKeyRequest {
    fn drop(&mut self) {
        let _ =
            self.pending
                .compare_exchange(self.request_id, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

struct HostKeyMismatch {
    algorithm: String,
    trusted_fingerprints_sha256: Vec<String>,
    received_fingerprint_sha256: String,
}

struct ClientHandler {
    events: mpsc::Sender<SessionEvent>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
    pending_observed_host_key: Arc<Mutex<Option<ObservedHostKey>>>,
    next_host_key_request: Arc<AtomicU64>,
    cancellation: SessionCancellation,
    hop: SessionHop,
    endpoint: SshEndpoint,
    host_key_policy: HostKeyPolicy,
    host_key_mismatch: Arc<Mutex<Option<HostKeyMismatch>>>,
    accepted_fingerprint: Option<String>,
}

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint_sha256 = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        if self.accepted_fingerprint.as_deref() == Some(fingerprint_sha256.as_str()) {
            return Ok(true);
        }

        if let HostKeyPolicy::RequireSha256Set { fingerprints } = &self.host_key_policy {
            if fingerprints.is_empty() || fingerprints.len() > MAX_TRUSTED_HOST_KEYS {
                return Err(russh::Error::UnknownKey);
            }
            if fingerprints.contains(&fingerprint_sha256) {
                self.accepted_fingerprint = Some(fingerprint_sha256);
                return Ok(true);
            }

            *self.host_key_mismatch.lock().await = Some(HostKeyMismatch {
                algorithm: server_public_key.algorithm().to_string(),
                trusted_fingerprints_sha256: fingerprints.clone(),
                received_fingerprint_sha256: fingerprint_sha256,
            });
            return Ok(false);
        }

        let request_id = self.next_host_key_request.fetch_add(1, Ordering::Relaxed);
        let Some(_pending_request) =
            PendingHostKeyRequest::begin(request_id, self.pending_host_key_request.clone())
        else {
            warn!(%self.hop, "another host-key decision is already pending");
            return Ok(false);
        };

        let public_key = server_public_key.to_bytes()?;
        if public_key.is_empty() || public_key.len() > MAX_HOST_PUBLIC_KEY_BYTES {
            return Err(russh::Error::PacketSize(public_key.len()));
        }
        let algorithm = server_public_key.algorithm().to_string();
        *self.pending_observed_host_key.lock().await = Some(ObservedHostKey {
            request_id,
            hop: self.hop.clone(),
            endpoint: self.endpoint.clone(),
            algorithm: algorithm.clone(),
            fingerprint_sha256: fingerprint_sha256.clone(),
            public_key,
        });

        let info = HostKeyInfo {
            request_id,
            hop: self.hop.clone(),
            endpoint: self.endpoint.clone(),
            algorithm,
            fingerprint_sha256: fingerprint_sha256.clone(),
        };

        let event_sent = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => false,
            result = self.events.send(SessionEvent::HostKey(info)) => result.is_ok(),
        };
        if !event_sent {
            clear_pending_observed_host_key(&self.pending_observed_host_key, request_id).await;
            return Ok(false);
        }

        let receive_decision = async {
            let mut decisions = self.host_key_decisions.lock().await;
            loop {
                match decisions.recv().await {
                    Some(decision) if decision.request_id == request_id => {
                        return Some(decision.accepted);
                    }
                    Some(decision) => {
                        warn!(
                            expected_request_id = request_id,
                            received_request_id = decision.request_id,
                            "ignored stale host-key decision"
                        );
                    }
                    None => return None,
                }
            }
        };

        let accepted = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => false,
            decision = tokio::time::timeout(HOST_KEY_DECISION_TIMEOUT, receive_decision) => {
                decision.ok().flatten().unwrap_or(false)
            }
        };
        clear_pending_observed_host_key(&self.pending_observed_host_key, request_id).await;

        if accepted {
            self.accepted_fingerprint = Some(fingerprint_sha256);
        }

        Ok(accepted)
    }
}

async fn clear_pending_observed_host_key(
    pending: &Mutex<Option<ObservedHostKey>>,
    request_id: u64,
) {
    let mut pending = pending.lock().await;
    if pending
        .as_ref()
        .is_some_and(|observed| observed.request_id == request_id)
    {
        pending.take();
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionControlError {
    #[error("SSH session is already closed")]
    SessionClosed,
    #[error("host-key confirmation request is no longer active")]
    HostKeyRequestExpired,
    #[error("authentication challenge request is no longer active")]
    AuthenticationRequestExpired,
}

#[derive(Debug, Error)]
enum SessionError {
    #[error("{hop} {operation} failed: {source}")]
    SshOperation {
        hop: SessionHop,
        operation: &'static str,
        #[source]
        source: russh::Error,
    },
    #[error("{hop} {operation} timed out")]
    OperationTimedOut {
        hop: SessionHop,
        operation: &'static str,
    },
    #[error("{hop} authentication failed")]
    AuthenticationFailed { hop: SessionHop },
    #[error("{hop} keyboard-interactive authentication failed")]
    KeyboardInteractiveAuthenticationFailed { hop: SessionHop },
    #[error("{hop} authentication challenge was cancelled")]
    AuthenticationCancelled { hop: SessionHop },
    #[error("{hop} authentication challenge expired")]
    AuthenticationChallengeExpired { hop: SessionHop },
    #[error("{hop} authentication challenge is already pending")]
    AuthenticationChallengeAlreadyPending { hop: SessionHop },
    #[error("{hop} authentication challenge exceeded {maximum} rounds")]
    AuthenticationChallengeRoundsExceeded { hop: SessionHop, maximum: usize },
    #[error("{hop} authentication challenge is too large")]
    AuthenticationChallengeTooLarge { hop: SessionHop },
    #[error("{hop} authentication response count does not match")]
    AuthenticationResponseCountMismatch { hop: SessionHop },
    #[error("{hop} authentication response is too large")]
    AuthenticationResponseTooLarge { hop: SessionHop },
    #[error("{hop} host key changed for {host}:{port}")]
    HostKeyChanged {
        hop: SessionHop,
        host: String,
        port: u16,
        algorithm: String,
        trusted_fingerprints_sha256: Vec<String>,
        received_fingerprint_sha256: String,
    },
    #[error("{hop} SSH username is empty")]
    InvalidUsername { hop: SessionHop },
    #[error("{hop} private key is invalid")]
    InvalidPrivateKey { hop: SessionHop },
    #[error("{hop} private key could not be decoded")]
    PrivateKeyDecode {
        hop: SessionHop,
        #[source]
        source: russh::keys::Error,
    },
    #[error("{hop} private-key decoding task failed")]
    PrivateKeyTaskFailed { hop: SessionHop },
    #[error("{hop} system SSH Agent is unavailable")]
    SystemAgentUnavailable { hop: SessionHop },
    #[error("{hop} system SSH Agent is unsupported on this platform")]
    SystemAgentUnsupported { hop: SessionHop },
    #[error("{hop} system SSH Agent returned too many identities")]
    SystemAgentTooManyIdentities { hop: SessionHop },
    #[error("{hop} selected SSH Agent identity is no longer available")]
    SystemAgentIdentityUnavailable { hop: SessionHop },
    #[error("{hop} system SSH Agent refused the signing request")]
    SystemAgentSigningFailed { hop: SessionHop },
    #[error("SSH connection timeout must be greater than zero")]
    InvalidConnectionTimeout,
    #[error("SSH Jump Route contains {count} hosts; maximum is {maximum}")]
    TooManyJumpHosts { count: usize, maximum: usize },
    #[error("SSH Jump Route is invalid")]
    InvalidJumpRoute,
    #[error("SSH event receiver closed")]
    EventReceiverClosed,
    #[error("SSH session was cancelled")]
    Cancelled,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("private key is invalid or requires an unsupported passphrase")]
pub struct PrivateKeyValidationError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    use russh::server::{self, Server as _};
    use tokio::net::TcpListener;

    #[derive(Clone, Copy)]
    enum TestServerMode {
        MultiRound,
        EndlessEmpty,
    }

    #[derive(Clone)]
    struct KeyboardInteractiveTestServer {
        mode: TestServerMode,
        round: usize,
    }

    impl server::Server for KeyboardInteractiveTestServer {
        type Handler = Self;

        fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
            Self {
                mode: self.mode,
                round: 0,
            }
        }
    }

    impl server::Handler for KeyboardInteractiveTestServer {
        type Error = russh::Error;

        async fn auth_keyboard_interactive<'a>(
            &'a mut self,
            _user: &str,
            _submethods: &str,
            response: Option<server::Response<'a>>,
        ) -> Result<server::Auth, Self::Error> {
            let responses = response
                .map(|responses| {
                    responses
                        .map(|response| String::from_utf8_lossy(&response).into_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if matches!(self.mode, TestServerMode::EndlessEmpty) {
                self.round += 1;
                return Ok(server::Auth::Partial {
                    name: Cow::Borrowed(""),
                    instructions: Cow::Borrowed(""),
                    prompts: Cow::Borrowed(&[]),
                });
            }

            match (self.round, responses.as_slice()) {
                (0, []) => {
                    self.round = 1;
                    Ok(server::Auth::Partial {
                        name: Cow::Borrowed("AnySSH MFA"),
                        instructions: Cow::Borrowed("Enter both values"),
                        prompts: Cow::Borrowed(&[
                            (Cow::Borrowed("Verification code:"), false),
                            (Cow::Borrowed("Device name:"), true),
                        ]),
                    })
                }
                (1, [code, device]) if code == "654321" && device == "laptop" => {
                    self.round = 2;
                    Ok(server::Auth::Partial {
                        name: Cow::Borrowed("Recovery"),
                        instructions: Cow::Borrowed("Enter the backup code"),
                        prompts: Cow::Borrowed(&[(Cow::Borrowed("Backup code:"), false)]),
                    })
                }
                (2, [backup]) if backup == "backup-7" => {
                    self.round = 3;
                    Ok(server::Auth::Partial {
                        name: Cow::Borrowed(""),
                        instructions: Cow::Borrowed(""),
                        prompts: Cow::Borrowed(&[]),
                    })
                }
                (3, []) => Ok(server::Auth::Accept),
                _ => Ok(server::Auth::reject()),
            }
        }

        async fn channel_open_session(
            &mut self,
            _channel: russh::Channel<server::Msg>,
            reply: server::ChannelOpenHandle,
            _session: &mut server::Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }

        async fn pty_request(
            &mut self,
            _channel: russh::ChannelId,
            _term: &str,
            _col_width: u32,
            _row_height: u32,
            _pix_width: u32,
            _pix_height: u32,
            _modes: &[(russh::Pty, u32)],
            session: &mut server::Session,
        ) -> Result<(), Self::Error> {
            session.request_success();
            Ok(())
        }

        async fn shell_request(
            &mut self,
            _channel: russh::ChannelId,
            session: &mut server::Session,
        ) -> Result<(), Self::Error> {
            session.request_success();
            Ok(())
        }
    }

    async fn spawn_keyboard_interactive_test_server(
        mode: TestServerMode,
    ) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind test server");
        let port = listener.local_addr().expect("test address").port();
        let config = Arc::new(server::Config {
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            keys: vec![
                ssh_key::PrivateKey::random(&mut rand::rng(), ssh_key::Algorithm::Ed25519)
                    .expect("server host key"),
            ],
            ..Default::default()
        });
        let task = tokio::spawn(async move {
            let mut server = KeyboardInteractiveTestServer { mode, round: 0 };
            server
                .run_on_socket(config, &listener)
                .await
                .expect("test server");
        });
        (port, task)
    }

    async fn complete_keyboard_interactive_session(
        authentication: SessionAuthentication,
        expected_rounds: &[&[&str]],
    ) {
        let (port, server) =
            spawn_keyboard_interactive_test_server(TestServerMode::MultiRound).await;
        let SpawnedSession {
            control,
            mut events,
        } = spawn_session(SshSessionConfig {
            target: SshConnectionConfig {
                endpoint: SshEndpoint::new("127.0.0.1", port).expect("endpoint"),
                username: "anyssh".to_owned(),
                authentication,
                host_key_policy: HostKeyPolicy::Prompt,
            },
            jump_hosts: Vec::new(),
            terminal_size: TerminalSize::default(),
            connection_timeout: Duration::from_secs(5),
        });
        let mut round = 0usize;
        let mut authenticated = false;

        while let Some(event) = tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("session event timeout")
        {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => control
                    .confirm_host_key(info.request_id, true)
                    .await
                    .expect("accept test Host Key"),
                SessionEvent::AuthenticationChallenge(info) => {
                    let responses = expected_rounds
                        .get(round)
                        .unwrap_or_else(|| panic!("unexpected challenge round {round}"));
                    assert_eq!(info.hop, SessionHop::Target);
                    assert_eq!(info.prompts.len(), responses.len());
                    control
                        .respond_authentication(
                            info.request_id,
                            Some(
                                responses
                                    .iter()
                                    .map(|response| Zeroizing::new((*response).to_owned()))
                                    .collect(),
                            ),
                        )
                        .await
                        .expect("answer challenge");
                    round += 1;
                }
                SessionEvent::Authenticated => {
                    authenticated = true;
                    control.disconnect().await.expect("disconnect test session");
                }
                SessionEvent::Error(error) => panic!("unexpected SSH error: {error}"),
                SessionEvent::Closed => break,
                SessionEvent::HostKeyChanged(_)
                | SessionEvent::Connected
                | SessionEvent::Data(_)
                | SessionEvent::ExitStatus(_) => {}
            }
        }

        server.abort();
        assert!(authenticated);
        assert_eq!(round, expected_rounds.len());
    }

    #[test]
    fn password_debug_output_is_redacted() {
        let config = PasswordSessionConfig {
            endpoint: SshEndpoint::new("example.com", 22).expect("valid endpoint"),
            username: "alice".to_owned(),
            password: Zeroizing::new("do-not-log-me".to_owned()),
            terminal_size: TerminalSize::default(),
        };

        let debug = format!("{config:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("do-not-log-me"));
    }

    #[test]
    fn private_key_debug_output_is_redacted() {
        let config = PrivateKeySessionConfig {
            endpoint: SshEndpoint::new("example.com", 22).expect("valid endpoint"),
            username: "alice".to_owned(),
            private_key: Zeroizing::new("private-key-material".to_owned()),
            passphrase: Some(Zeroizing::new("private-key-passphrase".to_owned())),
            terminal_size: TerminalSize::default(),
        };

        let debug = format!("{config:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("private-key-material"));
        assert!(!debug.contains("private-key-passphrase"));
    }

    #[test]
    fn openssh_private_key_inspection_distinguishes_encryption_without_a_passphrase() {
        let key = ssh_key::PrivateKey::random(&mut rand::rng(), ssh_key::Algorithm::Ed25519)
            .expect("fixture key");
        let unencrypted = key
            .to_openssh(ssh_key::LineEnding::LF)
            .expect("encode unencrypted fixture");
        let encrypted = key
            .encrypt(&mut rand::rng(), "fixture-passphrase")
            .expect("encrypt fixture")
            .to_openssh(ssh_key::LineEnding::LF)
            .expect("encode encrypted fixture");

        assert_eq!(
            inspect_openssh_private_key_text(unencrypted.as_str()),
            Ok(PrivateKeyTextEncryption::Unencrypted)
        );
        assert_eq!(
            inspect_openssh_private_key_text(encrypted.as_str()),
            Ok(PrivateKeyTextEncryption::Encrypted)
        );
        assert!(validate_private_key_text(encrypted.as_str(), None).is_err());
        assert!(validate_private_key_text(encrypted.as_str(), Some("wrong")).is_err());
        validate_private_key_text(encrypted.as_str(), Some("fixture-passphrase"))
            .expect("correct passphrase");
        assert!(inspect_openssh_private_key_text("not-a-private-key").is_err());
    }

    #[test]
    fn system_agent_debug_output_does_not_expose_the_selected_fingerprint() {
        let authentication = SessionAuthentication::SystemAgent {
            identity_fingerprint_sha256: "SHA256:selected-agent-key".to_owned(),
        };

        let debug = format!("{authentication:?}");
        assert!(debug.contains("<selected>"));
        assert!(!debug.contains("selected-agent-key"));
    }

    #[test]
    fn keyboard_interactive_debug_output_does_not_contain_responses_or_server_text() {
        let authentication = SessionAuthentication::KeyboardInteractive;
        let challenge = AuthenticationChallengeInfo {
            request_id: 9,
            hop: SessionHop::Target,
            endpoint: SshEndpoint::new("example.com", 22).expect("endpoint"),
            name: "Sensitive account notice".to_owned(),
            instructions: "Enter a one-time code".to_owned(),
            prompts: vec![AuthenticationPrompt {
                text: "Verification code:".to_owned(),
                echo: false,
            }],
        };

        let authentication_debug = format!("{authentication:?}");
        let challenge_debug = format!("{challenge:?}");
        assert!(authentication_debug.contains("KeyboardInteractive"));
        assert!(!challenge_debug.contains("Sensitive account notice"));
        assert!(!challenge_debug.contains("Enter a one-time code"));
        assert!(!challenge_debug.contains("Verification code"));
        assert!(challenge_debug.contains("prompt_count"));
    }

    #[test]
    fn primary_authentication_only_continues_after_explicit_partial_success() {
        let hop = SessionHop::Target;
        let keyboard_interactive = russh::MethodSet::from(&[MethodKind::KeyboardInteractive][..]);

        assert_eq!(
            classify_primary_authentication(AuthResult::Success, &hop)
                .expect("successful primary authentication"),
            PrimaryAuthenticationOutcome::Success
        );
        assert!(matches!(
            classify_primary_authentication(
                AuthResult::Failure {
                    remaining_methods: keyboard_interactive.clone(),
                    partial_success: false,
                },
                &hop,
            ),
            Err(SessionError::AuthenticationFailed { .. })
        ));
        assert_eq!(
            classify_primary_authentication(
                AuthResult::Failure {
                    remaining_methods: keyboard_interactive,
                    partial_success: true,
                },
                &hop,
            )
            .expect("partial success should continue"),
            PrimaryAuthenticationOutcome::ContinueKeyboardInteractive
        );
    }

    #[test]
    fn authentication_challenge_is_bounded_and_sanitized() {
        let hop = SessionHop::Target;
        let (name, instructions, prompts) = sanitize_authentication_challenge(
            "OTP\u{1b}[31m".to_owned(),
            "Line one\r\nLine two\u{7}".to_owned(),
            vec![russh::client::Prompt {
                prompt: "Verification\u{1b} code:".to_owned(),
                echo: false,
            }],
            &hop,
        )
        .expect("bounded challenge");

        assert_eq!(name, "OTP [31m");
        assert_eq!(instructions, "Line one \nLine two ");
        assert_eq!(prompts[0].text, "Verification  code:");
        assert!(!prompts[0].echo);

        assert!(matches!(
            sanitize_authentication_challenge(
                "name".to_owned(),
                "instructions".to_owned(),
                (0..=MAX_KEYBOARD_INTERACTIVE_PROMPTS)
                    .map(|_| russh::client::Prompt {
                        prompt: "code".to_owned(),
                        echo: false,
                    })
                    .collect(),
                &hop,
            ),
            Err(SessionError::AuthenticationChallengeTooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn keyboard_interactive_authentication_supports_multiple_rounds_and_prompts() {
        complete_keyboard_interactive_session(
            SessionAuthentication::KeyboardInteractive,
            &[&["654321", "laptop"], &["backup-7"]],
        )
        .await;
    }

    #[tokio::test]
    async fn keyboard_interactive_zero_prompt_rounds_are_bounded() {
        let (port, server) =
            spawn_keyboard_interactive_test_server(TestServerMode::EndlessEmpty).await;
        let SpawnedSession {
            control,
            mut events,
        } = spawn_session(SshSessionConfig {
            target: SshConnectionConfig {
                endpoint: SshEndpoint::new("127.0.0.1", port).expect("endpoint"),
                username: "anyssh".to_owned(),
                authentication: SessionAuthentication::KeyboardInteractive,
                host_key_policy: HostKeyPolicy::Prompt,
            },
            jump_hosts: Vec::new(),
            terminal_size: TerminalSize::default(),
            connection_timeout: Duration::from_secs(5),
        });
        let mut error = None;

        while let Some(event) = tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("session event timeout")
        {
            match event {
                SessionEvent::HostKey(info) => control
                    .confirm_host_key(info.request_id, true)
                    .await
                    .expect("accept test Host Key"),
                SessionEvent::AuthenticationChallenge(_) => {
                    panic!("zero Prompt rounds must not open a UI challenge")
                }
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Connecting
                | SessionEvent::Authenticated
                | SessionEvent::Connected
                | SessionEvent::Data(_)
                | SessionEvent::ExitStatus(_)
                | SessionEvent::HostKeyChanged(_) => {}
            }
        }

        server.abort();
        let error = error.expect("round limit error");
        assert!(error.contains("exceeded 8 rounds"));
    }

    #[test]
    fn agent_identity_summaries_skip_certificates_and_sanitize_comments() {
        let key = ssh_key::PrivateKey::random(&mut rand::rng(), ssh_key::Algorithm::Ed25519)
            .expect("fixture key");
        let identities = vec![
            AgentIdentity::PublicKey {
                key: key.public_key().clone(),
                comment: "  workstation key  ".to_owned(),
            },
            AgentIdentity::PublicKey {
                key: key.public_key().clone(),
                comment: "line\nbreak".to_owned(),
            },
        ];

        let summaries = summarize_system_agent_identities(identities).expect("identity summaries");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].algorithm(), "ssh-ed25519");
        assert!(summaries[0].fingerprint_sha256().starts_with("SHA256:"));
        assert_eq!(summaries[0].comment(), "workstation key");
        assert_eq!(summaries[1].comment(), "");
    }

    #[test]
    fn agent_identity_summary_limit_fails_closed() {
        let key = ssh_key::PrivateKey::random(&mut rand::rng(), ssh_key::Algorithm::Ed25519)
            .expect("fixture key");
        let identities = (0..=MAX_SYSTEM_AGENT_IDENTITIES)
            .map(|index| AgentIdentity::PublicKey {
                key: key.public_key().clone(),
                comment: format!("identity-{index}"),
            })
            .collect();

        assert_eq!(
            summarize_system_agent_identities(identities),
            Err(SystemAgentError::TooManyIdentities)
        );
    }

    #[test]
    fn dependency_log_facade_is_capped_below_agent_debug_payloads() {
        assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Info);
    }

    #[tokio::test]
    async fn host_key_confirmation_requires_the_active_request() {
        let (command_sender, _command_receiver) = mpsc::channel(1);
        let (decision_sender, mut decision_receiver) = mpsc::channel(1);
        let (authentication_sender, _authentication_receiver) = mpsc::channel(1);
        let pending = Arc::new(AtomicU64::new(17));
        let control = SessionControl {
            inner: Arc::new(SessionControlInner {
                commands: command_sender,
                host_key_decisions: decision_sender,
                authentication_responses: authentication_sender,
                pending_host_key_request: pending.clone(),
                pending_authentication_request: Arc::new(AtomicU64::new(0)),
                pending_observed_host_key: Arc::new(Mutex::new(None)),
                cancellation: SessionCancellation::default(),
            }),
        };

        assert_eq!(
            control.confirm_host_key(16, true).await,
            Err(SessionControlError::HostKeyRequestExpired)
        );
        control
            .confirm_host_key(17, true)
            .await
            .expect("active request should be accepted");

        let decision = decision_receiver
            .recv()
            .await
            .expect("decision should reach handler");
        assert_eq!(decision.request_id, 17);
        assert!(decision.accepted);
        assert_eq!(pending.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn authentication_response_requires_the_active_request_and_redacts_debug() {
        let (command_sender, _command_receiver) = mpsc::channel(1);
        let (decision_sender, _decision_receiver) = mpsc::channel(1);
        let (authentication_sender, mut authentication_receiver) = mpsc::channel(1);
        let pending_authentication = Arc::new(AtomicU64::new(41));
        let control = SessionControl {
            inner: Arc::new(SessionControlInner {
                commands: command_sender,
                host_key_decisions: decision_sender,
                authentication_responses: authentication_sender,
                pending_host_key_request: Arc::new(AtomicU64::new(0)),
                pending_authentication_request: pending_authentication.clone(),
                pending_observed_host_key: Arc::new(Mutex::new(None)),
                cancellation: SessionCancellation::default(),
            }),
        };

        assert_eq!(
            control.respond_authentication(40, None).await,
            Err(SessionControlError::AuthenticationRequestExpired)
        );
        control
            .respond_authentication(41, Some(vec![Zeroizing::new("one-time-secret".to_owned())]))
            .await
            .expect("active response");

        let response = authentication_receiver
            .recv()
            .await
            .expect("response should reach SSH worker");
        let debug = format!("{response:?}");
        assert!(debug.contains("response_count"));
        assert!(!debug.contains("one-time-secret"));
        assert_eq!(pending_authentication.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn authentication_interaction_binds_response_count_and_request_id() {
        let (event_sender, mut event_receiver) = mpsc::channel(1);
        let (response_sender, response_receiver) = mpsc::channel(1);
        let pending = Arc::new(AtomicU64::new(0));
        let interaction = AuthenticationInteraction {
            events: event_sender,
            responses: Arc::new(Mutex::new(response_receiver)),
            pending_request: pending.clone(),
            next_request: Arc::new(AtomicU64::new(7)),
            response_timeout: AUTHENTICATION_RESPONSE_TIMEOUT,
            cancellation: SessionCancellation::default(),
        };
        let endpoint = SshEndpoint::new("example.com", 22).expect("endpoint");
        let request = tokio::spawn({
            let interaction = interaction.clone();
            let endpoint = endpoint.clone();
            async move {
                interaction
                    .request(
                        &SessionHop::Target,
                        &endpoint,
                        "OTP".to_owned(),
                        "Enter both values".to_owned(),
                        vec![
                            AuthenticationPrompt {
                                text: "Code:".to_owned(),
                                echo: false,
                            },
                            AuthenticationPrompt {
                                text: "Device:".to_owned(),
                                echo: true,
                            },
                        ],
                    )
                    .await
            }
        });

        let Some(SessionEvent::AuthenticationChallenge(challenge)) = event_receiver.recv().await
        else {
            panic!("challenge event");
        };
        assert_eq!(challenge.request_id, 7);
        assert_eq!(challenge.prompts.len(), 2);
        assert_eq!(pending.load(Ordering::Acquire), 7);

        response_sender
            .send(AuthenticationResponse {
                request_id: 7,
                responses: Some(vec![Zeroizing::new("only-one".to_owned())]),
            })
            .await
            .expect("response");
        assert!(matches!(
            request.await.expect("request task"),
            Err(SessionError::AuthenticationResponseCountMismatch { .. })
        ));
        assert_eq!(pending.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn authentication_interaction_times_out_and_rejects_oversized_responses() {
        let endpoint = SshEndpoint::new("example.com", 22).expect("endpoint");

        let (event_sender, mut event_receiver) = mpsc::channel(1);
        let (_response_sender, response_receiver) = mpsc::channel(1);
        let pending = Arc::new(AtomicU64::new(0));
        let interaction = AuthenticationInteraction {
            events: event_sender,
            responses: Arc::new(Mutex::new(response_receiver)),
            pending_request: pending.clone(),
            next_request: Arc::new(AtomicU64::new(1)),
            response_timeout: Duration::from_millis(20),
            cancellation: SessionCancellation::default(),
        };
        let request = tokio::spawn({
            let interaction = interaction.clone();
            let endpoint = endpoint.clone();
            async move {
                interaction
                    .request(
                        &SessionHop::Target,
                        &endpoint,
                        String::new(),
                        String::new(),
                        vec![AuthenticationPrompt {
                            text: "OTP:".to_owned(),
                            echo: false,
                        }],
                    )
                    .await
            }
        });
        assert!(matches!(
            event_receiver.recv().await,
            Some(SessionEvent::AuthenticationChallenge(_))
        ));
        assert!(matches!(
            request.await.expect("request task"),
            Err(SessionError::AuthenticationChallengeExpired { .. })
        ));
        assert_eq!(pending.load(Ordering::Acquire), 0);

        let (event_sender, mut event_receiver) = mpsc::channel(1);
        let (response_sender, response_receiver) = mpsc::channel(1);
        let interaction = AuthenticationInteraction {
            events: event_sender,
            responses: Arc::new(Mutex::new(response_receiver)),
            pending_request: Arc::new(AtomicU64::new(0)),
            next_request: Arc::new(AtomicU64::new(2)),
            response_timeout: AUTHENTICATION_RESPONSE_TIMEOUT,
            cancellation: SessionCancellation::default(),
        };
        let request = tokio::spawn({
            let interaction = interaction.clone();
            async move {
                interaction
                    .request(
                        &SessionHop::Target,
                        &endpoint,
                        String::new(),
                        String::new(),
                        vec![AuthenticationPrompt {
                            text: "OTP:".to_owned(),
                            echo: false,
                        }],
                    )
                    .await
            }
        });
        let Some(SessionEvent::AuthenticationChallenge(challenge)) = event_receiver.recv().await
        else {
            panic!("challenge event");
        };
        response_sender
            .send(AuthenticationResponse {
                request_id: challenge.request_id,
                responses: Some(vec![Zeroizing::new(
                    "x".repeat(MAX_KEYBOARD_INTERACTIVE_RESPONSE_BYTES + 1),
                )]),
            })
            .await
            .expect("oversized response");
        assert!(matches!(
            request.await.expect("request task"),
            Err(SessionError::AuthenticationResponseTooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn observed_host_key_evidence_requires_the_active_request_and_stays_out_of_debug() {
        let (command_sender, _command_receiver) = mpsc::channel(1);
        let (decision_sender, _decision_receiver) = mpsc::channel(1);
        let (authentication_sender, _authentication_receiver) = mpsc::channel(1);
        let pending = Arc::new(AtomicU64::new(23));
        let observed = ObservedHostKey {
            request_id: 23,
            hop: SessionHop::Target,
            endpoint: SshEndpoint::new("example.com", 22).expect("endpoint"),
            algorithm: "ssh-ed25519".to_owned(),
            fingerprint_sha256: "SHA256:observed".to_owned(),
            public_key: vec![0, 1, 2, 3, 4],
        };
        let control = SessionControl {
            inner: Arc::new(SessionControlInner {
                commands: command_sender,
                host_key_decisions: decision_sender,
                authentication_responses: authentication_sender,
                pending_host_key_request: pending,
                pending_authentication_request: Arc::new(AtomicU64::new(0)),
                pending_observed_host_key: Arc::new(Mutex::new(Some(observed))),
                cancellation: SessionCancellation::default(),
            }),
        };

        assert_eq!(
            control.observed_host_key(22).await,
            Err(SessionControlError::HostKeyRequestExpired)
        );
        let evidence = control
            .observed_host_key(23)
            .await
            .expect("active observed Host Key");
        assert_eq!(evidence.endpoint().host, "example.com");
        assert_eq!(evidence.algorithm(), "ssh-ed25519");
        assert_eq!(evidence.fingerprint_sha256(), "SHA256:observed");
        assert_eq!(evidence.public_key(), [0, 1, 2, 3, 4]);
        let debug = format!("{evidence:?}");
        assert!(!debug.contains("[0, 1, 2, 3, 4]"));
    }

    #[tokio::test]
    async fn oversized_jump_route_is_rejected_before_network_access() {
        let jump_hosts = (0..=MAX_JUMP_HOSTS)
            .map(|index| SshConnectionConfig {
                endpoint: SshEndpoint::new(format!("jump-{index}.invalid"), 22)
                    .expect("jump endpoint"),
                username: "alice".to_owned(),
                authentication: SessionAuthentication::Password {
                    password: Zeroizing::new(format!("secret-{index}")),
                },
                host_key_policy: HostKeyPolicy::Prompt,
            })
            .collect();
        let spawned = spawn_session(SshSessionConfig {
            target: SshConnectionConfig {
                endpoint: SshEndpoint::new("target.invalid", 22).expect("target endpoint"),
                username: "alice".to_owned(),
                authentication: SessionAuthentication::Password {
                    password: Zeroizing::new("target-secret".to_owned()),
                },
                host_key_policy: HostKeyPolicy::Prompt,
            },
            jump_hosts,
            terminal_size: TerminalSize::default(),
            connection_timeout: Duration::from_secs(1),
        });
        let mut events = spawned.events;
        assert_eq!(events.recv().await, Some(SessionEvent::Connecting));
        let Some(SessionEvent::Error(error)) = events.recv().await else {
            panic!("oversized Jump Route must emit an error");
        };
        assert!(error.contains("maximum is 32"));
        assert!(!error.contains("target-secret"));
        assert!(!error.contains("secret-0"));
        assert_eq!(events.recv().await, Some(SessionEvent::Closed));
    }
}
