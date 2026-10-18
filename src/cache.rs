//! Saved translations, so the same text is never sent to the model twice.

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;

pub struct Cache {
    conn: Mutex<Connection>,
}

impl Cache {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS translation (
                 key TEXT PRIMARY KEY,
                 src_lang TEXT NOT NULL,
                 dst_lang TEXT NOT NULL,
                 source TEXT NOT NULL,
                 output TEXT NOT NULL,
                 model TEXT NOT NULL,
                 created INTEGER NOT NULL
             );",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn key(model: &str, from: &str, to: &str, text: &str) -> String {
        let mut h = Sha256::new();
        for part in [crate::PROMPT_VERSION, model, from, to, text] {
            h.update(part.as_bytes());
            h.update([0u8]);
        }
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT output FROM translation WHERE key = ?1", params![key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn put(&self, key: &str, model: &str, from: &str, to: &str, source: &str, output: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO translation (key, src_lang, dst_lang, source, output, model, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![key, from, to, source, output, model, now],
        );
    }
}
