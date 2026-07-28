use std::{fmt, future::Future};

use anyssh_app::{PrivateKeyPassphrasePrompt, PrivateKeyPromptContext, PrivateKeyPromptError};
use tauri::AppHandle;
#[cfg(any(target_os = "linux", windows))]
use tauri::Manager;
use tokio::sync::oneshot;
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) struct NativePrivateKeyPassphrasePrompt {
    app: AppHandle,
}

impl NativePrivateKeyPassphrasePrompt {
    pub(crate) const fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl fmt::Debug for NativePrivateKeyPassphrasePrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativePrivateKeyPassphrasePrompt")
            .finish_non_exhaustive()
    }
}

impl PrivateKeyPassphrasePrompt for NativePrivateKeyPassphrasePrompt {
    fn request(
        &self,
        context: PrivateKeyPromptContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, PrivateKeyPromptError>> + Send {
        let app = self.app.clone();
        async move { request_on_main_thread(app, context).await }
    }
}

async fn request_on_main_thread(
    app: AppHandle,
    context: PrivateKeyPromptContext,
) -> Result<Option<Zeroizing<String>>, PrivateKeyPromptError> {
    let (sender, receiver) = oneshot::channel();
    let task_app = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(prompt_on_main_thread(&task_app, &context));
    })
    .map_err(|_| PrivateKeyPromptError::Unavailable)?;

    receiver
        .await
        .map_err(|_| PrivateKeyPromptError::Unavailable)?
}

#[cfg(target_os = "linux")]
fn prompt_on_main_thread(
    app: &AppHandle,
    context: &PrivateKeyPromptContext,
) -> Result<Option<Zeroizing<String>>, PrivateKeyPromptError> {
    use gtk::prelude::*;
    use gtk::{Align, DialogFlags, InputPurpose, Orientation, ResponseType};

    let parent = app
        .get_webview_window("main")
        .and_then(|window| window.gtk_window().ok());
    let dialog = gtk::Dialog::with_buttons(
        Some("Unlock SSH private key"),
        parent.as_ref(),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Import key", ResponseType::Accept),
        ],
    );
    dialog.set_default_response(ResponseType::Accept);
    dialog.set_resizable(false);

    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let stack = gtk::Box::new(Orientation::Vertical, 10);
    let message = gtk::Label::new(Some(&format!(
        "Enter the passphrase for “{}”.",
        context.label()
    )));
    message.set_halign(Align::Start);
    message.set_line_wrap(true);
    stack.pack_start(&message, false, false, 0);

    if context.previous_passphrase_incorrect() {
        let retry = gtk::Label::new(Some("The previous passphrase was not accepted."));
        retry.set_halign(Align::Start);
        retry.set_line_wrap(true);
        stack.pack_start(&retry, false, false, 0);
    }

    let attempt = gtk::Label::new(Some(&format!(
        "Attempt {} of {}",
        context.attempt(),
        context.max_attempts()
    )));
    attempt.set_halign(Align::Start);
    stack.pack_start(&attempt, false, false, 0);

    let entry = gtk::Entry::new();
    entry.set_visibility(false);
    entry.set_input_purpose(InputPurpose::Password);
    entry.set_activates_default(true);
    entry.set_placeholder_text(Some("Private key passphrase"));
    stack.pack_start(&entry, false, false, 0);
    content.pack_start(&stack, true, true, 0);

    dialog.show_all();
    entry.grab_focus();
    let response = dialog.run();
    let passphrase = if response == ResponseType::Accept {
        let value = Zeroizing::new(entry.text().as_str().to_owned());
        entry.set_text("");
        Some(value)
    } else {
        entry.set_text("");
        None
    };
    dialog.close();
    Ok(passphrase)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn prompt_on_main_thread(
    app: &AppHandle,
    context: &PrivateKeyPromptContext,
) -> Result<Option<Zeroizing<String>>, PrivateKeyPromptError> {
    use std::mem::size_of;

    use windows::{
        Win32::{
            Foundation::{ERROR_CANCELLED, ERROR_SUCCESS},
            Security::Credentials::{
                CREDUI_FLAGS_ALWAYS_SHOW_UI, CREDUI_FLAGS_DO_NOT_PERSIST,
                CREDUI_FLAGS_EXCLUDE_CERTIFICATES, CREDUI_FLAGS_GENERIC_CREDENTIALS,
                CREDUI_FLAGS_INCORRECT_PASSWORD, CREDUI_FLAGS_KEEP_USERNAME,
                CREDUI_FLAGS_PASSWORD_ONLY_OK, CREDUI_INFOW, CREDUI_MAX_USERNAME_LENGTH,
                CredUIPromptForCredentialsW,
            },
        },
        core::PCWSTR,
    };

    const WINDOWS_PASSWORD_BUFFER_CODE_UNITS: usize = 1024;

    let window = app
        .get_webview_window("main")
        .ok_or(PrivateKeyPromptError::Unavailable)?;
    let caption = wide_null_terminated("Unlock SSH private key");
    let message = wide_null_terminated(&format!(
        "Enter the passphrase for “{}” (attempt {} of {}).",
        context.label(),
        context.attempt(),
        context.max_attempts()
    ));
    let target = wide_null_terminated("AnySSH encrypted private key");
    let mut username = wide_null_terminated("AnySSH");
    username.resize(CREDUI_MAX_USERNAME_LENGTH as usize, 0);
    let mut password = Zeroizing::new(vec![0_u16; WINDOWS_PASSWORD_BUFFER_CODE_UNITS]);
    let info = CREDUI_INFOW {
        cbSize: size_of::<CREDUI_INFOW>() as u32,
        hwndParent: window
            .hwnd()
            .map_err(|_| PrivateKeyPromptError::Unavailable)?,
        pszMessageText: PCWSTR(message.as_ptr()),
        pszCaptionText: PCWSTR(caption.as_ptr()),
        hbmBanner: Default::default(),
    };
    let mut flags = CREDUI_FLAGS_ALWAYS_SHOW_UI
        | CREDUI_FLAGS_DO_NOT_PERSIST
        | CREDUI_FLAGS_EXCLUDE_CERTIFICATES
        | CREDUI_FLAGS_GENERIC_CREDENTIALS
        | CREDUI_FLAGS_KEEP_USERNAME
        | CREDUI_FLAGS_PASSWORD_ONLY_OK;
    if context.previous_passphrase_incorrect() {
        flags |= CREDUI_FLAGS_INCORRECT_PASSWORD;
    }

    // SAFETY: all pointers refer to live, NUL-terminated buffers for the
    // duration of the synchronous call. The password buffer is mutable,
    // bounded, and wrapped in Zeroizing so it is cleared on every exit path.
    let result = unsafe {
        CredUIPromptForCredentialsW(
            Some(&info),
            PCWSTR(target.as_ptr()),
            None,
            0,
            &mut username,
            &mut password,
            None,
            flags,
        )
    };
    if result == ERROR_CANCELLED {
        return Ok(None);
    }
    if result != ERROR_SUCCESS {
        return Err(PrivateKeyPromptError::Unavailable);
    }

    let password_length = password
        .iter()
        .position(|code_unit| *code_unit == 0)
        .unwrap_or(password.len());
    String::from_utf16(&password[..password_length])
        .map(Zeroizing::new)
        .map(Some)
        .map_err(|_| PrivateKeyPromptError::Unavailable)
}

#[cfg(windows)]
fn wide_null_terminated(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn prompt_on_main_thread(
    _app: &AppHandle,
    _context: &PrivateKeyPromptContext,
) -> Result<Option<Zeroizing<String>>, PrivateKeyPromptError> {
    Err(PrivateKeyPromptError::Unavailable)
}
