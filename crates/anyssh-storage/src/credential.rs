use std::fmt::{self, Write as _};

use zeroize::Zeroizing;

use crate::StorageError;

const CREDENTIAL_ID_PREFIX: &str = "cred-";
const CREDENTIAL_ID_RANDOM_BYTES: usize = 16;
const MAX_CREDENTIAL_ID_BYTES: usize = 256;
const MAX_CREDENTIAL_LABEL_BYTES: usize = 4096;
const MAX_CREDENTIAL_USERNAME_BYTES: usize = 4096;
const MAX_PASSWORD_BYTES: usize = 64 * 1024;
const MAX_PRIVATE_KEY_BYTES: usize = 1024 * 1024;
const MAX_PRIVATE_KEY_PASSPHRASE_BYTES: usize = 64 * 1024;
const MAX_SYSTEM_AGENT_FINGERPRINT_BYTES: usize = 256;

const PASSWORD_KIND: &str = "password";
const PRIVATE_KEY_KIND: &str = "private_key";
const SYSTEM_AGENT_KIND: &str = "system_agent";
const KEYBOARD_INTERACTIVE_KIND: &str = "keyboard_interactive";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    Password,
    PrivateKey,
    SystemAgent,
    KeyboardInteractive,
}

impl CredentialKind {
    pub(crate) const fn storage_value(self) -> &'static str {
        match self {
            Self::Password => PASSWORD_KIND,
            Self::PrivateKey => PRIVATE_KEY_KIND,
            Self::SystemAgent => SYSTEM_AGENT_KIND,
            Self::KeyboardInteractive => KEYBOARD_INTERACTIVE_KIND,
        }
    }

    pub(crate) fn from_storage_value(value: &str) -> Result<Self, StorageError> {
        match value {
            PASSWORD_KIND => Ok(Self::Password),
            PRIVATE_KEY_KIND => Ok(Self::PrivateKey),
            SYSTEM_AGENT_KIND => Ok(Self::SystemAgent),
            KEYBOARD_INTERACTIVE_KIND => Ok(Self::KeyboardInteractive),
            _ => Err(StorageError::RecordIntegrity),
        }
    }
}

pub enum CredentialSecret {
    Password {
        password: Zeroizing<String>,
    },
    PrivateKey {
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    },
    SystemAgent {
        identity_fingerprint_sha256: Zeroizing<String>,
    },
    KeyboardInteractive,
}

impl CredentialSecret {
    pub const fn kind(&self) -> CredentialKind {
        match self {
            Self::Password { .. } => CredentialKind::Password,
            Self::PrivateKey { .. } => CredentialKind::PrivateKey,
            Self::SystemAgent { .. } => CredentialKind::SystemAgent,
            Self::KeyboardInteractive => CredentialKind::KeyboardInteractive,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        match self {
            Self::Password { password } if password.len() <= MAX_PASSWORD_BYTES => Ok(()),
            Self::PrivateKey {
                private_key,
                passphrase,
            } if !private_key.is_empty()
                && private_key.len() <= MAX_PRIVATE_KEY_BYTES
                && passphrase
                    .as_ref()
                    .is_none_or(|value| value.len() <= MAX_PRIVATE_KEY_PASSPHRASE_BYTES) =>
            {
                Ok(())
            }
            Self::SystemAgent {
                identity_fingerprint_sha256,
            } if valid_system_agent_fingerprint(identity_fingerprint_sha256) => Ok(()),
            Self::KeyboardInteractive => Ok(()),
            Self::Password { .. } | Self::PrivateKey { .. } | Self::SystemAgent { .. } => {
                Err(StorageError::InvalidCredential)
            }
        }
    }
}

impl fmt::Debug for CredentialSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password { .. } => formatter
                .debug_struct("Password")
                .field("password", &"<redacted>")
                .finish(),
            Self::PrivateKey { passphrase, .. } => formatter
                .debug_struct("PrivateKey")
                .field("private_key", &"<redacted>")
                .field("passphrase", &passphrase.as_ref().map(|_| "<redacted>"))
                .finish(),
            Self::SystemAgent { .. } => formatter
                .debug_struct("SystemAgent")
                .field("identity_fingerprint_sha256", &"<selected>")
                .finish(),
            Self::KeyboardInteractive => formatter
                .debug_struct("KeyboardInteractive")
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialSummary {
    id: String,
    label: String,
    username: String,
    kind: CredentialKind,
}

impl CredentialSummary {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub const fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        username: impl Into<String>,
        kind: CredentialKind,
    ) -> Result<Self, StorageError> {
        let id = id.into();
        let label = label.into();
        let username = username.into();
        validate_credential_id(&id)?;
        let label = normalize_required_text(label, MAX_CREDENTIAL_LABEL_BYTES)?;
        let username = normalize_required_text(username, MAX_CREDENTIAL_USERNAME_BYTES)?;
        Ok(Self {
            id,
            label,
            username,
            kind,
        })
    }
}

pub struct ResolvedCredential {
    username: String,
    secret: CredentialSecret,
}

impl ResolvedCredential {
    pub fn into_parts(self) -> (String, CredentialSecret) {
        (self.username, self.secret)
    }
}

impl fmt::Debug for ResolvedCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedCredential")
            .field("username", &self.username)
            .field("secret", &self.secret)
            .finish()
    }
}

pub(crate) struct CredentialRecord {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) username: String,
    pub(crate) secret: CredentialSecret,
}

impl CredentialRecord {
    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        username: impl Into<String>,
        secret: CredentialSecret,
    ) -> Result<Self, StorageError> {
        secret.validate()?;
        let summary = CredentialSummary::new(id, label, username, secret.kind())?;
        Ok(Self {
            id: summary.id,
            label: summary.label,
            username: summary.username,
            secret,
        })
    }

    pub(crate) fn summary(&self) -> CredentialSummary {
        CredentialSummary {
            id: self.id.clone(),
            label: self.label.clone(),
            username: self.username.clone(),
            kind: self.secret.kind(),
        }
    }

    pub(crate) fn into_resolved(self) -> ResolvedCredential {
        ResolvedCredential {
            username: self.username,
            secret: self.secret,
        }
    }
}

pub(crate) fn generate_credential_id() -> Result<String, StorageError> {
    let mut bytes = [0_u8; CREDENTIAL_ID_RANDOM_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| StorageError::RecordIntegrity)?;

    let mut id = String::with_capacity(CREDENTIAL_ID_PREFIX.len() + bytes.len() * 2);
    id.push_str(CREDENTIAL_ID_PREFIX);
    for byte in bytes {
        write!(&mut id, "{byte:02x}").map_err(|_| StorageError::RecordIntegrity)?;
    }
    Ok(id)
}

pub(crate) fn validate_credential_id(id: &str) -> Result<(), StorageError> {
    if id.is_empty()
        || id.len() > MAX_CREDENTIAL_ID_BYTES
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(StorageError::InvalidCredential);
    }
    Ok(())
}

fn normalize_required_text(value: String, max_bytes: usize) -> Result<String, StorageError> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(StorageError::InvalidCredential);
    }
    Ok(value.to_owned())
}

fn valid_system_agent_fingerprint(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("SHA256:") else {
        return false;
    };
    !encoded.is_empty()
        && value.len() <= MAX_SYSTEM_AGENT_FINGERPRINT_BYTES
        && encoded.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_output_is_redacted() {
        let password = CredentialSecret::Password {
            password: Zeroizing::new("password-do-not-log".to_owned()),
        };
        let key = CredentialSecret::PrivateKey {
            private_key: Zeroizing::new("private-key-do-not-log".to_owned()),
            passphrase: Some(Zeroizing::new("passphrase-do-not-log".to_owned())),
        };
        let agent = CredentialSecret::SystemAgent {
            identity_fingerprint_sha256: Zeroizing::new(
                "SHA256:agent-fingerprint-do-not-log".to_owned(),
            ),
        };
        let interactive = CredentialSecret::KeyboardInteractive;

        let debug = format!("{password:?} {key:?} {agent:?} {interactive:?}");
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("<selected>"));
        assert!(debug.contains("KeyboardInteractive"));
        assert!(!debug.contains("password-do-not-log"));
        assert!(!debug.contains("private-key-do-not-log"));
        assert!(!debug.contains("passphrase-do-not-log"));
        assert!(!debug.contains("agent-fingerprint-do-not-log"));
    }

    #[test]
    fn system_agent_fingerprint_must_use_sha256_format() {
        assert!(
            CredentialSecret::SystemAgent {
                identity_fingerprint_sha256: Zeroizing::new("SHA256:abcdefghijklmnop".to_owned()),
            }
            .validate()
            .is_ok()
        );
        for invalid in ["", "MD5:abcdef", "SHA256:", "SHA256:contains space"] {
            assert!(matches!(
                CredentialSecret::SystemAgent {
                    identity_fingerprint_sha256: Zeroizing::new(invalid.to_owned()),
                }
                .validate(),
                Err(StorageError::InvalidCredential)
            ));
        }
    }

    #[test]
    fn generated_ids_are_opaque_and_valid() {
        let first = generate_credential_id().expect("first ID");
        let second = generate_credential_id().expect("second ID");

        assert_ne!(first, second);
        assert!(first.starts_with(CREDENTIAL_ID_PREFIX));
        validate_credential_id(&first).expect("valid generated ID");
    }
}
