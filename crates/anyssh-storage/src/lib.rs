#![forbid(unsafe_code)]

use std::{
    fmt::Write as _,
    fs::{self, File},
    path::{Path, PathBuf},
    time::Duration,
};

use anyssh_vault::{
    BootstrapDocument, DerivedVaultKeys, PinKdfParameters, UnlockedVault, VaultError,
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use thiserror::Error;
use zeroize::Zeroizing;

pub const BOOTSTRAP_FILE_NAME: &str = "vault.bootstrap.json";
pub const DATABASE_FILE_NAME: &str = "vault.db";

const SCHEMA_VERSION: i64 = 1;
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;
const MAX_ID_BYTES: usize = 256;
const MAX_LABEL_BYTES: usize = 4096;
const MAX_HOST_BYTES: usize = 4096;
const MAX_USERNAME_BYTES: usize = 4096;
const MAX_PASSWORD_BYTES: usize = 64 * 1024;

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

pub struct HostRecord {
    pub id: String,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    password: Zeroizing<String>,
}

impl HostRecord {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        username: impl Into<String>,
        password: Zeroizing<String>,
    ) -> Result<Self, StorageError> {
        let record = Self {
            id: id.into(),
            display_name: display_name.into(),
            host: host.into(),
            port,
            username: username.into(),
            password,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    fn validate(&self) -> Result<(), StorageError> {
        if self.id.is_empty()
            || self.id.len() > MAX_ID_BYTES
            || self.display_name.is_empty()
            || self.display_name.len() > MAX_LABEL_BYTES
            || self.host.is_empty()
            || self.host.len() > MAX_HOST_BYTES
            || self.port == 0
            || self.username.is_empty()
            || self.username.len() > MAX_USERNAME_BYTES
            || self.password.len() > MAX_PASSWORD_BYTES
        {
            return Err(StorageError::InvalidHost);
        }
        Ok(())
    }
}

pub struct LocalVault {
    root: PathBuf,
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
        Ok(Self {
            root: root.to_owned(),
            unlocked,
            database,
        })
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
        Ok(Self {
            root: root.to_owned(),
            unlocked,
            database,
        })
    }

    pub fn vault_id(&self) -> &str {
        self.unlocked.vault_id()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn cipher_version(&self) -> &str {
        &self.database.cipher_version
    }

    pub fn save_host(&mut self, record: &HostRecord) -> Result<(), StorageError> {
        self.database.save_host(record)
    }

    pub fn load_host(&self, id: &str) -> Result<Option<HostRecord>, StorageError> {
        self.database.load_host(id)
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
        migrate_schema(&mut database.connection, false)?;
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
        let database = Self::configure(connection, keys)?;
        ensure_schema_version(&database.connection)?;

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

    fn save_host(&mut self, record: &HostRecord) -> Result<(), StorageError> {
        record.validate()?;
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| StorageError::RecordIntegrity)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.record_key[..])
            .map_err(|_| StorageError::RecordIntegrity)?;
        let aad = record_aad(&self.vault_id, &record.id);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: record.password.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StorageError::RecordIntegrity)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "
            INSERT INTO hosts(
                id, display_name, host, port, username, password_nonce,
                password_ciphertext
            )
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                host = excluded.host,
                port = excluded.port,
                username = excluded.username,
                password_nonce = excluded.password_nonce,
                password_ciphertext = excluded.password_ciphertext
            ",
            params![
                &record.id,
                &record.display_name,
                &record.host,
                i64::from(record.port),
                &record.username,
                nonce.as_slice(),
                ciphertext,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn load_host(&self, id: &str) -> Result<Option<HostRecord>, StorageError> {
        if id.is_empty() || id.len() > MAX_ID_BYTES {
            return Err(StorageError::InvalidHost);
        }

        let stored = self
            .connection
            .query_row(
                "
                SELECT display_name, host, port, username, password_nonce,
                       password_ciphertext
                FROM hosts
                WHERE id = ?1
                ",
                [id],
                |row| {
                    Ok(StoredHost {
                        display_name: row.get(0)?,
                        host: row.get(1)?,
                        port: row.get(2)?,
                        username: row.get(3)?,
                        password_nonce: row.get(4)?,
                        password_ciphertext: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let Some(stored) = stored else {
            return Ok(None);
        };

        let nonce: [u8; NONCE_BYTES] = stored
            .password_nonce
            .try_into()
            .map_err(|_| StorageError::RecordIntegrity)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.record_key[..])
            .map_err(|_| StorageError::RecordIntegrity)?;
        let aad = record_aad(&self.vault_id, id);
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &stored.password_ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StorageError::RecordIntegrity)?;
        let password = String::from_utf8(plaintext).map_err(|_| StorageError::RecordIntegrity)?;

        let port = u16::try_from(stored.port).map_err(|_| StorageError::RecordIntegrity)?;
        HostRecord::new(
            id,
            stored.display_name,
            stored.host,
            port,
            stored.username,
            Zeroizing::new(password),
        )
        .map(Some)
    }
}

struct StoredHost {
    display_name: String,
    host: String,
    port: i64,
    username: String,
    password_nonce: Vec<u8>,
    password_ciphertext: Vec<u8>,
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

fn migrate_schema(
    connection: &mut Connection,
    simulate_interruption: bool,
) -> Result<(), StorageError> {
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(StorageError::Sql)?;
    match current_version {
        SCHEMA_VERSION => Ok(()),
        0 => migrate_to_v1(connection, simulate_interruption),
        version => Err(StorageError::UnsupportedSchema(version)),
    }
}

fn ensure_schema_version(connection: &Connection) -> Result<(), StorageError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(StorageError::Sql)?;
    if version == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(StorageError::UnsupportedSchema(version))
    }
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
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn record_aad(vault_id: &str, host_id: &str) -> String {
    format!("anyssh/record/v1|{vault_id}|host|{host_id}|password")
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

    fn fixture_host() -> HostRecord {
        HostRecord::new(
            "host-fixture",
            "Secret production host",
            "secret.internal.example",
            2222,
            "fixture-user",
            Zeroizing::new("fixture-password-should-never-leak".to_owned()),
        )
        .expect("host")
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
        let host = fixture_host();
        vault.save_host(&host).expect("save host");

        assert_files_do_not_contain(
            &root,
            &[
                host.display_name.as_bytes(),
                host.host.as_bytes(),
                host.username.as_bytes(),
                host.password().as_bytes(),
                b"SQLite format 3",
            ],
        );

        drop(vault);
        let reopened = LocalVault::unlock(&root, "123456").expect("unlock");
        let loaded = reopened
            .load_host("host-fixture")
            .expect("load")
            .expect("host exists");
        assert_eq!(loaded.display_name, host.display_name);
        assert_eq!(loaded.host, host.host);
        assert_eq!(loaded.port, host.port);
        assert_eq!(loaded.username, host.username);
        assert_eq!(loaded.password(), host.password());
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
        let error = migrate_schema(&mut connection, true).expect_err("migration must fail");
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
}
