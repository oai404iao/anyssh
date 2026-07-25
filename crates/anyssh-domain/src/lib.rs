#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshEndpoint {
    pub host: String,
    pub port: u16,
}

impl SshEndpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, DomainError> {
        let host = host.into();
        let host = host.trim();

        if host.is_empty() {
            return Err(DomainError::EmptyHost);
        }

        if port == 0 {
            return Err(DomainError::InvalidPort);
        }

        Ok(Self {
            host: host.to_owned(),
            port,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSize {
    pub columns: u32,
    pub rows: u32,
}

impl TerminalSize {
    pub const DEFAULT: Self = Self {
        columns: 120,
        rows: 32,
    };

    pub fn new(columns: u32, rows: u32) -> Result<Self, DomainError> {
        if columns == 0 || rows == 0 {
            return Err(DomainError::InvalidTerminalSize);
        }

        Ok(Self { columns, rows })
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("SSH host must not be empty")]
    EmptyHost,
    #[error("SSH port must be between 1 and 65535")]
    InvalidPort,
    #[error("terminal columns and rows must be greater than zero")]
    InvalidTerminalSize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_trims_host() {
        let endpoint = SshEndpoint::new("  example.com ", 22).expect("valid endpoint");
        assert_eq!(endpoint.host, "example.com");
    }

    #[test]
    fn endpoint_rejects_empty_host() {
        assert_eq!(SshEndpoint::new(" ", 22), Err(DomainError::EmptyHost));
    }

    #[test]
    fn terminal_size_rejects_zero() {
        assert_eq!(
            TerminalSize::new(80, 0),
            Err(DomainError::InvalidTerminalSize)
        );
    }
}
