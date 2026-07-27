use crate::{
    Override, StorageError,
    credential::validate_credential_id,
    entity_id::{generate_opaque_id, is_valid_opaque_id},
    jump_route::validate_jump_route_id,
};

const GROUP_ID_PREFIX: &str = "group-";
const MAX_GROUP_LABEL_BYTES: usize = 4096;
pub const MAX_GROUP_DEPTH: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSummary {
    id: String,
    label: String,
    parent_group_id: Option<String>,
    credential_override: Override<String>,
    jump_route_override: Override<String>,
    effective_credential_id: Option<String>,
    effective_jump_route_id: Option<String>,
}

impl GroupSummary {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn parent_group_id(&self) -> Option<&str> {
        self.parent_group_id.as_deref()
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

    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        parent_group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> Result<Self, StorageError> {
        let effective_credential_id = credential_override.value().cloned();
        let effective_jump_route_id = jump_route_override.value().cloned();
        Self::from_stored(
            id,
            label,
            parent_group_id,
            credential_override,
            jump_route_override,
            effective_credential_id,
            effective_jump_route_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_stored(
        id: impl Into<String>,
        label: impl Into<String>,
        parent_group_id: Option<String>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
        effective_credential_id: Option<String>,
        effective_jump_route_id: Option<String>,
    ) -> Result<Self, StorageError> {
        let id = id.into();
        validate_group_id(&id)?;
        let label = normalize_label(label.into())?;
        if let Some(parent_group_id) = parent_group_id.as_deref() {
            validate_group_id(parent_group_id)?;
        }
        validate_overrides(&credential_override, &jump_route_override)?;
        if let Some(credential_id) = effective_credential_id.as_deref() {
            validate_credential_id(credential_id)?;
        }
        if let Some(jump_route_id) = effective_jump_route_id.as_deref() {
            validate_jump_route_id(jump_route_id)?;
        }

        Ok(Self {
            id,
            label,
            parent_group_id,
            credential_override,
            jump_route_override,
            effective_credential_id,
            effective_jump_route_id,
        })
    }
}

pub(crate) fn generate_group_id() -> Result<String, StorageError> {
    generate_opaque_id(GROUP_ID_PREFIX)
}

pub(crate) fn validate_group_id(id: &str) -> Result<(), StorageError> {
    if is_valid_opaque_id(id) {
        Ok(())
    } else {
        Err(StorageError::InvalidGroup)
    }
}

fn validate_overrides(
    credential_override: &Override<String>,
    jump_route_override: &Override<String>,
) -> Result<(), StorageError> {
    if let Some(credential_id) = credential_override.value() {
        validate_credential_id(credential_id)?;
    }
    if let Some(jump_route_id) = jump_route_override.value() {
        validate_jump_route_id(jump_route_id)?;
    }
    Ok(())
}

fn normalize_label(value: String) -> Result<String, StorageError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_GROUP_LABEL_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidGroup);
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_group_ids_are_opaque_and_valid() {
        let first = generate_group_id().expect("first ID");
        let second = generate_group_id().expect("second ID");

        assert_ne!(first, second);
        assert!(first.starts_with(GROUP_ID_PREFIX));
        validate_group_id(&first).expect("valid generated ID");
    }

    #[test]
    fn group_normalizes_metadata_and_keeps_only_reference_overrides() {
        let group = GroupSummary::new(
            "group-production",
            "  Production  ",
            Some("group-root".to_owned()),
            Override::Set("cred-production".to_owned()),
            Override::Clear,
        )
        .expect("valid Group");

        assert_eq!(group.label(), "Production");
        assert_eq!(group.parent_group_id(), Some("group-root"));
        assert_eq!(
            group.credential_override(),
            &Override::Set("cred-production".to_owned())
        );
        assert_eq!(group.jump_route_override(), &Override::Clear);
        assert_eq!(group.effective_credential_id(), Some("cred-production"));
        assert_eq!(group.effective_jump_route_id(), None);
    }
}
