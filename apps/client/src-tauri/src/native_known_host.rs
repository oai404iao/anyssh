use anyssh_app::{KnownHostForgetPrompt, KnownHostForgetPromptContext, KnownHostForgetPromptError};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

pub struct NativeKnownHostForgetPrompt {
    app: AppHandle,
}

impl NativeKnownHostForgetPrompt {
    pub const fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl KnownHostForgetPrompt for NativeKnownHostForgetPrompt {
    async fn confirm(
        &self,
        context: KnownHostForgetPromptContext,
    ) -> Result<bool, KnownHostForgetPromptError> {
        let app = self.app.clone();
        let fingerprints = context.fingerprints_sha256().join("\n");
        let message = format!(
            "Forget the trusted SSH host keys for {}:{}?\n\n{}\n\nThe next connection will require a new trust decision.",
            context.host(),
            context.port(),
            fingerprints
        );
        tauri::async_runtime::spawn_blocking(move || {
            app.dialog()
                .message(message)
                .title("Forget trusted host keys")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Forget trust".to_owned(),
                    "Cancel".to_owned(),
                ))
                .blocking_show()
        })
        .await
        .map_err(|_| KnownHostForgetPromptError::Unavailable)
    }
}
