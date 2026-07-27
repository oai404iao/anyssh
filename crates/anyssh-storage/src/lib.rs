#![forbid(unsafe_code)]

mod actor;
mod credential;
mod entity_id;
mod host;
mod jump_route;

pub use actor::{
    DEFAULT_DATABASE_COMMAND_QUEUE_CAPACITY, DatabaseActorConfig, DatabaseActorError,
    DatabaseActorHandle, DatabaseActorStartError, VaultState, VaultStatus,
};
pub use credential::{CredentialKind, CredentialSecret, CredentialSummary, ResolvedCredential};
pub use host::HostSummary;
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
    host::validate_host_id,
    jump_route::validate_jump_route_id,
};

pub const BOOTSTRAP_FILE_NAME: &str = "vault.bootstrap.json";
pub const DATABASE_FILE_NAME: &str = "vault.db";

const SCHEMA_VERSION: i64 = 3;
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
    #[error("host is referenced by a Jump Route")]
    HostInUse,
    #[error("Jump Route record is invalid")]
    InvalidJumpRoute,
    #[error("Jump Route was not found")]
    JumpRouteNotFound,
    #[error("Jump Route is referenced by a Host")]
    JumpRouteInUse,
    #[error("Jump Route references form a cycle")]
    JumpRouteCycle,
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

    fn create_host(&mut self, record: &HostSummary) -> Result<HostSummary, StorageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_host_references(&transaction, record)?;
        transaction.execute(
            "
            INSERT INTO hosts(
                id, display_name, host, port, credential_id, jump_route_id
            )
            VALUES(?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                record.id(),
                record.display_name(),
                record.host(),
                i64::from(record.port()),
                record.credential_id(),
                record.jump_route_id(),
            ],
        )?;
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        Ok(record.clone())
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
                credential_id = ?5,
                jump_route_id = ?6
            WHERE id = ?1
            ",
            params![
                record.id(),
                record.display_name(),
                record.host(),
                i64::from(record.port()),
                record.credential_id(),
                record.jump_route_id(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::HostNotFound);
        }
        validate_jump_route_graph(&transaction)?;
        transaction.commit()?;
        Ok(record.clone())
    }

    fn list_hosts(&self) -> Result<Vec<HostSummary>, StorageError> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, display_name, host, port, credential_id, jump_route_id
            FROM hosts
            ORDER BY display_name COLLATE NOCASE, id
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredHostSummary {
                id: row.get(0)?,
                display_name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                credential_id: row.get(4)?,
                jump_route_id: row.get(5)?,
            })
        })?;

        rows.map(|row| {
            let row = row?;
            HostSummary::new(
                row.id,
                row.display_name,
                row.host,
                u16::try_from(row.port).map_err(|_| StorageError::RecordIntegrity)?,
                row.credential_id,
                row.jump_route_id,
            )
        })
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
            "SELECT EXISTS(SELECT 1 FROM hosts WHERE jump_route_id = ?1)",
            [id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(StorageError::JumpRouteInUse);
        }
        let changed = transaction.execute("DELETE FROM jump_routes WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(changed == 1)
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
            "SELECT EXISTS(SELECT 1 FROM hosts WHERE credential_id = ?1)",
            [id],
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
                self.migrate_to_v3(false)
            }
            2 => self.migrate_to_v3(false),
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
    credential_id: Option<String>,
    jump_route_id: Option<String>,
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

fn validate_host_references(
    transaction: &Transaction<'_>,
    host: &HostSummary,
) -> Result<(), StorageError> {
    if let Some(credential_id) = host.credential_id()
        && !transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM credentials WHERE id = ?1)",
            [credential_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(StorageError::CredentialNotFound);
    }
    if let Some(route_id) = host.jump_route_id()
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
            SELECT id, jump_route_id
            FROM hosts
            WHERE jump_route_id IS NOT NULL
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut graph = HashMap::with_capacity(host_routes.len());
    for (host_id, route_id) in host_routes {
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
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
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

        let password_summary = vault
            .create_credential(&password)
            .expect("create password credential");
        let private_key_summary = vault
            .create_credential(&private_key)
            .expect("create private-key credential");
        assert_eq!(password_summary.kind(), CredentialKind::Password);
        assert_eq!(private_key_summary.kind(), CredentialKind::PrivateKey);

        let summaries = vault.list_credentials().expect("list credentials");
        assert_eq!(summaries.len(), 2);
        let summary_debug = format!("{summaries:?}");
        assert!(!summary_debug.contains("credential-password-secret"));
        assert!(!summary_debug.contains("fixture-private-key"));

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
        assert_eq!(hosts[0].jump_route_id(), None);
        let credential_id = hosts[0]
            .credential_id()
            .expect("migrated Credential reference");
        assert!(credential_id.starts_with("cred-"));

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
                "credential_id",
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
    fn unlocking_a_v1_vault_migrates_it_to_v3() {
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
