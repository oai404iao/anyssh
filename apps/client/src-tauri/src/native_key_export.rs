use std::{fmt, future::Future};

use anyssh_app::{
    PrivateKeyExportPassphraseCandidate, PrivateKeyExportPassphraseContext,
    PrivateKeyExportPassphrasePrompt, PrivateKeyExportPassphrasePromptError, VaultStepUpContext,
    VaultStepUpPrompt, VaultStepUpPromptError,
};
use tauri::AppHandle;
#[cfg(any(target_os = "linux", windows))]
use tauri::Manager;
use tokio::sync::oneshot;
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) struct NativeVaultStepUpPrompt {
    app: AppHandle,
}

impl NativeVaultStepUpPrompt {
    pub(crate) const fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl fmt::Debug for NativeVaultStepUpPrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeVaultStepUpPrompt")
            .finish_non_exhaustive()
    }
}

impl VaultStepUpPrompt for NativeVaultStepUpPrompt {
    fn request(
        &self,
        context: VaultStepUpContext,
    ) -> impl Future<Output = Result<Option<Zeroizing<String>>, VaultStepUpPromptError>> + Send
    {
        let app = self.app.clone();
        async move {
            let (sender, receiver) = oneshot::channel();
            let task_app = app.clone();
            app.run_on_main_thread(move || {
                let _ = sender.send(prompt_step_up_on_main_thread(&task_app, &context));
            })
            .map_err(|_| VaultStepUpPromptError::Unavailable)?;
            receiver
                .await
                .map_err(|_| VaultStepUpPromptError::Unavailable)?
        }
    }
}

#[derive(Clone)]
pub(crate) struct NativePrivateKeyExportPassphrasePrompt {
    app: AppHandle,
}

impl NativePrivateKeyExportPassphrasePrompt {
    pub(crate) const fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl fmt::Debug for NativePrivateKeyExportPassphrasePrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativePrivateKeyExportPassphrasePrompt")
            .finish_non_exhaustive()
    }
}

impl PrivateKeyExportPassphrasePrompt for NativePrivateKeyExportPassphrasePrompt {
    fn request(
        &self,
        context: PrivateKeyExportPassphraseContext,
    ) -> impl Future<
        Output = Result<
            Option<PrivateKeyExportPassphraseCandidate>,
            PrivateKeyExportPassphrasePromptError,
        >,
    > + Send {
        let app = self.app.clone();
        async move {
            let (sender, receiver) = oneshot::channel();
            let task_app = app.clone();
            app.run_on_main_thread(move || {
                let _ = sender.send(prompt_export_passphrase_on_main_thread(&task_app, &context));
            })
            .map_err(|_| PrivateKeyExportPassphrasePromptError::Unavailable)?;
            receiver
                .await
                .map_err(|_| PrivateKeyExportPassphrasePromptError::Unavailable)?
        }
    }
}

#[cfg(target_os = "linux")]
fn prompt_step_up_on_main_thread(
    app: &AppHandle,
    context: &VaultStepUpContext,
) -> Result<Option<Zeroizing<String>>, VaultStepUpPromptError> {
    use gtk::prelude::*;
    use gtk::{Align, DialogFlags, InputPurpose, Orientation, ResponseType};

    let parent = app
        .get_webview_window("main")
        .and_then(|window| window.gtk_window().ok());
    let dialog = gtk::Dialog::with_buttons(
        Some("Confirm AnySSH PIN"),
        parent.as_ref(),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Continue", ResponseType::Accept),
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
        "Enter your AnySSH PIN to {}.",
        context.operation_label()
    )));
    message.set_halign(Align::Start);
    message.set_line_wrap(true);
    stack.pack_start(&message, false, false, 0);

    if context.previous_pin_incorrect() {
        let retry = gtk::Label::new(Some("The previous PIN was not accepted."));
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
    entry.set_placeholder_text(Some("AnySSH PIN"));
    stack.pack_start(&entry, false, false, 0);
    content.pack_start(&stack, true, true, 0);

    dialog.show_all();
    entry.grab_focus();
    let response = dialog.run();
    let pin = if response == ResponseType::Accept {
        let value = Zeroizing::new(entry.text().as_str().to_owned());
        entry.set_text("");
        Some(value)
    } else {
        entry.set_text("");
        None
    };
    dialog.close();
    Ok(pin)
}

#[cfg(target_os = "linux")]
fn prompt_export_passphrase_on_main_thread(
    app: &AppHandle,
    context: &PrivateKeyExportPassphraseContext,
) -> Result<Option<PrivateKeyExportPassphraseCandidate>, PrivateKeyExportPassphrasePromptError> {
    use gtk::prelude::*;
    use gtk::{Align, DialogFlags, InputPurpose, Orientation, ResponseType};

    let parent = app
        .get_webview_window("main")
        .and_then(|window| window.gtk_window().ok());
    let dialog = gtk::Dialog::with_buttons(
        Some("Encrypt exported private key"),
        parent.as_ref(),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Encrypt and export", ResponseType::Accept),
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
    let message = gtk::Label::new(Some(
        "Create a new Passphrase for the exported OpenSSH private key.",
    ));
    message.set_halign(Align::Start);
    message.set_line_wrap(true);
    stack.pack_start(&message, false, false, 0);

    if context.previous_confirmation_mismatch() {
        let retry = gtk::Label::new(Some(
            "The previous Passphrase confirmation did not match or was empty.",
        ));
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

    let passphrase = gtk::Entry::new();
    passphrase.set_visibility(false);
    passphrase.set_input_purpose(InputPurpose::Password);
    passphrase.set_placeholder_text(Some("New export Passphrase"));
    stack.pack_start(&passphrase, false, false, 0);

    let confirmation = gtk::Entry::new();
    confirmation.set_visibility(false);
    confirmation.set_input_purpose(InputPurpose::Password);
    confirmation.set_activates_default(true);
    confirmation.set_placeholder_text(Some("Confirm export Passphrase"));
    stack.pack_start(&confirmation, false, false, 0);
    content.pack_start(&stack, true, true, 0);

    dialog.show_all();
    passphrase.grab_focus();
    let response = dialog.run();
    let candidate = if response == ResponseType::Accept {
        let candidate = PrivateKeyExportPassphraseCandidate::new(
            Zeroizing::new(passphrase.text().as_str().to_owned()),
            Zeroizing::new(confirmation.text().as_str().to_owned()),
        );
        passphrase.set_text("");
        confirmation.set_text("");
        Some(candidate)
    } else {
        passphrase.set_text("");
        confirmation.set_text("");
        None
    };
    dialog.close();
    Ok(candidate)
}

#[cfg(windows)]
fn prompt_step_up_on_main_thread(
    app: &AppHandle,
    context: &VaultStepUpContext,
) -> Result<Option<Zeroizing<String>>, VaultStepUpPromptError> {
    prompt_windows_secret(
        app,
        "Confirm AnySSH PIN",
        &format!(
            "Enter your AnySSH PIN to {} (attempt {} of {}).",
            context.operation_label(),
            context.attempt(),
            context.max_attempts()
        ),
        "AnySSH Vault step-up",
        context.previous_pin_incorrect(),
    )
    .map_err(|_| VaultStepUpPromptError::Unavailable)
}

#[cfg(windows)]
fn prompt_export_passphrase_on_main_thread(
    app: &AppHandle,
    context: &PrivateKeyExportPassphraseContext,
) -> Result<Option<PrivateKeyExportPassphraseCandidate>, PrivateKeyExportPassphrasePromptError> {
    let Some(passphrase) = prompt_windows_secret(
        app,
        "Encrypt exported private key",
        &format!(
            "Create a new Passphrase for the exported OpenSSH private key (attempt {} of {}).",
            context.attempt(),
            context.max_attempts()
        ),
        "AnySSH Private Key export Passphrase",
        context.previous_confirmation_mismatch(),
    )
    .map_err(|_| PrivateKeyExportPassphrasePromptError::Unavailable)?
    else {
        return Ok(None);
    };
    let Some(confirmation) = prompt_windows_secret(
        app,
        "Confirm export Passphrase",
        "Enter the same export Passphrase again.",
        "AnySSH Private Key export Passphrase confirmation",
        false,
    )
    .map_err(|_| PrivateKeyExportPassphrasePromptError::Unavailable)?
    else {
        return Ok(None);
    };
    Ok(Some(PrivateKeyExportPassphraseCandidate::new(
        passphrase,
        confirmation,
    )))
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn prompt_windows_secret(
    app: &AppHandle,
    caption: &str,
    message: &str,
    target: &str,
    previous_incorrect: bool,
) -> Result<Option<Zeroizing<String>>, ()> {
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

    let window = app.get_webview_window("main").ok_or(())?;
    let caption = wide_null_terminated(caption);
    let message = wide_null_terminated(message);
    let target = wide_null_terminated(target);
    let mut username = wide_null_terminated("AnySSH");
    username.resize(CREDUI_MAX_USERNAME_LENGTH as usize, 0);
    let mut password = Zeroizing::new(vec![0_u16; WINDOWS_PASSWORD_BUFFER_CODE_UNITS]);
    let info = CREDUI_INFOW {
        cbSize: size_of::<CREDUI_INFOW>() as u32,
        hwndParent: window.hwnd().map_err(|_| ())?,
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
    if previous_incorrect {
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
        return Err(());
    }

    let password_length = password
        .iter()
        .position(|code_unit| *code_unit == 0)
        .unwrap_or(password.len());
    String::from_utf16(&password[..password_length])
        .map(Zeroizing::new)
        .map(Some)
        .map_err(|_| ())
}

#[cfg(windows)]
fn wide_null_terminated(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn prompt_step_up_on_main_thread(
    _app: &AppHandle,
    _context: &VaultStepUpContext,
) -> Result<Option<Zeroizing<String>>, VaultStepUpPromptError> {
    Err(VaultStepUpPromptError::Unavailable)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn prompt_export_passphrase_on_main_thread(
    _app: &AppHandle,
    _context: &PrivateKeyExportPassphraseContext,
) -> Result<Option<PrivateKeyExportPassphraseCandidate>, PrivateKeyExportPassphrasePromptError> {
    Err(PrivateKeyExportPassphrasePromptError::Unavailable)
}
