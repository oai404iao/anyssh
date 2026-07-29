use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore, mpsc},
    task::{JoinHandle, JoinSet},
};

use super::SessionCancellation;

pub const MAX_ACTIVE_PORT_FORWARDS: usize = 16;
pub const MAX_PORT_FORWARD_CONNECTIONS: usize = 64;
pub const PORT_FORWARD_QUEUE_CAPACITY: usize = 64;
pub const MAX_FORWARD_HOST_BYTES: usize = 255;
pub const MAX_PORT_FORWARD_ID_BYTES: usize = 64;
pub(crate) const PORT_FORWARD_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
const SOCKS5_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

const SOCKS5_VERSION: u8 = 0x05;
const SOCKS5_NO_AUTHENTICATION: u8 = 0x00;
const SOCKS5_NO_ACCEPTABLE_METHODS: u8 = 0xff;
const SOCKS5_CONNECT: u8 = 0x01;
const SOCKS5_ADDRESS_IPV4: u8 = 0x01;
const SOCKS5_ADDRESS_DOMAIN: u8 = 0x03;
const SOCKS5_ADDRESS_IPV6: u8 = 0x04;
const SOCKS5_REPLY_SUCCESS: u8 = 0x00;
const SOCKS5_REPLY_GENERAL_FAILURE: u8 = 0x01;
const SOCKS5_REPLY_COMMAND_NOT_SUPPORTED: u8 = 0x07;
const SOCKS5_REPLY_ADDRESS_NOT_SUPPORTED: u8 = 0x08;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortForwardKind {
    Local,
    Remote,
    Dynamic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortForwardRequest {
    kind: PortForwardKind,
    bind_host: String,
    bind_port: u16,
    destination_host: Option<String>,
    destination_port: Option<u16>,
}

impl PortForwardRequest {
    pub fn new(
        kind: PortForwardKind,
        bind_host: impl Into<String>,
        bind_port: u16,
        destination_host: Option<String>,
        destination_port: Option<u16>,
    ) -> Result<Self, PortForwardError> {
        let bind_host = normalize_loopback_host(&bind_host.into())?;
        let (destination_host, destination_port) = match kind {
            PortForwardKind::Dynamic => {
                if destination_host.is_some() || destination_port.is_some() {
                    return Err(PortForwardError::InvalidRequest);
                }
                (None, None)
            }
            PortForwardKind::Local | PortForwardKind::Remote => {
                let host =
                    normalize_destination_host(destination_host.as_deref().unwrap_or_default())?;
                let port = destination_port
                    .filter(|port| *port != 0)
                    .ok_or(PortForwardError::InvalidDestination)?;
                (Some(host), Some(port))
            }
        };

        Ok(Self {
            kind,
            bind_host,
            bind_port,
            destination_host,
            destination_port,
        })
    }

    pub const fn kind(&self) -> PortForwardKind {
        self.kind
    }

    pub fn bind_host(&self) -> &str {
        &self.bind_host
    }

    pub const fn bind_port(&self) -> u16 {
        self.bind_port
    }

    pub fn destination_host(&self) -> Option<&str> {
        self.destination_host.as_deref()
    }

    pub const fn destination_port(&self) -> Option<u16> {
        self.destination_port
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortForwardSummary {
    id: String,
    kind: PortForwardKind,
    bind_host: String,
    bound_port: u16,
    destination_host: Option<String>,
    destination_port: Option<u16>,
}

impl PortForwardSummary {
    pub(crate) fn new(id: String, request: &PortForwardRequest, bound_port: u16) -> Self {
        Self {
            id,
            kind: request.kind,
            bind_host: request.bind_host.clone(),
            bound_port,
            destination_host: request.destination_host.clone(),
            destination_port: request.destination_port,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn kind(&self) -> PortForwardKind {
        self.kind
    }

    pub fn bind_host(&self) -> &str {
        &self.bind_host
    }

    pub const fn bound_port(&self) -> u16 {
        self.bound_port
    }

    pub fn destination_host(&self) -> Option<&str> {
        self.destination_host.as_deref()
    }

    pub const fn destination_port(&self) -> Option<u16> {
        self.destination_port
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PortForwardError {
    #[error("SSH session is already closed")]
    SessionClosed,
    #[error("port forward request is invalid")]
    InvalidRequest,
    #[error("port forward bind address must be loopback")]
    NonLoopbackBind,
    #[error("port forward destination is invalid")]
    InvalidDestination,
    #[error("maximum active port forwards reached")]
    TooManyForwards,
    #[error("port forward listener could not bind")]
    BindFailed,
    #[error("SSH server rejected the port forward")]
    RequestDenied,
    #[error("port forward operation timed out")]
    OperationTimedOut,
}

#[derive(Clone, Debug)]
pub(crate) struct ForwardDestination {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectConnectionProtocol {
    Raw,
    Socks5,
}

pub(crate) struct PendingDirectConnection {
    pub stream: TcpStream,
    pub destination: ForwardDestination,
    pub originator: SocketAddr,
    pub protocol: DirectConnectionProtocol,
    pub cancellation: SessionCancellation,
    pub permit: OwnedSemaphorePermit,
}

pub(crate) struct ActivePortForward {
    pub summary: PortForwardSummary,
    pub cancellation: SessionCancellation,
    pub listener: Option<JoinHandle<()>>,
    pub connections: Arc<Semaphore>,
    pub destination: Option<ForwardDestination>,
}

impl ActivePortForward {
    pub(crate) fn remote(summary: PortForwardSummary, destination: ForwardDestination) -> Self {
        Self {
            summary,
            cancellation: SessionCancellation::default(),
            listener: None,
            connections: Arc::new(Semaphore::new(MAX_PORT_FORWARD_CONNECTIONS)),
            destination: Some(destination),
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.cancellation.cancel();
        if let Some(listener) = self.listener.take() {
            listener.abort();
        }
    }
}

pub(crate) async fn start_listener_forward(
    id: String,
    request: &PortForwardRequest,
    direct_connections: mpsc::Sender<PendingDirectConnection>,
    session_cancellation: SessionCancellation,
) -> Result<ActivePortForward, PortForwardError> {
    let listener = tokio::select! {
        biased;
        _ = session_cancellation.cancelled() => {
            return Err(PortForwardError::SessionClosed);
        }
        result = tokio::time::timeout(
            PORT_FORWARD_OPERATION_TIMEOUT,
            TcpListener::bind((request.bind_host.as_str(), request.bind_port)),
        ) => {
            result
                .map_err(|_| PortForwardError::OperationTimedOut)?
                .map_err(|_| PortForwardError::BindFailed)?
        }
    };
    let bound_port = listener
        .local_addr()
        .map_err(|_| PortForwardError::BindFailed)?
        .port();
    let summary = PortForwardSummary::new(id.clone(), request, bound_port);
    let cancellation = SessionCancellation::default();
    let connections = Arc::new(Semaphore::new(MAX_PORT_FORWARD_CONNECTIONS));
    let kind = request.kind;
    let fixed_destination = request
        .destination_host
        .as_ref()
        .map(|host| ForwardDestination {
            host: host.clone(),
            port: request
                .destination_port
                .expect("validated fixed destination port"),
        });
    let listener_cancellation = cancellation.clone();
    let listener_connections = connections.clone();
    let listener_task = tokio::spawn(async move {
        run_listener(
            kind,
            fixed_destination,
            listener,
            direct_connections,
            listener_connections,
            listener_cancellation,
            session_cancellation,
        )
        .await;
    });

    Ok(ActivePortForward {
        summary,
        cancellation,
        listener: Some(listener_task),
        connections,
        destination: None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_listener(
    kind: PortForwardKind,
    fixed_destination: Option<ForwardDestination>,
    listener: TcpListener,
    direct_connections: mpsc::Sender<PendingDirectConnection>,
    connections: Arc<Semaphore>,
    cancellation: SessionCancellation,
    session_cancellation: SessionCancellation,
) {
    let mut handshake_tasks = JoinSet::new();
    loop {
        while handshake_tasks.try_join_next().is_some() {}
        let accepted = tokio::select! {
            biased;
            _ = session_cancellation.cancelled() => break,
            _ = cancellation.cancelled() => break,
            result = listener.accept() => result,
        };
        let Ok((stream, originator)) = accepted else {
            break;
        };
        let Ok(permit) = connections.clone().try_acquire_owned() else {
            drop(stream);
            continue;
        };

        match kind {
            PortForwardKind::Local => {
                let pending = PendingDirectConnection {
                    stream,
                    destination: fixed_destination
                        .clone()
                        .expect("validated Local destination"),
                    originator,
                    protocol: DirectConnectionProtocol::Raw,
                    cancellation: cancellation.clone(),
                    permit,
                };
                let _ = direct_connections.try_send(pending);
            }
            PortForwardKind::Dynamic => {
                let direct_connections = direct_connections.clone();
                let cancellation = cancellation.clone();
                let session_cancellation = session_cancellation.clone();
                handshake_tasks.spawn(async move {
                    let mut stream = stream;
                    let destination = tokio::select! {
                        biased;
                        _ = session_cancellation.cancelled() => return,
                        _ = cancellation.cancelled() => return,
                        result = tokio::time::timeout(
                            SOCKS5_HANDSHAKE_TIMEOUT,
                            read_socks5_destination(&mut stream),
                        ) => {
                            match result {
                                Ok(Ok(destination)) => destination,
                                Ok(Err(error)) => {
                                    if let Some(reply) = error.reply_code {
                                        let _ = write_socks5_reply(&mut stream, reply).await;
                                    }
                                    return;
                                }
                                Err(_) => return,
                            }
                        }
                    };
                    let pending = PendingDirectConnection {
                        stream,
                        destination,
                        originator,
                        protocol: DirectConnectionProtocol::Socks5,
                        cancellation,
                        permit,
                    };
                    if let Err(error) = direct_connections.try_send(pending) {
                        let mut pending = error.into_inner();
                        let _ =
                            write_socks5_reply(&mut pending.stream, SOCKS5_REPLY_GENERAL_FAILURE)
                                .await;
                    }
                });
            }
            PortForwardKind::Remote => unreachable!("Remote forwarding has no local listener"),
        }
    }
    handshake_tasks.abort_all();
    while handshake_tasks.join_next().await.is_some() {}
}

#[derive(Debug)]
struct Socks5HandshakeError {
    reply_code: Option<u8>,
}

async fn read_socks5_destination(
    stream: &mut TcpStream,
) -> Result<ForwardDestination, Socks5HandshakeError> {
    let mut greeting = [0u8; 2];
    stream
        .read_exact(&mut greeting)
        .await
        .map_err(|_| Socks5HandshakeError { reply_code: None })?;
    if greeting[0] != SOCKS5_VERSION || greeting[1] == 0 {
        return Err(Socks5HandshakeError { reply_code: None });
    }

    let mut methods = vec![0u8; usize::from(greeting[1])];
    stream
        .read_exact(&mut methods)
        .await
        .map_err(|_| Socks5HandshakeError { reply_code: None })?;
    if !methods.contains(&SOCKS5_NO_AUTHENTICATION) {
        let _ = stream
            .write_all(&[SOCKS5_VERSION, SOCKS5_NO_ACCEPTABLE_METHODS])
            .await;
        return Err(Socks5HandshakeError { reply_code: None });
    }
    stream
        .write_all(&[SOCKS5_VERSION, SOCKS5_NO_AUTHENTICATION])
        .await
        .map_err(|_| Socks5HandshakeError { reply_code: None })?;

    let mut request = [0u8; 4];
    stream
        .read_exact(&mut request)
        .await
        .map_err(|_| Socks5HandshakeError { reply_code: None })?;
    if request[0] != SOCKS5_VERSION || request[2] != 0 {
        return Err(Socks5HandshakeError {
            reply_code: Some(SOCKS5_REPLY_GENERAL_FAILURE),
        });
    }
    if request[1] != SOCKS5_CONNECT {
        return Err(Socks5HandshakeError {
            reply_code: Some(SOCKS5_REPLY_COMMAND_NOT_SUPPORTED),
        });
    }

    let host = match request[3] {
        SOCKS5_ADDRESS_IPV4 => {
            let mut address = [0u8; 4];
            stream
                .read_exact(&mut address)
                .await
                .map_err(|_| Socks5HandshakeError { reply_code: None })?;
            IpAddr::from(address).to_string()
        }
        SOCKS5_ADDRESS_IPV6 => {
            let mut address = [0u8; 16];
            stream
                .read_exact(&mut address)
                .await
                .map_err(|_| Socks5HandshakeError { reply_code: None })?;
            IpAddr::from(address).to_string()
        }
        SOCKS5_ADDRESS_DOMAIN => {
            let length = stream
                .read_u8()
                .await
                .map_err(|_| Socks5HandshakeError { reply_code: None })?;
            if length == 0 {
                return Err(Socks5HandshakeError {
                    reply_code: Some(SOCKS5_REPLY_ADDRESS_NOT_SUPPORTED),
                });
            }
            let mut domain = vec![0u8; usize::from(length)];
            stream
                .read_exact(&mut domain)
                .await
                .map_err(|_| Socks5HandshakeError { reply_code: None })?;
            let domain = String::from_utf8(domain).map_err(|_| Socks5HandshakeError {
                reply_code: Some(SOCKS5_REPLY_ADDRESS_NOT_SUPPORTED),
            })?;
            normalize_destination_host(&domain).map_err(|_| Socks5HandshakeError {
                reply_code: Some(SOCKS5_REPLY_ADDRESS_NOT_SUPPORTED),
            })?
        }
        _ => {
            return Err(Socks5HandshakeError {
                reply_code: Some(SOCKS5_REPLY_ADDRESS_NOT_SUPPORTED),
            });
        }
    };
    let port = stream
        .read_u16()
        .await
        .map_err(|_| Socks5HandshakeError { reply_code: None })?;
    if port == 0 {
        return Err(Socks5HandshakeError {
            reply_code: Some(SOCKS5_REPLY_ADDRESS_NOT_SUPPORTED),
        });
    }

    Ok(ForwardDestination { host, port })
}

pub(crate) async fn write_socks5_success(stream: &mut TcpStream) -> std::io::Result<()> {
    write_socks5_reply(stream, SOCKS5_REPLY_SUCCESS).await
}

pub(crate) async fn write_socks5_failure(stream: &mut TcpStream) -> std::io::Result<()> {
    write_socks5_reply(stream, SOCKS5_REPLY_GENERAL_FAILURE).await
}

async fn write_socks5_reply(stream: &mut TcpStream, reply: u8) -> std::io::Result<()> {
    stream
        .write_all(&[
            SOCKS5_VERSION,
            reply,
            0,
            SOCKS5_ADDRESS_IPV4,
            0,
            0,
            0,
            0,
            0,
            0,
        ])
        .await
}

fn normalize_loopback_host(value: &str) -> Result<String, PortForwardError> {
    let value = value.trim();
    let address = value
        .parse::<IpAddr>()
        .map_err(|_| PortForwardError::NonLoopbackBind)?;
    if !address.is_loopback() {
        return Err(PortForwardError::NonLoopbackBind);
    }
    Ok(address.to_string())
}

pub(crate) fn loopback_hosts_match(left: &str, right: &str) -> bool {
    match (
        normalize_loopback_host(left),
        normalize_loopback_host(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn valid_forward_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PORT_FORWARD_ID_BYTES
        && value.strip_prefix("forward-").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn normalize_destination_host(value: &str) -> Result<String, PortForwardError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_FORWARD_HOST_BYTES
        || value.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '\0'
        })
    {
        return Err(PortForwardError::InvalidDestination);
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn forwarding_requests_enforce_loopback_and_kind_shapes() {
        let local = PortForwardRequest::new(
            PortForwardKind::Local,
            "127.0.0.1",
            0,
            Some("target.internal".to_owned()),
            Some(8080),
        )
        .expect("valid Local forward");
        assert_eq!(local.bind_host(), "127.0.0.1");
        assert_eq!(local.destination_host(), Some("target.internal"));

        let dynamic = PortForwardRequest::new(PortForwardKind::Dynamic, "::1", 1080, None, None)
            .expect("valid Dynamic forward");
        assert_eq!(dynamic.bind_host(), "::1");

        assert_eq!(
            PortForwardRequest::new(
                PortForwardKind::Local,
                "0.0.0.0",
                8080,
                Some("target.internal".to_owned()),
                Some(80),
            ),
            Err(PortForwardError::NonLoopbackBind)
        );
        assert_eq!(
            PortForwardRequest::new(
                PortForwardKind::Dynamic,
                "127.0.0.1",
                1080,
                Some("unexpected".to_owned()),
                Some(80),
            ),
            Err(PortForwardError::InvalidRequest)
        );
        assert_eq!(
            PortForwardRequest::new(
                PortForwardKind::Remote,
                "127.0.0.1",
                0,
                Some("local host".to_owned()),
                Some(80),
            ),
            Err(PortForwardError::InvalidDestination)
        );
        assert!(valid_forward_id("forward-1"));
        assert!(!valid_forward_id("forward-"));
        assert!(!valid_forward_id("preview-forward-1"));
        assert!(!loopback_hosts_match("invalid", "also-invalid"));
        assert!(loopback_hosts_match("127.0.0.1", "127.0.0.1"));
    }

    #[tokio::test]
    async fn socks5_parser_accepts_fragmented_domain_connect() {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind SOCKS parser test");
        let address = listener.local_addr().expect("SOCKS parser address");
        let client = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.expect("connect parser");
            for chunk in [
                vec![5],
                vec![1, 0],
                vec![5, 1],
                vec![0, 3, 15],
                b"target.internal".to_vec(),
                vec![0x1f, 0x90],
            ] {
                stream.write_all(&chunk).await.expect("write SOCKS chunk");
                tokio::task::yield_now().await;
            }
            let mut method = [0u8; 2];
            stream.read_exact(&mut method).await.expect("method reply");
            method
        });
        let (mut server, _) = listener.accept().await.expect("accept parser");
        let destination = read_socks5_destination(&mut server)
            .await
            .expect("parse SOCKS destination");
        assert_eq!(destination.host, "target.internal");
        assert_eq!(destination.port, 8080);
        assert_eq!(client.await.expect("client task"), [5, 0]);
    }

    #[tokio::test]
    async fn socks5_parser_rejects_unsupported_commands() {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind SOCKS parser test");
        let address = listener.local_addr().expect("SOCKS parser address");
        let client = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.expect("connect parser");
            stream
                .write_all(&[5, 1, 0, 5, 2, 0, 1, 127, 0, 0, 1, 0, 80])
                .await
                .expect("write unsupported SOCKS request");
            let mut method = [0u8; 2];
            stream.read_exact(&mut method).await.expect("method reply");
            method
        });
        let (mut server, _) = listener.accept().await.expect("accept parser");
        let error = read_socks5_destination(&mut server)
            .await
            .expect_err("unsupported command");
        assert_eq!(error.reply_code, Some(SOCKS5_REPLY_COMMAND_NOT_SUPPORTED));
        assert_eq!(client.await.expect("client task"), [5, 0]);
    }
}
