use std::{env, fs, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    HostKeyPolicy, PrivateKeySessionConfig, SessionAuthentication, SessionEvent, SessionHop,
    SpawnedSession, SshConnectionConfig, SshSessionConfig, spawn_private_key_session,
    spawn_session,
};
use tokio::time::timeout;
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

struct PrivateKeyFixture {
    jump_host: String,
    jump_port: u16,
    target_host: String,
    target_port: u16,
    unencrypted_key_path: String,
    encrypted_key_path: String,
    unauthorized_key_path: String,
    key_passphrase: String,
}

impl PrivateKeyFixture {
    fn from_environment() -> Self {
        Self {
            jump_host: required_env("ANYSSH_TEST_JUMP_HOST"),
            jump_port: required_port("ANYSSH_TEST_JUMP_PORT"),
            target_host: required_env("ANYSSH_TEST_TARGET_HOST"),
            target_port: required_port("ANYSSH_TEST_TARGET_PORT"),
            unencrypted_key_path: required_env("ANYSSH_TEST_UNENCRYPTED_KEY"),
            encrypted_key_path: required_env("ANYSSH_TEST_ENCRYPTED_KEY"),
            unauthorized_key_path: required_env("ANYSSH_TEST_UNAUTHORIZED_KEY"),
            key_passphrase: required_env("ANYSSH_TEST_KEY_PASSPHRASE"),
        }
    }

    fn direct_config(
        &self,
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    ) -> PrivateKeySessionConfig {
        PrivateKeySessionConfig {
            endpoint: SshEndpoint::new(&self.jump_host, self.jump_port)
                .expect("valid direct fixture endpoint"),
            username: FIXTURE_USERNAME.to_owned(),
            private_key,
            passphrase,
            terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
        }
    }
}

#[tokio::test]
#[ignore = "requires the isolated OpenSSH key fixture; run pnpm test:ssh:smoke"]
async fn private_key_authentication_suite_against_openssh() {
    let fixture = PrivateKeyFixture::from_environment();

    let direct_unencrypted = spawn_private_key_session(
        fixture.direct_config(read_key(&fixture.unencrypted_key_path), None),
    );
    let direct_host_keys =
        expect_shell_success(direct_unencrypted, "ANYSSH_KEY_UNENCRYPTED_OK").await;
    assert_eq!(direct_host_keys, vec![SessionHop::Target]);

    let direct_encrypted = spawn_private_key_session(fixture.direct_config(
        read_key(&fixture.encrypted_key_path),
        Some(Zeroizing::new(fixture.key_passphrase.clone())),
    ));
    let encrypted_host_keys =
        expect_shell_success(direct_encrypted, "ANYSSH_KEY_ENCRYPTED_OK").await;
    assert_eq!(encrypted_host_keys, vec![SessionHop::Target]);

    let mixed_route = spawn_session(SshSessionConfig {
        target: SshConnectionConfig {
            endpoint: SshEndpoint::new(&fixture.target_host, fixture.target_port)
                .expect("valid target endpoint"),
            username: FIXTURE_USERNAME.to_owned(),
            authentication: SessionAuthentication::PrivateKey {
                private_key: read_key(&fixture.encrypted_key_path),
                passphrase: Some(Zeroizing::new(fixture.key_passphrase.clone())),
            },
            host_key_policy: HostKeyPolicy::Prompt,
        },
        jump_host: Some(SshConnectionConfig {
            endpoint: SshEndpoint::new(&fixture.jump_host, fixture.jump_port)
                .expect("valid jump endpoint"),
            username: FIXTURE_USERNAME.to_owned(),
            authentication: SessionAuthentication::Password {
                password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
            },
            host_key_policy: HostKeyPolicy::Prompt,
        }),
        terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
        connection_timeout: Duration::from_secs(10),
    });
    let mixed_host_keys = expect_shell_success(mixed_route, "ANYSSH_KEY_JUMP_OK").await;
    assert_eq!(
        mixed_host_keys,
        vec![SessionHop::JumpHost { index: 1 }, SessionHop::Target]
    );

    let wrong_passphrase = spawn_private_key_session(fixture.direct_config(
        read_key(&fixture.encrypted_key_path),
        Some(Zeroizing::new("incorrect-passphrase".to_owned())),
    ));
    let decode_error = expect_failure(wrong_passphrase).await;
    assert!(
        decode_error.contains("target private key could not be decoded"),
        "wrong-passphrase error was unexpected: {decode_error}"
    );
    assert!(!decode_error.contains("incorrect-passphrase"));
    assert!(!decode_error.contains("BEGIN OPENSSH PRIVATE KEY"));

    let unauthorized = spawn_private_key_session(
        fixture.direct_config(read_key(&fixture.unauthorized_key_path), None),
    );
    let authentication_error = expect_failure(unauthorized).await;
    assert!(
        authentication_error.contains("target authentication failed"),
        "unauthorized-key error was unexpected: {authentication_error}"
    );
    assert!(!authentication_error.contains("BEGIN OPENSSH PRIVATE KEY"));
}

async fn expect_shell_success(spawned: SpawnedSession, marker: &str) -> Vec<SessionHop> {
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
                    host_keys.push(info.hop.clone());
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("host-key decision should reach the active hop");
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input(format!("printf '{marker}\\n'; exit\r"))
                        .await
                        .expect("input should reach the key-authenticated shell");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("private-key fixture session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("private-key success scenario timed out");

    assert!(saw_authenticated, "authentication event missing");
    assert!(saw_connected, "interactive shell was not opened");
    assert!(
        String::from_utf8_lossy(&output).contains(marker),
        "remote marker missing from output: {}",
        String::from_utf8_lossy(&output)
    );

    host_keys
}

async fn expect_failure(spawned: SpawnedSession) -> String {
    let control = spawned.control;
    let mut events = spawned.events;
    let mut error = None;
    let mut saw_authenticated = false;
    let mut saw_connected = false;

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
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => saw_connected = true,
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Data(_) | SessionEvent::ExitStatus(_) => {}
            }
        }
    })
    .await
    .expect("private-key failure scenario timed out");

    assert!(!saw_authenticated);
    assert!(!saw_connected);
    error.expect("private-key failure must emit an error")
}

fn read_key(path: &str) -> Zeroizing<String> {
    Zeroizing::new(fs::read_to_string(path).expect("fixture private key should be readable"))
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
