#![forbid(unsafe_code)]

use std::net::IpAddr;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshEndpointIdentity {
    host: String,
    port: u16,
}

impl SshEndpointIdentity {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, DomainError> {
        let host = normalize_endpoint_identity_host(&host.into())?;
        if port == 0 {
            return Err(DomainError::InvalidPort);
        }
        Ok(Self { host, port })
    }

    pub fn from_endpoint(endpoint: &SshEndpoint) -> Result<Self, DomainError> {
        Self::new(endpoint.host.clone(), endpoint.port)
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub const fn port(&self) -> u16 {
        self.port
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
    #[error("SSH host identity is invalid")]
    InvalidHostIdentity,
    #[error("terminal columns and rows must be greater than zero")]
    InvalidTerminalSize,
}

fn normalize_endpoint_identity_host(host: &str) -> Result<String, DomainError> {
    let host = host.trim();
    if host.is_empty() {
        return Err(DomainError::EmptyHost);
    }
    if host.chars().any(char::is_control) {
        return Err(DomainError::InvalidHostIdentity);
    }

    let unbracketed = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .filter(|host| host.parse::<std::net::Ipv6Addr>().is_ok())
        .unwrap_or(host);

    if let Ok(address) = unbracketed.parse::<IpAddr>() {
        return Ok(address.to_string());
    }

    let host = unbracketed.strip_suffix('.').unwrap_or(unbracketed);
    if host.is_empty() || host.chars().any(char::is_whitespace) {
        return Err(DomainError::InvalidHostIdentity);
    }
    Ok(host.to_ascii_lowercase())
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
    fn endpoint_identity_normalizes_dns_and_ports() {
        let identity =
            SshEndpointIdentity::new(" EXAMPLE.COM. ", 2222).expect("valid endpoint identity");
        assert_eq!(identity.host(), "example.com");
        assert_eq!(identity.port(), 2222);
    }

    #[test]
    fn endpoint_identity_normalizes_ipv6_literals() {
        let bracketed =
            SshEndpointIdentity::new("[2001:0db8::1]", 22).expect("bracketed IPv6 identity");
        let plain = SshEndpointIdentity::new("2001:db8:0:0::1", 22).expect("plain IPv6 identity");
        assert_eq!(bracketed, plain);
        assert_eq!(bracketed.host(), "2001:db8::1");
    }

    #[test]
    fn endpoint_identity_rejects_ambiguous_whitespace() {
        assert_eq!(
            SshEndpointIdentity::new("example .com", 22),
            Err(DomainError::InvalidHostIdentity)
        );
    }

    #[test]
    fn terminal_size_rejects_zero() {
        assert_eq!(
            TerminalSize::new(80, 0),
            Err(DomainError::InvalidTerminalSize)
        );
    }
}
