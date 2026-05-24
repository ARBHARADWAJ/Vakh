use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;
use serde::Serialize;


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

    pub fn get_dictation_logs(&self) -> Result<Vec<LogEntry>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, datetime(timestamp, 'localtime'), transcribed_text, target_app 
             FROM dictation_logs 
             ORDER BY id DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LogEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                transcribed_text: row.get(2)?,
                target_app: row.get(3)?,
            })
        })?;
        
        let mut logs = Vec::new();
        for r in rows {
            if let Ok(entry) = r {
                logs.push(entry);
            }
        }
        Ok(logs)
    }

    pub fn delete_log(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM dictation_logs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_logs(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM dictation_logs", [])?;
        Ok(())
    }

    pub fn get_stats(&self) -> Result<AppStats> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        
        let total_sessions: usize = conn.query_row(
            "SELECT COUNT(*) FROM dictation_logs",
            [],
            |r| r.get(0)
        )?;
        
        let mut stmt = conn.prepare("SELECT transcribed_text FROM dictation_logs")?;
        let texts = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut total_words = 0;
        for t in texts {
            if let Ok(text) = t {
                total_words += text.split_whitespace().count();
            }
        }
        
        let mut stmt = conn.prepare(
            "SELECT target_app, COUNT(*) as cnt 
             FROM dictation_logs 
             WHERE target_app IS NOT NULL AND target_app != '' 
             GROUP BY target_app 
             ORDER BY cnt DESC 
             LIMIT 5"
        )?;
        let rows = stmt.query_map([], |row| {
            let app: String = row.get(0)?;
            let count: usize = row.get(1)?;
            Ok((app, count))
        })?;
        
        let mut top_apps = Vec::new();
        for r in rows {
            if let Ok(item) = r {
                top_apps.push(item);
            }
        }
        
        Ok(AppStats {
            total_words,
            total_sessions,
            top_apps,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub id: i64,
    pub timestamp: String,
    pub transcribed_text: String,
    pub target_app: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AppStats {
    pub total_words: usize,
    pub total_sessions: usize,
    pub top_apps: Vec<(String, usize)>,
}

