use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn init() -> Result<Self> {
        // Use the user's home directory / .vakh to ensure it never triggers a reload
        let mut path = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
        
        path.push("VAKH");
        if let Err(e) = std::fs::create_dir_all(&path) {
            eprintln!("Warning: Could not create database directory: {}", e);
        }
        path.push("vakh_history.db");
        
        println!("Database located at: {:?}", path);
        let conn = Connection::open(path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dictation_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                transcribed_text TEXT NOT NULL,
                target_app TEXT
            )",
            [],
        )?;

        // Auto-cleaning protocol: Delete logs older than 30 days
        let _ = conn.execute(
            "DELETE FROM dictation_logs WHERE timestamp < datetime('now', '-30 days')",
            [],
        );

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn log_dictation(&self, text: &str, app_name: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT INTO dictation_logs (transcribed_text, target_app) VALUES (?1, ?2)",
            params![text, app_name],
        )?;
        Ok(())
    }
}
