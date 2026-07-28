use std::{env, fs, time::Duration};

use anyssh_app::{ApplicationCore, DatabaseActorConfig, Override};
use anyssh_domain::TerminalSize;
use anyssh_ssh::{SessionEvent, SessionHop};
use anyssh_vault::PinKdfParameters;
use tokio::time::timeout;
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

#[tokio::test]
#[ignore = "requires the isolated two-Jump OpenSSH topology; run pnpm test:ssh:smoke"]
async fn saved_host_id_executes_two_jump_route_with_rust_only_credentials() {
    let jump_one_host = required_env("ANYSSH_TEST_JUMP_HOST");
    let jump_one_port = required_port("ANYSSH_TEST_JUMP_PORT");
    let jump_two_host = required_env("ANYSSH_TEST_JUMP_TWO_HOST");
    let target_host = required_env("ANYSSH_TEST_DEEP_TARGET_HOST");
    let private_key = Zeroizing::new(
        fs::read_to_string(required_env("ANYSSH_TEST_ENCRYPTED_KEY"))
            .expect("fixture private key should be readable"),
    );
    let passphrase = Zeroizing::new(required_env("ANYSSH_TEST_KEY_PASSPHRASE"));
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
    let jump_one_credential = core
        .create_password_credential(
            "Jump one password".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        )
        .await
        .expect("store Jump 1 credential");
    let jump_two_credential = core
        .create_password_credential(
            "Jump two password".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        )
        .await
        .expect("store Jump 2 credential");
    let target_credential = core
        .store_private_key_credential(
            "Deep target key".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            private_key,
            Some(passphrase),
        )
        .await
        .expect("store Target credential");

    let jump_one = core
        .create_host(
            "Jump one".to_owned(),
            jump_one_host.clone(),
            jump_one_port,
            Some(jump_one_credential.id().to_owned()),
            None,
        )
        .await
        .expect("create Jump 1 Host");
    let route_to_jump_two = core
        .create_jump_route(
            "Route to Jump two".to_owned(),
            vec![jump_one.id().to_owned()],
        )
        .await
        .expect("create route to Jump 2");
    let jump_two_group = core
        .create_group(
            "Jump two defaults".to_owned(),
            None,
            Override::Set(jump_two_credential.id().to_owned()),
            Override::Set(route_to_jump_two.id().to_owned()),
        )
        .await
        .expect("create Jump 2 Group");
    let jump_two = core
        .create_host_with_overrides(
            "Jump two".to_owned(),
            jump_two_host.clone(),
            22,
            Some(jump_two_group.id().to_owned()),
            Override::Inherit,
            Override::Inherit,
        )
        .await
        .expect("create Jump 2 Host");
    let route_to_target = core
        .create_jump_route(
            "Route to deep target".to_owned(),
            vec![jump_two.id().to_owned()],
        )
        .await
        .expect("create route to Target");
    let target_group = core
        .create_group(
            "Deep target defaults".to_owned(),
            None,
            Override::Set(target_credential.id().to_owned()),
            Override::Set(route_to_target.id().to_owned()),
        )
        .await
        .expect("create Target Group");
    let target = core
        .create_host_with_overrides(
            "Deep target".to_owned(),
            target_host.clone(),
            22,
            Some(target_group.id().to_owned()),
            Override::Inherit,
            Override::Inherit,
        )
        .await
        .expect("create Target Host");
    let target_id = target.id().to_owned();

    core.lock_vault().await.expect("lock test vault");
    core.unlock_vault(Zeroizing::new("123456".to_owned()))
        .await
        .expect("reopen test vault");

    let spawned = core
        .spawn_saved_host_session(
            target_id,
            TerminalSize::new(100, 30).expect("terminal size"),
        )
        .await
        .expect("spawn saved Host SSH session");
    let control = spawned.control;
    let mut events = spawned.events;
    let mut output = Vec::new();
    let mut host_keys = Vec::new();
    let mut saw_authenticated = false;
    let mut saw_connected = false;

    timeout(Duration::from_secs(30), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    host_keys.push((
                        info.hop.clone(),
                        info.endpoint.host.clone(),
                        info.fingerprint_sha256.clone(),
                    ));
                    core.decide_host_key(&control, info.request_id, true)
                        .await
                        .expect("host-key decision should reach the active hop");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("saved route host key unexpectedly changed: {info:?}")
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input("printf 'ANYSSH_SAVED_ROUTE_OK\\n'; exit\r")
                        .await
                        .expect("input should reach the deep Target shell");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("saved Host Route session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("saved Host Route session timed out");

    assert_eq!(
        host_keys
            .iter()
            .map(|(hop, host, _)| (hop.clone(), host.as_str()))
            .collect::<Vec<_>>(),
        [
            (SessionHop::JumpHost { index: 1 }, jump_one_host.as_str()),
            (SessionHop::JumpHost { index: 2 }, jump_two_host.as_str()),
            (SessionHop::Target, target_host.as_str()),
        ]
    );
    let fingerprints = host_keys
        .iter()
        .map(|(_, _, fingerprint)| fingerprint)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        fingerprints.len(),
        3,
        "each SSH hop must expose a distinct Host Key"
    );
    assert!(saw_authenticated, "Target authentication event missing");
    assert!(saw_connected, "Target shell was not opened");
    assert!(
        String::from_utf8_lossy(&output).contains("ANYSSH_SAVED_ROUTE_OK"),
        "deep Target marker missing: {}",
        String::from_utf8_lossy(&output)
    );

    let incorrect_credential = core
        .create_password_credential(
            "Incorrect Jump two password".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            Zeroizing::new("incorrect-jump-two-password".to_owned()),
        )
        .await
        .expect("store incorrect Jump 2 credential");
    core.update_group(
        jump_two_group.id().to_owned(),
        jump_two_group.label().to_owned(),
        jump_two_group.parent_group_id().map(str::to_owned),
        Override::Set(incorrect_credential.id().to_owned()),
        jump_two_group.jump_route_override().clone(),
    )
    .await
    .expect("point Jump 2 Group at incorrect Credential");

    let spawned = core
        .spawn_saved_host_session(
            target.id().to_owned(),
            TerminalSize::new(100, 30).expect("terminal size"),
        )
        .await
        .expect("spawn failing saved Host SSH session");
    let control = spawned.control;
    let mut events = spawned.events;
    let mut failed_hops = Vec::new();
    let mut error = None;

    timeout(Duration::from_secs(20), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting => {}
                SessionEvent::HostKey(info) => {
                    failed_hops.push(info.hop.clone());
                    core.decide_host_key(&control, info.request_id, true)
                        .await
                        .expect("host-key decision should reach the failing route");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("saved route host key unexpectedly changed: {info:?}")
                }
                SessionEvent::Error(message) => error = Some(message),
                SessionEvent::Closed => break,
                SessionEvent::Authenticated | SessionEvent::Connected => {
                    panic!("Target session advanced after Jump 2 authentication failure")
                }
                SessionEvent::Data(_) | SessionEvent::ExitStatus(_) => {}
            }
        }
    })
    .await
    .expect("Jump 2 authentication failure timed out");

    assert!(
        failed_hops.is_empty(),
        "durably trusted Jump Hosts must not prompt again: {failed_hops:?}"
    );
    let error = error.expect("Jump 2 authentication failure must emit an error");
    assert!(
        error.contains("jump host 2 authentication failed"),
        "failure was not scoped to Jump 2: {error}"
    );
    assert!(!error.contains("incorrect-jump-two-password"));
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn required_port(name: &str) -> u16 {
    required_env(name)
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be a valid u16"))
}
