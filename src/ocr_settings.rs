//! Workspace OCR credentials are write-only through the API and excluded from exports.
use crate::{
    api::{Api, bad, lock},
    db::{self, Store},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub type Values = BTreeMap<String, String>;
const KEYS: &[&str] = &[
    "PAPERDESK_PADDLE_URL",
    "PAPERDESK_PADDLE_TOKEN",
    "PAPERDESK_MATHPIX_APP_ID",
    "PAPERDESK_MATHPIX_APP_KEY",
    "PAPERDESK_OCR_URL",
    "PAPERDESK_OCR_TOKEN",
];
pub fn values(s: &Store) -> Values {
    s.db.lock()
        .ok()
        .and_then(|c| db::get(&c, "settings", "ocr_credentials").ok())
        .unwrap_or_default()
}
pub fn value(config: &Values, key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| config.get(key).cloned().filter(|s| !s.trim().is_empty()))
}
fn redacted(config: &Values) -> Value {
    let fields: Vec<Value> = KEYS.iter().map(|key| {
        let resolved = value(config, key);
        let secret = !key.ends_with("_URL");
        json!({"key":key,"value":if secret {String::new()} else {resolved.clone().unwrap_or_default()},"configured":resolved.is_some(),"environment":std::env::var(key).is_ok_and(|v| !v.trim().is_empty())})
    }).collect();
    json!({"fields":fields})
}
pub async fn read(State(s): State<Store>) -> Json<Value> {
    Json(redacted(&values(&s)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub values: Values,
    #[serde(default)]
    pub clear: Vec<String>,
}
pub async fn save(State(s): State<Store>, Json(update): Json<Update>) -> Api<Value> {
    if update
        .values
        .keys()
        .chain(update.clear.iter())
        .any(|k| !KEYS.contains(&k.as_str()))
    {
        return Err(bad("未知的 OCR 配置字段"));
    }
    for (key, raw) in &update.values {
        let v = raw.trim();
        if v.len() > 4096 || v.chars().any(|c| c.is_control()) {
            return Err(bad("配置字段过长或包含控制字符"));
        }
        if key.ends_with("_URL") && !v.is_empty() {
            let url = reqwest::Url::parse(v).map_err(|_| bad("请输入完整的 OCR 服务地址"))?;
            let local = matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            );
            if url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || !["http", "https"].contains(&url.scheme())
                || (!local && url.scheme() != "https")
            {
                return Err(bad(
                    "外部服务须使用 HTTPS；地址不能包含凭据、查询参数或片段，请使用独立令牌字段",
                ));
            }
        }
    }
    let conn = lock(&s)?;
    let mut config: Values = db::get(&conn, "settings", "ocr_credentials").unwrap_or_default();
    for key in update.clear {
        config.remove(&key);
    }
    for (key, v) in update.values {
        if !v.trim().is_empty() {
            config.insert(key, v.trim().to_owned());
        }
    }
    db::put(&conn, "settings", "ocr_credentials", &config)?;
    Ok(Json(redacted(&config)))
}
