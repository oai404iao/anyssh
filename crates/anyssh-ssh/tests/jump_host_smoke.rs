use std::{env, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    HostKeyInfo, JumpPasswordSessionConfig, PasswordJumpHostConfig, PasswordSessionConfig,
    SessionEvent, SessionHop, spawn_jump_password_session, spawn_password_session,
};
use tokio::{net::TcpStream, process::Command, time::timeout};
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

struct JumpFixture {
    jump_host: String,
    jump_port: u16,
    target_host: String,
    target_ip: String,
    target_port: u16,
    blackhole_host: String,
    blackhole_port: u16,
    jump_container: String,
}

impl JumpFixture {
    fn from_environment() -> Self {
        Self {
            jump_host: required_env("ANYSSH_TEST_JUMP_HOST"),
            jump_port: required_port("ANYSSH_TEST_JUMP_PORT"),
            target_host: required_env("ANYSSH_TEST_TARGET_HOST"),
            target_ip: required_env("ANYSSH_TEST_TARGET_IP"),
            target_port: required_port("ANYSSH_TEST_TARGET_PORT"),
            blackhole_host: required_env("ANYSSH_TEST_BLACKHOLE_HOST"),
            blackhole_port: required_port("ANYSSH_TEST_BLACKHOLE_PORT"),
            jump_container: required_env("ANYSSH_TEST_JUMP_CONTAINER"),
        }
    }

    fn config(
        &self,
        target_host: &str,
        target_port: u16,
        target_password: &str,
        connection_timeout: Duration,
    ) -> JumpPasswordSessionConfig {
        JumpPasswordSessionConfig {
            jump_host: PasswordJumpHostConfig {
                endpoint: SshEndpoint::new(&self.jump_host, self.jump_port)
                    .expect("valid jump endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
            },
            target: PasswordSessionConfig {
                endpoint: SshEndpoint::new(target_host, target_port)
                    .expect("valid target endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                password: Zeroizing::new(target_password.to_owned()),
                terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
            },
            connection_timeout,
        }
    }
}

#[tokio::test]
#[ignore = "requires the isolated Jump Host Docker topology; run pnpm test:ssh:smoke"]
async fn password_jump_host_suite_against_openssh() {
    let fixture = JumpFixture::from_environment();

    target_is_not_directly_reachable(&fixture).await;
    jump_shell_reaches_internal_target(&fixture).await;
    cancellation_interrupts_host_key_wait(&fixture).await;
    target_authentication_failure_is_scoped(&fixture).await;
    target_handshake_timeout_is_bounded(&fixture).await;
    first_hop_loss_closes_the_target_session(&fixture).await;
}

async fn target_is_not_directly_reachable(fixture: &JumpFixture) {
    let unresolved_alias = timeout(
        Duration::from_secs(2),
        TcpStream::connect((fixture.target_host.as_str(), fixture.target_port)),
    )
    .await;

    assert!(
        !matches!(unresolved_alias, Ok(Ok(_))),
        "the internal target alias unexpectedly resolved for the direct client"
    );

    let spawned = spawn_password_session(PasswordSessionConfig {
        endpoint: SshEndpoint::new(&fixture.target_ip, fixture.target_port)
            .expect("valid direct probe endpoint"),
        username: FIXTURE_USERNAME.to_owned(),
        password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        terminal_size: TerminalSize::new(80, 24).expect("valid terminal size"),
    });
    let control = spawned.control;
    let mut events = spawned.events;
    let mut error = None;
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(10), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("direct probe host-key decision should be accepted");
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => saw_connected = true,
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Data(_) | SessionEvent::ExitStatus(_) => {}
            }
        }
    })
    .await
    .expect("direct target access probe timed out");

    assert!(
        !saw_authenticated && !saw_connected,
        "the internal target authenticated a client that bypassed the Jump Host"
    );
    let error = error.expect("direct target access must be rejected");
    assert!(
        error.contains("target authentication failed"),
        "direct target rejection was unexpected: {error}"
    );
}

async fn jump_shell_reaches_internal_target(fixture: &JumpFixture) {
    let spawned = spawn_jump_password_session(fixture.config(
        &fixture.target_host,
        fixture.target_port,
        FIXTURE_PASSWORD,
        Duration::from_secs(10),
    ));
    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut host_keys = Vec::new();
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(25), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("host-key decision should reach the active hop");
                    host_keys.push(info);
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input("printf 'ANYSSH_JUMP_OK\\n'; exit\r")
                        .await
                        .expect("input should reach the target shell");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("Jump Host fixture session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("Jump Host success scenario timed out");

    assert_eq!(
        host_keys.len(),
        2,
        "both the jump and target host keys must be confirmed"
    );
    assert_host_key_scope(
        &host_keys[0],
        SessionHop::JumpHost { index: 1 },
        &fixture.jump_host,
        fixture.jump_port,
    );
    assert_host_key_scope(
        &host_keys[1],
        SessionHop::Target,
        &fixture.target_host,
        fixture.target_port,
    );
    assert_ne!(
        host_keys[0].fingerprint_sha256, host_keys[1].fingerprint_sha256,
        "fixture containers must expose distinct host identities"
    );
    assert!(saw_authenticated, "target authentication event missing");
    assert!(saw_connected, "target shell was not opened");
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_JUMP_OK"),
        "target command marker missing from output: {}",
        String::from_utf8_lossy(&output)
    );
}

async fn cancellation_interrupts_host_key_wait(fixture: &JumpFixture) {
    let spawned = spawn_jump_password_session(fixture.config(
        &fixture.target_host,
        fixture.target_port,
        FIXTURE_PASSWORD,
        Duration::from_secs(10),
    ));
    let control = spawned.control;
    let mut events = spawned.events;
    let mut saw_jump_host_key = false;
    let mut saw_closed = false;

    timeout(Duration::from_secs(4), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    assert_eq!(info.hop, SessionHop::JumpHost { index: 1 });
                    saw_jump_host_key = true;
                    control
                        .disconnect()
                        .await
                        .expect("cancellation should reach the handshake");
                }
                SessionEvent::Closed => {
                    saw_closed = true;
                    break;
                }
                SessionEvent::Error(message) => {
                    panic!("cancellation must close cleanly, got: {message}")
                }
                SessionEvent::Authenticated
                | SessionEvent::Connected
                | SessionEvent::Data(_)
                | SessionEvent::ExitStatus(_) => {
                    panic!("session advanced after cancellation")
                }
            }
        }
    })
    .await
    .expect("cancellation did not interrupt host-key confirmation");

    assert!(saw_jump_host_key);
    assert!(saw_closed);
}

async fn target_authentication_failure_is_scoped(fixture: &JumpFixture) {
    let spawned = spawn_jump_password_session(fixture.config(
        &fixture.target_host,
        fixture.target_port,
        "incorrect-target-password",
        Duration::from_secs(10),
    ));
    let control = spawned.control;
    let mut events = spawned.events;
    let mut host_key_count = 0;
    let mut error = None;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    host_key_count += 1;
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("host-key decision should reach the active hop");
                }
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Connected => {
                    panic!("target shell opened with an incorrect target password")
                }
                SessionEvent::Authenticated
                | SessionEvent::Data(_)
                | SessionEvent::ExitStatus(_) => {}
            }
        }
    })
    .await
    .expect("target authentication failure scenario timed out");

    assert_eq!(host_key_count, 2);
    let error = error.expect("target authentication failure must emit an error");
    assert!(
        error.contains("target authentication failed"),
        "authentication error was not scoped to the target: {error}"
    );
    assert!(
        !error.contains(FIXTURE_PASSWORD),
        "authentication error exposed a fixture password"
    );
}

async fn target_handshake_timeout_is_bounded(fixture: &JumpFixture) {
    let spawned = spawn_jump_password_session(fixture.config(
        &fixture.blackhole_host,
        fixture.blackhole_port,
        FIXTURE_PASSWORD,
        Duration::from_secs(2),
    ));
    let control = spawned.control;
    let mut events = spawned.events;
    let mut host_keys = Vec::new();
    let mut error = None;

    timeout(Duration::from_secs(8), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    host_keys.push(info.hop.clone());
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("jump host-key decision should be accepted");
                }
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Authenticated
                | SessionEvent::Connected
                | SessionEvent::Data(_)
                | SessionEvent::ExitStatus(_) => {
                    panic!("blackhole target unexpectedly advanced the SSH session")
                }
            }
        }
    })
    .await
    .expect("target handshake timeout was not bounded");

    assert_eq!(host_keys, vec![SessionHop::JumpHost { index: 1 }]);
    let error = error.expect("target timeout must emit an error");
    assert!(
        error.contains("target SSH handshake timed out"),
        "timeout error was not scoped to the target handshake: {error}"
    );
}

async fn first_hop_loss_closes_the_target_session(fixture: &JumpFixture) {
    let spawned = spawn_jump_password_session(fixture.config(
        &fixture.target_host,
        fixture.target_port,
        FIXTURE_PASSWORD,
        Duration::from_secs(10),
    ));
    let control = spawned.control;
    let mut events = spawned.events;
    let mut saw_connected = false;
    let mut jump_was_killed = false;
    let mut saw_closed = false;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("host-key decision should reach the active hop");
                }
                SessionEvent::Authenticated => {}
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input("sleep 30\r")
                        .await
                        .expect("target shell should accept input before first-hop loss");

                    let output = Command::new("docker")
                        .args(["kill", fixture.jump_container.as_str()])
                        .output()
                        .await
                        .expect("docker CLI should kill the jump fixture");
                    assert!(
                        output.status.success(),
                        "docker failed to kill the jump fixture: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    jump_was_killed = true;
                }
                SessionEvent::Error(_) | SessionEvent::Data(_) | SessionEvent::ExitStatus(_) => {}
                SessionEvent::Closed => {
                    saw_closed = true;
                    break;
                }
            }
        }
    })
    .await
    .expect("target session did not close after first-hop loss");

    assert!(saw_connected, "target session never connected");
    assert!(jump_was_killed, "jump fixture was not terminated");
    assert!(
        saw_closed,
        "target session stayed open after first-hop loss"
    );
}

fn assert_host_key_scope(
    info: &HostKeyInfo,
    expected_hop: SessionHop,
    expected_host: &str,
    expected_port: u16,
) {
    assert_eq!(info.hop, expected_hop);
    assert_eq!(info.endpoint.host, expected_host);
    assert_eq!(info.endpoint.port, expected_port);
    assert!(info.fingerprint_sha256.starts_with("SHA256:"));
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
