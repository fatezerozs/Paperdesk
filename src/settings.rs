use crate::{
    api::{Api, Error, bad, lock},
    db::{self, Store},
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub revision: u64,
    pub library_status: String,
    pub engine: String,
    pub layout: String,
    pub formula_enhanced: bool,
    pub agent_assist: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            revision: 0,
            library_status: String::new(),
            engine: "local".into(),
            layout: "auto".into(),
            formula_enhanced: true,
            agent_assist: false,
        }
    }
}
pub async fn read(State(s): State<Store>) -> Api<Settings> {
    let conn = lock(&s)?;
    Ok(Json(
        db::get(&conn, "settings", "workspace").unwrap_or_default(),
    ))
}
pub async fn save(State(s): State<Store>, Json(mut value): Json<Settings>) -> Api<Settings> {
    if !["", "pending", "valuable", "discarded"].contains(&value.library_status.as_str())
        || !["local", "paddle", "mathpix", "service"].contains(&value.engine.as_str())
        || !["auto", "single", "columns", "manual"].contains(&value.layout.as_str())
    {
        return Err(bad("设置值无效"));
    }
    let conn = lock(&s)?;
    let old: Settings = db::get(&conn, "settings", "workspace").unwrap_or_default();
    if value.revision != old.revision {
        return Err(Error(
            StatusCode::CONFLICT,
            "设置已在其他窗口修改，请重新打开设置".into(),
        ));
    }
    value.revision += 1;
    db::put(&conn, "settings", "workspace", &value)?;
    Ok(Json(value))
}
