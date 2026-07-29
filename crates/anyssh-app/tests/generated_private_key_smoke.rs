use std::{env, future::Future, path::PathBuf, process::Command, time::Duration};

use anyssh_app::{
    ApplicationCore, AuthenticationSource, DatabaseActorConfig,
    PrivateKeyExportPassphraseCandidate, PrivateKeyExportPassphraseContext,
    PrivateKeyExportPassphrasePrompt, PrivateKeyExportPassphrasePromptError,
    PrivateKeyGenerationAlgorithm, PrivateKeyPassphrasePrompt, PrivateKeyPromptContext,
    PrivateKeyPromptError, SshHopRequest, SshSessionRequest, VaultStepUpContext, VaultStepUpPrompt,
    VaultStepUpPromptError,
};
use anyssh_domain::{SshEndpoint, TerminalSize};
use anyssh_ssh::{SessionEvent, SpawnedSession};
use anyssh_vault::PinKdfParameters;
use tokio::time::timeout;
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";
const VAULT_PIN: &str = "123456";
const EXPORT_PASSPHRASE: &str = "generated-export-passphrase";

struct FixtureStepUpPrompt;

impl VaultStepUpPrompt for FixtureStepUpPrompt {
    fn request(
        &self,
        context: VaultStepUpContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, VaultStepUpPromptError>> + Send
    {
        assert_eq!(context.operation_label(), "Export SSH private key");
        assert_eq!(context.attempt(), 1);
        assert_eq!(context.max_attempts(), 3);
        assert!(!context.previous_pin_incorrect());
        async { Ok(Some(Zeroizing::new(VAULT_PIN.to_owned()))) }
    }
}

struct FixtureExportPassphrasePrompt;

impl PrivateKeyExportPassphrasePrompt for FixtureExportPassphrasePrompt {
    fn request(
        &self,
        context: PrivateKeyExportPassphraseContext,
    ) -> impl Future<
        Output = Result<
            Option<PrivateKeyExportPassphraseCandidate>,
            PrivateKeyExportPassphrasePromptError,
        >,
    > + Send {
        assert_eq!(context.attempt(), 1);
        assert_eq!(context.max_attempts(), 3);
        assert!(!context.previous_confirmation_mismatch());
        async {
            Ok(Some(PrivateKeyExportPassphraseCandidate::new(
                Zeroizing::new(EXPORT_PASSPHRASE.to_owned()),
                Zeroizing::new(EXPORT_PASSPHRASE.to_owned()),
            )))
        }
    }
}

struct FixtureImportPassphrasePrompt;

impl PrivateKeyPassphrasePrompt for FixtureImportPassphrasePrompt {
    fn request(
        &self,
        context: PrivateKeyPromptContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, PrivateKeyPromptError>> + Send {
        assert_eq!(context.label(), "Reimported generated Ed25519");
        assert_eq!(context.attempt(), 1);
        async { Ok(Some(Zeroizing::new(EXPORT_PASSPHRASE.to_owned()))) }
    }
}

#[tokio::test]
#[ignore = "requires the mutable OpenSSH topology; run pnpm test:ssh:smoke"]
async fn generated_and_exported_keys_authenticate_through_direct_saved_and_jump_paths() {
    let jump_host = required_env("ANYSSH_TEST_JUMP_HOST");
    let jump_port = required_port("ANYSSH_TEST_JUMP_PORT");
    let target_host = required_env("ANYSSH_TEST_TARGET_HOST");
    let jump_container = required_env("ANYSSH_TEST_JUMP_CONTAINER");
    let target_container = required_env("ANYSSH_TEST_TARGET_CONTAINER");
    let directory = tempfile::tempdir().expect("tempdir");
    let core = ApplicationCore::spawn(
        directory.path().join("vault"),
        DatabaseActorConfig {
            command_queue_capacity: 8,
            pin_kdf_parameters: PinKdfParameters::new(8 * 1024, 1, 1).expect("test KDF parameters"),
        },
    )
    .expect("application core");

    core.create_vault(Zeroizing::new(VAULT_PIN.to_owned()))
        .await
        .expect("create test vault");

    let generated_ed25519 = core
        .generate_private_key_credential(
            "Generated Ed25519".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            PrivateKeyGenerationAlgorithm::Ed25519,
        )
        .await
        .expect("generate Ed25519 Credential");
    let generated_ed25519_public = core
        .private_key_public_summary(generated_ed25519.id().to_owned())
        .await
        .expect("project Ed25519 Public Key");
    assert_eq!(generated_ed25519_public.algorithm(), "ssh-ed25519");
    append_authorized_key(
        &jump_container,
        generated_ed25519_public.openssh_public_key(),
    )
    .await;
    append_authorized_key(
        &target_container,
        generated_ed25519_public.openssh_public_key(),
    )
    .await;

    expect_session_marker(
        &core,
        core.spawn_ssh_session(SshSessionRequest {
            target: SshHopRequest {
                endpoint: SshEndpoint::new(jump_host.clone(), jump_port)
                    .expect("Jump fixture endpoint"),
                authentication: AuthenticationSource::Credential {
                    credential_id: generated_ed25519.id().to_owned(),
                },
            },
            jump_host: None,
            terminal_size: terminal_size(),
        })
        .await
        .expect("spawn generated Ed25519 direct session"),
        "ANYSSH_GENERATED_ED25519_DIRECT_OK",
    )
    .await;

    let generated_rsa = core
        .generate_private_key_credential(
            "Generated RSA 4096".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            PrivateKeyGenerationAlgorithm::Rsa4096,
        )
        .await
        .expect("generate RSA 4096 Credential");
    let generated_rsa_public = core
        .private_key_public_summary(generated_rsa.id().to_owned())
        .await
        .expect("project RSA Public Key");
    assert_eq!(generated_rsa_public.algorithm(), "ssh-rsa");
    append_authorized_key(&jump_container, generated_rsa_public.openssh_public_key()).await;

    let saved_host = core
        .create_host(
            "Generated RSA saved Host".to_owned(),
            jump_host.clone(),
            jump_port,
            Some(generated_rsa.id().to_owned()),
            None,
        )
        .await
        .expect("create generated RSA saved Host");
    expect_session_marker(
        &core,
        core.spawn_saved_host_session(saved_host.id().to_owned(), terminal_size())
            .await
            .expect("spawn generated RSA saved Host session"),
        "ANYSSH_GENERATED_RSA_SAVED_OK",
    )
    .await;

    let export_path = directory.path().join("generated-ed25519-export");
    let export_summary = core
        .export_private_key_credential_to_path_with_prompts(
            generated_ed25519.id().to_owned(),
            export_path.clone(),
            &FixtureStepUpPrompt,
            &FixtureExportPassphrasePrompt,
        )
        .await
        .expect("export generated Ed25519")
        .expect("export prompts should be accepted");
    assert!(export_summary.encrypted());
    assert_eq!(
        export_summary.fingerprint_sha256(),
        generated_ed25519_public.fingerprint_sha256()
    );

    let reimported = core
        .import_private_key_credential_from_path_with_prompt(
            "Reimported generated Ed25519".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            PathBuf::from(&export_path),
            &FixtureImportPassphrasePrompt,
        )
        .await
        .expect("reimport generated encrypted export")
        .expect("reimport prompt should be accepted");
    let reimported_public = core
        .private_key_public_summary(reimported.id().to_owned())
        .await
        .expect("project reimported Public Key");
    assert_eq!(
        reimported_public.fingerprint_sha256(),
        generated_ed25519_public.fingerprint_sha256()
    );
    assert_eq!(
        reimported_public.openssh_public_key(),
        generated_ed25519_public.openssh_public_key()
    );

    expect_session_marker(
        &core,
        core.spawn_ssh_session(SshSessionRequest {
            target: SshHopRequest {
                endpoint: SshEndpoint::new(target_host, 22).expect("Target fixture endpoint"),
                authentication: AuthenticationSource::Credential {
                    credential_id: reimported.id().to_owned(),
                },
            },
            jump_host: Some(SshHopRequest {
                endpoint: SshEndpoint::new(jump_host, jump_port).expect("Jump fixture endpoint"),
                authentication: AuthenticationSource::TemporaryPassword {
                    username: FIXTURE_USERNAME.to_owned(),
                    password: Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
                },
            }),
            terminal_size: terminal_size(),
        })
        .await
        .expect("spawn reimported generated Key jump session"),
        "ANYSSH_REIMPORTED_GENERATED_JUMP_OK",
    )
    .await;
}

async fn append_authorized_key(container: &str, public_key: &str) {
    let output = Command::new("docker")
        .arg("exec")
        .arg("--env")
        .arg(format!("ANYSSH_GENERATED_PUBLIC_KEY={public_key}"))
        .arg(container)
        .args([
            "sh",
            "-c",
            concat!(
                "printf '%s\\n' \"$ANYSSH_GENERATED_PUBLIC_KEY\" ",
                ">>/home/anyssh/.ssh/authorized_keys && ",
                "chown anyssh:anyssh /home/anyssh/.ssh/authorized_keys && ",
                "chmod 600 /home/anyssh/.ssh/authorized_keys"
            ),
        ])
        .output()
        .expect("docker CLI should append the generated Public Key");
    assert!(
        output.status.success(),
        "failed to authorize generated Public Key: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn expect_session_marker(core: &ApplicationCore, spawned: SpawnedSession, marker: &str) {
    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(45), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    core.decide_host_key(&control, info.request_id, true)
                        .await
                        .expect("persist generated-key fixture Host Key");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("generated-key fixture Host Key unexpectedly changed: {info:?}")
                }
                SessionEvent::AuthenticationChallenge(info) => {
                    panic!(
                        "generated-key fixture unexpectedly requested interactive auth: {info:?}"
                    )
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input(format!("printf '{marker}\\n'; exit\r"))
                        .await
                        .expect("send generated-key fixture command");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("generated-key SSH session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("generated-key SSH session timed out");

    assert!(saw_authenticated, "Target authentication event missing");
    assert!(saw_connected, "Target shell was not opened");
    assert!(
        String::from_utf8_lossy(&output).contains(marker),
        "remote marker missing: {}",
        String::from_utf8_lossy(&output)
    );
}

fn terminal_size() -> TerminalSize {
    TerminalSize::new(100, 30).expect("terminal size")
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
