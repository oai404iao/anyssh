use std::{
    env,
    future::Future,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyssh_app::{
    ApplicationCore, AuthenticationSource, DatabaseActorConfig, PrivateKeyPassphrasePrompt,
    PrivateKeyPromptContext, PrivateKeyPromptError, SshHopRequest, SshSessionRequest,
};
use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{SessionEvent, SessionHop};
use anyssh_vault::PinKdfParameters;
use tokio::time::timeout;
use zeroize::Zeroizing;

struct FixturePassphrasePrompt {
    passphrase: Mutex<Option<Zeroizing<String>>>,
    calls: AtomicUsize,
}

impl FixturePassphrasePrompt {
    fn new(passphrase: Zeroizing<String>) -> Self {
        Self {
            passphrase: Mutex::new(Some(passphrase)),
            calls: AtomicUsize::new(0),
        }
    }
}

impl PrivateKeyPassphrasePrompt for FixturePassphrasePrompt {
    fn request(
        &self,
        context: PrivateKeyPromptContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, PrivateKeyPromptError>> + Send {
        assert_eq!(context.label(), "Encrypted fixture key");
        assert_eq!(context.attempt(), 1);
        self.calls.fetch_add(1, Ordering::Relaxed);
        let passphrase = self
            .passphrase
            .lock()
            .expect("fixture prompt")
            .take()
            .ok_or(PrivateKeyPromptError::Unavailable);
        async move { passphrase.map(Some) }
    }
}

#[tokio::test]
#[ignore = "requires the isolated OpenSSH key fixture; run pnpm test:ssh:smoke"]
async fn encrypted_private_key_flows_from_credential_id_to_ssh_core() {
    let host = required_env("ANYSSH_TEST_JUMP_HOST");
    let port = required_port("ANYSSH_TEST_JUMP_PORT");
    let private_key_path = required_env("ANYSSH_TEST_ENCRYPTED_KEY").into();
    let prompt =
        FixturePassphrasePrompt::new(Zeroizing::new(required_env("ANYSSH_TEST_KEY_PASSPHRASE")));
    let directory = tempfile::tempdir().expect("tempdir");
    let core = ApplicationCore::spawn(
        directory.path().join("vault"),
        DatabaseActorConfig {
            command_queue_capacity: 8,
            pin_kdf_parameters: PinKdfParameters::new(8 * 1024, 1, 1).expect("test KDF parameters"),
        },
    )
    .expect("application core");

    core.create_vault(Zeroizing::new("123456".to_owned()))
        .await
        .expect("create test vault");
    let summary = core
        .import_private_key_credential_from_path_with_prompt(
            "Encrypted fixture key".to_owned(),
            "anyssh".to_owned(),
            private_key_path,
            &prompt,
        )
        .await
        .expect("import fixture private key")
        .expect("fixture prompt should be accepted");
    assert_eq!(prompt.calls.load(Ordering::Relaxed), 1);
    let credential_id = summary.id().to_owned();
    core.lock_vault().await.expect("lock test vault");
    core.unlock_vault(Zeroizing::new("123456".to_owned()))
        .await
        .expect("reopen test vault");

    let request = SshSessionRequest {
        target: SshHopRequest {
            endpoint: SshEndpoint::new(host, port).expect("fixture endpoint"),
            authentication: AuthenticationSource::Credential { credential_id },
        },
        jump_host: None,
        terminal_size: TerminalSize::new(100, 30).expect("terminal size"),
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("BEGIN OPENSSH PRIVATE KEY"));
    assert!(!debug.contains("anyssh-key-passphrase"));

    let spawned = core
        .spawn_ssh_session(request)
        .await
        .expect("spawn credential-backed SSH session");
    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut saw_host_key = false;
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(25), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    assert_eq!(info.hop, SessionHop::Target);
                    saw_host_key = true;
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("host-key decision");
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input("printf 'ANYSSH_VAULT_CREDENTIAL_OK\\n'; exit\r")
                        .await
                        .expect("send fixture command");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("credential-backed SSH session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("credential-backed SSH session timed out");

    assert!(saw_host_key);
    assert!(saw_authenticated);
    assert!(saw_connected);
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_VAULT_CREDENTIAL_OK"),
        "remote marker missing: {}",
        String::from_utf8_lossy(&output)
    );
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
