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
    ChannelMsg, Disconnect,
    client::{self, Config, Handle, Handler},
    keys::{
        PrivateKeyWithHashAlg, decode_secret_key,
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
pub const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_JUMP_HOSTS: usize = 32;
const CHANNEL_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
pub const MAX_PRIVATE_KEY_BYTES: usize = 1024 * 1024;
const MAX_PRIVATE_KEY_PASSPHRASE_BYTES: usize = 64 * 1024;

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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyPolicy {
    Prompt,
    RequireSha256 { fingerprint: String },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Connecting,
    HostKey(HostKeyInfo),
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
    pending_host_key_request: Arc<AtomicU64>,
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

        self.inner
            .host_key_decisions
            .send(HostKeyDecision {
                request_id,
                accepted,
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
    let cancellation = SessionCancellation::default();
    let pending_host_key_request = Arc::new(AtomicU64::new(0));
    let next_host_key_request = Arc::new(AtomicU64::new(1));

    let control = SessionControl {
        inner: Arc::new(SessionControlInner {
            commands: command_sender,
            host_key_decisions: host_key_sender,
            pending_host_key_request: pending_host_key_request.clone(),
            cancellation: cancellation.clone(),
        }),
    };

    tokio::spawn(run_session(
        config,
        event_sender,
        command_receiver,
        Arc::new(Mutex::new(host_key_receiver)),
        pending_host_key_request,
        next_host_key_request,
        cancellation,
    ));

    SpawnedSession {
        control,
        events: event_receiver,
    }
}

async fn run_session(
    config: SshSessionConfig,
    events: mpsc::Sender<SessionEvent>,
    mut commands: mpsc::Receiver<SessionCommand>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
    next_host_key_request: Arc<AtomicU64>,
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
        next_host_key_request,
        &cancellation,
    )
    .await;

    if let Err(error) = result
        && !matches!(error, SessionError::Cancelled)
    {
        let _ = events.send(SessionEvent::Error(error.to_string())).await;
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
    next_host_key_request: Arc<AtomicU64>,
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
                    expected: mismatch.expected,
                    actual: mismatch.actual,
                })
            } else {
                Err(error)
            }
        }
    }
}

async fn run_target_session(
    mut session: Handle<ClientHandler>,
    config: SshConnectionConfig,
    terminal_size: TerminalSize,
    events: &mpsc::Sender<SessionEvent>,
    commands: &mut mpsc::Receiver<SessionCommand>,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<(), SessionError> {
    let SshConnectionConfig {
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

async fn authenticate(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    username: &str,
    authentication: SessionAuthentication,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<(), SessionError> {
    if username.trim().is_empty() {
        return Err(SessionError::InvalidUsername { hop: hop.clone() });
    }

    match authentication {
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
    }
}

async fn authenticate_password(
    session: &mut Handle<ClientHandler>,
    hop: &SessionHop,
    username: &str,
    password: Zeroizing<String>,
    cancellation: &SessionCancellation,
    connection_timeout: Duration,
) -> Result<(), SessionError> {
    let authentication = await_ssh_operation(
        session.authenticate_password(username, password.as_str()),
        cancellation,
        connection_timeout,
        hop.clone(),
        "password authentication",
    )
    .await?;

    if authentication.success() {
        Ok(())
    } else {
        Err(SessionError::AuthenticationFailed { hop: hop.clone() })
    }
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
) -> Result<(), SessionError> {
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

    if authentication.success() {
        Ok(())
    } else {
        Err(SessionError::AuthenticationFailed { hop: hop.clone() })
    }
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
    expected: String,
    actual: String,
}

struct ClientHandler {
    events: mpsc::Sender<SessionEvent>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<HostKeyDecision>>>,
    pending_host_key_request: Arc<AtomicU64>,
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

        if let HostKeyPolicy::RequireSha256 { fingerprint } = &self.host_key_policy {
            if fingerprint == &fingerprint_sha256 {
                self.accepted_fingerprint = Some(fingerprint_sha256);
                return Ok(true);
            }

            *self.host_key_mismatch.lock().await = Some(HostKeyMismatch {
                expected: fingerprint.clone(),
                actual: fingerprint_sha256,
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

        let info = HostKeyInfo {
            request_id,
            hop: self.hop.clone(),
            endpoint: self.endpoint.clone(),
            algorithm: server_public_key.algorithm().to_string(),
            fingerprint_sha256: fingerprint_sha256.clone(),
        };

        let event_sent = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => false,
            result = self.events.send(SessionEvent::HostKey(info)) => result.is_ok(),
        };
        if !event_sent {
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

        if accepted {
            self.accepted_fingerprint = Some(fingerprint_sha256);
        }

        Ok(accepted)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionControlError {
    #[error("SSH session is already closed")]
    SessionClosed,
    #[error("host-key confirmation request is no longer active")]
    HostKeyRequestExpired,
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
    #[error("{hop} host key changed for {host}:{port} (expected {expected}, received {actual})")]
    HostKeyChanged {
        hop: SessionHop,
        host: String,
        port: u16,
        expected: String,
        actual: String,
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

    #[tokio::test]
    async fn host_key_confirmation_requires_the_active_request() {
        let (command_sender, _command_receiver) = mpsc::channel(1);
        let (decision_sender, mut decision_receiver) = mpsc::channel(1);
        let pending = Arc::new(AtomicU64::new(17));
        let control = SessionControl {
            inner: Arc::new(SessionControlInner {
                commands: command_sender,
                host_key_decisions: decision_sender,
                pending_host_key_request: pending.clone(),
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
