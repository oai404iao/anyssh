use std::{env, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    HostKeyPolicy, SessionAuthentication, SessionEvent, SshConnectionConfig, SshSessionConfig,
    list_system_agent_identities, spawn_session,
};
use tokio::time::timeout;

const FIXTURE_USERNAME: &str = "anyssh";

#[tokio::test]
#[ignore = "requires the OpenSSH Docker fixture and ssh-agent; run pnpm test:ssh:smoke"]
async fn selected_system_agent_identity_authenticates_without_private_key_material() {
    let host = required_env("ANYSSH_TEST_SSH_HOST");
    let port = required_port("ANYSSH_TEST_SSH_PORT");
    let fingerprint = required_env("ANYSSH_TEST_AGENT_FINGERPRINT");

    let identities = list_system_agent_identities()
        .await
        .expect("list system Agent identities");
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].fingerprint_sha256(), fingerprint);
    assert!(identities[0].algorithm().contains("ed25519"));

    let spawned = spawn_session(SshSessionConfig {
        target: SshConnectionConfig {
            endpoint: SshEndpoint::new(host, port).expect("fixture endpoint"),
            username: FIXTURE_USERNAME.to_owned(),
            authentication: SessionAuthentication::SystemAgent {
                identity_fingerprint_sha256: fingerprint.clone(),
            },
            host_key_policy: HostKeyPolicy::Prompt,
        },
        jump_hosts: Vec::new(),
        terminal_size: TerminalSize::new(100, 30).expect("terminal size"),
        connection_timeout: Duration::from_secs(10),
    });
    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut connected = false;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting | SessionEvent::Authenticated => {}
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("confirm Host Key");
                }
                SessionEvent::Connected => {
                    connected = true;
                    control
                        .send_input("printf 'ANYSSH_AGENT_OK\\n'; exit\r")
                        .await
                        .expect("send Agent-authenticated command");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("system Agent session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("system Agent session timed out");

    assert!(connected, "system Agent session did not connect");
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_AGENT_OK"),
        "system Agent marker missing: {}",
        String::from_utf8_lossy(&output)
    );

    let spawned = spawn_session(SshSessionConfig {
        target: SshConnectionConfig {
            endpoint: SshEndpoint::new("127.0.0.1", port).expect("fixture endpoint"),
            username: FIXTURE_USERNAME.to_owned(),
            authentication: SessionAuthentication::SystemAgent {
                identity_fingerprint_sha256: "SHA256:identity-is-not-loaded".to_owned(),
            },
            host_key_policy: HostKeyPolicy::Prompt,
        },
        jump_hosts: Vec::new(),
        terminal_size: TerminalSize::default(),
        connection_timeout: Duration::from_secs(10),
    });
    let control = spawned.control;
    let mut events = spawned.events;
    let mut error = None;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("confirm Host Key");
                }
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Connecting
                | SessionEvent::Authenticated
                | SessionEvent::Connected
                | SessionEvent::Data(_)
                | SessionEvent::ExitStatus(_) => {}
            }
        }
    })
    .await
    .expect("missing identity failure timed out");

    let error = error.expect("missing identity must fail");
    assert!(
        error.contains("selected SSH Agent identity is no longer available"),
        "unexpected missing identity error: {error}"
    );
    assert!(!error.contains(&fingerprint));
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
