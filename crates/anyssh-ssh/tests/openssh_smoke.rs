use std::{env, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{PasswordSessionConfig, SessionEvent, spawn_password_session};
use tokio::time::timeout;
use zeroize::Zeroizing;

#[tokio::test]
#[ignore = "requires the OpenSSH Docker fixture; run pnpm test:ssh:smoke"]
async fn password_shell_against_openssh() {
    let host = env::var("ANYSSH_TEST_SSH_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("ANYSSH_TEST_SSH_PORT")
        .expect("ANYSSH_TEST_SSH_PORT must be set")
        .parse::<u16>()
        .expect("test SSH port must be a valid u16");

    let spawned = spawn_password_session(PasswordSessionConfig {
        endpoint: SshEndpoint::new(host, port).expect("valid fixture endpoint"),
        username: "anyssh".to_owned(),
        password: Zeroizing::new("anyssh-test".to_owned()),
        terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
    });

    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut saw_host_key = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting | SessionEvent::Authenticated => {}
                SessionEvent::HostKey(info) => {
                    assert!(info.fingerprint_sha256.starts_with("SHA256:"));
                    saw_host_key = true;
                    control
                        .confirm_host_key(true)
                        .await
                        .expect("host key decision should reach the session");
                }
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input("printf 'ANYSSH_SMOKE_OK\\n'; exit\r")
                        .await
                        .expect("input should reach the session");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => panic!("SSH fixture session failed: {message}"),
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("SSH smoke test timed out");

    assert!(
        saw_host_key,
        "the client must request host-key confirmation"
    );
    assert!(saw_connected, "the client must open an interactive shell");
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_SMOKE_OK"),
        "remote command marker missing from output: {}",
        String::from_utf8_lossy(&output)
    );
}
