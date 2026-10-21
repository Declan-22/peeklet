//! Local persistence: a small SQLite store for shell state. Two tables:
//! `tabs` (the open documents) and `settings` (theme + dock layout).
//! Lives under the platform data dir.
//!
//! The old manuscript-era store is gone: it kept every tab, panel and
//! measure in one opaque key/value blob. Each table here owns one thing,
//! so new file-preview state (recent files, per-file-type docks) has an
//! obvious home.

use std::path::PathBuf;

/// One persisted tab: a title plus the file backing it, if any.
#[derive(Clone)]
pub struct TabRecord {
    pub title: String,
    pub path: Option<String>,
}

pub struct Store {
    conn: rusqlite::Connection,
}

impl Store {
    pub fn open_default() -> Self {
        let dir = dirs::data_dir()
            .map(|d| d.join("peekview"))
            .unwrap_or_else(|| PathBuf::from("."));
        let _ = std::fs::create_dir_all(&dir);
        let conn = rusqlite::Connection::open(dir.join("peekview.db")).expect("open peekview db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tabs (
                 position INTEGER PRIMARY KEY,
                 title    TEXT NOT NULL,
                 path     TEXT
             );
             CREATE TABLE IF NOT EXISTS settings (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )
        .expect("init peekview db");
        Self { conn }
    }

    /// Open tabs, in display order.
    pub fn tabs(&self) -> Vec<TabRecord> {
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT title, path FROM tabs ORDER BY position")
        else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |row| {
            Ok(TabRecord {
                title: row.get(0)?,
                path: row.get(1)?,
            })
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Replace the stored tab list with `tabs` (display order).
    pub fn set_tabs(&self, tabs: &[TabRecord]) {
        let _ = self.conn.execute("DELETE FROM tabs", []);
        let Ok(mut stmt) = self.conn.prepare(
            "INSERT INTO tabs (position, title, path) VALUES (?1, ?2, ?3)",
        ) else {
            return;
        };
        for (position, tab) in tabs.iter().enumerate() {
            let _ = stmt.execute(rusqlite::params![position as i64, tab.title, tab.path]);
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    pub fn set(&self, key: &str, value: &str) {
        let _ = self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        );
    }
}
