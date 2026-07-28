use std::{env, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    PasswordSessionConfig, SESSION_EVENT_BUFFER_CAPACITY, SessionEvent, spawn_password_session,
};
use tokio::time::timeout;
use zeroize::Zeroizing;

const OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[tokio::test]
#[ignore = "requires the OpenSSH output fixture; run pnpm test:ssh:smoke"]
async fn terminal_output_applies_bounded_backpressure() {
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
    let mut output = Vec::with_capacity(OUTPUT_BYTES + 4096);
    let mut queue_saturated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(35), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting | SessionEvent::Authenticated => {}
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("host-key decision should reach the output fixture");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("fixture host key unexpectedly changed: {info:?}")
                }
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input(format!(
                            "head -c {OUTPUT_BYTES} /dev/zero | tr '\\000' 'x'; \
                             printf '\\nANYSSH_BACKPRESSURE_DONE\\n'; exit\r"
                        ))
                        .await
                        .expect("large-output command should reach the fixture");

                    timeout(Duration::from_secs(8), async {
                        loop {
                            if events.len() == SESSION_EVENT_BUFFER_CAPACITY {
                                queue_saturated = true;
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .expect("bounded event queue never reached capacity");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => {
                    assert_eq!(code, 0);
                }
                SessionEvent::Error(message) => {
                    panic!("large-output fixture session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("large-output backpressure scenario timed out");

    assert!(saw_connected);
    assert!(queue_saturated, "event queue did not apply backpressure");
    assert!(
        output.len() >= OUTPUT_BYTES,
        "terminal output was truncated: expected at least {OUTPUT_BYTES} bytes, got {}",
        output.len()
    );
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_BACKPRESSURE_DONE"),
        "large-output completion marker was missing"
    );
}
