use std::{borrow::Cow, env, path::PathBuf, sync::Arc, time::Duration};

use russh::{
    Channel, ChannelId,
    keys::{Algorithm, PrivateKey},
    server::{self, Msg, Server as _, Session},
};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

#[tokio::main]
async fn main() {
    let port = required_env("ANYSSH_KBDINT_PORT")
        .parse::<u16>()
        .expect("ANYSSH_KBDINT_PORT must be a valid u16");
    let expected_response = Arc::new(Zeroizing::new(required_env("ANYSSH_KBDINT_RESPONSE")));
    let marker_path = Arc::new(PathBuf::from(required_env("ANYSSH_KBDINT_MARKER")));
    let ready_path = PathBuf::from(required_env("ANYSSH_KBDINT_READY"));
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind Keyboard-interactive fixture");
    let config = Arc::new(server::Config {
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        keys: vec![
            PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)
                .expect("generate fixture Host Key"),
        ],
        ..Default::default()
    });
    std::fs::write(&ready_path, b"ready").expect("write fixture readiness marker");

    let mut fixture = KeyboardInteractiveServer {
        expected_response,
        marker_path,
        round: 0,
        command_buffer: Vec::new(),
    };
    fixture
        .run_on_socket(config, &listener)
        .await
        .expect("run Keyboard-interactive fixture");
}

#[derive(Clone)]
struct KeyboardInteractiveServer {
    expected_response: Arc<Zeroizing<String>>,
    marker_path: Arc<PathBuf>,
    round: usize,
    command_buffer: Vec<u8>,
}

impl server::Server for KeyboardInteractiveServer {
    type Handler = Self;

    fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
        Self {
            expected_response: self.expected_response.clone(),
            marker_path: self.marker_path.clone(),
            round: 0,
            command_buffer: Vec::new(),
        }
    }
}

impl server::Handler for KeyboardInteractiveServer {
    type Error = russh::Error;

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        response: Option<server::Response<'a>>,
    ) -> Result<server::Auth, Self::Error> {
        let responses = response
            .map(|responses| {
                responses
                    .map(|response| String::from_utf8_lossy(&response).into_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        match (self.round, responses.as_slice()) {
            (0, []) => {
                self.round = 1;
                Ok(server::Auth::Partial {
                    name: Cow::Borrowed("AnySSH controlled challenge"),
                    instructions: Cow::Borrowed("Enter the session verification response."),
                    prompts: Cow::Borrowed(&[(Cow::Borrowed("Verification response:"), false)]),
                })
            }
            (1, [response]) if response == self.expected_response.as_str() => {
                self.round = 2;
                Ok(server::Auth::Accept)
            }
            _ => Ok(server::Auth::reject()),
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.request_success();
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.request_success();
        session.data(
            channel,
            b"AnySSH controlled SSH fixture ready.\r\n".to_vec(),
        )?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.command_buffer.extend_from_slice(data);
        if self
            .command_buffer
            .iter()
            .any(|byte| matches!(byte, b'\r' | b'\n'))
        {
            std::fs::write(self.marker_path.as_ref(), b"interactive-ok")
                .expect("write Keyboard-interactive command marker");
            self.command_buffer.clear();
            session.data(channel, b"ANYSSH_WINDOWS_INTERACTIVE_OK\r\n".to_vec())?;
        }
        Ok(())
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}
