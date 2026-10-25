use crate::model::*;
use rusqlite::{Connection, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Store {
    pub db: Arc<Mutex<Connection>>,
    pub dir: PathBuf,
    pub token: Arc<String>,
    pub operations: crate::operations::Registry,
    pub import_gate: Arc<tokio::sync::Semaphore>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(dir.join("papers"))?;
        std::fs::create_dir_all(dir.join("fonts"))?;
        std::fs::create_dir_all(dir.join("imports"))?;
        let conn = Connection::open(dir.join("paperdesk.sqlite3"))?;
        conn.execute_batch(include_str!("../migrations/001.sql"))?;
        let token = std::env::var("PAPERDESK_AGENT_TOKEN")
            .ok()
            .filter(|s| s.len() >= 24)
            .or_else(|| {
                std::fs::read_to_string(dir.join("agent-token.txt"))
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| s.len() >= 24)
            })
            .unwrap_or_else(|| {
                format!(
                    "{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                )
            });
        std::fs::write(dir.join("agent-token.txt"), &token)?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            dir: dir.to_path_buf(),
            token: Arc::new(token),
            operations: Default::default(),
            import_gate: Arc::new(tokio::sync::Semaphore::new(1)),
        })
    }
}
pub fn now(conn: &Connection) -> String {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
        r.get(0)
    })
    .unwrap()
}
pub fn all<T: DeserializeOwned>(conn: &Connection, table: &str) -> Result<Vec<T>, String> {
    // Table names are internal constants, never request data.
    let mut stmt = conn
        .prepare(&format!("SELECT body FROM {table} ORDER BY rowid DESC"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
pub fn get<T: DeserializeOwned>(conn: &Connection, table: &str, id: &str) -> Result<T, String> {
    let body: String = conn
        .query_row(
            &format!("SELECT body FROM {table} WHERE id=?1"),
            [id],
            |r| r.get(0),
        )
        .map_err(|_| "记录不存在".to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}
pub fn put<T: Serialize>(
    conn: &Connection,
    table: &str,
    id: &str,
    value: &T,
) -> Result<(), String> {
    conn.execute(&format!("INSERT INTO {table}(id,body) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body"), params![id, serde_json::to_string(value).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn questions(conn: &Connection, filter: &Filter) -> Result<Vec<Question>, String> {
    let mut questions = all::<Question>(conn, "questions")?;
    if filter.paper_id.is_some() {
        questions.reverse();
    }
    Ok(questions
        .into_iter()
        .filter(|q| filter.matches(q))
        .collect())
}
