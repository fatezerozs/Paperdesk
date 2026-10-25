//! Lightweight operation telemetry; worker permits remain owned by actual work.
use crate::{
    api::{Api, Error, bad},
    db::Store,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub id: String,
    pub status: String,
    pub phase: String,
    pub completed: usize,
    pub total: Option<usize>,
    pub unit: String,
    pub message: String,
    #[serde(skip)]
    pub started: Option<Instant>,
}
#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<BTreeMap<String, Progress>>>);
#[derive(Clone, Default)]
pub struct Progress(Option<Arc<Mutex<Snapshot>>>);
impl Registry {
    pub fn create(&self) -> Result<Snapshot, Error> {
        let mut records = self.0.lock().map_err(|_| bad("任务状态暂不可用"))?;
        if records.len() >= 128 {
            let oldest = records
                .iter()
                .filter_map(|(id, p)| {
                    p.0.as_ref()
                        .and_then(|p| p.lock().ok())
                        .filter(|p| {
                            ["succeeded", "failed"].contains(&p.status.as_str())
                                || (p.status == "created"
                                    && p.started.is_some_and(|t| t.elapsed().as_secs() > 300))
                        })
                        .map(|p| (id.clone(), p.started))
                })
                .min_by_key(|(_, start)| *start)
                .map(|(id, _)| id);
            if let Some(id) = oldest {
                records.remove(&id);
            } else {
                return Err(Error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "处理任务过多，请等待当前任务结束".into(),
                ));
            }
        }
        let value = Snapshot {
            id: uuid::Uuid::new_v4().to_string(),
            status: "created".into(),
            phase: "等待提交".into(),
            completed: 0,
            total: None,
            unit: String::new(),
            message: String::new(),
            started: Some(Instant::now()),
        };
        records.insert(
            value.id.clone(),
            Progress(Some(Arc::new(Mutex::new(value.clone())))),
        );
        Ok(value)
    }
    pub fn claim(&self, headers: &HeaderMap) -> Result<Progress, Error> {
        let Some(id) = headers
            .get("x-paperdesk-operation")
            .and_then(|v| v.to_str().ok())
        else {
            return Ok(Progress::default());
        };
        let progress = self
            .0
            .lock()
            .map_err(|_| bad("任务状态暂不可用"))?
            .get(id)
            .cloned()
            .ok_or_else(|| bad("处理任务不存在，请重新提交"))?;
        if let Some(p) = &progress.0 {
            let mut p = p.lock().map_err(|_| bad("任务状态暂不可用"))?;
            if p.status != "created" {
                return Err(Error(StatusCode::CONFLICT, "处理任务不能重复提交".into()));
            }
            p.status = "running".into();
            p.phase = "排队等待处理".into();
        }
        Ok(progress)
    }
    pub fn snapshot(&self, id: &str) -> Result<Snapshot, Error> {
        let p = self
            .0
            .lock()
            .map_err(|_| bad("任务状态暂不可用"))?
            .get(id)
            .cloned()
            .ok_or_else(|| Error(StatusCode::NOT_FOUND, "处理记录已过期".into()))?;
        p.0.unwrap()
            .lock()
            .map(|p| p.clone())
            .map_err(|_| bad("任务状态暂不可用"))
    }
}
pub struct CompletionGuard(Progress);
impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if let Some(p) = &self.0.0
            && let Ok(mut p) = p.lock()
            && p.status == "running"
        {
            p.status = "failed".into();
            p.message = "处理请求已中断，后台结果尚未确认；请检查来源记录后重试".into();
        }
    }
}
impl Progress {
    pub fn guard(&self) -> CompletionGuard {
        CompletionGuard(self.clone())
    }

    pub fn set(&self, phase: &str, completed: usize, total: Option<usize>, unit: &str) {
        if let Some(p) = &self.0
            && let Ok(mut p) = p.lock()
        {
            p.phase = phase.into();
            p.completed = completed;
            p.total = total;
            p.unit = unit.into();
        }
    }
    pub fn finish(&self, error: Option<String>) {
        if let Some(p) = &self.0
            && let Ok(mut p) = p.lock()
        {
            p.status = if error.is_some() {
                "failed"
            } else {
                "succeeded"
            }
            .into();
            p.message = error.unwrap_or_default();
            if p.status == "succeeded" {
                p.phase = "处理完成".into();
                if let Some(total) = p.total {
                    p.completed = total;
                }
            } else {
                p.phase = "处理失败".into();
            }
        }
    }
}
pub async fn create(State(s): State<Store>) -> Api<Snapshot> {
    Ok(Json(s.operations.create()?))
}
pub async fn read(State(s): State<Store>, Path(id): Path<String>) -> Api<Snapshot> {
    Ok(Json(s.operations.snapshot(&id)?))
}
