use anyssh_domain::SshEndpointIdentity;
use ssh_key::{HashAlg, PublicKey};

use crate::{
    StorageError,
    entity_id::{generate_opaque_id, is_valid_opaque_id},
};

const KNOWN_HOST_ID_PREFIX: &str = "known-";
const MAX_KNOWN_HOST_ID_BYTES: usize = 256;
const MAX_HOST_KEY_ALGORITHM_BYTES: usize = 256;
const MAX_HOST_KEY_FINGERPRINT_BYTES: usize = 256;
pub const MAX_KNOWN_HOST_KEYS: usize = 16;
pub const MAX_KNOWN_HOST_PUBLIC_KEY_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownHostKeySummary {
    algorithm: String,
    fingerprint_sha256: String,
}

impl KnownHostKeySummary {
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownHostSummary {
    id: String,
    endpoint: SshEndpointIdentity,
    keys: Vec<KnownHostKeySummary>,
}

impl KnownHostSummary {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn host(&self) -> &str {
        self.endpoint.host()
    }

    pub const fn port(&self) -> u16 {
        self.endpoint.port()
    }

    pub fn endpoint(&self) -> &SshEndpointIdentity {
        &self.endpoint
    }

    pub fn keys(&self) -> &[KnownHostKeySummary] {
        &self.keys
    }

    pub(crate) fn new(
        id: String,
        endpoint: SshEndpointIdentity,
        mut keys: Vec<KnownHostKeySummary>,
    ) -> Result<Self, StorageError> {
        validate_known_host_id(&id)?;
        if keys.is_empty() || keys.len() > MAX_KNOWN_HOST_KEYS {
            return Err(StorageError::InvalidKnownHost);
        }
        keys.sort_by(|left, right| {
            left.algorithm
                .cmp(&right.algorithm)
                .then_with(|| left.fingerprint_sha256.cmp(&right.fingerprint_sha256))
        });
        if keys
            .windows(2)
            .any(|keys| keys[0].fingerprint_sha256 == keys[1].fingerprint_sha256)
        {
            return Err(StorageError::RecordIntegrity);
        }
        Ok(Self { id, endpoint, keys })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedKnownHostPolicy {
    Prompt,
    RequireSha256Set(Vec<String>),
}

impl ResolvedKnownHostPolicy {
    pub(crate) fn trusted(fingerprints: Vec<String>) -> Result<Self, StorageError> {
        if fingerprints.is_empty() || fingerprints.len() > MAX_KNOWN_HOST_KEYS {
            return Err(StorageError::RecordIntegrity);
        }
        Ok(Self::RequireSha256Set(fingerprints))
    }
}

pub(crate) struct ValidatedKnownHostKey {
    algorithm: String,
    fingerprint_sha256: String,
    public_key: Vec<u8>,
}

impl ValidatedKnownHostKey {
    pub(crate) fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub(crate) fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }

    pub(crate) fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    pub(crate) fn summary(&self) -> KnownHostKeySummary {
        KnownHostKeySummary {
            algorithm: self.algorithm.clone(),
            fingerprint_sha256: self.fingerprint_sha256.clone(),
        }
    }
}

pub(crate) fn validate_known_host_key(
    algorithm: String,
    fingerprint_sha256: String,
    public_key: Vec<u8>,
) -> Result<ValidatedKnownHostKey, StorageError> {
    if algorithm.is_empty()
        || algorithm.len() > MAX_HOST_KEY_ALGORITHM_BYTES
        || algorithm.chars().any(char::is_control)
        || fingerprint_sha256.is_empty()
        || fingerprint_sha256.len() > MAX_HOST_KEY_FINGERPRINT_BYTES
        || fingerprint_sha256.chars().any(char::is_control)
        || public_key.is_empty()
        || public_key.len() > MAX_KNOWN_HOST_PUBLIC_KEY_BYTES
    {
        return Err(StorageError::InvalidKnownHost);
    }

    let parsed = PublicKey::from_bytes(&public_key).map_err(|_| StorageError::InvalidKnownHost)?;
    let computed_algorithm = parsed.algorithm().to_string();
    let computed_fingerprint = parsed.fingerprint(HashAlg::Sha256).to_string();
    if algorithm != computed_algorithm || fingerprint_sha256 != computed_fingerprint {
        return Err(StorageError::InvalidKnownHost);
    }

    Ok(ValidatedKnownHostKey {
        algorithm,
        fingerprint_sha256,
        public_key,
    })
}

pub(crate) fn generate_known_host_id() -> Result<String, StorageError> {
    generate_opaque_id(KNOWN_HOST_ID_PREFIX)
}

pub(crate) fn validate_known_host_id(id: &str) -> Result<(), StorageError> {
    if id.len() <= MAX_KNOWN_HOST_ID_BYTES
        && id.starts_with(KNOWN_HOST_ID_PREFIX)
        && is_valid_opaque_id(id)
    {
        Ok(())
    } else {
        Err(StorageError::InvalidKnownHost)
    }
}

#[cfg(test)]
mod tests {
    use ssh_key::{Algorithm, PrivateKey};

    use super::*;

    fn fixture_key() -> (String, String, Vec<u8>) {
        let private_key =
            PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("fixture key");
        let public_key = private_key.public_key();
        (
            public_key.algorithm().to_string(),
            public_key.fingerprint(HashAlg::Sha256).to_string(),
            public_key.to_bytes().expect("public key bytes"),
        )
    }

    #[test]
    fn generated_ids_are_opaque() {
        let first = generate_known_host_id().expect("first ID");
        let second = generate_known_host_id().expect("second ID");
        assert_ne!(first, second);
        validate_known_host_id(&first).expect("valid ID");
    }

    #[test]
    fn key_fields_are_recomputed_from_public_key_bytes() {
        let (algorithm, fingerprint, bytes) = fixture_key();
        let key = validate_known_host_key(algorithm.clone(), fingerprint.clone(), bytes)
            .expect("valid known host key");
        assert_eq!(key.algorithm(), algorithm);
        assert_eq!(key.fingerprint_sha256(), fingerprint);
    }

    #[test]
    fn inconsistent_key_metadata_is_rejected() {
        let (algorithm, fingerprint, bytes) = fixture_key();
        assert!(matches!(
            validate_known_host_key(format!("{algorithm}-changed"), fingerprint, bytes),
            Err(StorageError::InvalidKnownHost)
        ));
    }
}
