use std::collections::HashSet;

use crate::{
    StorageError,
    entity_id::{generate_opaque_id, is_valid_opaque_id},
    host::validate_host_id,
};

const JUMP_ROUTE_ID_PREFIX: &str = "route-";
const MAX_JUMP_ROUTE_LABEL_BYTES: usize = 4096;
pub const MAX_JUMP_ROUTE_STEPS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JumpRouteSummary {
    id: String,
    label: String,
    host_ids: Vec<String>,
}

impl JumpRouteSummary {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn host_ids(&self) -> &[String] {
        &self.host_ids
    }

    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        host_ids: Vec<String>,
    ) -> Result<Self, StorageError> {
        let id = id.into();
        validate_jump_route_id(&id)?;
        let label = normalize_label(label.into())?;
        if host_ids.is_empty() || host_ids.len() > MAX_JUMP_ROUTE_STEPS {
            return Err(StorageError::InvalidJumpRoute);
        }

        let mut unique_hosts = HashSet::with_capacity(host_ids.len());
        for host_id in &host_ids {
            validate_host_id(host_id)?;
            if !unique_hosts.insert(host_id.as_str()) {
                return Err(StorageError::InvalidJumpRoute);
            }
        }

        Ok(Self {
            id,
            label,
            host_ids,
        })
    }
}

pub(crate) fn generate_jump_route_id() -> Result<String, StorageError> {
    generate_opaque_id(JUMP_ROUTE_ID_PREFIX)
}

pub(crate) fn validate_jump_route_id(id: &str) -> Result<(), StorageError> {
    if is_valid_opaque_id(id) {
        Ok(())
    } else {
        Err(StorageError::InvalidJumpRoute)
    }
}

fn normalize_label(value: String) -> Result<String, StorageError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_JUMP_ROUTE_LABEL_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidJumpRoute);
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_route_ids_are_opaque_and_valid() {
        let first = generate_jump_route_id().expect("first ID");
        let second = generate_jump_route_id().expect("second ID");

        assert_ne!(first, second);
        assert!(first.starts_with(JUMP_ROUTE_ID_PREFIX));
        validate_jump_route_id(&first).expect("valid generated ID");
    }

    #[test]
    fn route_requires_unique_ordered_hosts() {
        let route = JumpRouteSummary::new(
            "route-fixture",
            "  Production path  ",
            vec!["host-one".to_owned(), "host-two".to_owned()],
        )
        .expect("valid route");
        assert_eq!(route.label(), "Production path");
        assert_eq!(route.host_ids(), ["host-one", "host-two"]);

        assert!(matches!(
            JumpRouteSummary::new(
                "route-duplicate",
                "Duplicate",
                vec!["host-one".to_owned(), "host-one".to_owned()]
            ),
            Err(StorageError::InvalidJumpRoute)
        ));
    }
}
