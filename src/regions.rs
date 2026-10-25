//! Non-destructive, revision-checked editing of a single question's source crop.
use crate::{
    api::{Api, Error, bad, lock},
    db::{self, Store},
    importer::ImportPlan,
    model::*,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    revision: u64,
    origin_index: usize,
    rect: RegionRect,
}
pub async fn update(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<Input>,
) -> Api<Question> {
    let (origin, page) = {
        let conn = lock(&s)?;
        let q: Question = db::get(&conn, "questions", &id)?;
        if q.revision != input.revision {
            return Err(Error(
                StatusCode::CONFLICT,
                "题目已修改，请重新打开范围编辑".into(),
            ));
        }
        if q.parent_id.is_some() {
            return Err(bad("变式请调整其原题的识别范围"));
        }
        let origin = q
            .origins
            .get(input.origin_index)
            .cloned()
            .ok_or_else(|| bad("来源区域不存在"))?;
        let plan: ImportPlan = db::get(&conn, "imports", &origin.paper_id)?;
        let page = plan
            .pages
            .iter()
            .find(|p| p.page == origin.page)
            .cloned()
            .ok_or_else(|| bad("此来源没有可视化原页"))?;
        if !input.rect.valid(page.width, page.height) {
            return Err(bad("选框超出原页或尺寸过小"));
        }
        (origin, page)
    };
    let permit = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        s.import_gate.clone().acquire_owned(),
    )
    .await
    .map_err(|_| bad("正在处理其他图片，请稍后保存；当前选框仍保留"))?
    .map_err(|_| bad("处理服务已关闭"))?;
    let dir = s.dir.join("imports").join(&origin.paper_id);
    let crop_id = uuid::Uuid::new_v4().to_string();
    let path = dir.join(format!("{crop_id}.png"));
    let output = path.clone();
    let rect = input.rect;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let image = crate::importer::decode(
            &std::fs::read(dir.join(format!("{}.png", page.page))).map_err(|_| "原页读取失败")?,
        )?;
        if image.width() != page.width || image.height() != page.height {
            return Err("原页尺寸已变化，请重新载入".to_string());
        }
        image
            .crop_imm(rect.x, rect.y, rect.width, rect.height)
            .save_with_format(output, image::ImageFormat::Png)
            .map_err(|_| "裁切保存失败".to_string())
    })
    .await
    .map_err(|_| bad("裁切进程失败，原范围未修改"))??;
    let result = (|| -> Result<Question, Error> {
        let mut conn = lock(&s)?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut q: Question = db::get(&tx, "questions", &id)?;
        if q.revision != input.revision || q.origins.get(input.origin_index) != Some(&origin) {
            return Err(Error(
                StatusCode::CONFLICT,
                "保存期间题目已修改，请重新打开范围编辑".into(),
            ));
        }
        db::put(
            &tx,
            "question_region_history",
            &format!("{}:{}", q.id, q.revision),
            &q,
        )?;
        q.origins[input.origin_index].rect = Some(input.rect);
        q.origins[input.origin_index].crop_id = Some(crop_id);
        q.revision += 1;
        q.verified = false;
        q.status = "pending".into();
        if let Some(recognition) = &mut q.recognition {
            recognition.status = "pending".into();
            recognition.error.clear();
        }
        for mut job in db::all::<Job>(&tx, "jobs")?.into_iter().filter(|j| {
            j.question_id == q.id && ["queued", "running", "review"].contains(&j.status.as_str())
        }) {
            job.status = "rejected".into();
            db::put(&tx, "jobs", &job.id, &job)?;
        }
        q.validate()?;
        db::put(&tx, "questions", &id, &q)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(q)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    Ok(Json(result?))
}
