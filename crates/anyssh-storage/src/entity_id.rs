use std::fmt::Write as _;

use crate::StorageError;

const RANDOM_BYTES: usize = 16;
const MAX_ID_BYTES: usize = 256;

pub(crate) fn generate_opaque_id(prefix: &str) -> Result<String, StorageError> {
    let mut bytes = [0_u8; RANDOM_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| StorageError::RecordIntegrity)?;

    let mut id = String::with_capacity(prefix.len() + bytes.len() * 2);
    id.push_str(prefix);
    for byte in bytes {
        write!(&mut id, "{byte:02x}").map_err(|_| StorageError::RecordIntegrity)?;
    }
    Ok(id)
}

pub(crate) fn is_valid_opaque_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
