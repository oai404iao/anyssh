use std::fmt;

use crate::ResolvedCredential;

pub struct ResolvedHostConnection {
    host_id: String,
    host: String,
    port: u16,
    credential: ResolvedCredential,
}

impl ResolvedHostConnection {
    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn into_parts(self) -> (String, String, u16, ResolvedCredential) {
        (self.host_id, self.host, self.port, self.credential)
    }

    pub(crate) fn new(
        host_id: String,
        host: String,
        port: u16,
        credential: ResolvedCredential,
    ) -> Self {
        Self {
            host_id,
            host,
            port,
            credential,
        }
    }
}

impl fmt::Debug for ResolvedHostConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedHostConnection")
            .field("host_id", &self.host_id)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("credential", &"<redacted>")
            .finish()
    }
}

pub struct ResolvedHostConnectionPlan {
    target: ResolvedHostConnection,
    jump_hosts: Vec<ResolvedHostConnection>,
}

impl ResolvedHostConnectionPlan {
    pub fn target(&self) -> &ResolvedHostConnection {
        &self.target
    }

    pub fn jump_hosts(&self) -> &[ResolvedHostConnection] {
        &self.jump_hosts
    }

    pub fn into_parts(self) -> (ResolvedHostConnection, Vec<ResolvedHostConnection>) {
        (self.target, self.jump_hosts)
    }

    pub(crate) fn new(
        target: ResolvedHostConnection,
        jump_hosts: Vec<ResolvedHostConnection>,
    ) -> Self {
        Self { target, jump_hosts }
    }
}

impl fmt::Debug for ResolvedHostConnectionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedHostConnectionPlan")
            .field("target", &self.target)
            .field("jump_hosts", &self.jump_hosts)
            .finish()
    }
}
