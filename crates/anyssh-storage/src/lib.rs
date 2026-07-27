#![forbid(unsafe_code)]

mod actor;
mod connection_plan;
mod credential;
mod entity_id;
mod group;
mod host;
mod inheritance;
mod jump_route;

pub use actor::{
    DEFAULT_DATABASE_COMMAND_QUEUE_CAPACITY, DatabaseActorConfig, DatabaseActorError,
    DatabaseActorHandle, DatabaseActorStartError, VaultState, VaultStatus,
};
pub use connection_plan::{ResolvedHostConnection, ResolvedHostConnectionPlan};
pub use credential::{CredentialKind, CredentialSecret, CredentialSummary, ResolvedCredential};
pub use group::{GroupSummary, MAX_GROUP_DEPTH};
pub use host::HostSummary;
pub use inheritance::Override;
pub use jump_route::{JumpRouteSummary, MAX_JUMP_ROUTE_STEPS};

use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    fs::{self, File},
    path::Path,
    time::Duration,
};

use anyssh_vault::{
    BootstrapDocument, DerivedVaultKeys, PinKdfParameters, UnlockedVault, VaultError,
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    credential::{CredentialRecord, generate_credential_id, validate_credential_id},
    group::validate_group_id,
    host::validate_host_id,
    inheritance::SET_STATE,
    jump_route::validate_jump_route_id,
};

pub const BOOTSTRAP_FILE_NAME: &str = "vault.bootstrap.json";
pub const DATABASE_FILE_NAME: &str = "vault.db";

const SCHEMA_VERSION: i64 = 5;
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("vault is already initialized")]
    AlreadyInitialized,
    #[error("vault is not initialized")]
    NotInitialized,
    #[error("vault files are incomplete or damaged")]
    DamagedLayout,
    #[error("vault database could not be decrypted or is damaged")]
    InvalidDatabase,
    #[error("vault database schema version {0} is not supported")]
    UnsupportedSchema(i64),
    #[error("vault record integrity check failed")]
    RecordIntegrity,
    #[error("host record is invalid")]
    InvalidHost,
    #[error("host was not found")]
    HostNotFound,
    #[error("host `{0}` does not reference a Credential")]
    HostCredentialMissing(String),
    #[error("host is referenced by a Jump Route")]
    HostInUse,
    #[error("Group record is invalid")]
    InvalidGroup,
    #[error("Group was not found")]
    GroupNotFound,
    #[error("Group is referenced by a child Group or Host")]
    GroupInUse,
    #[error("Group parent references form a cycle")]
    GroupCycle,
    #[error("Group hierarchy exceeds the maximum depth of {0}")]
    GroupTooDeep(usize),
    #[error("Jump Route record is invalid")]
    InvalidJumpRoute,
    #[error("Jump Route was not found")]
    JumpRouteNotFound,
    #[error("Jump Route is referenced by a Host")]
    JumpRouteInUse,
    #[error("Jump Route references form a cycle")]
    JumpRouteCycle,
    #[error("Jump Route expands the same Host more than once")]
    JumpRouteDuplicateHost,
    #[error("Jump Route exceeds the maximum of {0} hosts")]
    JumpRouteTooLong(usize),
    #[error("credential record is invalid")]
    InvalidCredential,
    #[error("credential was not found")]
    CredentialNotFound,
    #[error("credential is referenced by a Host")]
    CredentialInUse,
    #[error("vault schema migration was interrupted")]
    MigrationInterrupted,
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("vault storage I/O failed")]
    Io(#[source] std::io::Error),
    #[error("vault storage operation failed")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultPresence {
    Uninitialized,
    Locked,
    Damaged,
}

/// Low-level encrypted storage primitive.
///
/// Application code should use [`DatabaseActorHandle`] so the SQLCipher
/// connection and unlocked Vault state stay confined to the database thread.
pub struct LocalVault {
    unlocked: UnlockedVault,
    database: VaultDatabase,
}

impl LocalVault {
    pub fn presence(root: &Path) -> VaultPresence {
        let root_metadata = match fs::symlink_metadata(root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return VaultPresence::Uninitialized;
            }
            Err(_) => return VaultPresence::Damaged,
        };
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return VaultPresence::Damaged;
        }

        let bootstrap = root.join(BOOTSTRAP_FILE_NAME);
        let database = root.join(DATABASE_FILE_NAME);
        if is_regular_file_without_symlink(&bootstrap) && is_regular_file_without_symlink(&database)
        {
            VaultPresence::Locked
        } else {
            VaultPresence::Damaged
        }
    }

    pub fn create(
        root: &Path,
        pin: &str,
        parameters: PinKdfParameters,
    ) -> Result<Self, StorageError> {
        if root.exists() {
            return Err(StorageError::AlreadyInitialized);
        }

        let parent = root
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        create_private_directory(parent)?;
        let (bootstrap, unlocked) = BootstrapDocument::create_pin(pin, parameters)?;
        let temporary_root = parent.join(format!(
            ".{}.creating-{}",
            root.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("vault"),
            bootstrap.vault_id()
        ));
        if temporary_root.exists() {
            return Err(StorageError::AlreadyInitialized);
        }
        create_private_directory(&temporary_root)?;

        let initialization = (|| {
            bootstrap.write_new(&temporary_root.join(BOOTSTRAP_FILE_NAME))?;
            let keys = unlocked.derive_keys()?;
            let database = VaultDatabase::create(&temporary_root.join(DATABASE_FILE_NAME), &keys)?;
            drop(database);
            set_private_file_permissions(&temporary_root.join(DATABASE_FILE_NAME))?;
            sync_directory(&temporary_root);
            fs::rename(&temporary_root, root).map_err(StorageError::Io)?;
            sync_directory(parent);
            Ok(())
        })();

        if let Err(error) = initialization {
            let _ = fs::remove_dir_all(&temporary_root);
            return Err(error);
        }

        let keys = unlocked.derive_keys()?;
        let database = VaultDatabase::open(&root.join(DATABASE_FILE_NAME), &keys)?;
        Ok(Self { unlocked, database })
    }

    pub fn unlock(root: &Path, pin: &str) -> Result<Self, StorageError> {
        match Self::presence(root) {
            VaultPresence::Uninitialized => return Err(StorageError::NotInitialized),
            VaultPresence::Damaged => return Err(StorageError::DamagedLayout),
            VaultPresence::Locked => {}
        }

        let bootstrap = BootstrapDocument::load(&root.join(BOOTSTRAP_FILE_NAME))?;
        let unlocked = bootstrap.unlock_pin(pin)?;
        let keys = unlocked.derive_keys()?;
        let database = VaultDatabase::open(&root.join(DATABASE_FILE_NAME), &keys)?;
        Ok(Self { unlocked, database })
    }

    pub fn vault_id(&self) -> &str {
        self.unlocked.vault_id()
    }

    pub fn cipher_version(&self) -> &str {
        &self.database.cipher_version
    }

    fn create_group(&mut self, record: &GroupSummary) -> Result<GroupSummary, StorageError> {
        self.database.create_group(record)
    }

    fn update_group(&mut self, record: &GroupSummary) -> Result<GroupSummary, StorageError> {
        self.database.update_group(record)
    }

    fn list_groups(&self) -> Result<Vec<GroupSummary>, StorageError> {
        self.database.list_groups()
    }

    fn delete_group(&mut self, id: &str) -> Result<bool, StorageError> {
        self.database.delete_group(id)
    }

    fn create_host(&mut self, record: &HostSummary) -> Result<HostSummary, StorageError> {
        self.database.create_host(record)
    }

    fn update_host(&mut self, record: &HostSummary) -> Result<HostSummary, StorageError> {
        self.database.update_host(record)
    }

    fn list_hosts(&self) -> Result<Vec<HostSummary>, StorageError> {
        self.database.list_hosts()
    }

    fn delete_host(&mut self, id: &str) -> Result<bool, StorageError> {
        self.database.delete_host(id)
    }

    fn create_jump_route(
        &mut self,
        route: &JumpRouteSummary,
    ) -> Result<JumpRouteSummary, StorageError> {
        self.database.create_jump_route(route)
    }

    fn update_jump_route(
        &mut self,
        route: &JumpRouteSummary,
    ) -> Result<JumpRouteSummary, StorageError> {
        self.database.update_jump_route(route)
    }

    fn list_jump_routes(&self) -> Result<Vec<JumpRouteSummary>, StorageError> {
        self.database.list_jump_routes()
    }

    fn delete_jump_route(&mut self, id: &str) -> Result<bool, StorageError> {
        self.database.delete_jump_route(id)
    }

    fn resolve_host_connection_plan(
        &self,
        id: &str,
    ) -> Result<ResolvedHostConnectionPlan, StorageError> {
        self.database.resolve_host_connection_plan(id)
    }

    fn create_credential(
        &mut self,
        record: &CredentialRecord,
    ) -> Result<CredentialSummary, StorageError> {
        self.database.create_credential(record)
    }

    fn update_credential(
        &mut self,
        record: &CredentialRecord,
    ) -> Result<CredentialSummary, StorageError> {
        self.database.update_credential(record)
    }

    fn list_credentials(&self) -> Result<Vec<CredentialSummary>, StorageError> {
        self.database.list_credentials()
    }

    fn delete_credential(&mut self, id: &str) -> Result<bool, StorageError> {
        self.database.delete_credential(id)
    }

    fn resolve_credential(&self, id: &str) -> Result<ResolvedCredential, StorageError> {
        self.database.resolve_credential(id)
    }
}

struct VaultDatabase {
    connection: Connection,
    vault_id: String,
    record_key: Zeroizing<[u8; KEY_BYTES]>,
    cipher_version: String,
}

impl VaultDatabase {
    fn create(path: &Path, keys: &DerivedVaultKeys) -> Result<Self, StorageError> {
        if path.exists() {
            return Err(StorageError::AlreadyInitialized);
        }

        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection = Connection::open_with_flags(path, flags).map_err(StorageError::Sql)?;
        let mut database = Self::configure(connection, keys)?;
        initialize_schema(&mut database.connection)?;
        database.migrate_to_v3(false)?;
        database.migrate_to_v4(false)?;
        database.migrate_to_v5(false)?;
        database.connection.execute(
            "INSERT INTO vault_meta(key, value) VALUES('vault_id', ?1)",
            [keys.vault_id()],
        )?;
        Ok(database)
    }

    fn open(path: &Path, keys: &DerivedVaultKeys) -> Result<Self, StorageError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection =
            Connection::open_with_flags(path, flags).map_err(|_| StorageError::InvalidDatabase)?;
        let mut database = Self::configure(connection, keys)?;

        let stored_vault_id: String = database
            .connection
            .query_row(
                "SELECT value FROM vault_meta WHERE key = 'vault_id'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::InvalidDatabase)?;
        if stored_vault_id != keys.vault_id() {
            return Err(StorageError::InvalidDatabase);
        }
        database.migrate_existing_schema()?;
        Ok(database)
    }

    fn configure(connection: Connection, keys: &DerivedVaultKeys) -> Result<Self, StorageError> {
        apply_database_key(&connection, keys.database_key())?;
        connection
            .pragma_update(None, "cipher_memory_security", "ON")
            .map_err(|_| StorageError::InvalidDatabase)?;
        connection
            .query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))
            .map_err(|_| StorageError::InvalidDatabase)?;

        let cipher_version: String = connection
            .pragma_query_value(None, "cipher_version", |row| row.get(0))
            .map_err(|_| StorageError::InvalidDatabase)?;
        if cipher_version.trim().is_empty() {
            return Err(StorageError::InvalidDatabase);
        }

        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(StorageError::Sql)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(StorageError::Sql)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(StorageError::Sql)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(StorageError::Sql)?;
        connection
            .pragma_update(None, "temp_store", "MEMORY")
            .map_err(StorageError::Sql)?;
        connection
            .pragma_update(None, "secure_delete", "ON")
            .map_err(StorageError::Sql)?;
        connection
            .pragma_update(None, "wal_autocheckpoint", 1000_i64)
            .map_err(StorageError::Sql)?;

        Ok(Self {
            connection,
            vault_id: keys.vault_id().to_owned(),
            record_key: Zeroizing::new(*keys.record_key()),
            cipher_version,
        })
    }

    fn create_group(&mut self, record: &GroupSummary) -> Result<GroupSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_group_references(&transaction, record)?;
        transaction.execute(
            "
            INSERT INTO host_groups(id, label, parent_group_id)
            VALUES(?1, ?2, ?3)
            ",
            params![record.id(), record.label(), record.parent_group_id()],
        )?;
        transaction.execute(
            "
            INSERT INTO group_overrides(
                group_id, credential_state, credential_id,
                jump_route_state, jump_route_id
            )
            VALUES(?1, ?2, ?3, ?4, ?5)
            ",
            params![
                record.id(),
                record.credential_override().storage_state(),
                record.credential_override().value().map(String::as_str),
                record.jump_route_override().storage_state(),
                record.jump_route_override().value().map(String::as_str),
            ],
        )?;
        validate_group_graph(&transaction)?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        self.load_group(record.id())
    }

    fn update_group(&mut self, record: &GroupSummary) -> Result<GroupSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_group_references(&transaction, record)?;
        let changed = transaction.execute(
            "
            UPDATE host_groups
            SET label = ?2,
                parent_group_id = ?3
            WHERE id = ?1
            ",
            params![record.id(), record.label(), record.parent_group_id()],
        )?;
        if changed != 1 {
            return Err(StorageError::GroupNotFound);
        }
        let changed = transaction.execute(
            "
            UPDATE group_overrides
            SET credential_state = ?2,
                credential_id = ?3,
                jump_route_state = ?4,
                jump_route_id = ?5
            WHERE group_id = ?1
            ",
            params![
                record.id(),
                record.credential_override().storage_state(),
                record.credential_override().value().map(String::as_str),
                record.jump_route_override().storage_state(),
                record.jump_route_override().value().map(String::as_str),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::RecordIntegrity);
        }
        validate_group_graph(&transaction)?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        self.load_group(record.id())
    }

    fn list_groups(&self) -> Result<Vec<GroupSummary>, StorageError> {
        let stored_groups = {
            let mut statement = self.connection.prepare(
                "
                SELECT g.id, g.label, g.parent_group_id,
                       o.credential_state, o.credential_id,
                       o.jump_route_state, o.jump_route_id
                FROM host_groups AS g
                JOIN group_overrides AS o ON o.group_id = g.id
                ORDER BY g.label COLLATE NOCASE, g.id
                ",
            )?;
            let rows = statement.query_map([], stored_group_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        stored_groups
            .into_iter()
            .map(|stored| group_summary_from_stored(&self.connection, stored))
            .collect()
    }

    fn delete_group(&mut self, id: &str) -> Result<bool, StorageError> {
        validate_group_id(id)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.query_row(
            "
            SELECT EXISTS(
                SELECT 1 FROM host_groups WHERE parent_group_id = ?1
                UNION ALL
                SELECT 1 FROM hosts WHERE group_id = ?1
            )
            ",
            [id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(StorageError::GroupInUse);
        }
        let changed = transaction.execute("DELETE FROM host_groups WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(changed == 1)
    }

    fn create_host(&mut self, record: &HostSummary) -> Result<HostSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_host_references(&transaction, record)?;
        transaction.execute(
            "
            INSERT INTO hosts(
                id, display_name, host, port, group_id,
                credential_state, credential_id,
                jump_route_state, jump_route_id
            )
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                record.id(),
                record.display_name(),
                record.host(),
                i64::from(record.port()),
                record.group_id(),
                record.credential_override().storage_state(),
                record.credential_override().value().map(String::as_str),
                record.jump_route_override().storage_state(),
                record.jump_route_override().value().map(String::as_str),
            ],
        )?;
        validate_group_graph(&transaction)?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        self.load_host(record.id())
    }

    fn update_host(&mut self, record: &HostSummary) -> Result<HostSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_host_references(&transaction, record)?;
        let changed = transaction.execute(
            "
            UPDATE hosts
            SET display_name = ?2,
                host = ?3,
                port = ?4,
                group_id = ?5,
                credential_state = ?6,
                credential_id = ?7,
                jump_route_state = ?8,
                jump_route_id = ?9
            WHERE id = ?1
            ",
            params![
                record.id(),
                record.display_name(),
                record.host(),
                i64::from(record.port()),
                record.group_id(),
                record.credential_override().storage_state(),
                record.credential_override().value().map(String::as_str),
                record.jump_route_override().storage_state(),
                record.jump_route_override().value().map(String::as_str),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::HostNotFound);
        }
        validate_group_graph(&transaction)?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        self.load_host(record.id())
    }

    fn list_hosts(&self) -> Result<Vec<HostSummary>, StorageError> {
        let stored_hosts = {
            let mut statement = self.connection.prepare(
                "
                SELECT id, display_name, host, port, group_id,
                       credential_state, credential_id,
                       jump_route_state, jump_route_id
                FROM hosts
                ORDER BY display_name COLLATE NOCASE, id
                ",
            )?;
            let rows = statement.query_map([], stored_host_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        stored_hosts
            .into_iter()
            .map(|stored| host_summary_from_stored(&self.connection, stored))
            .collect()
    }

    fn delete_host(&mut self, id: &str) -> Result<bool, StorageError> {
        validate_host_id(id)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM jump_route_steps WHERE host_id = ?1)",
            [id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(StorageError::HostInUse);
        }
        let changed = transaction.execute("DELETE FROM hosts WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(changed == 1)
    }

    fn create_jump_route(
        &mut self,
        route: &JumpRouteSummary,
    ) -> Result<JumpRouteSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_route_host_references(&transaction, route)?;
        transaction.execute(
            "INSERT INTO jump_routes(id, label) VALUES(?1, ?2)",
            params![route.id(), route.label()],
        )?;
        insert_jump_route_steps(&transaction, route)?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        Ok(route.clone())
    }

    fn update_jump_route(
        &mut self,
        route: &JumpRouteSummary,
    ) -> Result<JumpRouteSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_route_host_references(&transaction, route)?;
        let changed = transaction.execute(
            "UPDATE jump_routes SET label = ?2 WHERE id = ?1",
            params![route.id(), route.label()],
        )?;
        if changed != 1 {
            return Err(StorageError::JumpRouteNotFound);
        }
        transaction.execute(
            "DELETE FROM jump_route_steps WHERE route_id = ?1",
            [route.id()],
        )?;
        insert_jump_route_steps(&transaction, route)?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        Ok(route.clone())
    }

    fn list_jump_routes(&self) -> Result<Vec<JumpRouteSummary>, StorageError> {
        let route_rows = {
            let mut statement = self.connection.prepare(
                "
                SELECT id, label
                FROM jump_routes
                ORDER BY label COLLATE NOCASE, id
                ",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        route_rows
            .into_iter()
            .map(|(id, label)| {
                let mut statement = self.connection.prepare(
                    "
                    SELECT host_id
                    FROM jump_route_steps
                    WHERE route_id = ?1
                    ORDER BY position
                    ",
                )?;
                let rows = statement.query_map([&id], |row| row.get::<_, String>(0))?;
                let host_ids = rows.collect::<Result<Vec<_>, _>>()?;
                JumpRouteSummary::new(id, label, host_ids)
            })
            .collect()
    }

    fn delete_jump_route(&mut self, id: &str) -> Result<bool, StorageError> {
        validate_jump_route_id(id)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM hosts
                WHERE jump_route_state = ?2 AND jump_route_id = ?1
                UNION ALL
                SELECT 1
                FROM group_overrides
                WHERE jump_route_state = ?2 AND jump_route_id = ?1
            )
            ",
            params![id, SET_STATE],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(StorageError::JumpRouteInUse);
        }
        let changed = transaction.execute("DELETE FROM jump_routes WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(changed == 1)
    }

    fn resolve_host_connection_plan(
        &self,
        id: &str,
    ) -> Result<ResolvedHostConnectionPlan, StorageError> {
        let target = self.load_host(id)?;
        let mut active_hosts = HashSet::new();
        let mut emitted_hosts = HashSet::new();
        let mut jump_hosts = Vec::new();
        self.expand_jump_hosts(
            &target,
            &mut active_hosts,
            &mut emitted_hosts,
            &mut jump_hosts,
        )?;

        let jump_hosts = jump_hosts
            .into_iter()
            .map(|host| self.resolve_host_connection(host))
            .collect::<Result<Vec<_>, _>>()?;
        let target = self.resolve_host_connection(target)?;
        Ok(ResolvedHostConnectionPlan::new(target, jump_hosts))
    }

    fn expand_jump_hosts(
        &self,
        host: &HostSummary,
        active_hosts: &mut HashSet<String>,
        emitted_hosts: &mut HashSet<String>,
        output: &mut Vec<HostSummary>,
    ) -> Result<(), StorageError> {
        if !active_hosts.insert(host.id().to_owned()) {
            return Err(StorageError::JumpRouteCycle);
        }

        if let Some(route_id) = host.jump_route_id() {
            let route = self.load_jump_route(route_id)?;
            for step_id in route.host_ids() {
                let step = self.load_host(step_id)?;
                self.expand_jump_hosts(&step, active_hosts, emitted_hosts, output)?;
                if !emitted_hosts.insert(step.id().to_owned()) {
                    return Err(StorageError::JumpRouteDuplicateHost);
                }
                if output.len() >= MAX_JUMP_ROUTE_STEPS {
                    return Err(StorageError::JumpRouteTooLong(MAX_JUMP_ROUTE_STEPS));
                }
                output.push(step);
            }
        }

        active_hosts.remove(host.id());
        Ok(())
    }

    fn resolve_host_connection(
        &self,
        host: HostSummary,
    ) -> Result<ResolvedHostConnection, StorageError> {
        let credential_id = host
            .credential_id()
            .ok_or_else(|| StorageError::HostCredentialMissing(host.id().to_owned()))?;
        let credential = self.resolve_credential(credential_id)?;
        Ok(ResolvedHostConnection::new(
            host.id().to_owned(),
            host.host().to_owned(),
            host.port(),
            credential,
        ))
    }

    fn load_host(&self, id: &str) -> Result<HostSummary, StorageError> {
        validate_host_id(id)?;
        let stored = self
            .connection
            .query_row(
                "
                SELECT id, display_name, host, port, group_id,
                       credential_state, credential_id,
                       jump_route_state, jump_route_id
                FROM hosts
                WHERE id = ?1
                ",
                [id],
                stored_host_from_row,
            )
            .optional()?
            .ok_or(StorageError::HostNotFound)?;
        host_summary_from_stored(&self.connection, stored)
    }

    fn load_group(&self, id: &str) -> Result<GroupSummary, StorageError> {
        group_summary_from_stored(&self.connection, load_stored_group(&self.connection, id)?)
    }

    fn load_jump_route(&self, id: &str) -> Result<JumpRouteSummary, StorageError> {
        validate_jump_route_id(id)?;
        let label = self
            .connection
            .query_row("SELECT label FROM jump_routes WHERE id = ?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?
            .ok_or(StorageError::JumpRouteNotFound)?;
        let mut statement = self.connection.prepare(
            "
            SELECT host_id
            FROM jump_route_steps
            WHERE route_id = ?1
            ORDER BY position
            ",
        )?;
        let rows = statement.query_map([id], |row| row.get::<_, String>(0))?;
        JumpRouteSummary::new(id, label, rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn create_credential(
        &mut self,
        record: &CredentialRecord,
    ) -> Result<CredentialSummary, StorageError> {
        let encrypted = self.encrypt_credential(record)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "
            INSERT INTO credentials(
                id, label, username, kind, secret_nonce, secret_ciphertext,
                passphrase_nonce, passphrase_ciphertext
            )
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                &record.id,
                &record.label,
                &record.username,
                record.secret.kind().storage_value(),
                encrypted.secret.nonce.as_slice(),
                encrypted.secret.ciphertext,
                encrypted
                    .passphrase
                    .as_ref()
                    .map(|field| field.nonce.as_slice()),
                encrypted
                    .passphrase
                    .as_ref()
                    .map(|field| field.ciphertext.as_slice()),
            ],
        )?;
        transaction.commit()?;
        Ok(record.summary())
    }

    fn update_credential(
        &mut self,
        record: &CredentialRecord,
    ) -> Result<CredentialSummary, StorageError> {
        let encrypted = self.encrypt_credential(record)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "
            UPDATE credentials
            SET label = ?2,
                username = ?3,
                kind = ?4,
                secret_nonce = ?5,
                secret_ciphertext = ?6,
                passphrase_nonce = ?7,
                passphrase_ciphertext = ?8
            WHERE id = ?1
            ",
            params![
                &record.id,
                &record.label,
                &record.username,
                record.secret.kind().storage_value(),
                encrypted.secret.nonce.as_slice(),
                encrypted.secret.ciphertext,
                encrypted
                    .passphrase
                    .as_ref()
                    .map(|field| field.nonce.as_slice()),
                encrypted
                    .passphrase
                    .as_ref()
                    .map(|field| field.ciphertext.as_slice()),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::CredentialNotFound);
        }
        transaction.commit()?;
        Ok(record.summary())
    }

    fn list_credentials(&self) -> Result<Vec<CredentialSummary>, StorageError> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, label, username, kind
            FROM credentials
            ORDER BY label COLLATE NOCASE, id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredCredentialSummary {
                id: row.get(0)?,
                label: row.get(1)?,
                username: row.get(2)?,
                kind: row.get(3)?,
            })
        })?;

        rows.map(|row| {
            let row = row?;
            CredentialSummary::new(
                row.id,
                row.label,
                row.username,
                CredentialKind::from_storage_value(&row.kind)?,
            )
        })
        .collect()
    }

    fn delete_credential(&mut self, id: &str) -> Result<bool, StorageError> {
        validate_credential_id(id)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM hosts
                WHERE credential_state = ?2 AND credential_id = ?1
                UNION ALL
                SELECT 1
                FROM group_overrides
                WHERE credential_state = ?2 AND credential_id = ?1
            )
            ",
            params![id, SET_STATE],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(StorageError::CredentialInUse);
        }
        let changed = transaction.execute("DELETE FROM credentials WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(changed == 1)
    }

    fn resolve_credential(&self, id: &str) -> Result<ResolvedCredential, StorageError> {
        validate_credential_id(id)?;
        let stored = self
            .connection
            .query_row(
                "
                SELECT label, username, kind, secret_nonce, secret_ciphertext,
                       passphrase_nonce, passphrase_ciphertext
                FROM credentials
                WHERE id = ?1
                ",
                [id],
                |row| {
                    Ok(StoredCredential {
                        label: row.get(0)?,
                        username: row.get(1)?,
                        kind: row.get(2)?,
                        secret_nonce: row.get(3)?,
                        secret_ciphertext: row.get(4)?,
                        passphrase_nonce: row.get(5)?,
                        passphrase_ciphertext: row.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or(StorageError::CredentialNotFound)?;

        let kind = CredentialKind::from_storage_value(&stored.kind)?;
        let secret = match kind {
            CredentialKind::Password => {
                if stored.passphrase_nonce.is_some() || stored.passphrase_ciphertext.is_some() {
                    return Err(StorageError::RecordIntegrity);
                }
                CredentialSecret::Password {
                    password: self.decrypt_credential_field(
                        id,
                        kind,
                        "secret",
                        stored.secret_nonce,
                        stored.secret_ciphertext,
                    )?,
                }
            }
            CredentialKind::PrivateKey => {
                let passphrase = match (stored.passphrase_nonce, stored.passphrase_ciphertext) {
                    (None, None) => None,
                    (Some(nonce), Some(ciphertext)) => Some(self.decrypt_credential_field(
                        id,
                        kind,
                        "passphrase",
                        nonce,
                        ciphertext,
                    )?),
                    _ => return Err(StorageError::RecordIntegrity),
                };
                CredentialSecret::PrivateKey {
                    private_key: self.decrypt_credential_field(
                        id,
                        kind,
                        "secret",
                        stored.secret_nonce,
                        stored.secret_ciphertext,
                    )?,
                    passphrase,
                }
            }
            CredentialKind::SystemAgent => {
                if stored.passphrase_nonce.is_some() || stored.passphrase_ciphertext.is_some() {
                    return Err(StorageError::RecordIntegrity);
                }
                CredentialSecret::SystemAgent {
                    identity_fingerprint_sha256: self.decrypt_credential_field(
                        id,
                        kind,
                        "secret",
                        stored.secret_nonce,
                        stored.secret_ciphertext,
                    )?,
                }
            }
        };

        CredentialRecord::new(id, stored.label, stored.username, secret)
            .map(|record| record.into_resolved())
    }

    fn encrypt_credential(
        &self,
        record: &CredentialRecord,
    ) -> Result<EncryptedCredential, StorageError> {
        let kind = record.secret.kind();
        match &record.secret {
            CredentialSecret::Password { password } => Ok(EncryptedCredential {
                secret: self.encrypt_credential_field(
                    &record.id,
                    kind,
                    "secret",
                    password.as_bytes(),
                )?,
                passphrase: None,
            }),
            CredentialSecret::PrivateKey {
                private_key,
                passphrase,
            } => Ok(EncryptedCredential {
                secret: self.encrypt_credential_field(
                    &record.id,
                    kind,
                    "secret",
                    private_key.as_bytes(),
                )?,
                passphrase: passphrase
                    .as_ref()
                    .map(|value| {
                        self.encrypt_credential_field(
                            &record.id,
                            kind,
                            "passphrase",
                            value.as_bytes(),
                        )
                    })
                    .transpose()?,
            }),
            CredentialSecret::SystemAgent {
                identity_fingerprint_sha256,
            } => Ok(EncryptedCredential {
                secret: self.encrypt_credential_field(
                    &record.id,
                    kind,
                    "secret",
                    identity_fingerprint_sha256.as_bytes(),
                )?,
                passphrase: None,
            }),
        }
    }

    fn encrypt_credential_field(
        &self,
        credential_id: &str,
        kind: CredentialKind,
        field: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedField, StorageError> {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| StorageError::RecordIntegrity)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.record_key[..])
            .map_err(|_| StorageError::RecordIntegrity)?;
        let aad = credential_record_aad(&self.vault_id, credential_id, kind, field);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StorageError::RecordIntegrity)?;
        Ok(EncryptedField { nonce, ciphertext })
    }

    fn decrypt_credential_field(
        &self,
        credential_id: &str,
        kind: CredentialKind,
        field: &str,
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
    ) -> Result<Zeroizing<String>, StorageError> {
        let nonce: [u8; NONCE_BYTES] = nonce
            .try_into()
            .map_err(|_| StorageError::RecordIntegrity)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.record_key[..])
            .map_err(|_| StorageError::RecordIntegrity)?;
        let aad = credential_record_aad(&self.vault_id, credential_id, kind, field);
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: aad.as_bytes(),
                    },
                )
                .map_err(|_| StorageError::RecordIntegrity)?,
        );
        let value = std::str::from_utf8(&plaintext).map_err(|_| StorageError::RecordIntegrity)?;
        Ok(Zeroizing::new(value.to_owned()))
    }

    fn migrate_existing_schema(&mut self) -> Result<(), StorageError> {
        let current_version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(StorageError::Sql)?;
        match current_version {
            SCHEMA_VERSION => Ok(()),
            1 => {
                migrate_to_v2(&mut self.connection, false)?;
                self.migrate_to_v3(false)?;
                self.migrate_to_v4(false)?;
                self.migrate_to_v5(false)
            }
            2 => {
                self.migrate_to_v3(false)?;
                self.migrate_to_v4(false)?;
                self.migrate_to_v5(false)
            }
            3 => {
                self.migrate_to_v4(false)?;
                self.migrate_to_v5(false)
            }
            4 => self.migrate_to_v5(false),
            version => Err(StorageError::UnsupportedSchema(version)),
        }
    }

    fn migrate_to_v3(&mut self, simulate_interruption: bool) -> Result<(), StorageError> {
        let migrated_hosts = self.prepare_legacy_host_migration()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "
            ALTER TABLE hosts RENAME TO legacy_hosts_v2;

            CREATE TABLE jump_routes(
                id TEXT PRIMARY KEY NOT NULL,
                label TEXT NOT NULL
            ) WITHOUT ROWID;

            CREATE TABLE hosts(
                id TEXT PRIMARY KEY NOT NULL,
                display_name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                credential_id TEXT,
                jump_route_id TEXT,
                FOREIGN KEY(credential_id)
                    REFERENCES credentials(id) ON DELETE RESTRICT,
                FOREIGN KEY(jump_route_id)
                    REFERENCES jump_routes(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE TABLE jump_route_steps(
                route_id TEXT NOT NULL,
                position INTEGER NOT NULL CHECK(position >= 0),
                host_id TEXT NOT NULL,
                PRIMARY KEY(route_id, position),
                UNIQUE(route_id, host_id),
                FOREIGN KEY(route_id)
                    REFERENCES jump_routes(id) ON DELETE CASCADE,
                FOREIGN KEY(host_id)
                    REFERENCES hosts(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE INDEX hosts_credential_id_idx ON hosts(credential_id);
            CREATE INDEX hosts_jump_route_id_idx ON hosts(jump_route_id);
            CREATE INDEX jump_route_steps_host_id_idx ON jump_route_steps(host_id);
            ",
        )?;

        for migrated in migrated_hosts {
            transaction.execute(
                "
                INSERT INTO credentials(
                    id, label, username, kind, secret_nonce, secret_ciphertext,
                    passphrase_nonce, passphrase_ciphertext
                )
                VALUES(?1, ?2, ?3, 'password', ?4, ?5, NULL, NULL)
                ",
                params![
                    &migrated.credential_id,
                    &migrated.credential_label,
                    &migrated.credential_username,
                    migrated.encrypted.secret.nonce.as_slice(),
                    migrated.encrypted.secret.ciphertext,
                ],
            )?;
            transaction.execute(
                "
                INSERT INTO hosts(
                    id, display_name, host, port, credential_id, jump_route_id
                )
                VALUES(?1, ?2, ?3, ?4, ?5, NULL)
                ",
                params![
                    migrated.host.id(),
                    migrated.host.display_name(),
                    migrated.host.host(),
                    i64::from(migrated.host.port()),
                    &migrated.credential_id,
                ],
            )?;
        }

        if simulate_interruption {
            return Err(StorageError::MigrationInterrupted);
        }

        transaction.execute_batch("DROP TABLE legacy_hosts_v2;")?;
        transaction.pragma_update(None, "user_version", 3_i64)?;
        transaction.commit()?;
        Ok(())
    }

    fn migrate_to_v4(&mut self, simulate_interruption: bool) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "
            DROP INDEX hosts_credential_id_idx;
            DROP INDEX hosts_jump_route_id_idx;
            DROP INDEX jump_route_steps_host_id_idx;

            ALTER TABLE jump_route_steps RENAME TO legacy_jump_route_steps_v3;
            ALTER TABLE hosts RENAME TO legacy_hosts_v3;

            CREATE TABLE host_groups(
                id TEXT PRIMARY KEY NOT NULL,
                label TEXT NOT NULL,
                parent_group_id TEXT,
                FOREIGN KEY(parent_group_id)
                    REFERENCES host_groups(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE TABLE group_overrides(
                group_id TEXT PRIMARY KEY NOT NULL,
                credential_state INTEGER NOT NULL CHECK(credential_state IN (0, 1, 2)),
                credential_id TEXT,
                jump_route_state INTEGER NOT NULL CHECK(jump_route_state IN (0, 1, 2)),
                jump_route_id TEXT,
                CHECK(
                    (credential_state = 0 AND credential_id IS NULL)
                    OR (credential_state = 1 AND credential_id IS NOT NULL)
                    OR (credential_state = 2 AND credential_id IS NULL)
                ),
                CHECK(
                    (jump_route_state = 0 AND jump_route_id IS NULL)
                    OR (jump_route_state = 1 AND jump_route_id IS NOT NULL)
                    OR (jump_route_state = 2 AND jump_route_id IS NULL)
                ),
                FOREIGN KEY(group_id)
                    REFERENCES host_groups(id) ON DELETE CASCADE,
                FOREIGN KEY(credential_id)
                    REFERENCES credentials(id) ON DELETE RESTRICT,
                FOREIGN KEY(jump_route_id)
                    REFERENCES jump_routes(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE TABLE hosts(
                id TEXT PRIMARY KEY NOT NULL,
                display_name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                group_id TEXT,
                credential_state INTEGER NOT NULL CHECK(credential_state IN (0, 1, 2)),
                credential_id TEXT,
                jump_route_state INTEGER NOT NULL CHECK(jump_route_state IN (0, 1, 2)),
                jump_route_id TEXT,
                CHECK(
                    (credential_state = 0 AND credential_id IS NULL)
                    OR (credential_state = 1 AND credential_id IS NOT NULL)
                    OR (credential_state = 2 AND credential_id IS NULL)
                ),
                CHECK(
                    (jump_route_state = 0 AND jump_route_id IS NULL)
                    OR (jump_route_state = 1 AND jump_route_id IS NOT NULL)
                    OR (jump_route_state = 2 AND jump_route_id IS NULL)
                ),
                FOREIGN KEY(group_id)
                    REFERENCES host_groups(id) ON DELETE RESTRICT,
                FOREIGN KEY(credential_id)
                    REFERENCES credentials(id) ON DELETE RESTRICT,
                FOREIGN KEY(jump_route_id)
                    REFERENCES jump_routes(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE TABLE jump_route_steps(
                route_id TEXT NOT NULL,
                position INTEGER NOT NULL CHECK(position >= 0),
                host_id TEXT NOT NULL,
                PRIMARY KEY(route_id, position),
                UNIQUE(route_id, host_id),
                FOREIGN KEY(route_id)
                    REFERENCES jump_routes(id) ON DELETE CASCADE,
                FOREIGN KEY(host_id)
                    REFERENCES hosts(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            INSERT INTO hosts(
                id, display_name, host, port, group_id,
                credential_state, credential_id,
                jump_route_state, jump_route_id
            )
            SELECT id, display_name, host, port, NULL,
                   CASE WHEN credential_id IS NULL THEN 0 ELSE 1 END,
                   credential_id,
                   CASE WHEN jump_route_id IS NULL THEN 0 ELSE 1 END,
                   jump_route_id
            FROM legacy_hosts_v3;

            INSERT INTO jump_route_steps(route_id, position, host_id)
            SELECT route_id, position, host_id
            FROM legacy_jump_route_steps_v3;

            CREATE INDEX host_groups_parent_group_id_idx
                ON host_groups(parent_group_id);
            CREATE INDEX group_overrides_credential_id_idx
                ON group_overrides(credential_id);
            CREATE INDEX group_overrides_jump_route_id_idx
                ON group_overrides(jump_route_id);
            CREATE INDEX hosts_group_id_idx ON hosts(group_id);
            CREATE INDEX hosts_credential_id_idx ON hosts(credential_id);
            CREATE INDEX hosts_jump_route_id_idx ON hosts(jump_route_id);
            CREATE INDEX jump_route_steps_host_id_idx ON jump_route_steps(host_id);
            ",
        )?;

        if simulate_interruption {
            return Err(StorageError::MigrationInterrupted);
        }

        transaction.execute_batch(
            "
            DROP TABLE legacy_jump_route_steps_v3;
            DROP TABLE legacy_hosts_v3;
            ",
        )?;
        transaction.pragma_update(None, "user_version", 4_i64)?;
        transaction.commit()?;
        Ok(())
    }

    fn migrate_to_v5(&mut self, simulate_interruption: bool) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "
            DROP INDEX group_overrides_credential_id_idx;
            DROP INDEX group_overrides_jump_route_id_idx;
            DROP INDEX hosts_group_id_idx;
            DROP INDEX hosts_credential_id_idx;
            DROP INDEX hosts_jump_route_id_idx;
            DROP INDEX jump_route_steps_host_id_idx;

            ALTER TABLE jump_route_steps RENAME TO legacy_jump_route_steps_v4;
            ALTER TABLE hosts RENAME TO legacy_hosts_v4;
            ALTER TABLE group_overrides RENAME TO legacy_group_overrides_v4;
            ALTER TABLE credentials RENAME TO legacy_credentials_v4;

            CREATE TABLE credentials(
                id TEXT PRIMARY KEY NOT NULL,
                label TEXT NOT NULL,
                username TEXT NOT NULL,
                kind TEXT NOT NULL
                    CHECK(kind IN ('password', 'private_key', 'system_agent')),
                secret_nonce BLOB NOT NULL,
                secret_ciphertext BLOB NOT NULL,
                passphrase_nonce BLOB,
                passphrase_ciphertext BLOB,
                CHECK(
                    (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
                    OR
                    (passphrase_nonce IS NOT NULL AND passphrase_ciphertext IS NOT NULL)
                ),
                CHECK(
                    kind = 'private_key'
                    OR
                    (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
                )
            ) WITHOUT ROWID;

            CREATE TABLE group_overrides(
                group_id TEXT PRIMARY KEY NOT NULL,
                credential_state INTEGER NOT NULL CHECK(credential_state IN (0, 1, 2)),
                credential_id TEXT,
                jump_route_state INTEGER NOT NULL CHECK(jump_route_state IN (0, 1, 2)),
                jump_route_id TEXT,
                CHECK(
                    (credential_state = 0 AND credential_id IS NULL)
                    OR (credential_state = 1 AND credential_id IS NOT NULL)
                    OR (credential_state = 2 AND credential_id IS NULL)
                ),
                CHECK(
                    (jump_route_state = 0 AND jump_route_id IS NULL)
                    OR (jump_route_state = 1 AND jump_route_id IS NOT NULL)
                    OR (jump_route_state = 2 AND jump_route_id IS NULL)
                ),
                FOREIGN KEY(group_id)
                    REFERENCES host_groups(id) ON DELETE CASCADE,
                FOREIGN KEY(credential_id)
                    REFERENCES credentials(id) ON DELETE RESTRICT,
                FOREIGN KEY(jump_route_id)
                    REFERENCES jump_routes(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE TABLE hosts(
                id TEXT PRIMARY KEY NOT NULL,
                display_name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                group_id TEXT,
                credential_state INTEGER NOT NULL CHECK(credential_state IN (0, 1, 2)),
                credential_id TEXT,
                jump_route_state INTEGER NOT NULL CHECK(jump_route_state IN (0, 1, 2)),
                jump_route_id TEXT,
                CHECK(
                    (credential_state = 0 AND credential_id IS NULL)
                    OR (credential_state = 1 AND credential_id IS NOT NULL)
                    OR (credential_state = 2 AND credential_id IS NULL)
                ),
                CHECK(
                    (jump_route_state = 0 AND jump_route_id IS NULL)
                    OR (jump_route_state = 1 AND jump_route_id IS NOT NULL)
                    OR (jump_route_state = 2 AND jump_route_id IS NULL)
                ),
                FOREIGN KEY(group_id)
                    REFERENCES host_groups(id) ON DELETE RESTRICT,
                FOREIGN KEY(credential_id)
                    REFERENCES credentials(id) ON DELETE RESTRICT,
                FOREIGN KEY(jump_route_id)
                    REFERENCES jump_routes(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            CREATE TABLE jump_route_steps(
                route_id TEXT NOT NULL,
                position INTEGER NOT NULL CHECK(position >= 0),
                host_id TEXT NOT NULL,
                PRIMARY KEY(route_id, position),
                UNIQUE(route_id, host_id),
                FOREIGN KEY(route_id)
                    REFERENCES jump_routes(id) ON DELETE CASCADE,
                FOREIGN KEY(host_id)
                    REFERENCES hosts(id) ON DELETE RESTRICT
            ) WITHOUT ROWID;

            INSERT INTO credentials(
                id, label, username, kind, secret_nonce, secret_ciphertext,
                passphrase_nonce, passphrase_ciphertext
            )
            SELECT id, label, username, kind, secret_nonce, secret_ciphertext,
                   passphrase_nonce, passphrase_ciphertext
            FROM legacy_credentials_v4;

            INSERT INTO group_overrides(
                group_id, credential_state, credential_id,
                jump_route_state, jump_route_id
            )
            SELECT group_id, credential_state, credential_id,
                   jump_route_state, jump_route_id
            FROM legacy_group_overrides_v4;

            INSERT INTO hosts(
                id, display_name, host, port, group_id,
                credential_state, credential_id,
                jump_route_state, jump_route_id
            )
            SELECT id, display_name, host, port, group_id,
                   credential_state, credential_id,
                   jump_route_state, jump_route_id
            FROM legacy_hosts_v4;

            INSERT INTO jump_route_steps(route_id, position, host_id)
            SELECT route_id, position, host_id
            FROM legacy_jump_route_steps_v4;

            CREATE INDEX group_overrides_credential_id_idx
                ON group_overrides(credential_id);
            CREATE INDEX group_overrides_jump_route_id_idx
                ON group_overrides(jump_route_id);
            CREATE INDEX hosts_group_id_idx ON hosts(group_id);
            CREATE INDEX hosts_credential_id_idx ON hosts(credential_id);
            CREATE INDEX hosts_jump_route_id_idx ON hosts(jump_route_id);
            CREATE INDEX jump_route_steps_host_id_idx ON jump_route_steps(host_id);
            ",
        )?;

        if simulate_interruption {
            return Err(StorageError::MigrationInterrupted);
        }

        transaction.execute_batch(
            "
            DROP TABLE legacy_jump_route_steps_v4;
            DROP TABLE legacy_hosts_v4;
            DROP TABLE legacy_group_overrides_v4;
            DROP TABLE legacy_credentials_v4;
            ",
        )?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
        Ok(())
    }

    fn prepare_legacy_host_migration(&self) -> Result<Vec<MigratedLegacyHost>, StorageError> {
        let stored_hosts = {
            let mut statement = self.connection.prepare(
                "
                SELECT id, display_name, host, port, username, password_nonce,
                       password_ciphertext
                FROM hosts
                ORDER BY id
                ",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(StoredLegacyHost {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    host: row.get(2)?,
                    port: row.get(3)?,
                    username: row.get(4)?,
                    password_nonce: row.get(5)?,
                    password_ciphertext: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut reserved_ids = HashSet::with_capacity(stored_hosts.len());
        stored_hosts
            .into_iter()
            .map(|stored| {
                let credential_id = self.unique_migration_credential_id(&reserved_ids)?;
                reserved_ids.insert(credential_id.clone());
                let password = self.decrypt_legacy_host_password(
                    &stored.id,
                    stored.password_nonce,
                    stored.password_ciphertext,
                )?;
                let credential = CredentialRecord::new(
                    credential_id.clone(),
                    stored.display_name.clone(),
                    stored.username,
                    CredentialSecret::Password { password },
                )?;
                let encrypted = self.encrypt_credential(&credential)?;
                let port = u16::try_from(stored.port).map_err(|_| StorageError::RecordIntegrity)?;
                let host = HostSummary::new(
                    stored.id,
                    stored.display_name,
                    stored.host,
                    port,
                    Some(credential_id.clone()),
                    None,
                )?;
                Ok(MigratedLegacyHost {
                    host,
                    credential_id,
                    credential_label: credential.label,
                    credential_username: credential.username,
                    encrypted,
                })
            })
            .collect()
    }

    fn unique_migration_credential_id(
        &self,
        reserved_ids: &HashSet<String>,
    ) -> Result<String, StorageError> {
        loop {
            let id = generate_credential_id()?;
            if reserved_ids.contains(&id) {
                continue;
            }
            let exists = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM credentials WHERE id = ?1)",
                [&id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Ok(id);
            }
        }
    }

    fn decrypt_legacy_host_password(
        &self,
        host_id: &str,
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
    ) -> Result<Zeroizing<String>, StorageError> {
        let nonce: [u8; NONCE_BYTES] = nonce
            .try_into()
            .map_err(|_| StorageError::RecordIntegrity)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.record_key[..])
            .map_err(|_| StorageError::RecordIntegrity)?;
        let aad = legacy_host_record_aad(&self.vault_id, host_id);
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: aad.as_bytes(),
                    },
                )
                .map_err(|_| StorageError::RecordIntegrity)?,
        );
        let value = std::str::from_utf8(&plaintext).map_err(|_| StorageError::RecordIntegrity)?;
        Ok(Zeroizing::new(value.to_owned()))
    }
}

struct StoredLegacyHost {
    id: String,
    display_name: String,
    host: String,
    port: i64,
    username: String,
    password_nonce: Vec<u8>,
    password_ciphertext: Vec<u8>,
}

struct StoredHostSummary {
    id: String,
    display_name: String,
    host: String,
    port: i64,
    group_id: Option<String>,
    credential_state: i64,
    credential_id: Option<String>,
    jump_route_state: i64,
    jump_route_id: Option<String>,
}

struct StoredGroupSummary {
    id: String,
    label: String,
    parent_group_id: Option<String>,
    credential_state: i64,
    credential_id: Option<String>,
    jump_route_state: i64,
    jump_route_id: Option<String>,
}

fn stored_host_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredHostSummary> {
    Ok(StoredHostSummary {
        id: row.get(0)?,
        display_name: row.get(1)?,
        host: row.get(2)?,
        port: row.get(3)?,
        group_id: row.get(4)?,
        credential_state: row.get(5)?,
        credential_id: row.get(6)?,
        jump_route_state: row.get(7)?,
        jump_route_id: row.get(8)?,
    })
}

fn stored_group_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredGroupSummary> {
    Ok(StoredGroupSummary {
        id: row.get(0)?,
        label: row.get(1)?,
        parent_group_id: row.get(2)?,
        credential_state: row.get(3)?,
        credential_id: row.get(4)?,
        jump_route_state: row.get(5)?,
        jump_route_id: row.get(6)?,
    })
}

fn load_stored_group(
    connection: &Connection,
    id: &str,
) -> Result<StoredGroupSummary, StorageError> {
    validate_group_id(id)?;
    connection
        .query_row(
            "
            SELECT g.id, g.label, g.parent_group_id,
                   o.credential_state, o.credential_id,
                   o.jump_route_state, o.jump_route_id
            FROM host_groups AS g
            JOIN group_overrides AS o ON o.group_id = g.id
            WHERE g.id = ?1
            ",
            [id],
            stored_group_from_row,
        )
        .optional()?
        .ok_or(StorageError::GroupNotFound)
}

fn host_summary_from_stored(
    connection: &Connection,
    stored: StoredHostSummary,
) -> Result<HostSummary, StorageError> {
    let credential_override =
        Override::from_storage(stored.credential_state, stored.credential_id)?;
    let jump_route_override =
        Override::from_storage(stored.jump_route_state, stored.jump_route_id)?;
    let (effective_credential_id, effective_jump_route_id) = resolve_effective_references(
        connection,
        stored.group_id.as_deref(),
        &credential_override,
        &jump_route_override,
    )?;

    HostSummary::from_stored(
        stored.id,
        stored.display_name,
        stored.host,
        u16::try_from(stored.port).map_err(|_| StorageError::RecordIntegrity)?,
        stored.group_id,
        credential_override,
        jump_route_override,
        effective_credential_id,
        effective_jump_route_id,
    )
}

fn group_summary_from_stored(
    connection: &Connection,
    stored: StoredGroupSummary,
) -> Result<GroupSummary, StorageError> {
    let credential_override =
        Override::from_storage(stored.credential_state, stored.credential_id)?;
    let jump_route_override =
        Override::from_storage(stored.jump_route_state, stored.jump_route_id)?;
    let (effective_credential_id, effective_jump_route_id) = resolve_effective_references(
        connection,
        stored.parent_group_id.as_deref(),
        &credential_override,
        &jump_route_override,
    )?;

    GroupSummary::from_stored(
        stored.id,
        stored.label,
        stored.parent_group_id,
        credential_override,
        jump_route_override,
        effective_credential_id,
        effective_jump_route_id,
    )
}

fn resolve_effective_references(
    connection: &Connection,
    group_id: Option<&str>,
    credential_override: &Override<String>,
    jump_route_override: &Override<String>,
) -> Result<(Option<String>, Option<String>), StorageError> {
    let mut credential = resolved_override(credential_override);
    let mut jump_route = resolved_override(jump_route_override);
    let mut current_group_id = group_id.map(str::to_owned);
    let mut visited = HashSet::new();
    let mut depth = 0usize;

    while let Some(group_id) = current_group_id {
        if !visited.insert(group_id.clone()) {
            return Err(StorageError::GroupCycle);
        }
        depth += 1;
        if depth > MAX_GROUP_DEPTH {
            return Err(StorageError::GroupTooDeep(MAX_GROUP_DEPTH));
        }

        let group = load_stored_group(connection, &group_id)?;
        let group_credential = Override::from_storage(group.credential_state, group.credential_id)?;
        let group_jump_route = Override::from_storage(group.jump_route_state, group.jump_route_id)?;
        if credential.is_none() {
            credential = resolved_override(&group_credential);
        }
        if jump_route.is_none() {
            jump_route = resolved_override(&group_jump_route);
        }
        current_group_id = group.parent_group_id;
    }

    Ok((credential.unwrap_or(None), jump_route.unwrap_or(None)))
}

fn resolved_override(value: &Override<String>) -> Option<Option<String>> {
    match value {
        Override::Inherit => None,
        Override::Set(value) => Some(Some(value.clone())),
        Override::Clear => Some(None),
    }
}

struct StoredCredentialSummary {
    id: String,
    label: String,
    username: String,
    kind: String,
}

struct StoredCredential {
    label: String,
    username: String,
    kind: String,
    secret_nonce: Vec<u8>,
    secret_ciphertext: Vec<u8>,
    passphrase_nonce: Option<Vec<u8>>,
    passphrase_ciphertext: Option<Vec<u8>>,
}

struct EncryptedCredential {
    secret: EncryptedField,
    passphrase: Option<EncryptedField>,
}

struct EncryptedField {
    nonce: [u8; NONCE_BYTES],
    ciphertext: Vec<u8>,
}

struct MigratedLegacyHost {
    host: HostSummary,
    credential_id: String,
    credential_label: String,
    credential_username: String,
    encrypted: EncryptedCredential,
}

fn validate_group_references(
    transaction: &Transaction<'_>,
    group: &GroupSummary,
) -> Result<(), StorageError> {
    if let Some(parent_group_id) = group.parent_group_id()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM host_groups WHERE id = ?1)",
            [parent_group_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::GroupNotFound);
    }
    if let Some(credential_id) = group.credential_override().value()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM credentials WHERE id = ?1)",
            [credential_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::CredentialNotFound);
    }
    if let Some(route_id) = group.jump_route_override().value()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM jump_routes WHERE id = ?1)",
            [route_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::JumpRouteNotFound);
    }
    Ok(())
}

fn validate_host_references(
    transaction: &Transaction<'_>,
    host: &HostSummary,
) -> Result<(), StorageError> {
    if let Some(group_id) = host.group_id()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM host_groups WHERE id = ?1)",
            [group_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::GroupNotFound);
    }
    if let Some(credential_id) = host.credential_override().value()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM credentials WHERE id = ?1)",
            [credential_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::CredentialNotFound);
    }
    if let Some(route_id) = host.jump_route_override().value()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM jump_routes WHERE id = ?1)",
            [route_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::JumpRouteNotFound);
    }
    Ok(())
}

fn validate_group_graph(connection: &Connection) -> Result<(), StorageError> {
    let parents = {
        let mut statement = connection.prepare(
            "
            SELECT id, parent_group_id
            FROM host_groups
            ORDER BY id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()?
    };

    for group_id in parents.keys() {
        let mut visited = HashSet::new();
        let mut current = Some(group_id.as_str());
        let mut depth = 0usize;

        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(StorageError::GroupCycle);
            }
            depth += 1;
            if depth > MAX_GROUP_DEPTH {
                return Err(StorageError::GroupTooDeep(MAX_GROUP_DEPTH));
            }
            current = parents
                .get(id)
                .ok_or(StorageError::RecordIntegrity)?
                .as_deref();
        }
    }
    Ok(())
}

fn validate_route_host_references(
    transaction: &Transaction<'_>,
    route: &JumpRouteSummary,
) -> Result<(), StorageError> {
    for host_id in route.host_ids() {
        if !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM hosts WHERE id = ?1)",
            [host_id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(StorageError::HostNotFound);
        }
    }
    Ok(())
}

fn insert_jump_route_steps(
    transaction: &Transaction<'_>,
    route: &JumpRouteSummary,
) -> Result<(), StorageError> {
    for (position, host_id) in route.host_ids().iter().enumerate() {
        transaction.execute(
            "
            INSERT INTO jump_route_steps(route_id, position, host_id)
            VALUES(?1, ?2, ?3)
            ",
            params![
                route.id(),
                i64::try_from(position).map_err(|_| StorageError::InvalidJumpRoute)?,
                host_id,
            ],
        )?;
    }
    Ok(())
}

fn validate_jump_route_graph(connection: &Connection) -> Result<(), StorageError> {
    let route_steps = {
        let mut statement = connection.prepare(
            "
            SELECT route_id, host_id
            FROM jump_route_steps
            ORDER BY route_id, position
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut route_steps: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let (route_id, host_id) = row?;
            route_steps.entry(route_id).or_default().push(host_id);
        }
        route_steps
    };

    let host_routes = {
        let mut statement = connection.prepare(
            "
            SELECT id, group_id,
                   credential_state, credential_id,
                   jump_route_state, jump_route_id
            FROM hosts
            ORDER BY id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut graph = HashMap::with_capacity(host_routes.len());
    for (host_id, group_id, credential_state, credential_id, jump_route_state, jump_route_id) in
        host_routes
    {
        let credential_override = Override::from_storage(credential_state, credential_id)?;
        let jump_route_override = Override::from_storage(jump_route_state, jump_route_id)?;
        let (_, effective_route_id) = resolve_effective_references(
            connection,
            group_id.as_deref(),
            &credential_override,
            &jump_route_override,
        )?;
        let Some(route_id) = effective_route_id else {
            continue;
        };
        let steps = route_steps
            .get(&route_id)
            .ok_or(StorageError::RecordIntegrity)?;
        graph.insert(host_id, steps.clone());
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for host_id in graph.keys() {
        if jump_route_graph_has_cycle(host_id, &graph, &mut visiting, &mut visited) {
            return Err(StorageError::JumpRouteCycle);
        }
    }
    Ok(())
}

fn jump_route_graph_has_cycle(
    host_id: &str,
    graph: &HashMap<String, Vec<String>>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if visited.contains(host_id) {
        return false;
    }
    if !visiting.insert(host_id.to_owned()) {
        return true;
    }

    if let Some(next_hosts) = graph.get(host_id) {
        for next_host in next_hosts {
            if jump_route_graph_has_cycle(next_host, graph, visiting, visited) {
                return true;
            }
        }
    }

    visiting.remove(host_id);
    visited.insert(host_id.to_owned());
    false
}

fn apply_database_key(connection: &Connection, key: &[u8; KEY_BYTES]) -> Result<(), StorageError> {
    let mut literal = Zeroizing::new(String::with_capacity(2 + KEY_BYTES * 2 + 1));
    literal.push_str("x'");
    for byte in key {
        write!(&mut *literal, "{byte:02x}").map_err(|_| StorageError::InvalidDatabase)?;
    }
    literal.push('\'');
    connection
        .pragma_update(None, "key", literal.as_str())
        .map_err(|_| StorageError::InvalidDatabase)
}

fn initialize_schema(connection: &mut Connection) -> Result<(), StorageError> {
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(StorageError::Sql)?;
    if current_version != 0 {
        return Err(StorageError::UnsupportedSchema(current_version));
    }
    migrate_to_v1(connection, false)?;
    migrate_to_v2(connection, false)
}

fn migrate_to_v1(
    connection: &mut Connection,
    simulate_interruption: bool,
) -> Result<(), StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "
        CREATE TABLE vault_meta(
            key TEXT PRIMARY KEY NOT NULL,
            value BLOB NOT NULL
        ) WITHOUT ROWID;

        CREATE TABLE hosts(
            id TEXT PRIMARY KEY NOT NULL,
            display_name TEXT NOT NULL,
            host TEXT NOT NULL,
            port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
            username TEXT NOT NULL,
            password_nonce BLOB NOT NULL,
            password_ciphertext BLOB NOT NULL
        ) WITHOUT ROWID;
        ",
    )?;
    if simulate_interruption {
        return Err(StorageError::MigrationInterrupted);
    }
    transaction.pragma_update(None, "user_version", 1_i64)?;
    transaction.commit()?;
    Ok(())
}

fn migrate_to_v2(
    connection: &mut Connection,
    simulate_interruption: bool,
) -> Result<(), StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "
        CREATE TABLE credentials(
            id TEXT PRIMARY KEY NOT NULL,
            label TEXT NOT NULL,
            username TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('password', 'private_key')),
            secret_nonce BLOB NOT NULL,
            secret_ciphertext BLOB NOT NULL,
            passphrase_nonce BLOB,
            passphrase_ciphertext BLOB,
            CHECK(
                (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
                OR
                (passphrase_nonce IS NOT NULL AND passphrase_ciphertext IS NOT NULL)
            ),
            CHECK(
                kind = 'private_key'
                OR
                (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
            )
        ) WITHOUT ROWID;
        ",
    )?;
    if simulate_interruption {
        return Err(StorageError::MigrationInterrupted);
    }
    transaction.pragma_update(None, "user_version", 2_i64)?;
    transaction.commit()?;
    Ok(())
}

fn legacy_host_record_aad(vault_id: &str, host_id: &str) -> String {
    format!("anyssh/record/v1|{vault_id}|host|{host_id}|password")
}

fn credential_record_aad(
    vault_id: &str,
    credential_id: &str,
    kind: CredentialKind,
    field: &str,
) -> String {
    format!(
        "anyssh/record/v2|{vault_id}|credential|{credential_id}|{}|{field}",
        kind.storage_value()
    )
}

fn create_private_directory(path: &Path) -> Result<(), StorageError> {
    if path.exists() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).map_err(StorageError::Io)
    }

    #[cfg(not(unix))]
    fs::create_dir_all(path).map_err(StorageError::Io)
}

fn set_private_file_permissions(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(StorageError::Io)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) {
    #[cfg(unix)]
    {
        if let Ok(directory) = File::open(path) {
            let _ = directory.sync_all();
        }
    }
}

fn is_regular_file_without_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inheritance::{CLEAR_STATE, INHERIT_STATE, SET_STATE};

    fn test_parameters() -> PinKdfParameters {
        PinKdfParameters::new(8 * 1024, 1, 1).expect("test parameters")
    }

    fn fixture_host(
        id: &str,
        display_name: &str,
        host: &str,
        credential_id: Option<&str>,
        jump_route_id: Option<&str>,
    ) -> HostSummary {
        HostSummary::new(
            id,
            display_name,
            host,
            2222,
            credential_id.map(str::to_owned),
            jump_route_id.map(str::to_owned),
        )
        .expect("host")
    }

    fn fixture_route(id: &str, label: &str, host_ids: &[&str]) -> JumpRouteSummary {
        JumpRouteSummary::new(
            id,
            label,
            host_ids.iter().map(|id| (*id).to_owned()).collect(),
        )
        .expect("Jump Route")
    }

    fn fixture_group(
        id: &str,
        label: &str,
        parent_group_id: Option<&str>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> GroupSummary {
        GroupSummary::new(
            id,
            label,
            parent_group_id.map(str::to_owned),
            credential_override,
            jump_route_override,
        )
        .expect("Group")
    }

    #[allow(clippy::too_many_arguments)]
    fn fixture_host_with_overrides(
        id: &str,
        display_name: &str,
        host: &str,
        group_id: Option<&str>,
        credential_override: Override<String>,
        jump_route_override: Override<String>,
    ) -> HostSummary {
        HostSummary::new_with_overrides(
            id,
            display_name,
            host,
            2222,
            group_id.map(str::to_owned),
            credential_override,
            jump_route_override,
        )
        .expect("Host")
    }

    fn password_credential(id: &str, password: &str) -> CredentialRecord {
        CredentialRecord::new(
            id,
            "Production password",
            "credential-user",
            CredentialSecret::Password {
                password: Zeroizing::new(password.to_owned()),
            },
        )
        .expect("password credential")
    }

    fn private_key_credential(id: &str) -> CredentialRecord {
        CredentialRecord::new(
            id,
            "Production private key",
            "key-user",
            CredentialSecret::PrivateKey {
                private_key: Zeroizing::new(
                    "-----BEGIN OPENSSH PRIVATE KEY-----\nfixture-private-key\n".to_owned(),
                ),
                passphrase: Some(Zeroizing::new("fixture-private-key-passphrase".to_owned())),
            },
        )
        .expect("private-key credential")
    }

    fn system_agent_credential(id: &str, fingerprint: &str) -> CredentialRecord {
        CredentialRecord::new(
            id,
            "Workstation SSH Agent",
            "agent-user",
            CredentialSecret::SystemAgent {
                identity_fingerprint_sha256: Zeroizing::new(fingerprint.to_owned()),
            },
        )
        .expect("system-agent credential")
    }

    #[test]
    fn encrypted_vault_survives_restart_and_hides_plaintext() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        assert!(vault.cipher_version().starts_with("4."));
        eprintln!("SQLCipher {}", vault.cipher_version());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root)
                    .expect("root metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            for file_name in [BOOTSTRAP_FILE_NAME, DATABASE_FILE_NAME] {
                assert_eq!(
                    fs::metadata(root.join(file_name))
                        .expect("file metadata")
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
        let credential =
            password_credential("cred-shared-password", "fixture-password-should-never-leak");
        vault
            .create_credential(&credential)
            .expect("create credential");
        let jump_one = fixture_host(
            "host-jump-one",
            "Secret production jump one",
            "jump-one.internal.example",
            Some("cred-shared-password"),
            None,
        );
        let jump_two = fixture_host(
            "host-jump-two",
            "Secret production jump two",
            "jump-two.internal.example",
            Some("cred-shared-password"),
            None,
        );
        vault
            .create_host(&jump_one)
            .expect("create first jump Host");
        vault
            .create_host(&jump_two)
            .expect("create second jump Host");
        let route = fixture_route(
            "route-production",
            "Secret production route",
            &["host-jump-two", "host-jump-one"],
        );
        vault.create_jump_route(&route).expect("create Jump Route");
        let group = fixture_group(
            "group-production",
            "Secret production group",
            None,
            Override::Set("cred-shared-password".to_owned()),
            Override::Set("route-production".to_owned()),
        );
        vault.create_group(&group).expect("create Group");
        let target = fixture_host(
            "host-target",
            "Secret production target",
            "target.internal.example",
            Some("cred-shared-password"),
            Some("route-production"),
        );
        vault.create_host(&target).expect("create target host");

        assert_files_do_not_contain(
            &root,
            &[
                jump_one.display_name().as_bytes(),
                jump_one.host().as_bytes(),
                jump_two.display_name().as_bytes(),
                jump_two.host().as_bytes(),
                route.label().as_bytes(),
                group.label().as_bytes(),
                target.display_name().as_bytes(),
                target.host().as_bytes(),
                b"credential-user",
                b"fixture-password-should-never-leak",
                b"SQLite format 3",
            ],
        );

        drop(vault);
        let reopened = LocalVault::unlock(&root, "123456").expect("unlock");
        let hosts = reopened.list_hosts().expect("list hosts");
        assert_eq!(hosts, vec![jump_one, jump_two, target]);
        assert_eq!(
            reopened.list_jump_routes().expect("list Jump Routes"),
            vec![route]
        );
        assert_eq!(reopened.list_groups().expect("list Groups"), vec![group]);
        let (username, secret) = reopened
            .resolve_credential("cred-shared-password")
            .expect("resolve shared credential")
            .into_parts();
        assert_eq!(username, "credential-user");
        let CredentialSecret::Password { password } = secret else {
            panic!("expected password credential");
        };
        assert_eq!(password.as_str(), "fixture-password-should-never-leak");
    }

    #[test]
    fn wrong_pin_and_corrupt_slot_do_not_expose_secrets() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let vault = LocalVault::create(&root, "123456", test_parameters()).expect("create");
        drop(vault);

        let wrong_pin = LocalVault::unlock(&root, "654321")
            .err()
            .expect("wrong pin must fail");
        assert!(!wrong_pin.to_string().contains("654321"));

        let bootstrap_path = root.join(BOOTSTRAP_FILE_NAME);
        let mut bootstrap = fs::read_to_string(&bootstrap_path).expect("read bootstrap");
        let ciphertext_marker = "\"ciphertext\": \"";
        let position =
            bootstrap.find(ciphertext_marker).expect("ciphertext") + ciphertext_marker.len();
        bootstrap.replace_range(position..position + 1, "!");
        fs::write(&bootstrap_path, bootstrap).expect("corrupt bootstrap");

        let corrupt = LocalVault::unlock(&root, "123456")
            .err()
            .expect("corrupt slot must fail");
        assert!(!corrupt.to_string().contains("123456"));
    }

    #[test]
    fn credential_repository_round_trip_hides_secrets_and_metadata() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        let password = password_credential("cred-password", "credential-password-secret");
        let private_key = private_key_credential("cred-private-key");
        let agent = system_agent_credential(
            "cred-system-agent",
            "SHA256:system-agent-fingerprint-selector",
        );

        let password_summary = vault
            .create_credential(&password)
            .expect("create password credential");
        let private_key_summary = vault
            .create_credential(&private_key)
            .expect("create private-key credential");
        let agent_summary = vault
            .create_credential(&agent)
            .expect("create system-agent credential");
        assert_eq!(password_summary.kind(), CredentialKind::Password);
        assert_eq!(private_key_summary.kind(), CredentialKind::PrivateKey);
        assert_eq!(agent_summary.kind(), CredentialKind::SystemAgent);

        let summaries = vault.list_credentials().expect("list credentials");
        assert_eq!(summaries.len(), 3);
        let summary_debug = format!("{summaries:?}");
        assert!(!summary_debug.contains("credential-password-secret"));
        assert!(!summary_debug.contains("fixture-private-key"));
        assert!(!summary_debug.contains("system-agent-fingerprint-selector"));

        assert_files_do_not_contain(
            &root,
            &[
                b"Production password",
                b"credential-user",
                b"credential-password-secret",
                b"Production private key",
                b"key-user",
                b"fixture-private-key",
                b"fixture-private-key-passphrase",
                b"Workstation SSH Agent",
                b"agent-user",
                b"system-agent-fingerprint-selector",
            ],
        );

        drop(vault);
        let mut reopened = LocalVault::unlock(&root, "123456").expect("unlock");
        let resolved_password = reopened
            .resolve_credential("cred-password")
            .expect("resolve password");
        let (username, secret) = resolved_password.into_parts();
        assert_eq!(username, "credential-user");
        let CredentialSecret::Password { password } = secret else {
            panic!("expected password credential");
        };
        assert_eq!(password.as_str(), "credential-password-secret");

        let resolved_key = reopened
            .resolve_credential("cred-private-key")
            .expect("resolve private key");
        let (username, secret) = resolved_key.into_parts();
        assert_eq!(username, "key-user");
        let CredentialSecret::PrivateKey {
            private_key,
            passphrase,
        } = secret
        else {
            panic!("expected private-key credential");
        };
        assert!(private_key.contains("BEGIN OPENSSH PRIVATE KEY"));
        assert_eq!(
            passphrase.as_deref().map(String::as_str),
            Some("fixture-private-key-passphrase")
        );

        let resolved_agent = reopened
            .resolve_credential("cred-system-agent")
            .expect("resolve system agent");
        let (username, secret) = resolved_agent.into_parts();
        assert_eq!(username, "agent-user");
        let CredentialSecret::SystemAgent {
            identity_fingerprint_sha256,
        } = secret
        else {
            panic!("expected system-agent credential");
        };
        assert_eq!(
            identity_fingerprint_sha256.as_str(),
            "SHA256:system-agent-fingerprint-selector"
        );

        let updated = password_credential("cred-password", "updated-password-secret");
        reopened
            .update_credential(&updated)
            .expect("update password credential");
        let (_, updated_secret) = reopened
            .resolve_credential("cred-password")
            .expect("resolve updated credential")
            .into_parts();
        let CredentialSecret::Password { password } = updated_secret else {
            panic!("expected updated password");
        };
        assert_eq!(password.as_str(), "updated-password-secret");
        assert_files_do_not_contain(&root, &[b"updated-password-secret"]);

        assert!(
            reopened
                .delete_credential("cred-private-key")
                .expect("delete private key")
        );
        assert!(
            !reopened
                .delete_credential("cred-private-key")
                .expect("delete missing private key")
        );
        assert!(matches!(
            reopened.resolve_credential("cred-private-key"),
            Err(StorageError::CredentialNotFound)
        ));
    }

    #[test]
    fn host_and_jump_route_repositories_enforce_reference_integrity() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        vault
            .create_credential(&password_credential("cred-shared", "shared-secret"))
            .expect("create shared credential");

        let host_a = fixture_host("host-a", "Host A", "a.internal", Some("cred-shared"), None);
        let host_b = fixture_host("host-b", "Host B", "b.internal", Some("cred-shared"), None);
        vault.create_host(&host_a).expect("create Host A");
        vault.create_host(&host_b).expect("create Host B");

        let route_for_b = fixture_route("route-for-b", "Route for B", &["host-a"]);
        vault
            .create_jump_route(&route_for_b)
            .expect("create route for B");
        let host_b_with_route = fixture_host(
            "host-b",
            "Host B",
            "b.internal",
            Some("cred-shared"),
            Some("route-for-b"),
        );
        vault
            .update_host(&host_b_with_route)
            .expect("attach route to Host B");
        let cyclic_route_update = fixture_route("route-for-b", "Route for B", &["host-b"]);
        assert!(matches!(
            vault.update_jump_route(&cyclic_route_update),
            Err(StorageError::JumpRouteCycle)
        ));
        assert_eq!(
            vault
                .list_jump_routes()
                .expect("list after rejected Route cycle")
                .into_iter()
                .find(|route| route.id() == "route-for-b")
                .expect("Route for B")
                .host_ids(),
            ["host-a"]
        );

        let route_for_a = fixture_route("route-for-a", "Route for A", &["host-b"]);
        vault
            .create_jump_route(&route_for_a)
            .expect("create route for A");
        let host_a_with_cycle = fixture_host(
            "host-a",
            "Host A",
            "a.internal",
            Some("cred-shared"),
            Some("route-for-a"),
        );
        assert!(matches!(
            vault.update_host(&host_a_with_cycle),
            Err(StorageError::JumpRouteCycle)
        ));
        assert_eq!(
            vault
                .list_hosts()
                .expect("list after rejected cycle")
                .into_iter()
                .find(|host| host.id() == "host-a")
                .expect("Host A")
                .jump_route_id(),
            None
        );

        let direct_cycle = fixture_route("route-self", "Self route", &["host-a"]);
        vault
            .create_jump_route(&direct_cycle)
            .expect("create self route");
        let self_referencing_host = fixture_host(
            "host-a",
            "Host A",
            "a.internal",
            Some("cred-shared"),
            Some("route-self"),
        );
        assert!(matches!(
            vault.update_host(&self_referencing_host),
            Err(StorageError::JumpRouteCycle)
        ));

        assert!(matches!(
            vault.create_host(&fixture_host(
                "host-missing-credential",
                "Missing credential",
                "missing-credential.internal",
                Some("cred-missing"),
                None,
            )),
            Err(StorageError::CredentialNotFound)
        ));
        assert!(matches!(
            vault.create_jump_route(&fixture_route(
                "route-missing-host",
                "Missing host",
                &["host-missing"],
            )),
            Err(StorageError::HostNotFound)
        ));

        assert!(matches!(
            vault.delete_credential("cred-shared"),
            Err(StorageError::CredentialInUse)
        ));
        assert!(matches!(
            vault.delete_host("host-a"),
            Err(StorageError::HostInUse)
        ));
        assert!(matches!(
            vault.delete_jump_route("route-for-b"),
            Err(StorageError::JumpRouteInUse)
        ));
    }

    #[test]
    fn group_repository_resolves_three_state_inheritance_and_restricts_deletion() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        vault
            .create_credential(&password_credential("cred-root", "root-secret"))
            .expect("create root credential");
        vault
            .create_credential(&password_credential("cred-child", "child-secret"))
            .expect("create child credential");

        assert!(matches!(
            vault.create_group(&fixture_group(
                "group-missing-parent",
                "Missing parent",
                Some("group-missing"),
                Override::Inherit,
                Override::Inherit,
            )),
            Err(StorageError::GroupNotFound)
        ));
        assert!(matches!(
            vault.create_group(&fixture_group(
                "group-missing-credential",
                "Missing Credential",
                None,
                Override::Set("cred-missing".to_owned()),
                Override::Inherit,
            )),
            Err(StorageError::CredentialNotFound)
        ));
        assert!(matches!(
            vault.create_group(&fixture_group(
                "group-missing-route",
                "Missing Route",
                None,
                Override::Inherit,
                Override::Set("route-missing".to_owned()),
            )),
            Err(StorageError::JumpRouteNotFound)
        ));

        let root_group = fixture_group(
            "group-root",
            "Root",
            None,
            Override::Set("cred-root".to_owned()),
            Override::Inherit,
        );
        vault.create_group(&root_group).expect("create root Group");
        let child_group = fixture_group(
            "group-child",
            "Child",
            Some("group-root"),
            Override::Inherit,
            Override::Clear,
        );
        vault
            .create_group(&child_group)
            .expect("create child Group");
        assert!(matches!(
            vault.create_host(&fixture_host_with_overrides(
                "host-missing-group",
                "Missing Group",
                "missing-group.internal",
                Some("group-missing"),
                Override::Inherit,
                Override::Inherit,
            )),
            Err(StorageError::GroupNotFound)
        ));

        let groups = vault.list_groups().expect("list Groups");
        let child = groups
            .iter()
            .find(|group| group.id() == "group-child")
            .expect("child Group");
        assert_eq!(child.effective_credential_id(), Some("cred-root"));
        assert_eq!(child.effective_jump_route_id(), None);

        let host = fixture_host_with_overrides(
            "host-inherited",
            "Inherited",
            "inherited.internal",
            Some("group-child"),
            Override::Inherit,
            Override::Inherit,
        );
        let host = vault.create_host(&host).expect("create inherited Host");
        assert_eq!(host.credential_id(), Some("cred-root"));
        assert_eq!(host.jump_route_id(), None);

        let child_override = fixture_group(
            "group-child",
            "Child",
            Some("group-root"),
            Override::Set("cred-child".to_owned()),
            Override::Clear,
        );
        vault
            .update_group(&child_override)
            .expect("override child credential");
        let host = vault
            .list_hosts()
            .expect("list Hosts")
            .into_iter()
            .find(|host| host.id() == "host-inherited")
            .expect("inherited Host");
        assert_eq!(host.credential_id(), Some("cred-child"));

        let cleared_host = fixture_host_with_overrides(
            "host-inherited",
            "Inherited",
            "inherited.internal",
            Some("group-child"),
            Override::Clear,
            Override::Inherit,
        );
        let cleared_host = vault
            .update_host(&cleared_host)
            .expect("clear inherited credential");
        assert_eq!(cleared_host.credential_id(), None);
        assert_eq!(cleared_host.credential_override(), &Override::Clear);

        assert!(matches!(
            vault.delete_group("group-root"),
            Err(StorageError::GroupInUse)
        ));
        assert!(matches!(
            vault.delete_group("group-child"),
            Err(StorageError::GroupInUse)
        ));
        assert!(matches!(
            vault.delete_credential("cred-child"),
            Err(StorageError::CredentialInUse)
        ));

        assert!(vault.delete_host("host-inherited").expect("delete Host"));
        assert!(vault.delete_group("group-child").expect("delete child"));
        assert!(vault.delete_group("group-root").expect("delete root"));
        assert!(
            vault
                .delete_credential("cred-child")
                .expect("delete child credential")
        );
    }

    #[test]
    fn group_repository_rejects_cycles_depth_and_effective_jump_route_cycles() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        vault
            .create_credential(&password_credential("cred-shared", "shared-secret"))
            .expect("create credential");

        let mut parent_id = None;
        let mut group_ids = Vec::new();
        for index in 0..MAX_GROUP_DEPTH {
            let id = format!("group-depth-{index}");
            let group = fixture_group(
                &id,
                &format!("Depth {index}"),
                parent_id.as_deref(),
                Override::Inherit,
                Override::Inherit,
            );
            vault.create_group(&group).expect("create allowed depth");
            parent_id = Some(id.clone());
            group_ids.push(id);
        }
        let too_deep = fixture_group(
            "group-too-deep",
            "Too deep",
            parent_id.as_deref(),
            Override::Inherit,
            Override::Inherit,
        );
        assert!(matches!(
            vault.create_group(&too_deep),
            Err(StorageError::GroupTooDeep(MAX_GROUP_DEPTH))
        ));
        assert_eq!(
            vault
                .list_groups()
                .expect("list after depth rejection")
                .len(),
            MAX_GROUP_DEPTH
        );

        let root_cycle = fixture_group(
            &group_ids[0],
            "Depth 0",
            group_ids.last().map(String::as_str),
            Override::Inherit,
            Override::Inherit,
        );
        assert!(matches!(
            vault.update_group(&root_cycle),
            Err(StorageError::GroupCycle)
        ));
        assert_eq!(
            vault
                .list_groups()
                .expect("list after cycle rejection")
                .into_iter()
                .find(|group| group.id() == group_ids[0])
                .expect("root Group")
                .parent_group_id(),
            None
        );

        let route_group = fixture_group(
            "group-route-cycle",
            "Route cycle",
            None,
            Override::Set("cred-shared".to_owned()),
            Override::Inherit,
        );
        vault
            .create_group(&route_group)
            .expect("create route Group");
        let host = fixture_host_with_overrides(
            "host-route-cycle",
            "Route cycle",
            "route-cycle.internal",
            Some("group-route-cycle"),
            Override::Inherit,
            Override::Inherit,
        );
        vault.create_host(&host).expect("create grouped Host");
        let route = fixture_route("route-group-cycle", "Group cycle", &["host-route-cycle"]);
        vault.create_jump_route(&route).expect("create Route");
        let cyclic_group = fixture_group(
            "group-route-cycle",
            "Route cycle",
            None,
            Override::Set("cred-shared".to_owned()),
            Override::Set("route-group-cycle".to_owned()),
        );
        assert!(matches!(
            vault.update_group(&cyclic_group),
            Err(StorageError::JumpRouteCycle)
        ));
        assert_eq!(
            vault
                .list_groups()
                .expect("list after Route cycle rejection")
                .into_iter()
                .find(|group| group.id() == "group-route-cycle")
                .expect("route Group")
                .jump_route_override(),
            &Override::Inherit
        );
    }

    #[test]
    fn saved_host_plan_expands_nested_routes_and_resolves_credentials() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        for (id, username, password) in [
            ("cred-jump-one", "jump-one-user", "jump-one-secret"),
            ("cred-jump-two", "jump-two-user", "jump-two-secret"),
            ("cred-target", "target-user", "target-secret"),
        ] {
            vault
                .create_credential(
                    &CredentialRecord::new(
                        id,
                        id,
                        username,
                        CredentialSecret::Password {
                            password: Zeroizing::new(password.to_owned()),
                        },
                    )
                    .expect("credential"),
                )
                .expect("create credential");
        }

        let jump_one = fixture_host(
            "host-jump-one",
            "Jump one",
            "jump-one.internal",
            Some("cred-jump-one"),
            None,
        );
        vault.create_host(&jump_one).expect("create Jump 1");
        let route_to_jump_two =
            fixture_route("route-to-jump-two", "Route to Jump 2", &["host-jump-one"]);
        vault
            .create_jump_route(&route_to_jump_two)
            .expect("create route to Jump 2");
        let jump_two = fixture_host(
            "host-jump-two",
            "Jump two",
            "jump-two.internal",
            Some("cred-jump-two"),
            Some("route-to-jump-two"),
        );
        vault.create_host(&jump_two).expect("create Jump 2");
        let route_to_target =
            fixture_route("route-to-target", "Route to Target", &["host-jump-two"]);
        vault
            .create_jump_route(&route_to_target)
            .expect("create route to Target");
        let target = fixture_host(
            "host-target",
            "Target",
            "target.internal",
            Some("cred-target"),
            Some("route-to-target"),
        );
        vault.create_host(&target).expect("create Target");

        let plan = vault
            .resolve_host_connection_plan("host-target")
            .expect("resolve saved Host plan");
        assert_eq!(plan.target().host_id(), "host-target");
        assert_eq!(
            plan.jump_hosts()
                .iter()
                .map(ResolvedHostConnection::host_id)
                .collect::<Vec<_>>(),
            ["host-jump-one", "host-jump-two"]
        );
        let debug = format!("{plan:?}");
        for secret in ["jump-one-secret", "jump-two-secret", "target-secret"] {
            assert!(!debug.contains(secret));
        }

        let (target, jump_hosts) = plan.into_parts();
        let usernames = jump_hosts
            .into_iter()
            .chain(std::iter::once(target))
            .map(|connection| {
                let (_, _, _, credential) = connection.into_parts();
                credential.into_parts().0
            })
            .collect::<Vec<_>>();
        assert_eq!(usernames, ["jump-one-user", "jump-two-user", "target-user"]);
    }

    #[test]
    fn saved_host_plan_rejects_missing_duplicate_and_oversized_routes() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        vault
            .create_credential(&password_credential("cred-shared", "shared-secret"))
            .expect("create shared credential");

        let missing_credential = fixture_host(
            "host-no-credential",
            "No credential",
            "none.internal",
            None,
            None,
        );
        vault
            .create_host(&missing_credential)
            .expect("create Host without Credential");
        assert!(matches!(
            vault.resolve_host_connection_plan("host-no-credential"),
            Err(StorageError::HostCredentialMissing(id)) if id == "host-no-credential"
        ));

        let jump_one = fixture_host(
            "host-duplicate-one",
            "Duplicate one",
            "duplicate-one.internal",
            Some("cred-shared"),
            None,
        );
        vault
            .create_host(&jump_one)
            .expect("create duplicate Host 1");
        let nested_route = fixture_route(
            "route-nested-duplicate",
            "Nested duplicate",
            &["host-duplicate-one"],
        );
        vault
            .create_jump_route(&nested_route)
            .expect("create nested duplicate route");
        let jump_two = fixture_host(
            "host-duplicate-two",
            "Duplicate two",
            "duplicate-two.internal",
            Some("cred-shared"),
            Some("route-nested-duplicate"),
        );
        vault
            .create_host(&jump_two)
            .expect("create duplicate Host 2");
        let duplicate_route = fixture_route(
            "route-duplicate-expanded",
            "Expanded duplicate",
            &["host-duplicate-one", "host-duplicate-two"],
        );
        vault
            .create_jump_route(&duplicate_route)
            .expect("create expanded duplicate route");
        let duplicate_target = fixture_host(
            "host-duplicate-target",
            "Duplicate target",
            "duplicate-target.internal",
            Some("cred-shared"),
            Some("route-duplicate-expanded"),
        );
        vault
            .create_host(&duplicate_target)
            .expect("create duplicate Target");
        assert!(matches!(
            vault.resolve_host_connection_plan("host-duplicate-target"),
            Err(StorageError::JumpRouteDuplicateHost)
        ));

        let mut wide_host_ids = Vec::new();
        for index in 0..MAX_JUMP_ROUTE_STEPS {
            let id = format!("host-wide-{index}");
            let host = fixture_host(
                &id,
                &format!("Wide {index}"),
                &format!("wide-{index}.internal"),
                Some("cred-shared"),
                None,
            );
            vault.create_host(&host).expect("create wide Route Host");
            wide_host_ids.push(id);
        }
        let wide_route =
            JumpRouteSummary::new("route-wide", "Wide route", wide_host_ids).expect("wide route");
        vault
            .create_jump_route(&wide_route)
            .expect("create maximum-size route");
        let gateway = fixture_host(
            "host-wide-gateway",
            "Wide gateway",
            "wide-gateway.internal",
            Some("cred-shared"),
            Some("route-wide"),
        );
        vault.create_host(&gateway).expect("create wide gateway");
        let oversized_route =
            fixture_route("route-oversized", "Oversized route", &["host-wide-gateway"]);
        vault
            .create_jump_route(&oversized_route)
            .expect("create oversized parent route");
        let oversized_target = fixture_host(
            "host-oversized-target",
            "Oversized target",
            "oversized-target.internal",
            Some("cred-shared"),
            Some("route-oversized"),
        );
        vault
            .create_host(&oversized_target)
            .expect("create oversized Target");
        assert!(matches!(
            vault.resolve_host_connection_plan("host-oversized-target"),
            Err(StorageError::JumpRouteTooLong(MAX_JUMP_ROUTE_STEPS))
        ));
    }

    #[test]
    fn incomplete_layout_is_not_overwritten() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        fs::create_dir(&root).expect("create root");
        fs::write(root.join(BOOTSTRAP_FILE_NAME), b"{}").expect("write partial");

        assert_eq!(LocalVault::presence(&root), VaultPresence::Damaged);
        assert!(matches!(
            LocalVault::create(&root, "123456", test_parameters()),
            Err(StorageError::AlreadyInitialized)
        ));
        assert!(matches!(
            LocalVault::unlock(&root, "123456"),
            Err(StorageError::DamagedLayout)
        ));
    }

    #[test]
    fn interrupted_migration_rolls_back_schema() {
        let mut connection = Connection::open_in_memory().expect("connection");
        let error = migrate_to_v1(&mut connection, true).expect_err("migration must fail");
        assert!(matches!(error, StorageError::MigrationInterrupted));

        let hosts_table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_schema WHERE type='table' AND name='hosts'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query schema");
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");
        assert!(hosts_table.is_none());
        assert_eq!(version, 0);
    }

    #[test]
    fn interrupted_v2_migration_preserves_complete_v1_schema() {
        let mut connection = Connection::open_in_memory().expect("connection");
        migrate_to_v1(&mut connection, false).expect("create v1 schema");
        let error = migrate_to_v2(&mut connection, true).expect_err("migration must fail");
        assert!(matches!(error, StorageError::MigrationInterrupted));

        let hosts_table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_schema WHERE type='table' AND name='hosts'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query hosts schema");
        let credentials_table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_schema WHERE type='table' AND name='credentials'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query credentials schema");
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");

        assert_eq!(hosts_table.as_deref(), Some("hosts"));
        assert!(credentials_table.is_none());
        assert_eq!(version, 1);
    }

    #[test]
    fn interrupted_v3_migration_preserves_complete_v2_schema() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        downgrade_to_v2_schema(&vault);
        insert_legacy_host(
            &vault,
            "legacy.host:2222",
            "Legacy host",
            "legacy.internal",
            22,
            "legacy-user",
            "legacy-password",
        );

        let error = vault
            .database
            .migrate_to_v3(true)
            .expect_err("migration must fail");
        assert!(matches!(error, StorageError::MigrationInterrupted));

        let version: i64 = vault
            .database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        let jump_routes_table: Option<String> = vault
            .database
            .connection
            .query_row(
                "SELECT name FROM sqlite_schema WHERE type='table' AND name='jump_routes'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query Jump Route schema");
        let migrated_credentials: i64 = vault
            .database
            .connection
            .query_row("SELECT count(*) FROM credentials", [], |row| row.get(0))
            .expect("count credentials");
        let host_columns = table_columns(&vault.database.connection, "hosts");

        assert_eq!(version, 2);
        assert!(jump_routes_table.is_none());
        assert_eq!(migrated_credentials, 0);
        assert!(host_columns.contains(&"username".to_owned()));
        assert!(host_columns.contains(&"password_ciphertext".to_owned()));
    }

    #[test]
    fn interrupted_v4_migration_preserves_complete_v3_schema() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        downgrade_to_v3_schema(&vault);
        vault
            .database
            .connection
            .execute(
                "
                INSERT INTO hosts(
                    id, display_name, host, port, credential_id, jump_route_id
                )
                VALUES('host-v3', 'V3 Host', 'v3.internal', 22, NULL, NULL)
                ",
                [],
            )
            .expect("insert v3 Host");

        let error = vault
            .database
            .migrate_to_v4(true)
            .expect_err("migration must fail");
        assert!(matches!(error, StorageError::MigrationInterrupted));

        let version: i64 = vault
            .database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        let groups_table: Option<String> = vault
            .database
            .connection
            .query_row(
                "SELECT name FROM sqlite_schema WHERE type='table' AND name='host_groups'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query Group schema");
        let host_columns = table_columns(&vault.database.connection, "hosts");
        let host_count: i64 = vault
            .database
            .connection
            .query_row("SELECT count(*) FROM hosts", [], |row| row.get(0))
            .expect("count v3 Hosts");

        assert_eq!(version, 3);
        assert!(groups_table.is_none());
        assert_eq!(host_count, 1);
        assert_eq!(
            host_columns,
            vec![
                "id",
                "display_name",
                "host",
                "port",
                "credential_id",
                "jump_route_id",
            ]
        );
    }

    #[test]
    fn interrupted_v5_migration_preserves_complete_v4_schema() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        vault
            .create_credential(&password_credential("cred-v4", "v4-secret"))
            .expect("create v4 credential fixture");
        downgrade_to_v4_schema(&vault);

        let error = vault
            .database
            .migrate_to_v5(true)
            .expect_err("migration must fail");
        assert!(matches!(error, StorageError::MigrationInterrupted));

        let version: i64 = vault
            .database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        let credentials_sql: String = vault
            .database
            .connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='table' AND name='credentials'",
                [],
                |row| row.get(0),
            )
            .expect("credentials schema");
        let credential_count: i64 = vault
            .database
            .connection
            .query_row("SELECT count(*) FROM credentials", [], |row| row.get(0))
            .expect("count v4 credentials");
        let legacy_tables: i64 = vault
            .database
            .connection
            .query_row(
                "
                SELECT count(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name LIKE 'legacy_%_v4'
                ",
                [],
                |row| row.get(0),
            )
            .expect("count legacy tables");

        assert_eq!(version, 4);
        assert!(!credentials_sql.contains("system_agent"));
        assert_eq!(credential_count, 1);
        assert_eq!(legacy_tables, 0);
    }

    #[test]
    fn schema_v4_rejects_invalid_override_state_value_pairs() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let vault = LocalVault::create(&root, "123456", test_parameters()).expect("create vault");

        for (id, credential_state, credential_id, jump_route_state) in [
            ("host-set-without-value", SET_STATE, None, INHERIT_STATE),
            (
                "host-inherit-with-value",
                INHERIT_STATE,
                Some("cred-unreachable"),
                INHERIT_STATE,
            ),
            ("host-invalid-state", 99_i64, None, INHERIT_STATE),
            (
                "host-clear-route-with-value",
                INHERIT_STATE,
                None,
                CLEAR_STATE,
            ),
        ] {
            let jump_route_id =
                (id == "host-clear-route-with-value").then_some("route-unreachable");
            let result = vault.database.connection.execute(
                "
                INSERT INTO hosts(
                    id, display_name, host, port, group_id,
                    credential_state, credential_id,
                    jump_route_state, jump_route_id
                )
                VALUES(?1, 'Invalid override', 'invalid.internal', 22, NULL,
                       ?2, ?3, ?4, ?5)
                ",
                params![
                    id,
                    credential_state,
                    credential_id,
                    jump_route_state,
                    jump_route_id,
                ],
            );
            assert!(result.is_err(), "{id} must violate a Schema v4 CHECK");
        }
    }

    #[test]
    fn unlocking_a_v3_vault_migrates_reference_state_without_changing_effective_values() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        let credential = password_credential("cred-v3", "v3-secret");
        vault
            .create_credential(&credential)
            .expect("create v3 credential fixture");
        downgrade_to_v3_schema(&vault);
        vault
            .database
            .connection
            .execute(
                "
                INSERT INTO hosts(
                    id, display_name, host, port, credential_id, jump_route_id
                )
                VALUES('host-v3-set', 'V3 Set', 'set.internal', 22, 'cred-v3', NULL),
                      ('host-v3-empty', 'V3 Empty', 'empty.internal', 22, NULL, NULL)
                ",
                [],
            )
            .expect("insert v3 Hosts");
        drop(vault);

        let migrated = LocalVault::unlock(&root, "123456").expect("unlock and migrate");
        let hosts = migrated.list_hosts().expect("list migrated Hosts");
        let set_host = hosts
            .iter()
            .find(|host| host.id() == "host-v3-set")
            .expect("set Host");
        assert_eq!(
            set_host.credential_override(),
            &Override::Set("cred-v3".to_owned())
        );
        assert_eq!(set_host.credential_id(), Some("cred-v3"));
        assert_eq!(set_host.jump_route_override(), &Override::Inherit);
        let empty_host = hosts
            .iter()
            .find(|host| host.id() == "host-v3-empty")
            .expect("empty Host");
        assert_eq!(empty_host.credential_override(), &Override::Inherit);
        assert_eq!(empty_host.credential_id(), None);
        assert_eq!(empty_host.group_id(), None);

        let version: i64 = migrated
            .database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn unlocking_a_v2_vault_migrates_embedded_host_password_to_credential_reference() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let vault = LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        downgrade_to_v2_schema(&vault);
        insert_legacy_host(
            &vault,
            "legacy.host:2222",
            "Legacy host",
            "legacy.internal",
            2222,
            "legacy-user",
            "legacy-password-must-migrate",
        );
        drop(vault);

        let migrated = LocalVault::unlock(&root, "123456").expect("unlock and migrate");
        let hosts = migrated.list_hosts().expect("list migrated hosts");
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].id(), "legacy.host:2222");
        assert_eq!(hosts[0].display_name(), "Legacy host");
        assert_eq!(hosts[0].host(), "legacy.internal");
        assert_eq!(hosts[0].port(), 2222);
        assert_eq!(hosts[0].group_id(), None);
        assert_eq!(hosts[0].jump_route_id(), None);
        assert_eq!(hosts[0].jump_route_override(), &Override::Inherit);
        let credential_id = hosts[0]
            .credential_id()
            .expect("migrated Credential reference");
        assert!(credential_id.starts_with("cred-"));
        assert_eq!(
            hosts[0].credential_override(),
            &Override::Set(credential_id.to_owned())
        );

        let (username, secret) = migrated
            .resolve_credential(credential_id)
            .expect("resolve migrated Credential")
            .into_parts();
        assert_eq!(username, "legacy-user");
        let CredentialSecret::Password { password } = secret else {
            panic!("expected migrated password credential");
        };
        assert_eq!(password.as_str(), "legacy-password-must-migrate");

        let host_columns = table_columns(&migrated.database.connection, "hosts");
        assert_eq!(
            host_columns,
            vec![
                "id",
                "display_name",
                "host",
                "port",
                "group_id",
                "credential_state",
                "credential_id",
                "jump_route_state",
                "jump_route_id",
            ]
        );
        assert_files_do_not_contain(
            &root,
            &[
                b"Legacy host",
                b"legacy.internal",
                b"legacy-user",
                b"legacy-password-must-migrate",
            ],
        );
    }

    #[test]
    fn unlocking_a_v4_vault_adds_system_agent_credentials_without_changing_references() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let mut vault =
            LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        let credential = password_credential("cred-v4", "v4-secret");
        vault
            .create_credential(&credential)
            .expect("create password credential");
        let group = fixture_group(
            "group-v4",
            "V4 Group",
            None,
            Override::Set("cred-v4".to_owned()),
            Override::Inherit,
        );
        vault.create_group(&group).expect("create v4 Group");
        let host = fixture_host_with_overrides(
            "host-v4",
            "V4 Host",
            "v4.internal",
            Some("group-v4"),
            Override::Inherit,
            Override::Inherit,
        );
        vault.create_host(&host).expect("create v4 Host");
        downgrade_to_v4_schema(&vault);
        drop(vault);

        let mut migrated = LocalVault::unlock(&root, "123456").expect("unlock and migrate");
        let migrated_host = migrated
            .list_hosts()
            .expect("list Hosts")
            .into_iter()
            .find(|host| host.id() == "host-v4")
            .expect("migrated Host");
        assert_eq!(migrated_host.effective_credential_id(), Some("cred-v4"));
        let (_, secret) = migrated
            .resolve_credential("cred-v4")
            .expect("resolve migrated password")
            .into_parts();
        let CredentialSecret::Password { password } = secret else {
            panic!("expected password credential");
        };
        assert_eq!(password.as_str(), "v4-secret");

        let agent =
            system_agent_credential("cred-agent-v5", "SHA256:migrated-vault-agent-selector");
        migrated
            .create_credential(&agent)
            .expect("create system agent after migration");
        let (_, secret) = migrated
            .resolve_credential("cred-agent-v5")
            .expect("resolve system agent after migration")
            .into_parts();
        let CredentialSecret::SystemAgent {
            identity_fingerprint_sha256,
        } = secret
        else {
            panic!("expected system-agent credential");
        };
        assert_eq!(
            identity_fingerprint_sha256.as_str(),
            "SHA256:migrated-vault-agent-selector"
        );

        let version: i64 = migrated
            .database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn unlocking_a_v1_vault_migrates_it_to_v5() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("vault");
        let vault = LocalVault::create(&root, "123456", test_parameters()).expect("create vault");
        downgrade_to_v1_schema(&vault);
        drop(vault);

        let migrated = LocalVault::unlock(&root, "123456").expect("unlock and migrate");
        assert!(
            migrated
                .list_credentials()
                .expect("list migrated credentials")
                .is_empty()
        );
        let version: i64 = migrated
            .database
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_vault_root_is_treated_as_damaged() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("tempdir");
        let real_root = directory.path().join("real-vault");
        let linked_root = directory.path().join("linked-vault");
        fs::create_dir(&real_root).expect("real root");
        symlink(&real_root, &linked_root).expect("symlink");

        assert_eq!(LocalVault::presence(&linked_root), VaultPresence::Damaged);
    }

    fn assert_files_do_not_contain(root: &Path, needles: &[&[u8]]) {
        for entry in fs::read_dir(root).expect("read vault directory") {
            let path = entry.expect("entry").path();
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(&path).expect("read vault file");
            for needle in needles {
                assert!(
                    !bytes.windows(needle.len()).any(|window| window == *needle),
                    "{} leaked plaintext {:?}",
                    path.display(),
                    String::from_utf8_lossy(needle)
                );
            }
        }
    }

    fn downgrade_to_v2_schema(vault: &LocalVault) {
        vault
            .database
            .connection
            .execute_batch(
                "
                DROP TABLE jump_route_steps;
                DROP TABLE hosts;
                DROP TABLE group_overrides;
                DROP TABLE host_groups;
                DROP TABLE jump_routes;
                CREATE TABLE hosts(
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name TEXT NOT NULL,
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                    username TEXT NOT NULL,
                    password_nonce BLOB NOT NULL,
                    password_ciphertext BLOB NOT NULL
                ) WITHOUT ROWID;
                PRAGMA user_version = 2;
                ",
            )
            .expect("downgrade fixture to v2");
    }

    fn downgrade_to_v3_schema(vault: &LocalVault) {
        vault
            .database
            .connection
            .execute_batch(
                "
                DROP TABLE jump_route_steps;
                DROP TABLE hosts;
                DROP TABLE group_overrides;
                DROP TABLE host_groups;

                CREATE TABLE hosts(
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name TEXT NOT NULL,
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                    credential_id TEXT,
                    jump_route_id TEXT,
                    FOREIGN KEY(credential_id)
                        REFERENCES credentials(id) ON DELETE RESTRICT,
                    FOREIGN KEY(jump_route_id)
                        REFERENCES jump_routes(id) ON DELETE RESTRICT
                ) WITHOUT ROWID;

                CREATE TABLE jump_route_steps(
                    route_id TEXT NOT NULL,
                    position INTEGER NOT NULL CHECK(position >= 0),
                    host_id TEXT NOT NULL,
                    PRIMARY KEY(route_id, position),
                    UNIQUE(route_id, host_id),
                    FOREIGN KEY(route_id)
                        REFERENCES jump_routes(id) ON DELETE CASCADE,
                    FOREIGN KEY(host_id)
                        REFERENCES hosts(id) ON DELETE RESTRICT
                ) WITHOUT ROWID;

                CREATE INDEX hosts_credential_id_idx ON hosts(credential_id);
                CREATE INDEX hosts_jump_route_id_idx ON hosts(jump_route_id);
                CREATE INDEX jump_route_steps_host_id_idx ON jump_route_steps(host_id);
                PRAGMA user_version = 3;
                ",
            )
            .expect("downgrade fixture to v3");
    }

    fn downgrade_to_v4_schema(vault: &LocalVault) {
        vault
            .database
            .connection
            .execute_batch(
                "
                DROP INDEX group_overrides_credential_id_idx;
                DROP INDEX group_overrides_jump_route_id_idx;
                DROP INDEX hosts_group_id_idx;
                DROP INDEX hosts_credential_id_idx;
                DROP INDEX hosts_jump_route_id_idx;
                DROP INDEX jump_route_steps_host_id_idx;

                ALTER TABLE jump_route_steps RENAME TO downgrade_jump_route_steps_v5;
                ALTER TABLE hosts RENAME TO downgrade_hosts_v5;
                ALTER TABLE group_overrides RENAME TO downgrade_group_overrides_v5;
                ALTER TABLE credentials RENAME TO downgrade_credentials_v5;

                CREATE TABLE credentials(
                    id TEXT PRIMARY KEY NOT NULL,
                    label TEXT NOT NULL,
                    username TEXT NOT NULL,
                    kind TEXT NOT NULL CHECK(kind IN ('password', 'private_key')),
                    secret_nonce BLOB NOT NULL,
                    secret_ciphertext BLOB NOT NULL,
                    passphrase_nonce BLOB,
                    passphrase_ciphertext BLOB,
                    CHECK(
                        (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
                        OR
                        (passphrase_nonce IS NOT NULL AND passphrase_ciphertext IS NOT NULL)
                    ),
                    CHECK(
                        kind = 'private_key'
                        OR
                        (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
                    )
                ) WITHOUT ROWID;

                CREATE TABLE group_overrides(
                    group_id TEXT PRIMARY KEY NOT NULL,
                    credential_state INTEGER NOT NULL CHECK(credential_state IN (0, 1, 2)),
                    credential_id TEXT,
                    jump_route_state INTEGER NOT NULL CHECK(jump_route_state IN (0, 1, 2)),
                    jump_route_id TEXT,
                    CHECK(
                        (credential_state = 0 AND credential_id IS NULL)
                        OR (credential_state = 1 AND credential_id IS NOT NULL)
                        OR (credential_state = 2 AND credential_id IS NULL)
                    ),
                    CHECK(
                        (jump_route_state = 0 AND jump_route_id IS NULL)
                        OR (jump_route_state = 1 AND jump_route_id IS NOT NULL)
                        OR (jump_route_state = 2 AND jump_route_id IS NULL)
                    ),
                    FOREIGN KEY(group_id)
                        REFERENCES host_groups(id) ON DELETE CASCADE,
                    FOREIGN KEY(credential_id)
                        REFERENCES credentials(id) ON DELETE RESTRICT,
                    FOREIGN KEY(jump_route_id)
                        REFERENCES jump_routes(id) ON DELETE RESTRICT
                ) WITHOUT ROWID;

                CREATE TABLE hosts(
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name TEXT NOT NULL,
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                    group_id TEXT,
                    credential_state INTEGER NOT NULL CHECK(credential_state IN (0, 1, 2)),
                    credential_id TEXT,
                    jump_route_state INTEGER NOT NULL CHECK(jump_route_state IN (0, 1, 2)),
                    jump_route_id TEXT,
                    CHECK(
                        (credential_state = 0 AND credential_id IS NULL)
                        OR (credential_state = 1 AND credential_id IS NOT NULL)
                        OR (credential_state = 2 AND credential_id IS NULL)
                    ),
                    CHECK(
                        (jump_route_state = 0 AND jump_route_id IS NULL)
                        OR (jump_route_state = 1 AND jump_route_id IS NOT NULL)
                        OR (jump_route_state = 2 AND jump_route_id IS NULL)
                    ),
                    FOREIGN KEY(group_id)
                        REFERENCES host_groups(id) ON DELETE RESTRICT,
                    FOREIGN KEY(credential_id)
                        REFERENCES credentials(id) ON DELETE RESTRICT,
                    FOREIGN KEY(jump_route_id)
                        REFERENCES jump_routes(id) ON DELETE RESTRICT
                ) WITHOUT ROWID;

                CREATE TABLE jump_route_steps(
                    route_id TEXT NOT NULL,
                    position INTEGER NOT NULL CHECK(position >= 0),
                    host_id TEXT NOT NULL,
                    PRIMARY KEY(route_id, position),
                    UNIQUE(route_id, host_id),
                    FOREIGN KEY(route_id)
                        REFERENCES jump_routes(id) ON DELETE CASCADE,
                    FOREIGN KEY(host_id)
                        REFERENCES hosts(id) ON DELETE RESTRICT
                ) WITHOUT ROWID;

                INSERT INTO credentials(
                    id, label, username, kind, secret_nonce, secret_ciphertext,
                    passphrase_nonce, passphrase_ciphertext
                )
                SELECT id, label, username, kind, secret_nonce, secret_ciphertext,
                       passphrase_nonce, passphrase_ciphertext
                FROM downgrade_credentials_v5;

                INSERT INTO group_overrides(
                    group_id, credential_state, credential_id,
                    jump_route_state, jump_route_id
                )
                SELECT group_id, credential_state, credential_id,
                       jump_route_state, jump_route_id
                FROM downgrade_group_overrides_v5;

                INSERT INTO hosts(
                    id, display_name, host, port, group_id,
                    credential_state, credential_id,
                    jump_route_state, jump_route_id
                )
                SELECT id, display_name, host, port, group_id,
                       credential_state, credential_id,
                       jump_route_state, jump_route_id
                FROM downgrade_hosts_v5;

                INSERT INTO jump_route_steps(route_id, position, host_id)
                SELECT route_id, position, host_id
                FROM downgrade_jump_route_steps_v5;

                CREATE INDEX group_overrides_credential_id_idx
                    ON group_overrides(credential_id);
                CREATE INDEX group_overrides_jump_route_id_idx
                    ON group_overrides(jump_route_id);
                CREATE INDEX hosts_group_id_idx ON hosts(group_id);
                CREATE INDEX hosts_credential_id_idx ON hosts(credential_id);
                CREATE INDEX hosts_jump_route_id_idx ON hosts(jump_route_id);
                CREATE INDEX jump_route_steps_host_id_idx ON jump_route_steps(host_id);

                DROP TABLE downgrade_jump_route_steps_v5;
                DROP TABLE downgrade_hosts_v5;
                DROP TABLE downgrade_group_overrides_v5;
                DROP TABLE downgrade_credentials_v5;
                PRAGMA user_version = 4;
                ",
            )
            .expect("downgrade fixture to v4");
    }

    fn downgrade_to_v1_schema(vault: &LocalVault) {
        downgrade_to_v2_schema(vault);
        vault
            .database
            .connection
            .execute_batch("DROP TABLE credentials; PRAGMA user_version = 1;")
            .expect("downgrade fixture to v1");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_legacy_host(
        vault: &LocalVault,
        id: &str,
        display_name: &str,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
    ) {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).expect("legacy nonce");
        let cipher = XChaCha20Poly1305::new_from_slice(&vault.database.record_key[..])
            .expect("legacy cipher");
        let aad = legacy_host_record_aad(&vault.database.vault_id, id);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: password.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .expect("encrypt legacy password");
        vault
            .database
            .connection
            .execute(
                "
                INSERT INTO hosts(
                    id, display_name, host, port, username, password_nonce,
                    password_ciphertext
                )
                VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    id,
                    display_name,
                    host,
                    i64::from(port),
                    username,
                    nonce.as_slice(),
                    ciphertext,
                ],
            )
            .expect("insert legacy Host");
    }

    fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("prepare table info");
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query table info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect table columns")
    }
}
