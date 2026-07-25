#![forbid(unsafe_code)]

use std::{fmt, sync::Arc, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use bytes::Bytes;
use russh::{
    ChannelMsg, Disconnect,
    client::{self, Config, Handle, Handler},
    keys::ssh_key::{self, HashAlg},
};
use thiserror::Error;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, warn};
use zeroize::Zeroizing;

const EVENT_BUFFER: usize = 64;
const COMMAND_BUFFER: usize = 64;
const HOST_KEY_DECISION_BUFFER: usize = 1;
const HOST_KEY_DECISION_TIMEOUT: Duration = Duration::from_secs(60);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyInfo {
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
    Disconnect,
}

#[derive(Clone)]
pub struct SessionControl {
    commands: mpsc::Sender<SessionCommand>,
    host_key_decisions: mpsc::Sender<bool>,
}

impl SessionControl {
    pub async fn confirm_host_key(&self, accepted: bool) -> Result<(), SessionControlError> {
        self.host_key_decisions
            .send(accepted)
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn send_input(&self, input: impl Into<Bytes>) -> Result<(), SessionControlError> {
        self.commands
            .send(SessionCommand::Input(input.into()))
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn resize(&self, size: TerminalSize) -> Result<(), SessionControlError> {
        self.commands
            .send(SessionCommand::Resize(size))
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }

    pub async fn disconnect(&self) -> Result<(), SessionControlError> {
        self.commands
            .send(SessionCommand::Disconnect)
            .await
            .map_err(|_| SessionControlError::SessionClosed)
    }
}

pub struct SpawnedSession {
    pub control: SessionControl,
    pub events: mpsc::Receiver<SessionEvent>,
}

pub fn spawn_password_session(config: PasswordSessionConfig) -> SpawnedSession {
    let (event_sender, event_receiver) = mpsc::channel(EVENT_BUFFER);
    let (command_sender, command_receiver) = mpsc::channel(COMMAND_BUFFER);
    let (host_key_sender, host_key_receiver) = mpsc::channel(HOST_KEY_DECISION_BUFFER);

    let control = SessionControl {
        commands: command_sender,
        host_key_decisions: host_key_sender,
    };

    tokio::spawn(run_session(
        config,
        event_sender,
        command_receiver,
        host_key_receiver,
    ));

    SpawnedSession {
        control,
        events: event_receiver,
    }
}

async fn run_session(
    config: PasswordSessionConfig,
    events: mpsc::Sender<SessionEvent>,
    mut commands: mpsc::Receiver<SessionCommand>,
    host_key_decisions: mpsc::Receiver<bool>,
) {
    if events.send(SessionEvent::Connecting).await.is_err() {
        return;
    }

    if let Err(error) = connect_and_run(config, &events, &mut commands, host_key_decisions).await {
        let _ = events.send(SessionEvent::Error(error.to_string())).await;
    }

    let _ = events.send(SessionEvent::Closed).await;
}

async fn connect_and_run(
    config: PasswordSessionConfig,
    events: &mpsc::Sender<SessionEvent>,
    commands: &mut mpsc::Receiver<SessionCommand>,
    host_key_decisions: mpsc::Receiver<bool>,
) -> Result<(), SessionError> {
    let client_config = Arc::new(Config {
        inactivity_timeout: Some(Duration::from_secs(30)),
        nodelay: true,
        ..Default::default()
    });

    let handler = ClientHandler {
        events: events.clone(),
        host_key_decisions: Arc::new(Mutex::new(host_key_decisions)),
        accepted_fingerprint: None,
    };

    debug!(
        host = %config.endpoint.host,
        port = config.endpoint.port,
        "connecting SSH session"
    );

    let mut session = client::connect(
        client_config,
        (config.endpoint.host.as_str(), config.endpoint.port),
        handler,
    )
    .await?;

    let authentication = session
        .authenticate_password(config.username, config.password.as_str())
        .await?;

    if !authentication.success() {
        return Err(SessionError::AuthenticationFailed);
    }

    events
        .send(SessionEvent::Authenticated)
        .await
        .map_err(|_| SessionError::EventReceiverClosed)?;

    let mut channel = session.channel_open_session().await?;
    channel
        .request_pty(
            true,
            "xterm-256color",
            config.terminal_size.columns,
            config.terminal_size.rows,
            0,
            0,
            &[],
        )
        .await?;
    channel.request_shell(true).await?;

    events
        .send(SessionEvent::Connected)
        .await
        .map_err(|_| SessionError::EventReceiverClosed)?;

    run_shell_loop(&mut session, &mut channel, events, commands).await
}

async fn run_shell_loop(
    session: &mut Handle<ClientHandler>,
    channel: &mut russh::Channel<russh::client::Msg>,
    events: &mpsc::Sender<SessionEvent>,
    commands: &mut mpsc::Receiver<SessionCommand>,
) -> Result<(), SessionError> {
    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(SessionCommand::Input(data)) => {
                        channel.data_bytes(data).await?;
                    }
                    Some(SessionCommand::Resize(size)) => {
                        channel.window_change(size.columns, size.rows, 0, 0).await?;
                    }
                    Some(SessionCommand::Disconnect) | None => {
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        break;
                    }
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) |
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        events
                            .send(SessionEvent::Data(data))
                            .await
                            .map_err(|_| SessionError::EventReceiverClosed)?;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        events
                            .send(SessionEvent::ExitStatus(exit_status))
                            .await
                            .map_err(|_| SessionError::EventReceiverClosed)?;
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

    session
        .disconnect(Disconnect::ByApplication, "", "en")
        .await?;

    Ok(())
}

struct ClientHandler {
    events: mpsc::Sender<SessionEvent>,
    host_key_decisions: Arc<Mutex<mpsc::Receiver<bool>>>,
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

        let info = HostKeyInfo {
            algorithm: server_public_key.algorithm().to_string(),
            fingerprint_sha256: fingerprint_sha256.clone(),
        };

        if self.events.send(SessionEvent::HostKey(info)).await.is_err() {
            return Ok(false);
        }

        let accepted = {
            let mut decisions = self.host_key_decisions.lock().await;
            tokio::time::timeout(HOST_KEY_DECISION_TIMEOUT, decisions.recv())
                .await
                .ok()
                .flatten()
                .unwrap_or(false)
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
}

#[derive(Debug, Error)]
enum SessionError {
    #[error(transparent)]
    Ssh(#[from] russh::Error),
    #[error("SSH authentication failed")]
    AuthenticationFailed,
    #[error("SSH event receiver closed")]
    EventReceiverClosed,
}
