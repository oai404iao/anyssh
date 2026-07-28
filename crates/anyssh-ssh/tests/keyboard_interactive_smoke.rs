use std::{env, fs, time::Duration};

use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{
    HostKeyPolicy, SessionAuthentication, SessionEvent, SessionHop, SpawnedSession,
    SshConnectionConfig, SshSessionConfig, spawn_session,
};
use tokio::time::timeout;
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

struct KeyboardInteractiveFixture {
    direct_host: String,
    direct_port: u16,
    password_jump_host: String,
    password_jump_port: u16,
    mfa_target_host: String,
    password_mfa_target_host: String,
    interactive_jump_target_host: String,
    interactive_second_jump_target_host: String,
    target_port: u16,
    private_key: Zeroizing<String>,
    agent_fingerprint: String,
    response: String,
}

impl KeyboardInteractiveFixture {
    fn from_environment() -> Self {
        Self {
            direct_host: required_env("ANYSSH_TEST_INTERACTIVE_HOST"),
            direct_port: required_port("ANYSSH_TEST_INTERACTIVE_PORT"),
            password_jump_host: required_env("ANYSSH_TEST_JUMP_HOST"),
            password_jump_port: required_port("ANYSSH_TEST_JUMP_PORT"),
            mfa_target_host: required_env("ANYSSH_TEST_INTERACTIVE_TARGET_HOST"),
            password_mfa_target_host: required_env("ANYSSH_TEST_INTERACTIVE_PASSWORD_TARGET_HOST"),
            interactive_jump_target_host: required_env("ANYSSH_TEST_INTERACTIVE_JUMP_TARGET_HOST"),
            interactive_second_jump_target_host: required_env(
                "ANYSSH_TEST_INTERACTIVE_JUMP_TWO_TARGET_HOST",
            ),
            target_port: 22,
            private_key: Zeroizing::new(
                fs::read_to_string(required_env("ANYSSH_TEST_UNENCRYPTED_KEY"))
                    .expect("read Keyboard-interactive fixture Private Key"),
            ),
            agent_fingerprint: required_env("ANYSSH_TEST_AGENT_FINGERPRINT"),
            response: required_env("ANYSSH_TEST_INTERACTIVE_RESPONSE"),
        }
    }

    fn direct_config(&self) -> SshSessionConfig {
        session_config(
            interactive_connection(&self.direct_host, self.direct_port),
            Vec::new(),
        )
    }

    fn mfa_target_config(&self) -> SshSessionConfig {
        session_config(
            SshConnectionConfig {
                endpoint: SshEndpoint::new(&self.mfa_target_host, self.target_port)
                    .expect("valid MFA target endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                authentication: SessionAuthentication::PrivateKey {
                    private_key: self.private_key.clone(),
                    passphrase: None,
                },
                host_key_policy: HostKeyPolicy::Prompt,
            },
            vec![password_connection(
                &self.password_jump_host,
                self.password_jump_port,
            )],
        )
    }

    fn interactive_jump_config(&self) -> SshSessionConfig {
        session_config(
            password_connection(&self.interactive_jump_target_host, self.target_port),
            vec![interactive_connection(&self.direct_host, self.direct_port)],
        )
    }

    fn mfa_agent_target_config(&self) -> SshSessionConfig {
        session_config(
            SshConnectionConfig {
                endpoint: SshEndpoint::new(&self.mfa_target_host, self.target_port)
                    .expect("valid Agent MFA target endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                authentication: SessionAuthentication::SystemAgent {
                    identity_fingerprint_sha256: self.agent_fingerprint.clone(),
                },
                host_key_policy: HostKeyPolicy::Prompt,
            },
            vec![password_connection(
                &self.password_jump_host,
                self.password_jump_port,
            )],
        )
    }

    fn mfa_password_target_config(&self) -> SshSessionConfig {
        session_config(
            SshConnectionConfig {
                endpoint: SshEndpoint::new(&self.password_mfa_target_host, self.target_port)
                    .expect("valid Password MFA target endpoint"),
                username: FIXTURE_USERNAME.to_owned(),
                authentication: SessionAuthentication::Password {
                    password: Zeroizing::new(self.response.clone()),
                },
                host_key_policy: HostKeyPolicy::Prompt,
            },
            vec![password_connection(
                &self.password_jump_host,
                self.password_jump_port,
            )],
        )
    }

    fn mfa_second_jump_config(&self) -> SshSessionConfig {
        session_config(
            password_connection(&self.interactive_second_jump_target_host, self.target_port),
            vec![
                password_connection(&self.password_jump_host, self.password_jump_port),
                SshConnectionConfig {
                    endpoint: SshEndpoint::new(&self.mfa_target_host, self.target_port)
                        .expect("valid Interactive Jump 2 endpoint"),
                    username: FIXTURE_USERNAME.to_owned(),
                    authentication: SessionAuthentication::PrivateKey {
                        private_key: self.private_key.clone(),
                        passphrase: None,
                    },
                    host_key_policy: HostKeyPolicy::Prompt,
                },
            ],
        )
    }
}

#[tokio::test]
#[ignore = "requires the OpenSSH PAM Docker fixture; run pnpm test:ssh:smoke"]
async fn keyboard_interactive_and_partial_success_suite_against_openssh() {
    let fixture = KeyboardInteractiveFixture::from_environment();

    let direct_hops = expect_success(
        spawn_session(fixture.direct_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_DIRECT_OK",
    )
    .await;
    assert_eq!(direct_hops, vec![SessionHop::Target]);

    let error = expect_failure(
        spawn_session(fixture.direct_config()),
        "incorrect-interactive-response",
    )
    .await;
    assert!(
        error.contains("target keyboard-interactive authentication failed"),
        "wrong-response error was unexpected: {error}"
    );
    assert!(!error.contains("incorrect-interactive-response"));
    assert!(!error.contains(&fixture.response));

    let repeat_hops = expect_success(
        spawn_session(fixture.direct_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_REPEAT_OK",
    )
    .await;
    assert_eq!(repeat_hops, vec![SessionHop::Target]);

    let mfa_hops = expect_success(
        spawn_session(fixture.mfa_target_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_MFA_TARGET_OK",
    )
    .await;
    assert_eq!(mfa_hops, vec![SessionHop::Target]);

    let agent_mfa_hops = expect_success(
        spawn_session(fixture.mfa_agent_target_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_AGENT_MFA_OK",
    )
    .await;
    assert_eq!(agent_mfa_hops, vec![SessionHop::Target]);

    let password_mfa_hops = expect_success(
        spawn_session(fixture.mfa_password_target_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_PASSWORD_MFA_OK",
    )
    .await;
    assert_eq!(password_mfa_hops, vec![SessionHop::Target]);

    let jump_hops = expect_success(
        spawn_session(fixture.interactive_jump_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_JUMP_OK",
    )
    .await;
    assert_eq!(jump_hops, vec![SessionHop::JumpHost { index: 1 }]);

    let second_jump_hops = expect_success(
        spawn_session(fixture.mfa_second_jump_config()),
        &fixture.response,
        "ANYSSH_INTERACTIVE_JUMP_TWO_OK",
    )
    .await;
    assert_eq!(second_jump_hops, vec![SessionHop::JumpHost { index: 2 }]);
}

async fn expect_success(spawned: SpawnedSession, response: &str, marker: &str) -> Vec<SessionHop> {
    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut challenge_hops = Vec::new();
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(30), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("accept Keyboard-interactive fixture Host Key");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("fixture Host Key unexpectedly changed: {info:?}")
                }
                SessionEvent::AuthenticationChallenge(info) => {
                    assert!(!info.prompts.is_empty());
                    assert!(info.prompts.len() <= 16);
                    challenge_hops.push(info.hop.clone());
                    control
                        .respond_authentication(
                            info.request_id,
                            Some(
                                info.prompts
                                    .iter()
                                    .map(|_| Zeroizing::new(response.to_owned()))
                                    .collect(),
                            ),
                        )
                        .await
                        .expect("submit session-bound Keyboard-interactive response");
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input(format!("printf '{marker}\\n'; exit\r"))
                        .await
                        .expect("send Keyboard-interactive marker command");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("Keyboard-interactive fixture failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("Keyboard-interactive success scenario timed out");

    assert!(saw_authenticated, "authentication event missing");
    assert!(saw_connected, "interactive shell was not opened");
    assert!(
        String::from_utf8_lossy(&output).contains(marker),
        "remote marker missing from output"
    );
    challenge_hops
}

async fn expect_failure(spawned: SpawnedSession, response: &str) -> String {
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
                        .expect("accept failure fixture Host Key");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("fixture Host Key unexpectedly changed: {info:?}")
                }
                SessionEvent::AuthenticationChallenge(info) => {
                    control
                        .respond_authentication(
                            info.request_id,
                            Some(
                                info.prompts
                                    .iter()
                                    .map(|_| Zeroizing::new(response.to_owned()))
                                    .collect(),
                            ),
                        )
                        .await
                        .expect("submit wrong Keyboard-interactive response");
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
    .expect("Keyboard-interactive failure scenario timed out");

    assert!(!saw_authenticated);
    assert!(!saw_connected);
    error.expect("Keyboard-interactive failure must emit an error")
}

fn session_config(
    target: SshConnectionConfig,
    jump_hosts: Vec<SshConnectionConfig>,
) -> SshSessionConfig {
    SshSessionConfig {
        target,
        jump_hosts,
        terminal_size: TerminalSize::new(100, 30).expect("valid terminal size"),
        connection_timeout: Duration::from_secs(10),
    }
}

fn interactive_connection(host: &str, port: u16) -> SshConnectionConfig {
    SshConnectionConfig {
        endpoint: SshEndpoint::new(host, port).expect("valid interactive endpoint"),
        username: FIXTURE_USERNAME.to_owned(),
        authentication: SessionAuthentication::KeyboardInteractive,
        host_key_policy: HostKeyPolicy::Prompt,
    }
}

fn password_connection(host: &str, port: u16) -> SshConnectionConfig {
    SshConnectionConfig {
        endpoint: SshEndpoint::new(host, port).expect("valid password endpoint"),
        username: FIXTURE_USERNAME.to_owned(),
        authentication: SessionAuthentication::Password {
            password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        },
        host_key_policy: HostKeyPolicy::Prompt,
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
