use std::{env, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    HostKeyPolicy, SessionAuthentication, SessionEvent, SessionHop, SpawnedSession,
    SshConnectionConfig, SshSessionConfig, spawn_session,
};
use tokio::{process::Command, time::timeout};
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

struct HostKeyFixture {
    host: String,
    port: u16,
    container: String,
}

impl HostKeyFixture {
    fn from_environment() -> Self {
        Self {
            host: required_env("ANYSSH_TEST_JUMP_HOST"),
            port: required_port("ANYSSH_TEST_JUMP_PORT"),
            container: required_env("ANYSSH_TEST_JUMP_CONTAINER"),
        }
    }

    fn session(&self, host_key_policy: HostKeyPolicy) -> SpawnedSession {
        spawn_session(SshSessionConfig {
            target: SshConnectionConfig {
                endpoint: SshEndpoint::new(&self.host, self.port).expect("valid fixture endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                authentication: SessionAuthentication::Password {
                    password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
                },
                host_key_policy,
            },
            jump_hosts: Vec::new(),
            terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
            connection_timeout: Duration::from_secs(10),
        })
    }
}

#[tokio::test]
#[ignore = "requires the mutable OpenSSH host-key fixture; run pnpm test:ssh:smoke"]
async fn changed_host_key_is_blocked_without_a_second_prompt() {
    let fixture = HostKeyFixture::from_environment();

    let first = expect_success(fixture.session(HostKeyPolicy::Prompt), true).await;
    let trusted_fingerprint = first.expect("TOFU connection must expose a fingerprint");

    let matching = expect_success(
        fixture.session(HostKeyPolicy::RequireSha256Set {
            fingerprints: vec![
                "SHA256:another-trusted-algorithm-slot".to_owned(),
                trusted_fingerprint.clone(),
            ],
        }),
        false,
    )
    .await;
    assert!(
        matching.is_none(),
        "a matching saved fingerprint should not prompt again"
    );

    rotate_host_keys(&fixture).await;

    let spawned = fixture.session(HostKeyPolicy::RequireSha256Set {
        fingerprints: vec![trusted_fingerprint.clone()],
    });
    let mut events = spawned.events;
    let mut changed = None;
    let mut saw_host_key_prompt = false;
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(15), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(_) => saw_host_key_prompt = true,
                SessionEvent::HostKeyChanged(info) => changed = Some(info),
                SessionEvent::AuthenticationChallenge(info) => {
                    panic!("fixture unexpectedly requested interactive authentication: {info:?}")
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => saw_connected = true,
                SessionEvent::Error(message) => {
                    panic!("changed host key emitted an unexpected generic error: {message}")
                }
                SessionEvent::Closed => break,
                SessionEvent::Data(_) | SessionEvent::ExitStatus(_) => {}
            }
        }
    })
    .await
    .expect("changed host-key scenario timed out");

    assert!(
        !saw_host_key_prompt,
        "a changed saved host key must be blocked instead of prompting"
    );
    assert!(!saw_authenticated);
    assert!(!saw_connected);
    let changed = changed.expect("changed host key must emit typed evidence");
    assert_eq!(changed.hop, SessionHop::Target);
    assert!(
        changed
            .trusted_fingerprints_sha256
            .contains(&trusted_fingerprint),
        "changed-key evidence did not identify the trusted fingerprint"
    );
    assert_ne!(changed.received_fingerprint_sha256, trusted_fingerprint);
}

async fn expect_success(spawned: SpawnedSession, expect_prompt: bool) -> Option<String> {
    let control = spawned.control;
    let mut events = spawned.events;
    let mut fingerprint = None;
    let mut output = Vec::new();
    let mut saw_connected = false;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting | SessionEvent::Authenticated => {}
                SessionEvent::HostKey(info) => {
                    assert_eq!(info.hop, SessionHop::Target);
                    fingerprint = Some(info.fingerprint_sha256);
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("TOFU host-key decision should reach the session");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("verified host unexpectedly changed: {info:?}")
                }
                SessionEvent::AuthenticationChallenge(info) => {
                    panic!("fixture unexpectedly requested interactive authentication: {info:?}")
                }
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input("printf 'ANYSSH_HOST_KEY_OK\\n'; exit\r")
                        .await
                        .expect("input should reach the verified session");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("host-key fixture session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("host-key success scenario timed out");

    assert!(saw_connected);
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_HOST_KEY_OK"),
        "verified session command marker missing"
    );
    assert_eq!(fingerprint.is_some(), expect_prompt);
    fingerprint
}

async fn rotate_host_keys(fixture: &HostKeyFixture) {
    let rotate = Command::new("docker")
        .args([
            "exec",
            fixture.container.as_str(),
            "sh",
            "-c",
            "rm -f /etc/ssh/ssh_host_* && ssh-keygen -A >/dev/null 2>&1 && kill -HUP 1",
        ])
        .output()
        .await
        .expect("docker CLI should rotate fixture host keys");
    assert!(
        rotate.status.success(),
        "failed to rotate fixture host keys: {}",
        String::from_utf8_lossy(&rotate.stderr)
    );

    timeout(Duration::from_secs(10), async {
        loop {
            let port = fixture.port.to_string();
            let scan = Command::new("ssh-keyscan")
                .args(["-p", port.as_str(), fixture.host.as_str()])
                .output()
                .await
                .expect("ssh-keyscan should be available");
            if scan.status.success() && !scan.stdout.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("rotated host-key fixture did not become ready");
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
