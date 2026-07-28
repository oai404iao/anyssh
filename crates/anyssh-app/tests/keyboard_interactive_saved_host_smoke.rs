use std::{env, fs, path::Path, time::Duration};

use anyssh_app::{ApplicationCore, DatabaseActorConfig};
use anyssh_domain::TerminalSize;
use anyssh_ssh::{SessionEvent, SessionHop, SpawnedSession};
use anyssh_vault::PinKdfParameters;
use tokio::time::timeout;
use zeroize::Zeroizing;

const FIXTURE_USERNAME: &str = "anyssh";
const FIXTURE_PASSWORD: &str = "anyssh-test";

#[tokio::test]
#[ignore = "requires the OpenSSH PAM Docker fixture; run pnpm test:ssh:smoke"]
async fn saved_interactive_credential_authenticates_direct_and_as_a_jump_hop() {
    let interactive_host = required_env("ANYSSH_TEST_INTERACTIVE_HOST");
    let interactive_port = required_port("ANYSSH_TEST_INTERACTIVE_PORT");
    let target_host = required_env("ANYSSH_TEST_INTERACTIVE_JUMP_TARGET_HOST");
    let response = required_env("ANYSSH_TEST_INTERACTIVE_RESPONSE");
    let directory = tempfile::tempdir().expect("tempdir");
    let vault_root = directory.path().join("vault");
    let core = ApplicationCore::spawn(
        vault_root.clone(),
        DatabaseActorConfig {
            command_queue_capacity: 8,
            pin_kdf_parameters: PinKdfParameters::new(8 * 1024, 1, 1).expect("test KDF parameters"),
        },
    )
    .expect("application core");

    core.create_vault(Zeroizing::new("123456".to_owned()))
        .await
        .expect("create test vault");
    let interactive_credential = core
        .create_keyboard_interactive_credential(
            "Saved interactive".to_owned(),
            FIXTURE_USERNAME.to_owned(),
        )
        .await
        .expect("create saved interactive Credential");
    let password_credential = core
        .create_password_credential(
            "Saved target password".to_owned(),
            FIXTURE_USERNAME.to_owned(),
            Zeroizing::new(FIXTURE_PASSWORD.to_owned()),
        )
        .await
        .expect("create saved target Password");
    let interactive_host_record = core
        .create_host(
            "Interactive endpoint".to_owned(),
            interactive_host,
            interactive_port,
            Some(interactive_credential.id().to_owned()),
            None,
        )
        .await
        .expect("create interactive saved Host");

    let direct_hops = expect_saved_success(
        &core,
        core.spawn_saved_host_session(
            interactive_host_record.id().to_owned(),
            TerminalSize::new(100, 30).expect("terminal size"),
        )
        .await
        .expect("spawn direct saved interactive session"),
        &response,
        "ANYSSH_SAVED_INTERACTIVE_DIRECT_OK",
    )
    .await;
    assert_eq!(direct_hops, vec![SessionHop::Target]);

    let route = core
        .create_jump_route(
            "Interactive Jump".to_owned(),
            vec![interactive_host_record.id().to_owned()],
        )
        .await
        .expect("create interactive Jump Route");
    let target = core
        .create_host(
            "Password target".to_owned(),
            target_host,
            22,
            Some(password_credential.id().to_owned()),
            Some(route.id().to_owned()),
        )
        .await
        .expect("create password Target");

    let jump_hops = expect_saved_success(
        &core,
        core.spawn_saved_host_session(
            target.id().to_owned(),
            TerminalSize::new(100, 30).expect("terminal size"),
        )
        .await
        .expect("spawn saved interactive Jump session"),
        &response,
        "ANYSSH_SAVED_INTERACTIVE_JUMP_OK",
    )
    .await;
    assert_eq!(jump_hops, vec![SessionHop::JumpHost { index: 1 }]);

    core.lock_vault().await.expect("lock test vault");
    assert_files_do_not_contain(&vault_root, response.as_bytes());
}

async fn expect_saved_success(
    core: &ApplicationCore,
    spawned: SpawnedSession,
    response: &str,
    marker: &str,
) -> Vec<SessionHop> {
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
                    core.decide_host_key(&control, info.request_id, true)
                        .await
                        .expect("persist saved interactive Host Key");
                }
                SessionEvent::HostKeyChanged(info) => {
                    panic!("saved interactive Host Key unexpectedly changed: {info:?}")
                }
                SessionEvent::AuthenticationChallenge(info) => {
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
                        .expect("submit saved interactive response");
                }
                SessionEvent::Authenticated => saw_authenticated = true,
                SessionEvent::Connected => {
                    saw_connected = true;
                    control
                        .send_input(format!("printf '{marker}\\n'; exit\r"))
                        .await
                        .expect("send saved interactive marker command");
                }
                SessionEvent::Data(data) => output.extend_from_slice(&data),
                SessionEvent::ExitStatus(code) => assert_eq!(code, 0),
                SessionEvent::Error(message) => {
                    panic!("saved interactive session failed: {message}")
                }
                SessionEvent::Closed => break,
            }
        }
    })
    .await
    .expect("saved interactive session timed out");

    assert!(saw_authenticated, "authentication event missing");
    assert!(saw_connected, "interactive shell was not opened");
    assert!(
        String::from_utf8_lossy(&output).contains(marker),
        "saved interactive marker missing"
    );
    challenge_hops
}

fn assert_files_do_not_contain(root: &Path, needle: &[u8]) {
    for entry in fs::read_dir(root).expect("read Vault directory") {
        let path = entry.expect("Vault entry").path();
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).expect("read Vault file");
        assert!(
            !bytes.windows(needle.len()).any(|window| window == needle),
            "{} leaked the interactive response",
            path.display()
        );
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
