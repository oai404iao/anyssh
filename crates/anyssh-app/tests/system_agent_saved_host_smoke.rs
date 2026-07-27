use std::{env, fs, time::Duration};

use anyssh_app::{ApplicationCore, DatabaseActorConfig};
use anyssh_domain::TerminalSize;
use anyssh_ssh::{SessionEvent, SessionHop};
use anyssh_vault::PinKdfParameters;
use tokio::time::timeout;
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

#[tokio::test]
#[ignore = "requires the OpenSSH Docker fixture and ssh-agent; run pnpm test:ssh:smoke"]
async fn saved_hosts_mix_password_private_key_and_system_agent_per_hop() {
    let jump_host = required_env("ANYSSH_TEST_JUMP_HOST");
    let jump_port = required_port("ANYSSH_TEST_JUMP_PORT");
    let target_host = required_env("ANYSSH_TEST_TARGET_HOST");
    let agent_fingerprint = required_env("ANYSSH_TEST_AGENT_FINGERPRINT");
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
    let password = core
        .create_password_credential(
            "Jump password".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        )
        .await
        .expect("create password credential");
    let agent = core
        .create_system_agent_credential(
            "Workstation agent".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            agent_fingerprint,
        )
        .await
        .expect("create Agent credential");
    let key = core
        .store_private_key_credential(
            "Target private key".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            private_key,
            Some(passphrase),
        )
        .await
        .expect("create Private Key credential");

    let password_jump = core
        .create_host(
            "Password jump".to_owned(),
            jump_host.clone(),
            jump_port,
            Some(password.id().to_owned()),
            None,
        )
        .await
        .expect("create Password Jump Host");
    let password_route = core
        .create_jump_route(
            "Password to Agent".to_owned(),
            vec![password_jump.id().to_owned()],
        )
        .await
        .expect("create Password Route");
    let agent_target = core
        .create_host(
            "Agent target".to_owned(),
            target_host.clone(),
            22,
            Some(agent.id().to_owned()),
            Some(password_route.id().to_owned()),
        )
        .await
        .expect("create Agent Target");

    run_saved_session(
        &core,
        agent_target.id(),
        "ANYSSH_PASSWORD_TO_AGENT_OK",
        &[SessionHop::JumpHost { index: 1 }, SessionHop::Target],
    )
    .await;

    let agent_jump = core
        .create_host(
            "Agent jump".to_owned(),
            jump_host,
            jump_port,
            Some(agent.id().to_owned()),
            None,
        )
        .await
        .expect("create Agent Jump Host");
    let agent_route = core
        .create_jump_route(
            "Agent to Private Key".to_owned(),
            vec![agent_jump.id().to_owned()],
        )
        .await
        .expect("create Agent Route");
    let key_target = core
        .create_host(
            "Private Key target".to_owned(),
            target_host,
            22,
            Some(key.id().to_owned()),
            Some(agent_route.id().to_owned()),
        )
        .await
        .expect("create Private Key Target");

    run_saved_session(
        &core,
        key_target.id(),
        "ANYSSH_AGENT_TO_KEY_OK",
        &[SessionHop::JumpHost { index: 1 }, SessionHop::Target],
    )
    .await;
}

async fn run_saved_session(
    core: &ApplicationCore,
    host_id: &str,
    marker: &str,
    expected_hops: &[SessionHop],
) {
    let spawned = core
        .spawn_saved_host_session(
            host_id.to_owned(),
            TerminalSize::new(100, 30).expect("terminal size"),
        )
        .await
        .expect("spawn saved Host session");
    let control = spawned.control;
    let mut events = spawned.events;
    let mut hops = Vec::new();
    let mut output = Vec::new();

    timeout(Duration::from_secs(30), async {
        while let Some(event) = events.recv().await {
            match event {
                SessionEvent::Connecting | SessionEvent::Authenticated => {}
                SessionEvent::HostKey(info) => {
                    hops.push(info.hop);
                    control
                        .confirm_host_key(info.request_id, true)
                        .await
                        .expect("confirm Host Key");
                }
                SessionEvent::Connected => {
                    control
                        .send_input(format!("printf '{marker}\\n'; exit\r").into_bytes())
                        .await
                        .expect("send marker command");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("mixed Agent session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("mixed Agent session timed out");

    assert_eq!(hops, expected_hops);
    assert!(
        String::from_utf8_lossy(&output).contains(marker),
        "marker missing from output: {}",
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
