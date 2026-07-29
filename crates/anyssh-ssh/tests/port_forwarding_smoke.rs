use std::{env, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    JumpPasswordSessionConfig, MAX_PORT_FORWARD_CONNECTIONS, PasswordJumpHostConfig,
    PasswordSessionConfig, PortForwardError, PortForwardKind, PortForwardRequest, SessionControl,
    SessionEvent, SpawnedSession, spawn_jump_password_session, spawn_password_session,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

struct ForwardFixture {
    jump_host: String,
    jump_port: u16,
    target_host: String,
    echo_port: u16,
}

impl ForwardFixture {
    fn from_environment() -> Self {
        Self {
            jump_host: required_env("ANYSSH_TEST_JUMP_HOST"),
            jump_port: required_port("ANYSSH_TEST_JUMP_PORT"),
            target_host: required_env("ANYSSH_TEST_TARGET_HOST"),
            echo_port: required_port("ANYSSH_TEST_FORWARD_ECHO_PORT"),
        }
    }

    fn direct_config(&self) -> PasswordSessionConfig {
        password_config(&self.jump_host, self.jump_port)
    }

    fn jump_config(&self) -> JumpPasswordSessionConfig {
        JumpPasswordSessionConfig {
            jump_host: PasswordJumpHostConfig {
                endpoint: SshEndpoint::new(&self.jump_host, self.jump_port)
                    .expect("valid Jump Host endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
            },
            target: password_config(&self.target_host, 22),
            connection_timeout: Duration::from_secs(10),
        }
    }
}

enum Observation {
    Connected,
    Data(Vec<u8>),
    Error(String),
    Closed,
}

struct LiveSession {
    control: SessionControl,
    observations: mpsc::Receiver<Observation>,
    event_task: JoinHandle<()>,
}

impl LiveSession {
    async fn from_spawned(spawned: SpawnedSession) -> Self {
        let control = spawned.control;
        let event_control = control.clone();
        let (observation_sender, observations) = mpsc::channel(64);
        let event_task = tokio::spawn(async move {
            let mut events = spawned.events;
            while let Some(event) = events.recv().await {
                let observation = match event {
                    SessionEvent::Connecting
                    | SessionEvent::Authenticated
                    | SessionEvent::ExitStatus(_) => continue,
                    SessionEvent::HostKey(info) => {
                        if event_control
                            .confirm_host_key(info.request_id, true)
                            .await
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                    SessionEvent::HostKeyChanged(info) => {
                        Observation::Error(format!("fixture Host Key changed: {info:?}"))
                    }
                    SessionEvent::AuthenticationChallenge(info) => Observation::Error(format!(
                        "fixture requested unexpected interactive authentication: {info:?}"
                    )),
                    SessionEvent::Connected => Observation::Connected,
                    SessionEvent::Data(data) => Observation::Data(data.to_vec()),
                    SessionEvent::Error(message) => Observation::Error(message),
                    SessionEvent::Closed => Observation::Closed,
                };
                let closed = matches!(observation, Observation::Closed);
                if observation_sender.send(observation).await.is_err() || closed {
                    break;
                }
            }
        });
        let mut session = Self {
            control,
            observations,
            event_task,
        };
        session.wait_connected().await;
        session
    }

    async fn wait_connected(&mut self) {
        timeout(TEST_TIMEOUT, async {
            loop {
                match self.observations.recv().await {
                    Some(Observation::Connected) => break,
                    Some(Observation::Error(message)) => {
                        panic!("SSH session failed before connecting: {message}")
                    }
                    Some(Observation::Closed) | None => {
                        panic!("SSH session closed before connecting")
                    }
                    Some(Observation::Data(_)) => {}
                }
            }
        })
        .await
        .expect("SSH session did not connect");
    }

    async fn wait_for_output(&mut self, marker: &str) -> String {
        timeout(TEST_TIMEOUT, async {
            let mut output = Vec::new();
            loop {
                match self.observations.recv().await {
                    Some(Observation::Data(data)) => {
                        output.extend_from_slice(&data);
                        if String::from_utf8_lossy(&output).contains(marker) {
                            return String::from_utf8_lossy(&output).into_owned();
                        }
                    }
                    Some(Observation::Error(message)) => {
                        panic!("SSH session failed while waiting for output: {message}")
                    }
                    Some(Observation::Closed) | None => {
                        panic!("SSH session closed before output marker {marker:?}")
                    }
                    Some(Observation::Connected) => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for output marker {marker:?}"))
    }

    async fn disconnect(mut self) {
        let _ = self.control.disconnect().await;
        timeout(TEST_TIMEOUT, async {
            loop {
                match self.observations.recv().await {
                    Some(Observation::Closed) | None => break,
                    Some(Observation::Error(message)) => {
                        panic!("SSH session failed during disconnect: {message}")
                    }
                    Some(Observation::Connected) | Some(Observation::Data(_)) => {}
                }
            }
        })
        .await
        .expect("SSH session did not close");
        self.event_task.await.expect("SSH event task");
    }
}

#[tokio::test]
#[ignore = "requires the isolated OpenSSH Docker topology; run pnpm test:ssh:smoke"]
async fn local_and_dynamic_forwarding_use_the_direct_target_network() {
    let fixture = ForwardFixture::from_environment();
    let session = LiveSession::from_spawned(spawn_password_session(fixture.direct_config())).await;

    let local = session
        .control
        .start_port_forward(forward_request(
            PortForwardKind::Local,
            Some((&fixture.target_host, fixture.echo_port)),
        ))
        .await
        .expect("start Local forward");
    let payload = large_payload("ANYSSH_LOCAL_DIRECT", 4 * 1024 * 1024);
    assert_eq!(
        round_trip(local.bound_port(), &payload).await,
        payload,
        "Local forward changed the payload"
    );
    session
        .control
        .stop_port_forward(local.id().to_owned())
        .await
        .expect("stop Local forward");
    session
        .control
        .stop_port_forward(local.id().to_owned())
        .await
        .expect("repeated Local stop is idempotent");
    assert_listener_closed(local.bound_port()).await;

    let dynamic = session
        .control
        .start_port_forward(forward_request(PortForwardKind::Dynamic, None))
        .await
        .expect("start Dynamic forward");
    let dynamic_payload = large_payload("ANYSSH_SOCKS_DIRECT", 256 * 1024);
    assert_eq!(
        socks5_round_trip(
            dynamic.bound_port(),
            &fixture.target_host,
            fixture.echo_port,
            &dynamic_payload,
        )
        .await,
        dynamic_payload,
        "Dynamic forward changed the payload"
    );
    assert_socks5_bind_is_rejected(dynamic.bound_port()).await;
    session
        .control
        .stop_port_forward(dynamic.id().to_owned())
        .await
        .expect("stop Dynamic forward");
    assert_listener_closed(dynamic.bound_port()).await;

    let limited = session
        .control
        .start_port_forward(forward_request(
            PortForwardKind::Local,
            Some((&fixture.target_host, fixture.echo_port)),
        ))
        .await
        .expect("start connection-limited Local forward");
    let mut held_connections = Vec::with_capacity(MAX_PORT_FORWARD_CONNECTIONS);
    for index in 0..MAX_PORT_FORWARD_CONNECTIONS {
        let mut stream = TcpStream::connect(("127.0.0.1", limited.bound_port()))
            .await
            .expect("connect within per-Forward limit");
        let marker = [u8::try_from(index).expect("connection marker")];
        stream
            .write_all(&marker)
            .await
            .expect("write connection marker");
        let mut echoed = [0_u8; 1];
        timeout(TEST_TIMEOUT, stream.read_exact(&mut echoed))
            .await
            .expect("connection marker echo timed out")
            .expect("read connection marker");
        assert_eq!(echoed, marker);
        held_connections.push(stream);
    }
    assert_connection_is_rejected(limited.bound_port()).await;
    session
        .control
        .stop_port_forward(limited.id().to_owned())
        .await
        .expect("stop connection-limited Local forward");
    drop(held_connections);

    let mut active = Vec::new();
    for _ in 0..16 {
        active.push(
            session
                .control
                .start_port_forward(forward_request(PortForwardKind::Dynamic, None))
                .await
                .expect("start forward within the per-Session limit"),
        );
    }
    assert_eq!(
        session
            .control
            .start_port_forward(forward_request(PortForwardKind::Dynamic, None))
            .await,
        Err(PortForwardError::TooManyForwards)
    );
    for forward in active {
        session
            .control
            .stop_port_forward(forward.id().to_owned())
            .await
            .expect("stop limited Dynamic forward");
    }

    let late_control = session.control.clone();
    session.disconnect().await;
    assert_eq!(
        late_control
            .start_port_forward(forward_request(PortForwardKind::Dynamic, None))
            .await,
        Err(PortForwardError::SessionClosed)
    );
}

#[tokio::test]
#[ignore = "requires the isolated OpenSSH Docker topology; run pnpm test:ssh:smoke"]
async fn forwarding_over_a_jump_route_handles_remote_channels_and_cleanup() {
    let fixture = ForwardFixture::from_environment();
    let mut session =
        LiveSession::from_spawned(spawn_jump_password_session(fixture.jump_config())).await;
    session
        .control
        .send_input("stty -echo\r")
        .await
        .expect("disable fixture PTY echo");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let local = session
        .control
        .start_port_forward(forward_request(
            PortForwardKind::Local,
            Some(("127.0.0.1", fixture.echo_port)),
        ))
        .await
        .expect("start Jump Route Local forward");
    let payload = large_payload("ANYSSH_LOCAL_JUMP", 512 * 1024);
    assert_eq!(round_trip(local.bound_port(), &payload).await, payload);

    let dynamic = session
        .control
        .start_port_forward(forward_request(PortForwardKind::Dynamic, None))
        .await
        .expect("start Jump Route Dynamic forward");
    let dynamic_payload = b"ANYSSH_SOCKS_JUMP\n".repeat(4096);
    assert_eq!(
        socks5_round_trip(
            dynamic.bound_port(),
            "127.0.0.1",
            fixture.echo_port,
            &dynamic_payload,
        )
        .await,
        dynamic_payload
    );

    let (destination_port, destination_task, request_receiver) = spawn_remote_destination().await;
    let remote = session
        .control
        .start_port_forward(
            PortForwardRequest::new(
                PortForwardKind::Remote,
                "127.0.0.1",
                0,
                Some("127.0.0.1".to_owned()),
                Some(destination_port),
            )
            .expect("valid Remote forward request"),
        )
        .await
        .expect("start Remote forward");
    session
        .control
        .send_input(format!(
            "printf 'ANYSSH_REMOTE_REQUEST\\n' | nc -w 5 127.0.0.1 {}; printf 'ANYSSH_REMOTE_COMMAND_DONE\\n'\r",
            remote.bound_port()
        ))
        .await
        .expect("trigger the remote listener from the Target shell");
    let output = session.wait_for_output("ANYSSH_REMOTE_COMMAND_DONE").await;
    assert!(
        output.contains("ANYSSH_REMOTE_OK"),
        "Remote forward response was absent: {output}"
    );
    assert_eq!(
        timeout(TEST_TIMEOUT, request_receiver)
            .await
            .expect("Remote destination request timed out")
            .expect("Remote destination request sender"),
        b"ANYSSH_REMOTE_REQUEST\n"
    );
    destination_task.await.expect("Remote destination task");

    session
        .control
        .stop_port_forward(remote.id().to_owned())
        .await
        .expect("cancel Remote forward registration");
    session
        .control
        .send_input(format!(
            "if nc -z -w 1 127.0.0.1 {}; then printf 'ANYSSH_REMOTE_STILL_OPEN\\n'; else printf 'ANYSSH_REMOTE_STOPPED\\n'; fi\r",
            remote.bound_port()
        ))
        .await
        .expect("probe cancelled Remote listener");
    let stopped_output = session.wait_for_output("ANYSSH_REMOTE_STOPPED").await;
    assert!(!stopped_output.contains("ANYSSH_REMOTE_STILL_OPEN"));

    let cleanup_port = local.bound_port();
    session.disconnect().await;
    assert_listener_closed(cleanup_port).await;
}

fn password_config(host: &str, port: u16) -> PasswordSessionConfig {
    PasswordSessionConfig {
        endpoint: SshEndpoint::new(host, port).expect("valid fixture endpoint"),
        username: FIXTURE_USERNAME.to_owned(),
        password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
    }
}

fn forward_request(kind: PortForwardKind, destination: Option<(&str, u16)>) -> PortForwardRequest {
    PortForwardRequest::new(
        kind,
        "127.0.0.1",
        0,
        destination.map(|(host, _)| host.to_owned()),
        destination.map(|(_, port)| port),
    )
    .expect("valid test forward request")
}

fn large_payload(prefix: &str, size: usize) -> Vec<u8> {
    let mut payload = Vec::with_capacity(size);
    while payload.len() < size {
        payload.extend_from_slice(prefix.as_bytes());
        payload.push(b'\n');
    }
    payload.truncate(size);
    payload
}

async fn round_trip(port: u16, payload: &[u8]) -> Vec<u8> {
    timeout(TEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect Local forward");
        stream
            .write_all(payload)
            .await
            .expect("write forward payload");
        stream.shutdown().await.expect("half-close forward input");
        let mut echoed = Vec::with_capacity(payload.len());
        stream
            .read_to_end(&mut echoed)
            .await
            .expect("read forwarded echo");
        echoed
    })
    .await
    .expect("forward round trip timed out")
}

async fn socks5_round_trip(
    proxy_port: u16,
    destination_host: &str,
    destination_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    timeout(TEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(("127.0.0.1", proxy_port))
            .await
            .expect("connect Dynamic forward");
        stream
            .write_all(&[5, 1, 0])
            .await
            .expect("write SOCKS5 greeting");
        let mut greeting = [0_u8; 2];
        stream
            .read_exact(&mut greeting)
            .await
            .expect("read SOCKS5 greeting");
        assert_eq!(greeting, [5, 0]);

        let host = destination_host.as_bytes();
        assert!(host.len() <= u8::MAX as usize);
        let mut request = vec![5, 1, 0, 3, host.len() as u8];
        request.extend_from_slice(host);
        request.extend_from_slice(&destination_port.to_be_bytes());
        stream
            .write_all(&request)
            .await
            .expect("write SOCKS5 CONNECT");
        read_socks5_reply(&mut stream, 0).await;

        stream
            .write_all(payload)
            .await
            .expect("write SOCKS5 payload");
        stream.shutdown().await.expect("half-close SOCKS5 input");
        let mut echoed = Vec::with_capacity(payload.len());
        stream
            .read_to_end(&mut echoed)
            .await
            .expect("read SOCKS5 echo");
        echoed
    })
    .await
    .expect("SOCKS5 round trip timed out")
}

async fn assert_socks5_bind_is_rejected(proxy_port: u16) {
    timeout(TEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(("127.0.0.1", proxy_port))
            .await
            .expect("connect Dynamic forward for BIND rejection");
        stream.write_all(&[5, 1, 0]).await.expect("SOCKS greeting");
        let mut greeting = [0_u8; 2];
        stream
            .read_exact(&mut greeting)
            .await
            .expect("SOCKS method");
        assert_eq!(greeting, [5, 0]);
        stream
            .write_all(&[5, 2, 0, 1, 127, 0, 0, 1, 0, 22])
            .await
            .expect("SOCKS BIND request");
        read_socks5_reply(&mut stream, 7).await;
    })
    .await
    .expect("SOCKS5 BIND rejection timed out");
}

async fn read_socks5_reply(stream: &mut TcpStream, expected_reply: u8) {
    let mut header = [0_u8; 4];
    stream
        .read_exact(&mut header)
        .await
        .expect("read SOCKS5 reply header");
    assert_eq!(header[0], 5);
    assert_eq!(header[1], expected_reply);
    let address_bytes = match header[3] {
        1 => 4,
        3 => {
            let mut length = [0_u8; 1];
            stream
                .read_exact(&mut length)
                .await
                .expect("read SOCKS5 reply domain length");
            length[0] as usize
        }
        4 => 16,
        atyp => panic!("unexpected SOCKS5 reply address type {atyp}"),
    };
    let mut address_and_port = vec![0_u8; address_bytes + 2];
    stream
        .read_exact(&mut address_and_port)
        .await
        .expect("read SOCKS5 reply address");
}

async fn spawn_remote_destination() -> (u16, JoinHandle<()>, oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind Remote local destination");
    let port = listener.local_addr().expect("destination address").port();
    let (request_sender, request_receiver) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept Remote channel");
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while request.len() < 4096 {
            if stream.read_exact(&mut byte).await.is_err() {
                break;
            }
            request.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        request_sender.send(request).expect("report Remote request");
        stream
            .write_all(b"ANYSSH_REMOTE_OK\n")
            .await
            .expect("write Remote response");
        stream.shutdown().await.expect("close Remote response");
    });
    (port, task, request_receiver)
}

async fn assert_listener_closed(port: u16) {
    timeout(Duration::from_secs(3), async {
        loop {
            if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("forward listener on 127.0.0.1:{port} remained open"));
}

async fn assert_connection_is_rejected(port: u16) {
    timeout(Duration::from_secs(3), async {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("TCP accept may race the connection limit");
        let _ = stream.write_all(b"x").await;
        let mut byte = [0_u8; 1];
        match stream.read(&mut byte).await {
            Ok(0) | Err(_) => {}
            Ok(count) => panic!("connection beyond the limit returned {count} payload byte(s)"),
        }
    })
    .await
    .expect("connection beyond the per-Forward limit was not rejected");
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by scripts/test-ssh-smoke.sh"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16 port"))
}
