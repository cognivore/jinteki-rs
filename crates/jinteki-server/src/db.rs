//! SQLite storage for native-mode accounts, decks and game history
//! (ACCOUNTS-AND-DECKS.md §5). One file, WAL mode, versioned migrations
//! applied at boot before the listener binds.
//!
//! Concurrency: one connection behind a `tokio::sync::Mutex`; handlers lock,
//! run their few statements synchronously, unlock. At this deployment's
//! scale (dozens of users, one box) that is not a bottleneck worth
//! architecture (§5.1).

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::sync::{Mutex, MutexGuard};

/// Numbered migrations, applied in order inside one transaction each.
/// Version 1 is the normative DDL from ACCOUNTS-AND-DECKS.md §5.2; version 2
/// adds user-built Eternal decks (the `/api/decks` contract — decks stored
/// in the catalog's card-id vocabulary, `{"<id>": count}`).
const MIGRATIONS: &[(i64, &str)] = &[(
    1,
    r#"
CREATE TABLE users (
  id                TEXT PRIMARY KEY,      -- 128-bit base64url token
  kind              TEXT NOT NULL CHECK (kind IN ('anon','claimed','merged','suspended')),
  display_name      TEXT NOT NULL,         -- <=20 code points, no '://', no '</'
  email             TEXT UNIQUE,           -- lowercase-normalized; NULL for anon
  email_verified_at TEXT,
  merged_into       TEXT REFERENCES users(id),
  merged_at         TEXT,
  created_at        TEXT NOT NULL,
  last_seen_at      TEXT NOT NULL          -- coarse (daily), for the anon GC
);

CREATE TABLE sessions (
  id               TEXT PRIMARY KEY,        -- the cookie value
  user_id          TEXT NOT NULL REFERENCES users(id),
  created_at       TEXT NOT NULL,
  expires_at       TEXT NOT NULL,           -- +14d claimed, +400d anon; sliding
  last_activity_at TEXT NOT NULL            -- touch at most 1/hour
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

-- Ephemeral. NEVER folded into users (the Cubehall 2026-06-11 lesson:
-- no GC criterion may key off a field an auth attempt mutates).
-- Token stored hashed (improvement over both reference projects).
CREATE TABLE claims (
  token_hash  TEXT PRIMARY KEY,             -- hex sha256 of the link token
  session_id  TEXT NOT NULL,                -- the requesting session
  user_id     TEXT NOT NULL REFERENCES users(id),
  email       TEXT NOT NULL,                -- normalized target address
  created_at  TEXT NOT NULL,
  expires_at  TEXT NOT NULL                 -- created + 30 min
);
CREATE INDEX idx_claims_user ON claims(user_id);

CREATE TABLE merges (
  id             TEXT PRIMARY KEY,
  source_user_id TEXT NOT NULL,
  target_user_id TEXT NOT NULL,
  at             TEXT NOT NULL,
  via            TEXT NOT NULL DEFAULT 'claim'
);

CREATE TABLE decks (
  id          TEXT PRIMARY KEY,
  owner_id    TEXT NOT NULL REFERENCES users(id),
  name        TEXT NOT NULL,                -- <=120 chars
  side        TEXT NOT NULL CHECK (side IN ('corp','runner')),
  identity_title TEXT NOT NULL,             -- canonical title
  format      TEXT NOT NULL DEFAULT 'standard',
  cards_json  TEXT NOT NULL,                -- [{"title":...,"code":...,"qty":n}]
  notes       TEXT NOT NULL DEFAULT '',
  source_json TEXT,                         -- {"kind":"nrdb",...} | {"kind":"fork",...}
  published_at TEXT,                        -- NULL = private; set = in the library
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
CREATE INDEX idx_decks_owner   ON decks(owner_id, updated_at DESC);
CREATE INDEX idx_decks_library ON decks(published_at DESC) WHERE published_at IS NOT NULL;

CREATE TABLE games (
  id          TEXT PRIMARY KEY,             -- the local-game registry token
  owner_id    TEXT NOT NULL REFERENCES users(id),
  side        TEXT NOT NULL,
  deck_id     TEXT REFERENCES decks(id),    -- NULL: built-in demo decks
  seed        INTEGER,
  started_at  TEXT NOT NULL,
  finished_at TEXT,
  winner      TEXT,                         -- 'corp'|'runner'|NULL
  reason      TEXT
);
CREATE INDEX idx_games_owner ON games(owner_id, started_at DESC);

CREATE TABLE audit (
  id      TEXT PRIMARY KEY,
  at      TEXT NOT NULL,
  user_id TEXT,
  action  TEXT NOT NULL,
  detail  TEXT             -- JSON; NEVER contains raw tokens or full emails
);
"#,
), (
    2,
    r#"
CREATE TABLE eternal_decks (
  id          TEXT PRIMARY KEY,               -- public key is 'user-'||id
  owner_id    TEXT NOT NULL REFERENCES users(id),
  name        TEXT NOT NULL,                  -- <=120 chars
  identity    TEXT NOT NULL,                  -- catalog card id (NSG v2 slug)
  cards_json  TEXT NOT NULL,                  -- {"<id>": count, ...} verbatim
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
CREATE INDEX idx_eternal_decks_owner ON eternal_decks(owner_id, updated_at DESC);
"#,
)];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open (creating if absent) the database at `path`, enable WAL +
    /// foreign keys, and apply pending migrations.
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// In-memory database for tests: same migrations, no file.
    pub fn open_in_memory() -> rusqlite::Result<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Db> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
               version    INTEGER PRIMARY KEY,
               applied_at TEXT NOT NULL
             );",
        )?;
        for (version, sql) in MIGRATIONS {
            let done: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |r| r.get(0),
            )?;
            if !done {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(sql)?;
                tx.execute(
                    "INSERT INTO schema_migrations (version, applied_at)
                     VALUES (?1, datetime('now'))",
                    [version],
                )?;
                tx.commit()?;
            }
        }
        Ok(Db { conn: Mutex::new(conn) })
    }

    /// Lock the connection. Callers keep the guard for the duration of one
    /// logical operation (a transaction where multiple statements must land
    /// together — e.g. the claim-verify merge).
    pub async fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().await
    }

    /// Blocking lock for the rare sync contexts (tests).
    pub fn blocking_lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.blocking_lock()
    }
}

// ── tokens & hashing ───────────────────────────────────────────────────────

const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// 16 bytes of OS entropy, base64url without padding — 128-bit, the
/// draftroom `newToken` construction (`store.go:63-69`). Used for user ids,
/// session ids, deck ids and magic-link tokens alike.
pub fn new_token() -> String {
    use rand::TryRngCore;
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("OS entropy");
    base64url(&bytes)
}

fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        let keep = match chunk.len() {
            1 => 2,
            2 => 3,
            _ => 4,
        };
        for &c in chars.iter().take(keep) {
            out.push(B64URL[c as usize] as char);
        }
    }
    out
}

/// Hex SHA-256; how magic-link tokens are stored at rest (SYS-A-3) and how
/// emails are referenced in audit rows (never in full).
pub fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let digest = h.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Append an audit row. `detail` must never contain raw tokens or full
/// email addresses (ACCOUNTS-AND-DECKS.md §12.1).
pub fn audit(conn: &Connection, user_id: Option<&str>, action: &str, detail: &serde_json::Value) {
    let _ = conn.execute(
        "INSERT INTO audit (id, at, user_id, action, detail)
         VALUES (?1, datetime('now'), ?2, ?3, ?4)",
        rusqlite::params![new_token(), user_id, action, detail.to_string()],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once_and_schema_exists() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.blocking_lock();
        for table in [
            "users",
            "sessions",
            "claims",
            "merges",
            "decks",
            "games",
            "audit",
            "eternal_decks",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "table {table} exists");
        }
        let v: i64 = conn
            .query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
    }

    #[test]
    fn tokens_are_22_chars_urlsafe_and_unique() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 22); // 16 bytes -> ceil(16*4/3) unpadded
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
