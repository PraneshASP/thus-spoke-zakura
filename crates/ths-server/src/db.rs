use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};
use bip39::Mnemonic;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use uuid::Uuid;
use zcash_keys::{
    address::Address,
    keys::{Era, UnifiedAddressRequest, UnifiedFullViewingKey, UnifiedSpendingKey},
};
use zcash_protocol::local_consensus::LocalNetwork;

pub const ZATOSHIS_PER_ZEC: u64 = 100_000_000;
pub const USER_ACCOUNT_COUNT: u8 = 5;
pub const TREASURY_ACCOUNT_ID: u8 = 6;

#[derive(Debug, thiserror::Error)]
#[error("idempotency key was already used for a different payment")]
pub struct IdempotencyConflict;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasuryCursor {
    pub receiver: String,
    pub height: u32,
    pub block_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Account {
    pub id: u8,
    pub name: String,
    pub unified_address: String,
    pub transparent_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unified_full_viewing_key: Option<String>,
    pub transparent_zatoshi: u64,
    pub ironwood_zatoshi: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevelopmentAccountSecret {
    pub id: u8,
    pub unified_address: String,
    pub unified_spending_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevelopmentSecrets {
    pub mnemonic: String,
    pub accounts: Vec<DevelopmentAccountSecret>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub id: String,
    pub kind: String,
    pub from_account: Option<u8>,
    pub to_account: Option<u8>,
    pub to_address: Option<String>,
    pub source_pool: String,
    pub destination_pool: String,
    pub amount_zatoshi: u64,
    pub txid: String,
    pub block_hash: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTransaction {
    pub raw_transaction: Vec<u8>,
    pub expiry_height: u64,
}

/// Where a send is going: a development account, or any other Regtest address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipient<'a> {
    Account(u8),
    Address(&'a str),
}

#[derive(Clone)]
pub struct Store(Arc<Mutex<Connection>>);

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    pub fn initialize(&self) -> Result<()> {
        let db = self.0.lock().unwrap();
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, unified_address TEXT NOT NULL,
                transparent_address TEXT NOT NULL, transparent_zatoshi INTEGER NOT NULL DEFAULT 0,
                ironwood_zatoshi INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS idempotency (
                key TEXT PRIMARY KEY, activity_id TEXT NOT NULL, memo TEXT
            );
            CREATE TABLE IF NOT EXISTS prepared_payments (
                activity_id TEXT PRIMARY KEY, raw_transaction BLOB NOT NULL,
                expiry_height INTEGER NOT NULL
            );
        "#,
        )?;
        db.execute_batch(ACTIVITY_TABLE)?;
        migrate_activity_recipients(&db)?;
        let has_memo = db
            .prepare("PRAGMA table_info(idempotency)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .any(|column| column == "memo");
        if !has_memo {
            db.execute("ALTER TABLE idempotency ADD COLUMN memo TEXT", [])?;
        }
        let stored_seed = db
            .query_row("SELECT value FROM metadata WHERE key='seed'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        let seed = if let Some(seed) = stored_seed {
            hex::decode(seed).context("invalid wallet seed")?
        } else {
            let entropy = [0u8; 32];
            let mnemonic = Mnemonic::from_entropy(&entropy)
                .context("encoding wallet entropy as a BIP-39 mnemonic")?;
            let seed = mnemonic.to_seed("");
            db.execute(
                "INSERT INTO metadata(key, value) VALUES('seed', ?1)",
                [hex::encode(seed)],
            )?;
            db.execute(
                "INSERT INTO metadata(key, value) VALUES('mnemonic', ?1)",
                [mnemonic.to_string()],
            )?;
            seed.to_vec()
        };
        for id in 1u8..=TREASURY_ACCOUNT_ID {
            let (ua, taddr) = derived_addresses(&seed, id)?;
            db.execute("INSERT OR IGNORE INTO accounts(id,name,unified_address,transparent_address) VALUES(?1,?2,?3,?4)", params![id, format!("Account {id}"), ua, taddr])?;
        }
        Ok(())
    }

    pub fn accounts(&self) -> Result<Vec<Account>> {
        let mut accounts = {
            let db = self.0.lock().unwrap();
            let mut query = db.prepare("SELECT id,name,unified_address,transparent_address,transparent_zatoshi,ironwood_zatoshi FROM accounts ORDER BY id")?;
            query
                .query_map([], row_account)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let seed = hex::decode(self.seed()?).context("invalid wallet seed")?;
        let network = local_network();
        for account in &mut accounts {
            if (1..=USER_ACCOUNT_COUNT).contains(&account.id) {
                account.unified_full_viewing_key =
                    Some(derived_full_viewing_key(&seed, account.id)?.encode(&network));
            }
        }
        Ok(accounts)
    }

    #[cfg(test)]
    pub fn user_accounts(&self) -> Result<Vec<Account>> {
        Ok(self
            .accounts()?
            .into_iter()
            .filter(|account| account.id <= USER_ACCOUNT_COUNT)
            .collect())
    }

    pub fn account(&self, id: u8) -> Result<Account> {
        self.0.lock().unwrap().query_row("SELECT id,name,unified_address,transparent_address,transparent_zatoshi,ironwood_zatoshi FROM accounts WHERE id=?1", [id], row_account).with_context(|| format!("account {id} does not exist"))
    }

    pub fn activities(&self, limit: u32) -> Result<Vec<Activity>> {
        let db = self.0.lock().unwrap();
        let mut query = db.prepare("SELECT id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at FROM activity WHERE txid != '' ORDER BY rowid DESC LIMIT ?1")?;
        Ok(query
            .query_map([limit.min(100)], row_activity)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn unconfirmed_activities(&self) -> Result<Vec<Activity>> {
        let db = self.0.lock().unwrap();
        let mut query = db.prepare("SELECT id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at FROM activity WHERE txid!='' AND status!='confirmed' ORDER BY rowid ASC")?;
        Ok(query
            .query_map([], row_activity)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn activity_for_key(&self, key: &str) -> Result<Option<Activity>> {
        let db = self.0.lock().unwrap();
        activity_for_key(&db, key)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn claim_transfer(
        &self,
        from: u8,
        to: Recipient<'_>,
        source_pool: &str,
        destination_pool: &str,
        amount: u64,
        key: &str,
        memo: Option<&str>,
    ) -> Result<Activity> {
        validate_pool(source_pool)?;
        validate_pool(destination_pool)?;
        if amount == 0 {
            bail!("amount must be greater than zero");
        }
        let mut db = self.0.lock().unwrap();
        if let Some(activity) = activity_for_key(&db, key)? {
            ensure_same_payment(
                &activity,
                "send",
                Some(from),
                to,
                source_pool,
                destination_pool,
                amount,
            )?;
            let saved_memo: Option<String> =
                db.query_row("SELECT memo FROM idempotency WHERE key=?1", [key], |row| {
                    row.get(0)
                })?;
            if saved_memo.as_deref() != memo {
                return Err(IdempotencyConflict.into());
            }
            return Ok(activity);
        }
        let accounts = match to {
            Recipient::Account(id) => vec![from, id],
            Recipient::Address(_) => vec![from],
        };
        for id in accounts {
            if !db.query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1)",
                [id],
                |row| row.get::<_, bool>(0),
            )? {
                bail!("account {id} does not exist");
            }
        }
        let tx = db.transaction()?;
        let activity = new_activity(
            "send",
            Some(from),
            to,
            source_pool,
            destination_pool,
            amount,
        );
        insert_activity(&tx, &activity, key, memo)?;
        tx.commit()?;
        Ok(activity)
    }

    pub fn claim_faucet(&self, to: u8, pool: &str, amount: u64, key: &str) -> Result<Activity> {
        validate_pool(pool)?;
        if amount == 0 {
            bail!("amount must be greater than zero");
        }
        let mut db = self.0.lock().unwrap();
        if let Some(activity) = activity_for_key(&db, key)? {
            ensure_same_payment(
                &activity,
                "faucet",
                None,
                Recipient::Account(to),
                "ironwood",
                pool,
                amount,
            )?;
            return Ok(activity);
        }
        if !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1)",
            [to],
            |row| row.get::<_, bool>(0),
        )? {
            bail!("account {to} does not exist");
        }
        let tx = db.transaction()?;
        let activity = new_activity(
            "faucet",
            None,
            Recipient::Account(to),
            "ironwood",
            pool,
            amount,
        );
        insert_activity(&tx, &activity, key, None)?;
        tx.commit()?;
        Ok(activity)
    }

    pub fn discard_preparing(&self, id: &str) -> Result<()> {
        let mut db = self.0.lock().unwrap();
        let tx = db.transaction()?;
        if tx.execute(
            "DELETE FROM activity WHERE id=?1 AND status='preparing'",
            [id],
        )? == 1
        {
            tx.execute("DELETE FROM idempotency WHERE activity_id=?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn record_prepared(
        &self,
        id: &str,
        txid: &str,
        raw_transaction: &[u8],
        expiry_height: u64,
    ) -> Result<Activity> {
        let mut db = self.0.lock().unwrap();
        let tx = db.transaction()?;
        let updated = tx.execute(
            "UPDATE activity SET txid=?1,status='prepared' WHERE id=?2 AND status='preparing'",
            params![txid, id],
        )?;
        let activity = tx.query_row(
            "SELECT id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at FROM activity WHERE id=?1",
            [id],
            row_activity,
        )?;
        if updated == 0
            && (activity.txid != txid
                || !matches!(
                    activity.status.as_str(),
                    "prepared" | "broadcast" | "confirmed"
                ))
        {
            bail!("payment {id} is not waiting for this prepared transaction");
        }
        tx.execute(
            "INSERT INTO prepared_payments(activity_id,raw_transaction,expiry_height) VALUES(?1,?2,?3) ON CONFLICT(activity_id) DO NOTHING",
            params![id, raw_transaction, expiry_height],
        )?;
        tx.commit()?;
        Ok(activity)
    }

    pub fn prepared_transaction(&self, id: &str) -> Result<PreparedTransaction> {
        self.0
            .lock()
            .unwrap()
            .query_row(
                "SELECT raw_transaction,expiry_height FROM prepared_payments WHERE activity_id=?1",
                [id],
                |row| {
                    Ok(PreparedTransaction {
                        raw_transaction: row.get(0)?,
                        expiry_height: row.get(1)?,
                    })
                },
            )
            .with_context(|| format!("prepared transaction {id} is missing"))
    }

    pub fn reset_for_retry(&self, id: &str, txid: &str) -> Result<Activity> {
        let mut db = self.0.lock().unwrap();
        let tx = db.transaction()?;
        let updated = tx.execute(
            "UPDATE activity SET txid='',block_hash=NULL,status='preparing' WHERE id=?1 AND txid=?2 AND status IN ('prepared','broadcast')",
            params![id, txid],
        )?;
        if updated == 1 {
            tx.execute("DELETE FROM prepared_payments WHERE activity_id=?1", [id])?;
        }
        let activity = tx.query_row(
            "SELECT id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at FROM activity WHERE id=?1",
            [id],
            row_activity,
        )?;
        tx.commit()?;
        Ok(activity)
    }

    pub fn mark_broadcast(&self, id: &str, txid: &str) -> Result<Activity> {
        let db = self.0.lock().unwrap();
        let updated = db.execute(
            "UPDATE activity SET status='broadcast' WHERE id=?1 AND txid=?2 AND status IN ('prepared','broadcast')",
            params![id, txid],
        )?;
        let activity = db.query_row(
            "SELECT id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at FROM activity WHERE id=?1",
            [id],
            row_activity,
        )?;
        if updated == 0 && (activity.txid != txid || activity.status != "confirmed") {
            bail!("payment {id} has no matching prepared transaction");
        }
        Ok(activity)
    }

    pub fn confirm(&self, id: &str, txid: &str, block_hash: &str) -> Result<Activity> {
        if block_hash.is_empty() {
            bail!("block hash is required to confirm activity");
        }
        let db = self.0.lock().unwrap();
        db.execute(
            "UPDATE activity SET status='confirmed',block_hash=?1 WHERE id=?2 AND txid=?3",
            params![block_hash, id, txid],
        )?;
        db.query_row("SELECT id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at FROM activity WHERE id=?1", [id], row_activity).map_err(Into::into)
    }

    pub fn seed(&self) -> Result<String> {
        Ok(self.0.lock().unwrap().query_row(
            "SELECT value FROM metadata WHERE key='seed'",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn mnemonic(&self) -> Result<String> {
        self.0
            .lock()
            .unwrap()
            .query_row("SELECT value FROM metadata WHERE key='mnemonic'", [], |r| {
                r.get(0)
            })
            .context("wallet mnemonic is missing; restart this disposable environment")
    }

    pub fn development_secrets(&self) -> Result<DevelopmentSecrets> {
        let seed = hex::decode(self.seed()?).context("invalid wallet seed")?;
        let mnemonic = self.mnemonic()?;
        let network = local_network();
        let mut accounts = Vec::with_capacity(usize::from(USER_ACCOUNT_COUNT));
        for id in 1..=USER_ACCOUNT_COUNT {
            let account_index = zip32::AccountId::try_from(u32::from(id - 1))
                .map_err(|_| anyhow::anyhow!("invalid ZIP-32 account {id}"))?;
            let spending_key = UnifiedSpendingKey::from_seed(&network, &seed, account_index)
                .map_err(|error| anyhow::anyhow!("deriving account {id}: {error:?}"))?;
            accounts.push(DevelopmentAccountSecret {
                id,
                unified_address: self.account(id)?.unified_address,
                unified_spending_key_hex: hex::encode(spending_key.to_bytes(Era::Orchard)),
            });
        }
        Ok(DevelopmentSecrets { mnemonic, accounts })
    }
}

const ACTIVITY_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS activity (
        id TEXT PRIMARY KEY, kind TEXT NOT NULL, from_account INTEGER, to_account INTEGER,
        to_address TEXT, source_pool TEXT NOT NULL, destination_pool TEXT NOT NULL,
        amount_zatoshi INTEGER NOT NULL, txid TEXT NOT NULL, block_hash TEXT, status TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

/// Databases created before address sends have `to_account NOT NULL` and no
/// `to_address`. SQLite cannot relax a constraint in place, so rebuild the
/// table, keeping rowid order because activity is listed by rowid.
fn migrate_activity_recipients(db: &Connection) -> Result<()> {
    let migrated: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('activity') WHERE name='to_address')",
        [],
        |r| r.get(0),
    )?;
    if migrated {
        return Ok(());
    }
    let tx = db.unchecked_transaction()?;
    tx.execute_batch("ALTER TABLE activity RENAME TO activity_before_recipients;")?;
    tx.execute_batch(ACTIVITY_TABLE)?;
    tx.execute_batch(
        "INSERT INTO activity(id,kind,from_account,to_account,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at)
         SELECT id,kind,from_account,to_account,source_pool,destination_pool,amount_zatoshi,txid,block_hash,status,created_at
         FROM activity_before_recipients ORDER BY rowid;
         DROP TABLE activity_before_recipients;",
    )?;
    tx.commit()?;
    Ok(())
}

fn insert_activity(db: &Connection, a: &Activity, key: &str, memo: Option<&str>) -> Result<()> {
    db.execute("INSERT INTO activity(id,kind,from_account,to_account,to_address,source_pool,destination_pool,amount_zatoshi,txid,status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![a.id,a.kind,a.from_account,a.to_account,a.to_address,a.source_pool,a.destination_pool,a.amount_zatoshi,a.txid,a.status])?;
    db.execute(
        "INSERT INTO idempotency(key,activity_id,memo) VALUES(?1,?2,?3)",
        params![key, a.id, memo],
    )?;
    Ok(())
}

fn activity_for_key(db: &Connection, key: &str) -> Result<Option<Activity>> {
    db.query_row(
        "SELECT a.id,a.kind,a.from_account,a.to_account,a.to_address,a.source_pool,a.destination_pool,a.amount_zatoshi,a.txid,a.block_hash,a.status,a.created_at FROM activity a JOIN idempotency i ON i.activity_id=a.id WHERE i.key=?1",
        [key],
        row_activity,
    )
    .optional()
    .map_err(Into::into)
}
fn new_activity(
    kind: &str,
    from: Option<u8>,
    to: Recipient<'_>,
    source: &str,
    destination: &str,
    amount: u64,
) -> Activity {
    let id = Uuid::new_v4().to_string();
    Activity {
        id,
        kind: kind.into(),
        from_account: from,
        to_account: match to {
            Recipient::Account(id) => Some(id),
            Recipient::Address(_) => None,
        },
        to_address: match to {
            Recipient::Account(_) => None,
            Recipient::Address(address) => Some(address.to_owned()),
        },
        source_pool: source.into(),
        destination_pool: destination.into(),
        amount_zatoshi: amount,
        txid: String::new(),
        block_hash: None,
        status: "preparing".into(),
        created_at: String::new(),
    }
}
fn validate_pool(pool: &str) -> Result<()> {
    if matches!(pool, "transparent" | "ironwood") {
        Ok(())
    } else {
        bail!("pool must be transparent or ironwood")
    }
}

#[allow(clippy::too_many_arguments)]
fn ensure_same_payment(
    activity: &Activity,
    kind: &str,
    from: Option<u8>,
    to: Recipient<'_>,
    source_pool: &str,
    destination_pool: &str,
    amount: u64,
) -> Result<()> {
    let (to_account, to_address) = match to {
        Recipient::Account(id) => (Some(id), None),
        Recipient::Address(address) => (None, Some(address)),
    };
    if activity.kind != kind
        || activity.from_account != from
        || activity.to_account != to_account
        || activity.to_address.as_deref() != to_address
        || activity.source_pool != source_pool
        || activity.destination_pool != destination_pool
        || activity.amount_zatoshi != amount
    {
        return Err(IdempotencyConflict.into());
    }
    Ok(())
}
fn row_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        unified_address: row.get(2)?,
        transparent_address: row.get(3)?,
        unified_full_viewing_key: None,
        transparent_zatoshi: row.get(4)?,
        ironwood_zatoshi: row.get(5)?,
    })
}
fn row_activity(row: &rusqlite::Row<'_>) -> rusqlite::Result<Activity> {
    Ok(Activity {
        id: row.get(0)?,
        kind: row.get(1)?,
        from_account: row.get(2)?,
        to_account: row.get(3)?,
        to_address: row.get(4)?,
        source_pool: row.get(5)?,
        destination_pool: row.get(6)?,
        amount_zatoshi: row.get(7)?,
        txid: row.get(8)?,
        block_hash: row.get(9)?,
        status: row.get(10)?,
        created_at: row.get(11)?,
    })
}
fn derived_full_viewing_key(seed: &[u8], id: u8) -> Result<UnifiedFullViewingKey> {
    let index = id.checked_sub(1).context("invalid account id")?;
    let account = zip32::AccountId::try_from(u32::from(index))
        .map_err(|_| anyhow::anyhow!("invalid ZIP-32 account {id}"))?;
    let usk = UnifiedSpendingKey::from_seed(&local_network(), seed, account)
        .map_err(|error| anyhow::anyhow!("deriving account {id}: {error:?}"))?;
    Ok(usk.to_unified_full_viewing_key())
}

fn derived_addresses(seed: &[u8], id: u8) -> Result<(String, String)> {
    let network = local_network();
    let (ua, _) = derived_full_viewing_key(seed, id)?
        .default_address(UnifiedAddressRequest::AllAvailableKeys)
        .map_err(|error| anyhow::anyhow!("deriving account {id} address: {error:?}"))?;
    let transparent = ua
        .transparent()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("account {id} has no transparent receiver"))?;
    Ok((
        ua.encode(&network),
        Address::Transparent(transparent).encode(&network),
    ))
}

fn local_network() -> LocalNetwork {
    crate::wallet::regtest_network()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    #[test]
    fn creates_user_accounts_and_hidden_treasury() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        assert_eq!(store.accounts().unwrap().len(), 6);
        assert_eq!(store.user_accounts().unwrap().len(), 5);
        assert_eq!(
            store.account(TREASURY_ACCOUNT_ID).unwrap().name,
            "Account 6"
        );
        assert!(
            store
                .account(1)
                .unwrap()
                .transparent_address
                .starts_with("tm")
        );
        let first = store
            .claim_faucet(2, "ironwood", ZATOSHIS_PER_ZEC, "same")
            .unwrap();
        let second = store
            .claim_faucet(2, "ironwood", ZATOSHIS_PER_ZEC, "same")
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(store.account(2).unwrap().ironwood_zatoshi, 0);
    }
    #[test]
    fn fresh_stores_derive_the_same_development_accounts() {
        let first = Store::open(":memory:").unwrap();
        first.initialize().unwrap();
        let second = Store::open(":memory:").unwrap();
        second.initialize().unwrap();
        assert_eq!(first.accounts().unwrap(), second.accounts().unwrap());
        assert_eq!(
            first.mnemonic().unwrap(),
            format!("{}art", "abandon ".repeat(23))
        );
        assert_eq!(
            first.account(1).unwrap().transparent_address,
            "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd"
        );
    }
    #[test]
    fn restores_the_hidden_treasury_for_existing_stores() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        store
            .0
            .lock()
            .unwrap()
            .execute("DELETE FROM accounts WHERE id=?1", [TREASURY_ACCOUNT_ID])
            .unwrap();
        store.initialize().unwrap();
        assert_eq!(store.accounts().unwrap().len(), 6);
        assert_eq!(store.user_accounts().unwrap().len(), 5);
    }
    #[test]
    fn existing_store_exports_user_viewing_keys_without_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.db");
        let seed = [7u8; 32];
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(
                "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE accounts (
                     id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                     unified_address TEXT NOT NULL, transparent_address TEXT NOT NULL,
                     transparent_zatoshi INTEGER NOT NULL DEFAULT 0,
                     ironwood_zatoshi INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .unwrap();
            db.execute(
                "INSERT INTO metadata(key,value) VALUES('seed',?1)",
                [hex::encode(seed)],
            )
            .unwrap();
            for id in 1..=TREASURY_ACCOUNT_ID {
                let (ua, taddr) = derived_addresses(&seed, id).unwrap();
                db.execute(
                    "INSERT INTO accounts
                     (id,name,unified_address,transparent_address,transparent_zatoshi,ironwood_zatoshi)
                     VALUES(?1,?2,?3,?4,123,456)",
                    params![id, format!("Account {id}"), ua, taddr],
                )
                .unwrap();
            }
        }

        let store = Store::open(&path).unwrap();
        let users = store.user_accounts().unwrap();
        assert_eq!(
            users.iter().map(|a| a.id).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        let network = local_network();
        let mut distinct = std::collections::HashSet::new();
        for account in &users {
            let json = serde_json::to_value(account).unwrap();
            let encoded = json["unified_full_viewing_key"]
                .as_str()
                .expect("user projection must contain a viewing key");
            let viewing = zcash_keys::keys::UnifiedFullViewingKey::decode(&network, encoded)
                .unwrap_or_else(|_| panic!("user viewing key must decode for regtest"));
            let (ua, _) = viewing
                .default_address(UnifiedAddressRequest::AllAvailableKeys)
                .unwrap();
            assert!(ua.encode(&network) == account.unified_address);
            let transparent = ua.transparent().cloned().expect("transparent receiver");
            assert!(
                Address::Transparent(transparent).encode(&network) == account.transparent_address
            );
            let index = zip32::AccountId::try_from(u32::from(account.id - 1)).unwrap();
            let expected = UnifiedSpendingKey::from_seed(&network, &seed, index)
                .unwrap()
                .to_unified_full_viewing_key()
                .encode(&network);
            assert!(encoded == expected);
            assert!(distinct.insert(encoded.to_owned()));
            assert_eq!(account.transparent_zatoshi, 123);
            assert_eq!(account.ironwood_zatoshi, 456);
        }

        let all = store.accounts().unwrap();
        assert_eq!(all.len(), 6);
        let treasury = all.iter().find(|a| a.id == TREASURY_ACCOUNT_ID).unwrap();
        assert!(
            serde_json::to_value(treasury)
                .unwrap()
                .get("unified_full_viewing_key")
                .is_none()
        );
        assert!(
            serde_json::to_value(store.account(TREASURY_ACCOUNT_ID).unwrap())
                .unwrap()
                .get("unified_full_viewing_key")
                .is_none()
        );
        store.initialize().unwrap();
        assert!(users == store.user_accounts().unwrap());
        {
            let db = store.0.lock().unwrap();
            let columns = db
                .prepare("PRAGMA table_info(accounts)")
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(
                columns,
                [
                    "id",
                    "name",
                    "unified_address",
                    "transparent_address",
                    "transparent_zatoshi",
                    "ironwood_zatoshi",
                ]
            );
            let metadata_keys = db
                .prepare("SELECT key FROM metadata ORDER BY key")
                .unwrap()
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(metadata_keys, ["seed"]);
        }
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert!(users == reopened.user_accounts().unwrap());
    }
    #[test]
    fn records_real_transfer_without_mutating_balances() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let activity = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "send",
                None,
            )
            .unwrap();
        let activity = store
            .record_prepared(&activity.id, "real-txid", b"raw transaction", 140)
            .unwrap();
        let activity = store.mark_broadcast(&activity.id, &activity.txid).unwrap();
        assert_eq!(activity.txid, "real-txid");
        assert_eq!(store.account(1).unwrap().transparent_zatoshi, 0);
    }

    #[test]
    fn unconfirmed_activities_skip_unprepared_and_confirmed_rows() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                11_000,
                "preparing-key",
                None,
            )
            .unwrap();
        let pending = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "pending-key",
                None,
            )
            .unwrap();
        let pending = store
            .record_prepared(&pending.id, "txid-pending", b"pending", 140)
            .unwrap();
        let pending = store.mark_broadcast(&pending.id, &pending.txid).unwrap();
        let mined = store
            .claim_transfer(
                1,
                Recipient::Account(3),
                "ironwood",
                "ironwood",
                13_000,
                "mined-key",
                None,
            )
            .unwrap();
        let mined = store
            .record_prepared(&mined.id, "txid-mined", b"mined", 140)
            .unwrap();
        let mined = store.mark_broadcast(&mined.id, &mined.txid).unwrap();
        store
            .confirm(&mined.id, &mined.txid, &"c".repeat(64))
            .unwrap();

        let open = store.unconfirmed_activities().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, pending.id);
        assert_eq!(open[0].status, "broadcast");
        assert_eq!(open[0].txid, "txid-pending");
    }

    #[test]
    fn confirm_rejects_an_empty_block_hash() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let pending = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "empty-hash",
                None,
            )
            .unwrap();
        let pending = store
            .record_prepared(&pending.id, "txid-empty", b"raw", 140)
            .unwrap();
        let pending = store.mark_broadcast(&pending.id, &pending.txid).unwrap();

        assert!(store.confirm(&pending.id, &pending.txid, "").is_err());
        let again = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "empty-hash",
                None,
            )
            .unwrap();
        assert_eq!(again.id, pending.id);
        assert_eq!(again.status, "broadcast");
        assert_eq!(again.block_hash, None);
    }

    #[test]
    fn rejects_reusing_a_key_for_a_different_payment() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();

        let error = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                13_000,
                "same",
                None,
            )
            .unwrap_err();

        assert!(error.to_string().contains("different payment"));
        assert!(error.downcast_ref::<IdempotencyConflict>().is_some());

        let error = store
            .claim_faucet(2, "ironwood", 12_000, "same")
            .unwrap_err();
        assert!(error.downcast_ref::<IdempotencyConflict>().is_some());
    }

    #[test]
    fn memo_is_part_of_the_claimed_payment() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let first = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "memo-key",
                Some("rent"),
            )
            .unwrap();
        assert_eq!(
            store
                .claim_transfer(
                    1,
                    Recipient::Account(2),
                    "ironwood",
                    "ironwood",
                    12_000,
                    "memo-key",
                    Some("rent")
                )
                .unwrap()
                .id,
            first.id
        );
        for memo in [None, Some(""), Some("gift")] {
            let error = store
                .claim_transfer(
                    1,
                    Recipient::Account(2),
                    "ironwood",
                    "ironwood",
                    12_000,
                    "memo-key",
                    memo,
                )
                .unwrap_err();
            assert!(error.downcast_ref::<IdempotencyConflict>().is_some());
        }
    }

    #[test]
    fn existing_idempotency_tables_gain_a_memo_column() {
        let store = Store::open(":memory:").unwrap();
        store
            .0
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE idempotency (key TEXT PRIMARY KEY, activity_id TEXT NOT NULL)",
            )
            .unwrap();
        store.initialize().unwrap();
        store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "memo-key",
                Some("rent"),
            )
            .unwrap();
        assert_eq!(
            store
                .0
                .lock()
                .unwrap()
                .query_row(
                    "SELECT memo FROM idempotency WHERE key='memo-key'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "rent"
        );
    }

    #[test]
    fn concurrent_claims_create_one_payment() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let threads = (0..2)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .claim_transfer(
                            1,
                            Recipient::Account(2),
                            "ironwood",
                            "ironwood",
                            12_000,
                            "same",
                            None,
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let claims = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(claims[0].id, claims[1].id);
        assert_eq!(claims[0].status, "preparing");
        assert!(store.activities(10).unwrap().is_empty());
    }

    #[test]
    fn discarding_a_failed_prepare_removes_its_claim() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let failed = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();

        store.discard_preparing(&failed.id).unwrap();

        assert!(store.activity_for_key("same").unwrap().is_none());
        let retry = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        assert_ne!(retry.id, failed.id);
    }

    #[test]
    fn prepared_payment_can_be_reset_after_expiry() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let claim = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        store
            .record_prepared(&claim.id, "expired-txid", b"signed transaction", 140)
            .unwrap();

        let retry = store.reset_for_retry(&claim.id, "expired-txid").unwrap();

        assert_eq!(retry.status, "preparing");
        assert!(retry.txid.is_empty());
        assert!(store.prepared_transaction(&claim.id).is_err());
        assert!(store.activities(10).unwrap().is_empty());
    }

    #[test]
    fn recording_the_same_prepared_transaction_converges() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let claim = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        let first = store
            .record_prepared(&claim.id, "real-txid", b"signed transaction", 140)
            .unwrap();

        let second = store
            .record_prepared(&claim.id, "real-txid", b"signed transaction", 140)
            .unwrap();

        assert_eq!(second.id, first.id);
        assert_eq!(second.txid, first.txid);
        assert_eq!(second.status, "prepared");
    }

    #[test]
    fn recording_prepared_bytes_backfills_a_legacy_broadcast() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let claim = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        let prepared = store
            .record_prepared(&claim.id, "real-txid", b"signed transaction", 140)
            .unwrap();
        let broadcast = store.mark_broadcast(&prepared.id, &prepared.txid).unwrap();
        store
            .0
            .lock()
            .unwrap()
            .execute("DELETE FROM prepared_payments", [])
            .unwrap();

        store
            .record_prepared(
                &broadcast.id,
                &broadcast.txid,
                b"recovered transaction",
                140,
            )
            .unwrap();

        assert_eq!(
            store
                .prepared_transaction(&broadcast.id)
                .unwrap()
                .raw_transaction,
            b"recovered transaction"
        );
    }

    #[test]
    fn stale_retry_does_not_reset_a_replacement_transaction() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let claim = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        store
            .record_prepared(&claim.id, "old-txid", b"old transaction", 140)
            .unwrap();
        store.reset_for_retry(&claim.id, "old-txid").unwrap();
        store
            .record_prepared(&claim.id, "new-txid", b"new transaction", 180)
            .unwrap();

        let current = store.reset_for_retry(&claim.id, "old-txid").unwrap();

        assert_eq!(current.txid, "new-txid");
        assert_eq!(current.status, "prepared");
    }

    #[test]
    fn broadcast_reconciliation_preserves_a_concurrent_confirmation() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let claim = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        store
            .record_prepared(&claim.id, "real-txid", b"transaction", 140)
            .unwrap();
        store
            .confirm(&claim.id, "real-txid", "mined-block")
            .unwrap();

        let current = store.mark_broadcast(&claim.id, "real-txid").unwrap();

        assert_eq!(current.status, "confirmed");
        assert_eq!(current.block_hash.as_deref(), Some("mined-block"));
    }

    #[test]
    fn prepared_transaction_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.db");
        let store = Store::open(&path).unwrap();
        store.initialize().unwrap();
        let claim = store
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();
        let id = claim.id.clone();
        store
            .record_prepared(&id, "real-txid", b"raw transaction", 140)
            .unwrap();
        drop(store);

        let reopened = Store::open(path).unwrap();
        reopened.initialize().unwrap();
        let recovered = reopened
            .claim_transfer(
                1,
                Recipient::Account(2),
                "ironwood",
                "ironwood",
                12_000,
                "same",
                None,
            )
            .unwrap();

        assert_eq!(recovered.txid, "real-txid");
        assert_eq!(recovered.status, "prepared");
        let prepared = reopened.prepared_transaction(&id).unwrap();
        assert_eq!(prepared.raw_transaction, b"raw transaction");
        assert_eq!(prepared.expiry_height, 140);
    }

    #[test]
    fn records_a_send_to_an_external_address() {
        let store = Store::open(":memory:").unwrap();
        store.initialize().unwrap();
        let activity = store
            .claim_transfer(
                1,
                Recipient::Address("tmExternal"),
                "ironwood",
                "transparent",
                12_000,
                "external-key",
                None,
            )
            .unwrap();
        assert_eq!(activity.to_account, None);
        assert_eq!(activity.to_address.as_deref(), Some("tmExternal"));

        let replayed = store
            .claim_transfer(
                1,
                Recipient::Address("tmExternal"),
                "ironwood",
                "transparent",
                12_000,
                "external-key",
                None,
            )
            .unwrap();
        assert_eq!(replayed.id, activity.id);

        // Reusing the key for a different recipient is a different payment.
        let conflict = store
            .claim_transfer(
                1,
                Recipient::Address("tmOther"),
                "ironwood",
                "transparent",
                12_000,
                "external-key",
                None,
            )
            .unwrap_err();
        assert!(conflict.downcast_ref::<IdempotencyConflict>().is_some());
    }

    #[test]
    fn migrates_activity_created_before_address_sends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.db");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(
                "CREATE TABLE activity (
                     id TEXT PRIMARY KEY, kind TEXT NOT NULL, from_account INTEGER,
                     to_account INTEGER NOT NULL, source_pool TEXT NOT NULL,
                     destination_pool TEXT NOT NULL, amount_zatoshi INTEGER NOT NULL,
                     txid TEXT NOT NULL, block_hash TEXT, status TEXT NOT NULL,
                     created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );
                 INSERT INTO activity(id,kind,from_account,to_account,source_pool,destination_pool,amount_zatoshi,txid,status)
                 VALUES('older','send',1,2,'ironwood','ironwood',1,'txid-older','confirmed'),
                       ('newer','send',1,3,'ironwood','ironwood',2,'txid-newer','broadcast');",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        store.initialize().unwrap();
        let activity = store.activities(10).unwrap();
        assert_eq!(
            activity.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            ["newer", "older"]
        );
        assert_eq!(activity[0].to_account, Some(3));
        assert_eq!(activity[0].to_address, None);

        store
            .claim_transfer(
                1,
                Recipient::Address("tmExternal"),
                "ironwood",
                "transparent",
                3,
                "after-migration",
                None,
            )
            .unwrap();
        // Initializing again must keep rows written after the migration.
        store.initialize().unwrap();
        let claimed = store.activity_for_key("after-migration").unwrap().unwrap();
        assert_eq!(claimed.to_address.as_deref(), Some("tmExternal"));
        assert_eq!(store.activities(10).unwrap().len(), 2);
    }
}
