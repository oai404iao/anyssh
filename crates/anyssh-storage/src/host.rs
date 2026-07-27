use crate::{
    Override, StorageError,
    credential::validate_credential_id,
    entity_id::{generate_opaque_id, is_valid_opaque_id},
    group::validate_group_id,
};

const HOST_ID_PREFIX: &str = "host-";
const MAX_HOST_ID_BYTES: usize = 256;
const MAX_HOST_LABEL_BYTES: usize = 4096;
const MAX_HOST_ADDRESS_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostSummary {
    id: String,
    display_name: String,
    host: String,
    port: u16,
    group_id: Option<String>,
    credential_override: Override<String>,
    jump_route_override: Override<String>,
    effective_credential_id: Option<String>,
    effective_jump_route_id: Option<String>,
}

impl HostSummary {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }

    pub const fn credential_override(&self) -> &Override<String> {
        &self.credential_override
    }

    pub const fn jump_route_override(&self) -> &Override<String> {
        &self.jump_route_override
    }

    pub fn effective_credential_id(&self) -> Option<&str> {
        self.effective_credential_id.as_deref()
    }

    pub fn effective_jump_route_id(&self) -> Option<&str> {
        self.effective_jump_route_id.as_deref()
    }

    pub fn credential_id(&self) -> Option<&str> {
        self.effective_credential_id()
    }

    pub fn jump_route_id(&self) -> Option<&str> {
        self.effective_jump_route_id()
    }

    pub(crate) fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        credential_id: Option<String>,
        jump_route_id: Option<String>,
    ) -> Result<Self, StorageError> {
        let credential_override = credential_id
            .clone()
            .map_or(Override::Inherit, Override::Set);
        let jump_route_override = jump_route_id
            .clone()
            .map_or(Override::Inherit, Override::Set);
        Self::from_stored(
            id,
            display_name,
            host,
            port,
            None,
            credential_override,
            jump_route_override,
            credential_id,
            jump_route_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_overrides(
        id: impl Into<String>,
        display_name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> Result<Self, StorageError> {
        let effective_credential_id = credential_override.value().cloned();
        let effective_jump_route_id = jump_route_override.value().cloned();
        Self::from_stored(
            id,
            display_name,
            host,
            port,
            group_id,
            credential_override,
            jump_route_override,
            effective_credential_id,
            effective_jump_route_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_stored(
        id: impl Into<String>,
        display_name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
        effective_credential_id: Option<String>,
        effective_jump_route_id: Option<String>,
    ) -> Result<Self, StorageError> {
        let id = id.into();
        validate_host_id(&id)?;
        let display_name = normalize_required_text(display_name.into(), MAX_HOST_LABEL_BYTES)?;
        let host = normalize_required_text(host.into(), MAX_HOST_ADDRESS_BYTES)?;
        if port == 0 {
            return Err(StorageError::InvalidHost);
        }
        if let Some(group_id) = group_id.as_deref() {
            validate_group_id(group_id)?;
        }
        if let Some(credential_id) = credential_override.value() {
            validate_credential_id(credential_id)?;
        }
        if let Some(jump_route_id) = jump_route_override.value() {
            validate_jump_route_reference(jump_route_id)?;
        }
        if let Some(credential_id) = effective_credential_id.as_deref() {
            validate_credential_id(credential_id)?;
        }
        if let Some(jump_route_id) = effective_jump_route_id.as_deref() {
            validate_jump_route_reference(jump_route_id)?;
        }

        Ok(Self {
            id,
            display_name,
            host,
            port,
            group_id,
            credential_override,
            jump_route_override,
            effective_credential_id,
            effective_jump_route_id,
        })
    }
}

pub(crate) fn generate_host_id() -> Result<String, StorageError> {
    generate_opaque_id(HOST_ID_PREFIX)
}

pub(crate) fn validate_host_id(id: &str) -> Result<(), StorageError> {
    if !id.is_empty() && id.len() <= MAX_HOST_ID_BYTES && !id.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(StorageError::InvalidHost)
    }
}

fn validate_jump_route_reference(id: &str) -> Result<(), StorageError> {
    if is_valid_opaque_id(id) {
        Ok(())
    } else {
        Err(StorageError::InvalidJumpRoute)
    }
}

fn normalize_required_text(value: String, max_bytes: usize) -> Result<String, StorageError> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(StorageError::InvalidHost);
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_host_ids_are_opaque_and_valid() {
        let first = generate_host_id().expect("first ID");
        let second = generate_host_id().expect("second ID");

        assert_ne!(first, second);
        assert!(first.starts_with(HOST_ID_PREFIX));
        validate_host_id(&first).expect("valid generated ID");
    }

    #[test]
    fn host_normalizes_metadata_and_keeps_only_references() {
        let host = HostSummary::new(
            "host-fixture",
            "  Production  ",
            "  server.example  ",
            22,
            Some("cred-fixture".to_owned()),
            Some("route-fixture".to_owned()),
        )
        .expect("valid host");

        assert_eq!(host.display_name(), "Production");
        assert_eq!(host.host(), "server.example");
        assert_eq!(host.group_id(), None);
        assert_eq!(
            host.credential_override(),
            &Override::Set("cred-fixture".to_owned())
        );
        assert_eq!(
            host.jump_route_override(),
            &Override::Set("route-fixture".to_owned())
        );
        assert_eq!(host.credential_id(), Some("cred-fixture"));
        assert_eq!(host.jump_route_id(), Some("route-fixture"));
    }

    #[test]
    fn host_can_distinguish_inherit_set_and_clear() {
        let host = HostSummary::new_with_overrides(
            "host-fixture",
            "Fixture",
            "fixture.internal",
            22,
            Some("group-production".to_owned()),
            Override::Inherit,
            Override::Clear,
        )
        .expect("valid host");

        assert_eq!(host.group_id(), Some("group-production"));
        assert_eq!(host.credential_override(), &Override::Inherit);
        assert_eq!(host.jump_route_override(), &Override::Clear);
        assert_eq!(host.credential_id(), None);
        assert_eq!(host.jump_route_id(), None);
    }
}
