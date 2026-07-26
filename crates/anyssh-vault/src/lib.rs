#![forbid(unsafe_code)]

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use zeroize::Zeroizing;

const FORMAT_VERSION: u32 = 1;
const ARGON2_VERSION: u32 = 19;
const VMK_BYTES: usize = 32;
const KEY_BYTES: usize = 32;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const WRAPPED_VMK_BYTES: usize = VMK_BYTES + 16;
const ID_BYTES: usize = 16;
const MAX_BOOTSTRAP_BYTES: u64 = 64 * 1024;
const MIN_PIN_BYTES: usize = 4;
const MAX_PIN_BYTES: usize = 1024;
const MAX_KEY_SLOTS: usize = 16;
const MIN_ARGON2_MEMORY_KIB: u32 = 8 * 1024;
const MAX_ARGON2_MEMORY_KIB: u32 = 256 * 1024;
const MAX_ARGON2_ITERATIONS: u32 = 10;
const MAX_ARGON2_PARALLELISM: u32 = 4;

const PIN_SLOT_KIND: &str = "pin";
const ARGON2ID_ALGORITHM: &str = "argon2id";
const XCHACHA20_ALGORITHM: &str = "xchacha20poly1305";
const DATABASE_KEY_INFO: &[u8] = b"anyssh/v1/sqlcipher-database";
const RECORD_KEY_INFO: &[u8] = b"anyssh/v1/record-encryption";

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("PIN must contain between {MIN_PIN_BYTES} and {MAX_PIN_BYTES} bytes")]
    InvalidPinLength,
    #[error("Argon2id parameters are outside the supported safety bounds")]
    InvalidKdfParameters,
    #[error("vault bootstrap already exists")]
    AlreadyExists,
    #[error("vault bootstrap was not found")]
    NotFound,
    #[error("vault bootstrap is invalid")]
    InvalidBootstrap,
    #[error("vault format version {0} is not supported")]
    UnsupportedVersion(u32),
    #[error("vault unlock failed")]
    UnlockFailed,
    #[error("the operating system random number generator failed")]
    Random,
    #[error("vault key derivation failed")]
    KeyDerivation,
    #[error("vault bootstrap I/O failed")]
    Io(#[source] std::io::Error),
    #[error("vault bootstrap serialization failed")]
    Serialization(#[source] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinKdfParameters {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl PinKdfParameters {
    pub const fn phase0_default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }

    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self, VaultError> {
        let parameters = Self {
            memory_kib,
            iterations,
            parallelism,
        };
        parameters.validate()?;
        Ok(parameters)
    }

    pub const fn memory_kib(self) -> u32 {
        self.memory_kib
    }

    pub const fn iterations(self) -> u32 {
        self.iterations
    }

    pub const fn parallelism(self) -> u32 {
        self.parallelism
    }

    fn validate(self) -> Result<(), VaultError> {
        if !(MIN_ARGON2_MEMORY_KIB..=MAX_ARGON2_MEMORY_KIB).contains(&self.memory_kib)
            || !(1..=MAX_ARGON2_ITERATIONS).contains(&self.iterations)
            || !(1..=MAX_ARGON2_PARALLELISM).contains(&self.parallelism)
        {
            return Err(VaultError::InvalidKdfParameters);
        }
        Ok(())
    }
}

impl Default for PinKdfParameters {
    fn default() -> Self {
        Self::phase0_default()
    }
}

pub struct BootstrapDocument {
    document: BootstrapFile,
}

impl BootstrapDocument {
    pub fn create_pin(
        pin: &str,
        parameters: PinKdfParameters,
    ) -> Result<(Self, UnlockedVault), VaultError> {
        validate_pin(pin)?;
        parameters.validate()?;

        let vault_id = random_identifier()?;
        let slot_id = random_identifier()?;
        let mut vmk = Zeroizing::new([0_u8; VMK_BYTES]);
        let mut salt = [0_u8; SALT_BYTES];
        let mut nonce = [0_u8; NONCE_BYTES];
        fill_random(&mut vmk[..])?;
        fill_random(&mut salt)?;
        fill_random(&mut nonce)?;

        let salt_b64 = URL_SAFE_NO_PAD.encode(salt);
        let nonce_b64 = URL_SAFE_NO_PAD.encode(nonce);
        let kdf = PinKdfDocument {
            algorithm: ARGON2ID_ALGORITHM.to_owned(),
            version: ARGON2_VERSION,
            memory_kib: parameters.memory_kib,
            iterations: parameters.iterations,
            parallelism: parameters.parallelism,
            salt: salt_b64,
        };
        let aad = slot_aad(&vault_id, &slot_id, &kdf);
        let kek = derive_pin_key(pin, &salt, parameters)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(&kek[..]).map_err(|_| VaultError::KeyDerivation)?;
        let wrapped_vmk = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &vmk[..],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| VaultError::KeyDerivation)?;

        let document = BootstrapFile {
            format_version: FORMAT_VERSION,
            vault_id: vault_id.clone(),
            key_slots: vec![KeySlotDocument {
                id: slot_id,
                kind: PIN_SLOT_KIND.to_owned(),
                kdf,
                wrapping: WrappingDocument {
                    algorithm: XCHACHA20_ALGORITHM.to_owned(),
                    nonce: nonce_b64,
                    ciphertext: URL_SAFE_NO_PAD.encode(wrapped_vmk),
                },
            }],
        };

        Ok((Self { document }, UnlockedVault { vault_id, vmk }))
    }

    pub fn load(path: &Path) -> Result<Self, VaultError> {
        let metadata = fs::metadata(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => VaultError::NotFound,
            _ => VaultError::Io(error),
        })?;
        if metadata.len() > MAX_BOOTSTRAP_BYTES {
            return Err(VaultError::InvalidBootstrap);
        }

        let mut file = File::open(path).map_err(VaultError::Io)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes).map_err(VaultError::Io)?;
        let document: BootstrapFile =
            serde_json::from_slice(&bytes).map_err(VaultError::Serialization)?;
        let bootstrap = Self { document };
        bootstrap.validate()?;
        Ok(bootstrap)
    }

    pub fn write_new(&self, path: &Path) -> Result<(), VaultError> {
        self.validate()?;
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }

        let parent = path.parent().ok_or(VaultError::InvalidBootstrap)?;
        fs::create_dir_all(parent).map_err(VaultError::Io)?;
        let temporary_path = parent.join(format!(
            ".{}.tmp-{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("vault-bootstrap"),
            random_identifier()?
        ));
        let serialized =
            serde_json::to_vec_pretty(&self.document).map_err(VaultError::Serialization)?;

        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }

            let mut file = options.open(&temporary_path).map_err(VaultError::Io)?;
            file.write_all(&serialized).map_err(VaultError::Io)?;
            file.write_all(b"\n").map_err(VaultError::Io)?;
            file.sync_all().map_err(VaultError::Io)?;
            fs::rename(&temporary_path, path).map_err(VaultError::Io)?;
            sync_parent_directory(parent);
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        result
    }

    pub fn unlock_pin(&self, pin: &str) -> Result<UnlockedVault, VaultError> {
        validate_pin(pin)?;
        self.validate()?;

        for slot in &self.document.key_slots {
            if slot.kind != PIN_SLOT_KIND {
                continue;
            }

            let parameters = PinKdfParameters::new(
                slot.kdf.memory_kib,
                slot.kdf.iterations,
                slot.kdf.parallelism,
            )
            .map_err(|_| VaultError::UnlockFailed)?;
            let salt =
                decode_array::<SALT_BYTES>(&slot.kdf.salt).map_err(|_| VaultError::UnlockFailed)?;
            let nonce = decode_array::<NONCE_BYTES>(&slot.wrapping.nonce)
                .map_err(|_| VaultError::UnlockFailed)?;
            let ciphertext = URL_SAFE_NO_PAD
                .decode(&slot.wrapping.ciphertext)
                .map_err(|_| VaultError::UnlockFailed)?;
            if ciphertext.len() != WRAPPED_VMK_BYTES {
                continue;
            }

            let kek =
                derive_pin_key(pin, &salt, parameters).map_err(|_| VaultError::UnlockFailed)?;
            let cipher = XChaCha20Poly1305::new_from_slice(&kek[..])
                .map_err(|_| VaultError::UnlockFailed)?;
            let aad = slot_aad(&self.document.vault_id, &slot.id, &slot.kdf);
            let plaintext = cipher.decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: aad.as_bytes(),
                },
            );

            if let Ok(plaintext) = plaintext {
                let plaintext = Zeroizing::new(plaintext);
                if plaintext.len() != VMK_BYTES {
                    continue;
                }
                let mut vmk = Zeroizing::new([0_u8; VMK_BYTES]);
                vmk.copy_from_slice(&plaintext);
                return Ok(UnlockedVault {
                    vault_id: self.document.vault_id.clone(),
                    vmk,
                });
            }
        }

        Err(VaultError::UnlockFailed)
    }

    pub fn vault_id(&self) -> &str {
        &self.document.vault_id
    }

    fn validate(&self) -> Result<(), VaultError> {
        if self.document.format_version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(self.document.format_version));
        }
        if self.document.vault_id.is_empty()
            || self.document.key_slots.is_empty()
            || self.document.key_slots.len() > MAX_KEY_SLOTS
        {
            return Err(VaultError::InvalidBootstrap);
        }
        decode_array::<ID_BYTES>(&self.document.vault_id)
            .map_err(|_| VaultError::InvalidBootstrap)?;

        let mut slot_ids = HashSet::with_capacity(self.document.key_slots.len());
        for slot in &self.document.key_slots {
            if slot.id.is_empty()
                || !slot_ids.insert(slot.id.as_str())
                || slot.kind != PIN_SLOT_KIND
                || slot.kdf.algorithm != ARGON2ID_ALGORITHM
                || slot.kdf.version != ARGON2_VERSION
                || slot.wrapping.algorithm != XCHACHA20_ALGORITHM
            {
                return Err(VaultError::InvalidBootstrap);
            }
            decode_array::<ID_BYTES>(&slot.id).map_err(|_| VaultError::InvalidBootstrap)?;
            PinKdfParameters::new(
                slot.kdf.memory_kib,
                slot.kdf.iterations,
                slot.kdf.parallelism,
            )
            .map_err(|_| VaultError::InvalidBootstrap)?;
            decode_array::<SALT_BYTES>(&slot.kdf.salt).map_err(|_| VaultError::InvalidBootstrap)?;
            decode_array::<NONCE_BYTES>(&slot.wrapping.nonce)
                .map_err(|_| VaultError::InvalidBootstrap)?;
            let ciphertext = URL_SAFE_NO_PAD
                .decode(&slot.wrapping.ciphertext)
                .map_err(|_| VaultError::InvalidBootstrap)?;
            if ciphertext.len() != WRAPPED_VMK_BYTES {
                return Err(VaultError::InvalidBootstrap);
            }
        }

        Ok(())
    }
}

pub struct UnlockedVault {
    vault_id: String,
    vmk: Zeroizing<[u8; VMK_BYTES]>,
}

impl UnlockedVault {
    pub fn vault_id(&self) -> &str {
        &self.vault_id
    }

    pub fn derive_keys(&self) -> Result<DerivedVaultKeys, VaultError> {
        let hkdf = Hkdf::<Sha256>::new(Some(self.vault_id.as_bytes()), &self.vmk[..]);
        let mut database_key = Zeroizing::new([0_u8; KEY_BYTES]);
        let mut record_key = Zeroizing::new([0_u8; KEY_BYTES]);
        hkdf.expand(DATABASE_KEY_INFO, &mut database_key[..])
            .map_err(|_| VaultError::KeyDerivation)?;
        hkdf.expand(RECORD_KEY_INFO, &mut record_key[..])
            .map_err(|_| VaultError::KeyDerivation)?;

        Ok(DerivedVaultKeys {
            vault_id: self.vault_id.clone(),
            database_key,
            record_key,
        })
    }
}

pub struct DerivedVaultKeys {
    vault_id: String,
    database_key: Zeroizing<[u8; KEY_BYTES]>,
    record_key: Zeroizing<[u8; KEY_BYTES]>,
}

impl DerivedVaultKeys {
    pub fn vault_id(&self) -> &str {
        &self.vault_id
    }

    pub fn database_key(&self) -> &[u8; KEY_BYTES] {
        &self.database_key
    }

    pub fn record_key(&self) -> &[u8; KEY_BYTES] {
        &self.record_key
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapFile {
    format_version: u32,
    vault_id: String,
    key_slots: Vec<KeySlotDocument>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeySlotDocument {
    id: String,
    kind: String,
    kdf: PinKdfDocument,
    wrapping: WrappingDocument,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PinKdfDocument {
    algorithm: String,
    version: u32,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    salt: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WrappingDocument {
    algorithm: String,
    nonce: String,
    ciphertext: String,
}

fn validate_pin(pin: &str) -> Result<(), VaultError> {
    if !(MIN_PIN_BYTES..=MAX_PIN_BYTES).contains(&pin.len()) {
        return Err(VaultError::InvalidPinLength);
    }
    Ok(())
}

fn derive_pin_key(
    pin: &str,
    salt: &[u8; SALT_BYTES],
    parameters: PinKdfParameters,
) -> Result<Zeroizing<[u8; KEY_BYTES]>, VaultError> {
    let params = Params::new(
        parameters.memory_kib,
        parameters.iterations,
        parameters.parallelism,
        Some(KEY_BYTES),
    )
    .map_err(|_| VaultError::InvalidKdfParameters)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = Zeroizing::new([0_u8; KEY_BYTES]);
    argon2
        .hash_password_into(pin.as_bytes(), salt, &mut output[..])
        .map_err(|_| VaultError::KeyDerivation)?;
    Ok(output)
}

fn slot_aad(vault_id: &str, slot_id: &str, kdf: &PinKdfDocument) -> String {
    format!(
        "anyssh/pin-slot/v1|{vault_id}|{slot_id}|{}|{}|{}|{}|{}",
        kdf.algorithm, kdf.version, kdf.memory_kib, kdf.iterations, kdf.parallelism
    )
}

fn fill_random(output: &mut [u8]) -> Result<(), VaultError> {
    getrandom::fill(output).map_err(|_| VaultError::Random)
}

fn random_identifier() -> Result<String, VaultError> {
    let mut bytes = [0_u8; ID_BYTES];
    fill_random(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_array<const SIZE: usize>(encoded: &str) -> Result<[u8; SIZE], VaultError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| VaultError::InvalidBootstrap)?;
    decoded.try_into().map_err(|_| VaultError::InvalidBootstrap)
}

fn sync_parent_directory(path: &Path) {
    #[cfg(unix)]
    {
        if let Ok(directory) = File::open(path) {
            let _ = directory.sync_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parameters() -> PinKdfParameters {
        PinKdfParameters::new(8 * 1024, 1, 1).expect("test parameters")
    }

    #[test]
    fn pin_slot_round_trip_derives_stable_keys() {
        let (bootstrap, unlocked) =
            BootstrapDocument::create_pin("123456", test_parameters()).expect("create");
        let expected = unlocked.derive_keys().expect("derive");
        let reopened = bootstrap.unlock_pin("123456").expect("unlock");
        let actual = reopened.derive_keys().expect("derive");

        assert_eq!(expected.vault_id(), actual.vault_id());
        assert_eq!(expected.database_key(), actual.database_key());
        assert_eq!(expected.record_key(), actual.record_key());
        assert_ne!(actual.database_key(), actual.record_key());
    }

    #[test]
    fn wrong_pin_has_generic_error() {
        let (bootstrap, _) =
            BootstrapDocument::create_pin("123456", test_parameters()).expect("create");
        let error = bootstrap.unlock_pin("654321").err().expect("must fail");

        assert!(matches!(error, VaultError::UnlockFailed));
        assert!(!error.to_string().contains("654321"));
    }

    #[test]
    fn bootstrap_file_contains_no_pin() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("vault.bootstrap.json");
        let (bootstrap, _) =
            BootstrapDocument::create_pin("fixture-pin", test_parameters()).expect("create");
        bootstrap.write_new(&path).expect("write");

        let text = fs::read_to_string(path).expect("read");
        assert!(!text.contains("fixture-pin"));
        assert!(text.contains(ARGON2ID_ALGORITHM));
        assert!(text.contains(XCHACHA20_ALGORITHM));
    }

    #[test]
    fn corrupted_wrapped_key_does_not_unlock() {
        let (mut bootstrap, _) =
            BootstrapDocument::create_pin("123456", test_parameters()).expect("create");
        bootstrap.document.key_slots[0].wrapping.ciphertext =
            URL_SAFE_NO_PAD.encode([0_u8; WRAPPED_VMK_BYTES]);

        let error = bootstrap.unlock_pin("123456").err().expect("must fail");
        assert!(matches!(error, VaultError::UnlockFailed));
    }

    #[test]
    fn unsafe_kdf_parameters_are_rejected() {
        assert!(matches!(
            PinKdfParameters::new(1024, 1, 1),
            Err(VaultError::InvalidKdfParameters)
        ));
        assert!(matches!(
            PinKdfParameters::new(64 * 1024, 0, 1),
            Err(VaultError::InvalidKdfParameters)
        ));
    }
}
