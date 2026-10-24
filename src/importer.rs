//! Staged ingestion: immutable originals -> reviewable regions -> per-region OCR.
use crate::{
    api::{Api, Error, bad, lock},
    db::{self, Store},
    model::*,
    ocr, splitter,
};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read},
    path::{Path as FsPath, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const FILE_LIMIT: usize = 32 * 1024 * 1024;
const BATCH_LIMIT: usize = 128 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportOptions {
    pub engine: String,
    pub agent_assist: bool,
    pub allow_remote: bool,
    pub layout: String,
    pub prepare_only: bool,
    pub formula_enhanced: bool,
}
impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            engine: "local".into(),
            agent_assist: false,
            allow_remote: false,
            layout: "auto".into(),
            prepare_only: false,
            formula_enhanced: false,
        }
    }
}
impl ImportOptions {
    fn validate(&self) -> Result<(), String> {
        if !["local", "paddle", "mathpix", "service"].contains(&self.engine.as_str()) {
            return Err("未知 OCR 引擎".into());
        }
        if !["auto", "single", "columns", "manual"].contains(&self.layout.as_str()) {
            return Err("未知版面类型".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagePreview {
    pub page: usize,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPlan {
    pub id: String,
    pub revision: u64,
    pub options: ImportOptions,
    pub pages: Vec<PagePreview>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInput {
    pub page: usize,
    pub rect: RegionRect,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionsInput {
    pub revision: u64,
    pub regions: Vec<RegionInput>,
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProcessOptions {
    pub engine: Option<String>,
    pub allow_remote: Option<bool>,
    pub agent_assist: Option<bool>,
    pub only_agent: bool,
}
fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn disk(s: &Store, id: &str) -> PathBuf {
    s.dir.join("imports").join(id)
}
fn io_error(e: impl std::fmt::Display) -> String {
    format!("导入文件处理失败：{e}")
}
fn source_questions(conn: &rusqlite::Connection, id: &str) -> Result<Vec<Question>, String> {
    Ok(db::all::<Question>(conn, "questions")?
        .into_iter()
        .filter(|q| {
            q.parent_id.is_none()
                && (q.paper_id.as_deref() == Some(id) || q.origins.iter().any(|o| o.paper_id == id))
        })
        .collect())
}
fn editable(qs: &[Question]) -> Result<(), Error> {
    if qs
        .iter()
        .any(|q| q.revision > 0 || q.verified || q.status != "pending")
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "来源已有人工编辑或审核的题目，不能重划区域或自动覆盖；请作为新来源导入。".into(),
        ));
    }
    Ok(())
}
fn kind(name: &str, bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("文件为空".into());
    }
    if bytes.len() > FILE_LIMIT {
        return Err("每个文件不能超过 32 MB".into());
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "pdf" if bytes.starts_with(b"%PDF-") => Ok(ext),
        "txt" | "md" | "markdown" => {
            std::str::from_utf8(bytes).map_err(|_| "文字文件需要 UTF-8 编码")?;
            Ok(if ext == "markdown" { "md".into() } else { ext })
        }
        "png" | "jpg" | "jpeg" | "webp" | "bmp" => {
            let actual = image::guess_format(bytes).map_err(|_| "不是有效的图片")?;
            let expected = match ext.as_str() {
                "png" => image::ImageFormat::Png,
                "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                "webp" => image::ImageFormat::WebP,
                _ => image::ImageFormat::Bmp,
            };
            if actual != expected {
                return Err("图片扩展名与实际格式不一致".into());
            }
            Ok(if ext == "jpeg" { "jpg".into() } else { ext })
        }
        _ => Err("支持 PDF、UTF-8 Markdown / TXT，以及 PNG、JPG、WEBP、BMP 图片".into()),
    }
}
fn decode(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    let (w, h) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(io_error)?
        .into_dimensions()
        .map_err(io_error)?;
    if w == 0 || h == 0 || w > 16000 || h > 16000 || u64::from(w) * u64::from(h) > 40_000_000 {
        return Err("图片过大；请缩小到 4000 万像素以内".into());
    }
    image::load_from_memory(bytes).map_err(io_error)
}
fn preview_image(bytes: &[u8], path: &FsPath) -> Result<PagePreview, String> {
    let mut im = decode(bytes)?;
    if im.width().max(im.height()) > 2400 {
        im = im.resize(2400, 2400, image::imageops::FilterType::Lanczos3);
    }
    im.save_with_format(path, image::ImageFormat::Png)
        .map_err(io_error)?;
    Ok(PagePreview {
        page: 1,
        width: im.width(),
        height: im.height(),
    })
}
/// Conservatively detect two columns from an empty vertical gutter, or use the user's choice.
fn columns(im: &image::DynamicImage, layout: &str) -> Vec<(u32, u32)> {
    let w = im.width();
    let h = im.height();
    let two = layout == "columns"
        || (layout == "auto" && {
            let gray = im.to_luma8();
            let dark = |start: u32, end: u32| -> u64 {
                (start..end)
                    .map(|x| {
                        (h / 12..h - h / 12)
                            .step_by(3)
                            .filter(|y| gray.get_pixel(x, *y)[0] < 180)
                            .count() as u64
                    })
                    .sum()
            };
            let middle = dark(w * 49 / 100, w * 51 / 100);
            middle < u64::from(h.max(1)) / 12
                && dark(w / 8, w / 3) > u64::from(h)
                && dark(w * 2 / 3, w * 7 / 8) > u64::from(h)
        });
    if two && w >= 100 {
        vec![(0, w / 2), (w / 2, w)]
    } else {
        vec![(0, w)]
    }
}
/// The preliminary TSV is only a layout hint. Each proposed region is cropped before final OCR.
pub fn propose_regions(tsv: &str, page: &PagePreview, cols: &[(u32, u32)]) -> Vec<RegionInput> {
    let number = regex::Regex::new(r"^(?:第)?([0-9０-９]{1,3})[.．、，,:：)）题]").unwrap();
    let mut out = vec![];
    for &(left, right) in cols {
        let mut starts = vec![];
        let mut last = 0;
        let mut rows: Vec<_> = tsv
            .lines()
            .skip(1)
            .filter_map(|line| {
                let f: Vec<_> = line.splitn(12, '\t').collect();
                if f.len() != 12 || f[0] != "5" {
                    return None;
                }
                let x = f[6].parse::<u32>().ok()?;
                let y = f[7].parse::<u32>().ok()?;
                let caps = number.captures(f[11].trim())?;
                let n: String = caps[1]
                    .chars()
                    .map(|c| {
                        if ('０'..='９').contains(&c) {
                            char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap()
                        } else {
                            c
                        }
                    })
                    .collect();
                Some((x, y, n.parse::<u32>().ok()?))
            })
            .collect();
        rows.sort_by_key(|(_, y, _)| *y);
        for (x, y, n) in rows {
            if x >= left
                && x < left + (right - left) / 5
                && y < page.height
                && n > last
                && (starts.is_empty() || y > starts[starts.len() - 1] + 12)
            {
                starts.push(y.saturating_sub(10));
                last = n;
            }
        }
        if starts.is_empty() {
            starts.push(0);
        }
        // Preserve headers and instructions in the first crop rather than discarding source pixels.
        starts[0] = 0;
        for (i, &top) in starts.iter().enumerate() {
            let bottom = starts.get(i + 1).copied().unwrap_or(page.height);
            if bottom > top + 8 {
                out.push(RegionInput {
                    page: page.page,
                    rect: RegionRect {
                        x: left,
                        y: top,
                        width: right - left,
                        height: bottom - top,
                    },
                });
            }
        }
    }
    out
}
fn crop_questions(
    dir: &FsPath,
    id: &str,
    pages: &[PagePreview],
    regions: &[RegionInput],
) -> Result<Vec<Question>, String> {
    if regions.is_empty() || regions.len() > 500 {
        return Err("请保留 1–500 个题目区域".into());
    }
    // Validate every region before writing any crop.
    for region in regions {
        let p = pages
            .iter()
            .find(|p| p.page == region.page)
            .ok_or("区域页码不存在")?;
        if !region.rect.valid(p.width, p.height) {
            return Err("题目区域超出原页或尺寸小于 8 像素".into());
        }
    }
    let mut qs = vec![];
    for p in pages {
        let selected: Vec<_> = regions.iter().filter(|r| r.page == p.page).collect();
        if selected.is_empty() {
            continue;
        }
        let im = decode(&std::fs::read(dir.join(format!("{}.png", p.page))).map_err(io_error)?)?;
        for r in selected {
            let crop_id = uid();
            let b = r.rect;
            im.crop_imm(b.x, b.y, b.width, b.height)
                .save_with_format(dir.join(format!("{crop_id}.png")), image::ImageFormat::Png)
                .map_err(io_error)?;
            let mut q = Question::new(String::new());
            q.paper_id = Some(id.into());
            q.page_start = p.page;
            q.page_end = p.page;
            q.number = (qs.len() + 1).to_string();
            q.origins = vec![SourceSpan {
                paper_id: id.into(),
                page: p.page,
                rect: Some(b),
                crop_id: Some(crop_id),
                line_start: None,
                line_end: None,
            }];
            q.recognition = Some(Recognition {
                status: "pending".into(),
                engine: String::new(),
                raw_text: String::new(),
                error: String::new(),
                attempt: 0,
            });
            qs.push(q);
        }
    }
    Ok(qs)
}
fn prepare(
    s: &Store,
    name: String,
    bytes: Vec<u8>,
    options: ImportOptions,
) -> Result<(Paper, ImportPlan, Vec<Question>), String> {
    let k = kind(&name, &bytes)?;
    let id = uid();
    let dir = disk(s, &id);
    std::fs::create_dir_all(&dir).map_err(io_error)?;
    let raw = s.dir.join("papers").join(format!("{id}.{k}"));
    std::fs::write(&raw, &bytes).map_err(io_error)?;
    // From this point failures preserve the original as a recoverable source record.
    let mut plan = ImportPlan {
        id: id.clone(),
        revision: 0,
        options: options.clone(),
        pages: vec![],
        warnings: vec![],
    };
    plan.options.allow_remote = false;
    let preparation_deadline = Instant::now() + Duration::from_secs(180);
    let mut page_count = 1;
    let result = (|| -> Result<Vec<Question>, String> {
        if ["txt", "md"].contains(&k.as_str()) {
            let text = std::str::from_utf8(&bytes)
                .map_err(io_error)?
                .trim_start_matches('\u{feff}');
            let pages: Vec<String> = text.split('\u{0c}').map(str::to_string).collect();
            page_count = pages.len();
            return Ok(splitter::split(&pages));
        }
        if k == "pdf" {
            let texts = pdf_extract::extract_text_from_mem_by_pages(&bytes)
                .map_err(|_| "PDF 无法解析，可能已加密或损坏；原件已保留")?;
            page_count = texts.len();
            if page_count == 0 || page_count > 60 {
                return Err("每份 PDF 支持 1–60 页，请拆成较少页导入".into());
            }
            if !options.formula_enhanced
                && options.layout != "manual"
                && texts
                    .iter()
                    .all(|t| t.chars().filter(|c| !c.is_whitespace()).count() >= 12)
            {
                plan.warnings
                    .push("已按文字题号拆分并保留页码和行号；数学公式可能提取不完整。若发现乱码，请重新导入并开启公式增强。".into());
                return Ok(splitter::split(&texts));
            }
            let deadline = preparation_deadline;
            for p in 1..=page_count {
                let prefix = dir.join(p.to_string());
                ocr::run(
                    Command::new(ocr::executable("pdftoppm"))
                        .args([
                            "-f",
                            &p.to_string(),
                            "-l",
                            &p.to_string(),
                            "-scale-to",
                            "2400",
                            "-singlefile",
                            "-png",
                        ])
                        .arg(&raw)
                        .arg(&prefix),
                    deadline,
                )?;
                let im = decode(&std::fs::read(prefix.with_extension("png")).map_err(io_error)?)?;
                plan.pages.push(PagePreview {
                    page: p,
                    width: im.width(),
                    height: im.height(),
                });
            }
        } else {
            plan.pages.push(preview_image(&bytes, &dir.join("1.png"))?);
        }
        let mut regions = vec![];
        for p in &plan.pages {
            let path = dir.join(format!("{}.png", p.page));
            let im = decode(&std::fs::read(&path).map_err(io_error)?)?;
            if im
                .to_luma8()
                .pixels()
                .step_by(5)
                .filter(|p| p[0] < 200)
                .take(30)
                .count()
                < 30
            {
                plan.warnings
                    .push(format!("第 {} 页为空白，未创建 OCR 候选。", p.page));
                continue;
            }
            let tsv = if options.layout == "manual"
                || Instant::now() + Duration::from_secs(25) > preparation_deadline
            {
                String::new()
            } else {
                match ocr::image_output(&path, true) {
                    Ok(t) => t,
                    Err(e) => {
                        plan.warnings.push(format!(
                            "第 {} 页自动分区不可用：{e} 请在预览中手工框选。",
                            p.page
                        ));
                        String::new()
                    }
                }
            };
            let proposed = propose_regions(&tsv, p, &columns(&im, &options.layout));
            if proposed.len() <= 1 {
                plan.warnings.push(format!(
                    "第 {} 页未确定多题边界，已保留整页区域；多道题请先手动重划。",
                    p.page
                ));
            }
            regions.extend(proposed);
        }
        crop_questions(&dir, &id, &plan.pages, &regions)
    })();
    let mut qs = match result {
        Ok(qs) => qs,
        Err(e) => {
            plan.warnings.push(e);
            vec![]
        }
    };
    if qs.is_empty() {
        plan.warnings
            .push("未生成候选题；原件已保留，可手工录入或导入外部结构化结果。".into());
    }
    for q in &mut qs {
        q.paper_id = Some(id.clone());
        for o in &mut q.origins {
            o.paper_id = id.clone();
        }
    }
    plan.warnings
        .push("自动分区与识别均是待审候选。请核对题干、公式、图形与单位；原件始终保留。".into());
    let mut conn = s.db.lock().map_err(|_| "数据库暂不可用")?;
    let tx = conn.transaction().map_err(io_error)?;
    let paper = Paper {
        id: id.clone(),
        name,
        kind: k,
        pages: page_count,
        warning: plan.warnings.join(" "),
        created_at: db::now(&tx),
    };
    db::put(&tx, "papers", &id, &paper)?;
    db::put(&tx, "imports", &id, &plan)?;
    for q in &qs {
        db::put(&tx, "questions", &q.id, q)?;
    }
    tx.commit().map_err(io_error)?;
    Ok((paper, plan, qs))
}

pub async fn import(State(s): State<Store>, mut multi: Multipart) -> Api<Value> {
    let mut files = vec![];
    let mut options = ImportOptions::default();
    let mut total = 0;
    while let Some(field) = multi
        .next_field()
        .await
        .map_err(|_| bad("批量上传失败或超过 128 MB"))?
    {
        match field.name().unwrap_or("") {
            "options" => {
                let text = field.text().await.map_err(|_| bad("无法读取导入选项"))?;
                if text.len() > 4096 {
                    return Err(bad("导入选项过长"));
                }
                options = serde_json::from_str(&text).map_err(|_| bad("导入选项 JSON 无效"))?;
            }
            "file" | "files" => {
                if files.len() >= 20 {
                    return Err(bad("一次最多选择 20 个文件"));
                }
                let name = field
                    .file_name()
                    .unwrap_or("资料.txt")
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("资料.txt")
                    .to_string();
                if name.chars().count() > 150 {
                    return Err(bad("文件名最多 150 个字符"));
                }
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| bad("无法读取文件或批次超出 128 MB"))?
                    .to_vec();
                total += bytes.len();
                if total > BATCH_LIMIT {
                    return Err(bad("每批次不能超过 128 MB"));
                }
                files.push((name, bytes));
            }
            _ => {}
        }
    }
    options.validate()?;
    if files.is_empty() {
        return Err(bad("请选择至少一个文件"));
    }
    let single = files.len() == 1;
    let mut results = vec![];
    for (name, bytes) in files {
        let permit = s
            .import_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| bad("导入服务已关闭"))?;
        let store = s.clone();
        let opts = options.clone();
        let filename = name.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            prepare(&store, filename, bytes, opts)
        })
        .await
        .map_err(|_| bad("导入处理进程失败"))?;
        match prepared {
            Err(e) => {
                if single {
                    return Err(e.into());
                }
                results.push(json!({"name":name,"error":e}));
            }
            Ok((paper, plan, qs)) => {
                if !options.prepare_only {
                    for q in &qs {
                        if q.recognition.is_some() || options.agent_assist {
                            let _ = process_question(
                                &s,
                                &q.id,
                                ProcessOptions {
                                    allow_remote: Some(options.allow_remote),
                                    ..Default::default()
                                },
                            )
                            .await;
                        }
                    }
                }
                results.push(
                    json!({"name":name,"paper":paper,"count":qs.len(),"plan":plan,"prepared":true}),
                );
            }
        }
    }
    if single {
        let mut result = results.remove(0);
        result["results"] = json!([result.clone()]);
        Ok(Json(result))
    } else {
        Ok(Json(json!({"results":results})))
    }
}
pub async fn plan(State(s): State<Store>, Path(id): Path<String>) -> Api<Value> {
    let conn = lock(&s)?;
    let plan: ImportPlan = db::get(&conn, "imports", &id)?;
    let mut questions = source_questions(&conn, &id)?;
    questions.reverse();
    questions.sort_by_key(|q| {
        (
            q.page_start,
            q.origins.first().and_then(|o| o.rect).map(|b| (b.x, b.y)),
            q.number.parse::<u32>().unwrap_or(u32::MAX),
            q.number.clone(),
        )
    });
    Ok(Json(json!({"plan":plan,"questions":questions})))
}
pub async fn replace_regions(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<RegionsInput>,
) -> Api<Value> {
    let original: ImportPlan = {
        let conn = lock(&s)?;
        let p: ImportPlan = db::get(&conn, "imports", &id)?;
        editable(&source_questions(&conn, &id)?)?;
        p
    };
    if original.revision != input.revision {
        return Err(Error(
            StatusCode::CONFLICT,
            "拆分区域已更新，请刷新后再试".into(),
        ));
    }
    let store = s.clone();
    let copy = original.clone();
    let permit = s
        .import_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| bad("导入服务已关闭"))?;
    let qs = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        crop_questions(
            &disk(&store, &copy.id),
            &copy.id,
            &copy.pages,
            &input.regions,
        )
    })
    .await
    .map_err(|_| bad("图片裁切失败"))??;
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(io_error)?;
    let mut current: ImportPlan = db::get(&tx, "imports", &id)?;
    if current.revision != original.revision {
        return Err(Error(
            StatusCode::CONFLICT,
            "拆分区域已更新，请刷新后再试".into(),
        ));
    }
    let existing = source_questions(&tx, &id)?;
    editable(&existing)?;
    for q in existing {
        tx.execute("DELETE FROM questions WHERE id=?1", [q.id])
            .map_err(io_error)?;
    }
    for q in &qs {
        db::put(&tx, "questions", &q.id, q)?;
    }
    current
        .warnings
        .retain(|w| !w.contains("未确定多题边界") && !w.contains("自动分区不可用"));
    current.warnings.push(format!(
        "已人工调整为 {} 个题目区域；文字识别尚需逐题核验。",
        qs.len()
    ));
    current.revision += 1;
    db::put(&tx, "imports", &id, &current)?;
    tx.commit().map_err(io_error)?;
    Ok(Json(json!({"plan":current,"questions":qs})))
}
fn endpoint(engine: &str) -> Result<(reqwest::Url, Option<String>, bool), String> {
    if engine == "mathpix" {
        for key in ["PAPERDESK_MATHPIX_APP_ID", "PAPERDESK_MATHPIX_APP_KEY"] {
            if std::env::var(key).unwrap_or_default().trim().is_empty() {
                return Err(format!("Mathpix 尚未配置：{key}"));
            }
        }
        return Ok((
            reqwest::Url::parse("https://api.mathpix.com/v3/text").unwrap(),
            None,
            true,
        ));
    }
    let (url_key, token_key) = if engine == "paddle" {
        ("PAPERDESK_PADDLE_URL", "PAPERDESK_PADDLE_TOKEN")
    } else {
        ("PAPERDESK_OCR_URL", "PAPERDESK_OCR_TOKEN")
    };
    let url = reqwest::Url::parse(
        &std::env::var(url_key).map_err(|_| format!("此识别服务尚未配置：{url_key}"))?,
    )
    .map_err(|_| "识别服务地址无效")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if !url.username().is_empty()
        || url.password().is_some()
        || (!local && url.scheme() != "https")
        || !["http", "https"].contains(&url.scheme())
    {
        return Err("外部 OCR 需要 HTTPS 地址；令牌通过独立环境变量配置".into());
    }
    Ok((
        url,
        std::env::var(token_key).ok().filter(|s| !s.is_empty()),
        !local,
    ))
}
pub fn engine_info() -> Value {
    let service = |id: &str, name: &str| {
        let ep = endpoint(id);
        json!({"id":id,"name":name,"configured":ep.is_ok(),"remote":ep.map(|v|v.2).unwrap_or(true)})
    };
    json!({"engines":[{"id":"local","name":"本地 Tesseract · 离线","configured":true,"remote":false},service("paddle","PaddleOCR · 正文与公式"),service("mathpix","Mathpix · 在线数学识别"),service("service","自定义 / 在线 OCR")],"agent_assist":{"available":true,"mode":"external_worker","kind":"recognition"},"formats":["pdf","md","txt","png","jpg","webp","bmp"],"max_files":20,"max_file_mb":32,"max_batch_mb":128})
}
fn recognize_crop(path: &FsPath, options: &ImportOptions) -> Result<String, String> {
    if options.engine == "local" {
        return ocr::image_output(path, false);
    }
    recognize_service(path, options, endpoint(&options.engine)?)
}
fn recognize_service(
    path: &FsPath,
    options: &ImportOptions,
    target: (reqwest::Url, Option<String>, bool),
) -> Result<String, String> {
    let (url, token, remote) = target;
    if remote && !options.allow_remote {
        return Err("尚未同意将题目图片发送给所选外部 OCR 服务".into());
    }
    let bytes = std::fs::read(path).map_err(io_error)?;
    let encoded = STANDARD.encode(bytes);
    if options.engine == "mathpix" && encoded.len() > 2 * 1024 * 1024 {
        return Err("Mathpix 单题图片超过 2 MB 编码限制，请缩小裁切范围或使用本地公式服务".into());
    }
    let payload = if options.engine == "paddle" {
        json!({"file":encoded,"fileType":1,"visualize":false,"useFormulaRecognition":true})
    } else if options.engine == "mathpix" {
        json!({"src":format!("data:image/png;base64,{encoded}"),"formats":["text"],"math_inline_delimiters":["$","$"],"math_display_delimiters":["$$","$$"]})
    } else {
        json!({"image_base64":encoded,"mime_type":"image/png","task":"question_ocr","preserve_formulas":true})
    };
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(if options.engine == "paddle" {
            300
        } else {
            90
        }))
        .build()
        .map_err(|_| "无法创建 OCR 服务连接")?;
    let mut request = client.post(url).json(&payload);
    if options.engine == "mathpix" {
        request = request
            .header(
                "app_id",
                std::env::var("PAPERDESK_MATHPIX_APP_ID").map_err(|_| "Mathpix 缺少 app_id")?,
            )
            .header(
                "app_key",
                std::env::var("PAPERDESK_MATHPIX_APP_KEY").map_err(|_| "Mathpix 缺少 app_key")?,
            );
    }
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .map_err(|_| "OCR 服务连接失败或超时，请检查地址和网络")?;
    if !response.status().is_success() {
        return Err(format!(
            "OCR 服务返回 HTTP {}，未自动切换服务",
            response.status().as_u16()
        ));
    }
    let mut data = vec![];
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|_| "无法读取 OCR 结果")?;
    if data.len() > 4 * 1024 * 1024 {
        return Err("OCR 结果超过 4 MB".into());
    }
    let value: Value = serde_json::from_slice(&data).map_err(|_| "OCR 服务没有返回有效 JSON")?;
    parse_service_result(&value, &options.engine)
}
fn parse_service_result(value: &Value, engine: &str) -> Result<String, String> {
    if engine == "mathpix" && value.get("error").is_some_and(|v| !v.is_null()) {
        return Err("Mathpix 识别失败，请检查账户额度、权限与图片；未自动切换服务".into());
    }
    let text = if engine == "paddle" {
        if value
            .get("errorCode")
            .and_then(Value::as_i64)
            .is_some_and(|v| v != 0)
        {
            return Err("PaddleOCR 返回识别错误，请检查服务日志".into());
        }
        value
            .pointer("/result/layoutParsingResults")
            .and_then(Value::as_array)
            .ok_or("PaddleOCR 响应缺少 layoutParsingResults")?
            .iter()
            .map(|p| {
                p.pointer("/markdown/text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        value
            .get("text")
            .and_then(Value::as_str)
            .ok_or("在线 OCR 响应应包含 text 字符串")?
            .to_string()
    };
    if text.trim().is_empty() || text.len() > 200_000 {
        return Err("OCR 未返回文字或单题超过 200 KB；请手工核验区域".into());
    }
    Ok(text)
}
fn queue_agent(conn: &rusqlite::Connection, q: &Question) -> Result<(), String> {
    if db::all::<Job>(conn, "jobs")?.iter().any(|j| {
        j.question_id == q.id
            && j.kind == "recognition"
            && ["queued", "running", "review"].contains(&j.status.as_str())
    }) {
        return Ok(());
    }
    let job=Job{id:uid(),question_id:q.id.clone(),kind:"recognition".into(),instruction:"逐一读取此题 origins 对应的原图区域并校对转写；保留公式、选项与题图说明，不解题、不补造缺失内容。无法确定的文字用【待核验】标注。返回 content。图片通过 /api/agent/v1/questions/{id}/source/{index} 读取。".into(),status:"queued".into(),source_revision:q.revision,source:q.clone(),result:None,created_at:db::now(conn)};
    db::put(conn, "jobs", &job.id, &job)
}
async fn process_question(s: &Store, id: &str, request: ProcessOptions) -> Result<Question, Error> {
    let (snapshot, mut options) = {
        let conn = lock(s)?;
        let q: Question = db::get(&conn, "questions", id)?;
        let origin = q
            .origins
            .first()
            .ok_or_else(|| bad("题目没有可识别的来源区域"))?;
        let p: ImportPlan = db::get(&conn, "imports", &origin.paper_id)?;
        editable(std::slice::from_ref(&q))?;
        if q.parent_id.is_some() {
            return Err(bad("变式不能覆盖为原图识别结果"));
        }
        (q, p.options)
    };
    options.allow_remote = false; // Consent belongs to this operation, never a stored plan.
    if let Some(v) = request.engine {
        options.engine = v;
    }
    if let Some(v) = request.agent_assist {
        options.agent_assist = v;
    }
    if let Some(v) = request.allow_remote {
        options.allow_remote = v;
    }
    options.validate()?;
    if options.engine != "local" && !request.only_agent {
        let (_, _, remote) = endpoint(&options.engine)?;
        if remote && !options.allow_remote {
            return Err(bad("请先同意将本次题目图片发送给外部 OCR 服务"));
        }
    }
    if request.only_agent && !options.agent_assist {
        return Err(bad("仅创建 Agent 任务时需要开启辅助识别"));
    }
    let text = if request.only_agent {
        None
    } else if let Some(crop) = snapshot.origins.first().and_then(|o| o.crop_id.as_ref()) {
        let path = disk(s, &snapshot.origins[0].paper_id).join(format!("{crop}.png"));
        let copy = options.clone();
        let permit = s
            .import_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| bad("识别服务已关闭"))?;
        Some(
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                recognize_crop(&path, &copy)
            })
            .await
            .map_err(|_| bad("识别进程失败，原图已保留"))?,
        )
    } else {
        None
    };
    let mut conn = lock(s)?;
    let tx = conn.transaction().map_err(io_error)?;
    let mut q: Question = db::get(&tx, "questions", id)?;
    editable(std::slice::from_ref(&q))?;
    if q.revision != snapshot.revision
        || q.recognition.as_ref().map(|r| r.attempt)
            != snapshot.recognition.as_ref().map(|r| r.attempt)
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "题目在识别期间已更新，本次结果没有覆盖新内容".into(),
        ));
    }
    let mut content_changed = false;
    if let Some(text) = text {
        let attempt = q.recognition.as_ref().map_or(1, |r| r.attempt + 1);
        match text {
            Ok(text) if !text.trim().is_empty() => {
                content_changed = q.content != text;
                q.content = text.clone();
                q.recognition = Some(Recognition {
                    status: "done".into(),
                    engine: options.engine.clone(),
                    raw_text: text,
                    error: String::new(),
                    attempt,
                });
            }
            result => {
                let error = result
                    .err()
                    .unwrap_or_else(|| "此区域未识别到文字，请重新框选或人工录入。".into());
                let raw = q
                    .recognition
                    .as_ref()
                    .map_or_else(String::new, |r| r.raw_text.clone());
                q.recognition = Some(Recognition {
                    status: "error".into(),
                    engine: options.engine.clone(),
                    raw_text: raw,
                    error,
                    attempt,
                });
            }
        }
    }
    q.validate()?;
    if content_changed {
        for mut job in db::all::<Job>(&tx, "jobs")?.into_iter().filter(|j| {
            j.question_id == q.id && ["queued", "running", "review"].contains(&j.status.as_str())
        }) {
            job.status = "rejected".into();
            db::put(&tx, "jobs", &job.id, &job)?;
        }
    }
    db::put(&tx, "questions", id, &q)?;
    if options.agent_assist {
        queue_agent(&tx, &q)?;
    }
    tx.commit().map_err(io_error)?;
    Ok(q)
}
pub async fn recognize_one(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(options): Json<ProcessOptions>,
) -> Api<Question> {
    Ok(Json(process_question(&s, &id, options).await?))
}
pub async fn page_image(
    State(s): State<Store>,
    Path((id, page)): Path<(String, usize)>,
) -> Result<Response, Error> {
    let plan: ImportPlan = db::get(&*lock(&s)?, "imports", &id)?;
    if !plan.pages.iter().any(|p| p.page == page) {
        return Err(bad("页面不存在"));
    }
    let bytes = tokio::fs::read(disk(&s, &id).join(format!("{page}.png")))
        .await
        .map_err(|_| bad("原页预览不可用"))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}
pub async fn question_crop(
    State(s): State<Store>,
    Path((id, index)): Path<(String, usize)>,
) -> Result<Response, Error> {
    let q: Question = db::get(&*lock(&s)?, "questions", &id)?;
    let span = q.origins.get(index).ok_or_else(|| bad("原图区域不存在"))?;
    let crop = span
        .crop_id
        .as_ref()
        .ok_or_else(|| bad("该来源为文字，请查看原始文件及行号"))?;
    if uuid::Uuid::parse_str(crop).is_err() || uuid::Uuid::parse_str(&span.paper_id).is_err() {
        return Err(bad("原图标识无效"));
    }
    let bytes = tokio::fs::read(disk(&s, &span.paper_id).join(format!("{crop}.png")))
        .await
        .map_err(|_| bad("原图区域不可用"))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_regions_preserve_column_order_and_unrecognized_pages() {
        let page = PagePreview {
            page: 2,
            width: 1200,
            height: 800,
        };
        let tsv = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t10\t20\t20\t20\t95\t1.\n5\t1\t1\t1\t2\t1\t10\t400\t20\t20\t95\t2.\n5\t1\t2\t1\t1\t1\t610\t20\t20\t20\t95\t3.\n5\t1\t2\t1\t2\t1\t610\t400\t20\t20\t95\t4.";
        let regions = propose_regions(tsv, &page, &[(0, 600), (600, 1200)]);
        assert_eq!(regions.len(), 4);
        assert_eq!(
            regions[0].rect,
            RegionRect {
                x: 0,
                y: 0,
                width: 600,
                height: 390
            }
        );
        assert_eq!(regions[2].rect.x, 600);
        assert!(
            regions
                .iter()
                .all(|r| r.page == 2 && r.rect.valid(1200, 800))
        );
        assert_eq!(propose_regions("", &page, &[(0, 1200)]).len(), 1);
        assert!(
            !RegionRect {
                x: u32::MAX,
                y: 0,
                width: 100,
                height: 100
            }
            .valid(1200, 800)
        );
    }
    #[test]
    fn external_service_cannot_read_or_send_a_crop_without_consent() {
        let options = ImportOptions {
            engine: "service".into(),
            ..Default::default()
        };
        let error = recognize_service(
            FsPath::new("does-not-exist.png"),
            &options,
            ("https://example.invalid/ocr".parse().unwrap(), None, true),
        )
        .unwrap_err();
        assert!(error.contains("同意"));
    }
    #[test]
    fn provider_result_contract_rejects_errors_and_empty_output() {
        assert!(
            parse_service_result(
                &json!({"errorCode":500,"result":{"layoutParsingResults":[]}}),
                "paddle"
            )
            .is_err()
        );
        assert!(parse_service_result(&json!({"text":""}), "service").is_err());
        assert!(parse_service_result(&json!({"text":"x".repeat(200_001)}), "service").is_err());
        assert_eq!(
            parse_service_result(&json!({"text":"公式 $x^2$"}), "service").unwrap(),
            "公式 $x^2$"
        );
    }
    #[test]
    fn mathpix_preserves_latex_and_rejects_provider_errors() {
        let input = json!({"text":r"求 $\frac{x^2}{2}$ 和 \(a_1\)"});
        assert_eq!(
            parse_service_result(&input, "mathpix").unwrap(),
            input["text"]
        );
        assert!(
            parse_service_result(&json!({"error":"quota","text":"partial"}), "mathpix").is_err()
        );
        assert!(parse_service_result(&json!({"text":""}), "mathpix").is_err());
    }
    #[test]
    fn mathpix_cannot_send_without_operation_consent() {
        let opts = ImportOptions {
            engine: "mathpix".into(),
            ..Default::default()
        };
        let error = recognize_service(
            FsPath::new("no-image-must-be-read.png"),
            &opts,
            (
                "https://api.mathpix.com/v3/text".parse().unwrap(),
                None,
                true,
            ),
        )
        .unwrap_err();
        assert!(error.contains("同意"));
    }
    #[test]
    fn paddle_http_contract_sends_only_the_crop_and_reads_markdown() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /layout-parsing "));
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).unwrap();
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(request["fileType"], 1);
            assert_eq!(request["visualize"], false);
            assert_eq!(request["useFormulaRecognition"], true);
            let crop = STANDARD.decode(request["file"].as_str().unwrap()).unwrap();
            assert_eq!(&crop[..8], b"\x89PNG\r\n\x1a\n");
            let payload=json!({"errorCode":0,"result":{"layoutParsingResults":[{"markdown":{"text":"1. Formula $x^2$"}}]}}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",payload.len(),payload).unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("crop.png");
        image::RgbImage::new(16, 16).save(&path).unwrap();
        let options = ImportOptions {
            engine: "paddle".into(),
            ..Default::default()
        };
        let result = recognize_service(
            &path,
            &options,
            (
                format!("http://{address}/layout-parsing").parse().unwrap(),
                None,
                false,
            ),
        );
        worker.join().unwrap();
        assert_eq!(result.unwrap(), "1. Formula $x^2$");
    }
}
