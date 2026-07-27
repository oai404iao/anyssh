use crate::StorageError;

pub(crate) const INHERIT_STATE: i64 = 0;
pub(crate) const SET_STATE: i64 = 1;
pub(crate) const CLEAR_STATE: i64 = 2;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Override<T> {
    #[default]
    Inherit,
    Set(T),
    Clear,
}

impl<T> Override<T> {
    pub const fn is_inherit(&self) -> bool {
        matches!(self, Self::Inherit)
    }

    pub const fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Set(value) => Some(value),
            Self::Inherit | Self::Clear => None,
        }
    }

    pub(crate) const fn storage_state(&self) -> i64 {
        match self {
            Self::Inherit => INHERIT_STATE,
            Self::Set(_) => SET_STATE,
            Self::Clear => CLEAR_STATE,
        }
    }

    pub(crate) fn from_storage(state: i64, value: Option<T>) -> Result<Self, StorageError> {
        match (state, value) {
            (INHERIT_STATE, None) => Ok(Self::Inherit),
            (SET_STATE, Some(value)) => Ok(Self::Set(value)),
            (CLEAR_STATE, None) => Ok(Self::Clear),
            _ => Err(StorageError::RecordIntegrity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_state_round_trips_all_override_variants() {
        for value in [
            Override::Inherit,
            Override::Set("cred-test".to_owned()),
            Override::Clear,
        ] {
            let stored_value = value.value().cloned();
            let restored = Override::from_storage(value.storage_state(), stored_value)
                .expect("valid stored override");
            assert_eq!(restored, value);
        }
    }

    #[test]
    fn invalid_state_and_value_combinations_are_rejected() {
        for (state, value) in [
            (INHERIT_STATE, Some("unexpected".to_owned())),
            (SET_STATE, None),
            (CLEAR_STATE, Some("unexpected".to_owned())),
            (99, None),
        ] {
            assert!(matches!(
                Override::from_storage(state, value),
                Err(StorageError::RecordIntegrity)
            ));
        }
    }
}
