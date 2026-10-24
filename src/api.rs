use crate::{
    db::{self, Store},
    model::*,
    splitter,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) type Api<T> = Result<Json<T>, Error>;
pub struct Error(pub(crate) StatusCode, pub(crate) String);
impl From<String> for Error {
    fn from(v: String) -> Self {
        Self(StatusCode::BAD_REQUEST, v)
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}
pub(crate) fn bad(s: &str) -> Error {
    Error(StatusCode::BAD_REQUEST, s.into())
}
fn missing() -> Error {
    Error(StatusCode::NOT_FOUND, "记录不存在".into())
}
pub(crate) fn lock(s: &Store) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, Error> {
    s.db.lock()
        .map_err(|_| Error(StatusCode::INTERNAL_SERVER_ERROR, "数据库暂不可用".into()))
}

pub fn router(state: Store) -> Router {
    let agent = Router::new()
        .route("/questions", get(list_questions))
        .route("/questions/{id}", get(read_question))
        .route("/papers/{id}/source", get(source))
        .route(
            "/questions/{id}/source/{index}",
            get(crate::importer::question_crop),
        )
        .route("/imports/{id}", get(crate::importer::plan))
        .route(
            "/imports/{id}/pages/{page}",
            get(crate::importer::page_image),
        )
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}/claim", post(claim_job))
        .route("/jobs/{id}/result", post(complete_job))
        .route_layer(middleware::from_fn_with_state(state.clone(), agent_auth));
    Router::new()
        .route(
            "/settings.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/settings.js"),
                )
            }),
        )
        .route("/vendor/{*path}", get(vendor_asset))
        .route("/print.css", get(print_styles))
        .route(
            "/workspace.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/workspace.js"),
                )
            }),
        )
        .route("/api/questions/{id}/review", post(review_question))
        .route(
            "/math.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/math.js"),
                )
            }),
        )
        .route("/api/fonts/{id}/file", get(font_file))
        .route(
            "/icon.svg",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/svg+xml")],
                    include_str!("../assets/paperdesk.svg"),
                )
            }),
        )
        .route(
            "/features.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/features.js"),
                )
            }),
        )
        .route(
            "/theme.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/theme.js"),
                )
            }),
        )
        .route(
            "/",
            get(|| async { Html(include_str!("../web/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../web/style.css"),
                )
            }),
        )
        .route(
            "/api/health",
            get(|| async { Json(json!({"ok":true,"version":env!("CARGO_PKG_VERSION")})) }),
        )
        .route("/api/papers", get(list_papers).post(create_paper))
        .route(
            "/api/papers/{id}",
            axum::routing::put(rename_paper).delete(delete_paper),
        )
        .route(
            "/api/settings",
            get(crate::settings::read).put(crate::settings::save),
        )
        .route("/api/catalog", get(catalog))
        .route("/api/fonts", get(list_fonts).post(upload_font))
        .route("/api/engines", get(engine_info))
        .route(
            "/imports.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/imports.js"),
                )
            }),
        )
        .route("/api/import", post(crate::importer::import))
        .route("/api/imports/{id}", get(crate::importer::plan))
        .route(
            "/api/imports/{id}/regions",
            post(crate::importer::replace_regions),
        )
        .route(
            "/api/imports/{id}/pages/{page}",
            get(crate::importer::page_image),
        )
        .route(
            "/api/questions/{id}/recognize",
            post(crate::importer::recognize_one),
        )
        .route(
            "/api/questions/{id}/source/{index}",
            get(crate::importer::question_crop),
        )
        .route("/api/papers/{id}/source", get(source))
        .route("/api/papers/{id}/text", post(attach_text))
        .route("/api/papers/{id}/structured", post(attach_structured))
        .route("/api/papers/{id}/recognize", post(recognize_paper))
        .route("/api/questions", get(list_questions).post(create_question))
        .route("/api/questions/merge", post(merge_questions))
        .route(
            "/api/questions/{id}",
            get(read_question)
                .put(update_question)
                .delete(delete_question),
        )
        .route("/api/questions/{id}/split", post(split_question))
        .route("/api/quizzes", get(list_quizzes).post(create_quiz))
        .route("/api/quizzes/preview", post(preview_quiz))
        .route("/api/quizzes/{id}", get(read_quiz).delete(delete_quiz))
        .route("/api/quizzes/{id}/pdf", get(export_quiz_pdf))
        .route("/api/jobs", get(list_jobs).post(create_job))
        .route("/api/jobs/{id}/accept", post(accept_job))
        .route("/api/jobs/{id}/reject", post(reject_job))
        .route("/api/export", get(export))
        .nest("/api/agent/v1", agent)
        .layer(DefaultBodyLimit::max(128 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), request_guard))
        .with_state(state)
}
async fn request_guard(
    State(s): State<Store>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) {
        let parts: Vec<_> = req.uri().path().split('/').collect();
        if parts.len() > 3 && parts[1] == "api" && ["papers", "imports"].contains(&parts[2]) {
            let archived = match lock(&s) {
                Ok(conn) => db::get::<String>(&conn, "archived_papers", parts[3]).is_ok(),
                Err(e) => return e.into_response(),
            };
            if archived {
                return Error(StatusCode::CONFLICT, "来源已删除；历史原件仅供溯源".into())
                    .into_response();
            }
        }
    }
    // A custom header prevents cross-site forms from mutating a localhost database.
    // No CORS permission is issued, so foreign pages cannot send this header.
    let headers = req.headers();
    if let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok())
        && !host.starts_with("127.0.0.1:")
        && !host.starts_with("localhost:")
    {
        return (StatusCode::FORBIDDEN, "仅允许本机访问").into_response();
    }
    if !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && headers.get("x-paperdesk").and_then(|h| h.to_str().ok()) != Some("1")
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"缺少 X-Paperdesk: 1 请求头"})),
        )
            .into_response();
    }
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY, "default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; frame-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'self'".parse().unwrap());
    response
}
async fn agent_auth(
    State(s): State<Store>,
    headers: HeaderMap,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if supplied != format!("Bearer {}", s.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"需要有效的 agent Bearer token"})),
        )
            .into_response();
    }
    next.run(req).await
}
async fn list_papers(State(s): State<Store>) -> Api<Vec<Paper>> {
    let conn = lock(&s)?;
    let archived: Vec<String> = db::all(&conn, "archived_papers")?;
    Ok(Json(
        db::all::<Paper>(&conn, "papers")?
            .into_iter()
            .filter(|p| !archived.contains(&p.id))
            .collect(),
    ))
}
#[derive(Deserialize)]
struct DeletePaperInput {
    expected_name: String,
    expected_question_ids: Vec<String>,
    delete_questions: bool,
}
async fn delete_paper(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<DeletePaperInput>,
) -> Api<Value> {
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    let p: Paper = db::get(&tx, "papers", &id).map_err(|_| missing())?;
    let mut questions: Vec<Question> = db::all::<Question>(&tx, "questions")?
        .into_iter()
        .filter(|q| q.paper_id.as_ref() == Some(&id))
        .collect();
    let mut ids: Vec<String> = questions.iter().map(|q| q.id.clone()).collect();
    ids.sort();
    let mut expected = input.expected_question_ids;
    expected.sort();
    if p.name != input.expected_name || ids != expected {
        return Err(Error(
            StatusCode::CONFLICT,
            "来源或关联题目已变化，请重新打开删除窗口".into(),
        ));
    }
    for q in &mut questions {
        if input.delete_questions {
            tx.execute("DELETE FROM questions WHERE id=?1", [&q.id])
                .map_err(|e| Error::from(e.to_string()))?;
        } else {
            q.paper_id = None;
            q.revision += 1;
            db::put(&tx, "questions", &q.id, q)?;
        }
    }
    for mut job in db::all::<Job>(&tx, "jobs")? {
        if ids.contains(&job.question_id)
            && ["queued", "running", "review"].contains(&job.status.as_str())
        {
            job.status = "rejected".into();
            db::put(&tx, "jobs", &job.id, &job)?;
        }
    }
    db::put(&tx, "archived_papers", &id, &id)?;
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(
        json!({"deleted":id,"questions":ids.len(),"originals_retained":true}),
    ))
}
#[derive(Deserialize)]
struct PaperInput {
    name: String,
    expected_name: Option<String>,
}
fn valid_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 150 {
        return Err(bad("来源名称需要 1–150 个字符"));
    }
    Ok(name.into())
}
async fn create_paper(State(s): State<Store>, Json(input): Json<PaperInput>) -> Api<Paper> {
    let conn = lock(&s)?;
    let p = Paper {
        id: uuid::Uuid::new_v4().to_string(),
        name: valid_name(&input.name)?,
        kind: "manual".into(),
        pages: 0,
        warning: "手工来源，仅记录出处名称，没有原卷文件。".into(),
        created_at: db::now(&conn),
    };
    db::put(&conn, "papers", &p.id, &p)?;
    Ok(Json(p))
}
async fn rename_paper(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<PaperInput>,
) -> Api<Paper> {
    let conn = lock(&s)?;
    if db::get::<String>(&conn, "archived_papers", &id).is_ok() {
        return Err(missing());
    }
    let mut p: Paper = db::get(&conn, "papers", &id).map_err(|_| missing())?;
    if input.expected_name.as_ref().is_some_and(|n| n != &p.name) {
        return Err(Error(
            StatusCode::CONFLICT,
            "来源名称已修改，请刷新后重试".into(),
        ));
    }
    p.name = valid_name(&input.name)?;
    db::put(&conn, "papers", &id, &p)?;
    Ok(Json(p))
}
async fn catalog(State(s): State<Store>) -> Api<Value> {
    let questions = db::all::<Question>(&*lock(&s)?, "questions")?;
    let subjects: std::collections::BTreeSet<_> =
        questions.iter().map(|q| q.subject.clone()).collect();
    let tags: std::collections::BTreeSet<_> = questions
        .iter()
        .flat_map(|q| q.tags.iter().cloned())
        .collect();
    Ok(Json(
        json!({"subjects":subjects,"tags":tags,"total":questions.len()}),
    ))
}
async fn list_fonts(State(s): State<Store>) -> Api<Vec<FontItem>> {
    Ok(Json(crate::fonts::list(&s)?))
}
async fn upload_font(State(s): State<Store>, mut multipart: Multipart) -> Api<FontItem> {
    let mut font = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| bad("无法读取字体上传"))?
    {
        if field.name() != Some("file") {
            continue;
        }
        if font.is_some() {
            return Err(bad("每次请上传一个字体"));
        }
        let name = field.file_name().unwrap_or("自定义字体.ttf").to_string();
        if !name.to_ascii_lowercase().ends_with(".ttf") {
            return Err(bad("请上传静态 TTF 字体"));
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| bad("无法读取字体或超出上传限制"))?
            .to_vec();
        crate::fonts::validate(&bytes)?;
        font = Some((name, bytes));
    }
    let (name, bytes) = font.ok_or_else(|| bad("请选择字体文件"))?;
    let item = FontItem {
        id: uuid::Uuid::new_v4().to_string(),
        name,
    };
    let path = s.dir.join("fonts").join(format!("{}.ttf", item.id));
    std::fs::write(&path, bytes).map_err(|_| bad("无法保存字体"))?;
    if let Err(e) = db::put(&*lock(&s)?, "fonts", &item.id, &item) {
        let _ = std::fs::remove_file(path);
        return Err(e.into());
    }
    Ok(Json(item))
}
async fn engine_info() -> Json<Value> {
    Json(
        json!({"imports":crate::importer::engine_info(),"local_ocr":{"engine":"Tesseract + Poppler","bundled_in_installer":true,"max_pages":60,"timeout_seconds":180,"cloud":false},"native_pdf":{"available":true,"fonts":true,"columns":true,"latex":false},"browser_pdf":{"available":true,"latex":true,"mode":"print_to_pdf","offline":true},"structured_import":{"available":true,"endpoint":"/api/papers/{id}/structured","schema":"docs/document-adapter.schema.json"},"providers":[{"id":"mineru","integrated":false,"url":"https://mineru.net/doc/docs/index_en/"},{"id":"paddleocr","integrated":true,"url":"https://www.paddleocr.ai/"},{"id":"docling","integrated":false,"url":"https://docling-project.github.io/docling/"},{"id":"typst","integrated":false,"url":"https://typst.app/docs/"}]}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredInput {
    engine: String,
    #[serde(default)]
    replace_pending: bool,
    questions: Vec<StructuredQuestion>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredQuestion {
    number: String,
    content: String,
    page_start: usize,
    page_end: usize,
    #[serde(default)]
    tags: Vec<String>,
}
async fn attach_structured(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<StructuredInput>,
) -> Api<Value> {
    if input.engine.trim().is_empty()
        || input.engine.len() > 100
        || input.questions.is_empty()
        || input.questions.len() > 2000
    {
        return Err(bad("请提供引擎名称和 1–2000 道结构化候选题"));
    }
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    let mut paper: Paper = db::get(&tx, "papers", &id).map_err(|_| missing())?;
    let existing = db::questions(
        &tx,
        &Filter {
            paper_id: Some(id.clone()),
            ..Default::default()
        },
    )?;
    if !existing.is_empty()
        && (!input.replace_pending
            || existing
                .iter()
                .any(|q| q.status != "pending" || q.verified || q.revision > 0))
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "来源已有编辑或审核过的题目，不能自动覆盖。仅可替换尚未编辑的待审候选。".into(),
        ));
    }
    for q in &existing {
        tx.execute("DELETE FROM questions WHERE id=?1", [&q.id])
            .map_err(|e| Error::from(e.to_string()))?;
    }
    let count = input.questions.len();
    for supplied in input.questions {
        let mut q = Question::new(supplied.content);
        q.paper_id = Some(id.clone());
        for origin in &mut q.origins {
            origin.paper_id = id.clone();
        }
        q.number = supplied.number;
        q.page_start = supplied.page_start;
        q.page_end = supplied.page_end;
        q.tags = supplied.tags;
        q.validate()?;
        validate_source(&tx, &q)?;
        db::put(&tx, "questions", &q.id, &q)?;
    }
    paper.warning = format!(
        "{} 结构化候选；仍需对照原卷核验题干、公式、题图与单位。",
        input.engine
    );
    db::put(&tx, "papers", &id, &paper)?;
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(json!({"count":count})))
}
async fn list_questions(State(s): State<Store>, Query(f): Query<Filter>) -> Api<Vec<Question>> {
    Ok(Json(db::questions(&*lock(&s)?, &f)?))
}
async fn read_question(State(s): State<Store>, Path(id): Path<String>) -> Api<Question> {
    Ok(Json(
        db::get(&*lock(&s)?, "questions", &id).map_err(|_| missing())?,
    ))
}

async fn source(State(s): State<Store>, Path(id): Path<String>) -> Result<Response, Error> {
    let paper: Paper = db::get(&*lock(&s)?, "papers", &id).map_err(|_| missing())?;
    let bytes = tokio::fs::read(
        s.dir
            .join("papers")
            .join(format!("{}.{}", paper.id, paper.kind)),
    )
    .await
    .map_err(|_| missing())?;
    Ok((
        [(
            header::CONTENT_TYPE,
            match paper.kind.as_str() {
                "pdf" => "application/pdf",
                "png" => "image/png",
                "jpg" => "image/jpeg",
                "webp" => "image/webp",
                "bmp" => "image/bmp",
                _ => "text/plain; charset=utf-8",
            },
        )],
        Body::from(bytes),
    )
        .into_response())
}

fn pending_source(conn: &rusqlite::Connection, id: &str) -> Result<Vec<Question>, Error> {
    let existing = db::questions(
        conn,
        &Filter {
            paper_id: Some(id.into()),
            ..Default::default()
        },
    )?;
    if existing
        .iter()
        .any(|q| q.status != "pending" || q.verified || q.revision > 0)
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "此来源已有人工编辑或审核的题目，不能重新识别覆盖。请将原 PDF 作为新来源导入。".into(),
        ));
    }
    Ok(existing)
}

async fn recognize_paper(State(s): State<Store>, Path(id): Path<String>) -> Api<Value> {
    if db::get::<crate::importer::ImportPlan>(&*lock(&s)?, "imports", &id)
        .is_ok_and(|p| !p.pages.is_empty())
    {
        return Err(bad(
            "此来源已保存逐题区域，请通过「拆分预览 / 逐题识别」重试，避免丢失原图定位。",
        ));
    }
    {
        let conn = lock(&s)?;
        let paper: Paper = db::get(&conn, "papers", &id).map_err(|_| missing())?;
        if paper.kind != "pdf" {
            return Err(bad("只有 PDF 来源支持重新识别"));
        }
        pending_source(&conn, &id)?;
    }
    let bytes = tokio::fs::read(s.dir.join("papers").join(format!("{id}.pdf")))
        .await
        .map_err(|_| missing())?;
    let (pages, warning) = tokio::task::spawn_blocking(move || crate::ocr::extract(&bytes))
        .await
        .map_err(|_| bad("识别进程失败"))?;
    let candidates = splitter::split(&pages);
    if candidates.is_empty() {
        return Err(Error::from(format!("未识别到题目。{warning}")));
    }
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    // Recheck after the slow OCR operation to protect concurrent human edits.
    let existing = pending_source(&tx, &id)?;
    let mut paper: Paper = db::get(&tx, "papers", &id).map_err(|_| missing())?;
    for q in existing {
        tx.execute("DELETE FROM questions WHERE id=?1", [&q.id])
            .map_err(|e| Error::from(e.to_string()))?;
    }
    let count = candidates.len();
    for mut q in candidates {
        q.paper_id = Some(id.clone());
        for origin in &mut q.origins {
            origin.paper_id = id.clone();
        }
        db::put(&tx, "questions", &q.id, &q)?;
    }
    paper.pages = pages.len();
    paper.warning = if warning.is_empty() {
        "已重新提取文字层，仍需核验原卷。".into()
    } else {
        warning
    };
    db::put(&tx, "papers", &id, &paper)?;
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(json!({"count":count,"paper":paper})))
}
#[derive(Deserialize)]
struct TextPages {
    pages: Vec<String>,
}
async fn attach_text(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<TextPages>,
) -> Api<Value> {
    if input.pages.is_empty() {
        return Err(bad("至少提供一页文字"));
    }
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    let mut paper: Paper = db::get(&tx, "papers", &id).map_err(|_| missing())?;
    let existing = db::questions(
        &tx,
        &Filter {
            paper_id: Some(id.clone()),
            ..Default::default()
        },
    )?;
    if !existing.is_empty() {
        return Err(Error(
            StatusCode::CONFLICT,
            "试卷已有题目；为避免重复导入，请先删除已有候选题".into(),
        ));
    }
    if paper.kind == "pdf" && paper.pages > 0 && input.pages.len() != paper.pages {
        return Err(bad(
            "分页文字数量必须与原 PDF 页数一致，空白页请保留空字符串",
        ));
    }
    let candidates = splitter::split(&input.pages);
    let count = candidates.len();
    for mut q in candidates {
        q.paper_id = Some(id.clone());
        for origin in &mut q.origins {
            origin.paper_id = id.clone();
        }
        db::put(&tx, "questions", &q.id, &q)?;
    }
    paper.pages = paper.pages.max(input.pages.len());
    paper.warning = "外部 OCR / 手工文字，仅作候选；公式、单位、图表须对照原卷核验。".into();
    db::put(&tx, "papers", &id, &paper)?;
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(json!({"count":count})))
}
async fn create_question(State(s): State<Store>, Json(mut q): Json<Question>) -> Api<Question> {
    q.id = uuid::Uuid::new_v4().to_string();
    q.origins.clear();
    q.recognition = None;
    q.revision = 0;
    q.status = "pending".into();
    q.verified = false;
    q.validate()?;
    let conn = lock(&s)?;
    validate_source(&conn, &q)?;
    db::put(&conn, "questions", &q.id, &q)?;
    Ok(Json(q))
}
fn validate_source(conn: &rusqlite::Connection, q: &Question) -> Result<(), Error> {
    if let Some(id) = &q.paper_id {
        if db::get::<String>(conn, "archived_papers", id).is_ok() {
            return Err(bad("来源已删除，请选择其他来源"));
        }
        let p: Paper = db::get(conn, "papers", id).map_err(|_| bad("来源试卷不存在"))?;
        if p.pages > 0 && q.page_end > p.pages {
            return Err(bad("页码超出来源试卷"));
        }
    }
    Ok(())
}
async fn update_question(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(mut q): Json<Question>,
) -> Api<Question> {
    let conn = lock(&s)?;
    let old: Question = db::get(&conn, "questions", &id).map_err(|_| missing())?;
    if q.revision != old.revision {
        return Err(Error(
            StatusCode::CONFLICT,
            "题目已被修改，请刷新后重试".into(),
        ));
    }
    q.id = id;
    if q.paper_id != old.paper_id {
        q.verified = false;
        q.status = "pending".into();
    }
    q.origins = old.origins;
    q.recognition = old.recognition;
    q.parent_id = old.parent_id;
    q.revision += 1;
    q.validate()?;
    validate_source(&conn, &q)?;
    db::put(&conn, "questions", &q.id, &q)?;
    Ok(Json(q))
}
async fn delete_question(State(s): State<Store>, Path(id): Path<String>) -> Api<Value> {
    let conn = lock(&s)?;
    let _: Question = db::get(&conn, "questions", &id).map_err(|_| missing())?;
    conn.execute("DELETE FROM questions WHERE id=?1", [&id])
        .map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(json!({"deleted":id})))
}
#[derive(Deserialize)]
struct Merge {
    ids: Vec<String>,
}
async fn merge_questions(State(s): State<Store>, Json(input): Json<Merge>) -> Api<Question> {
    let unique: std::collections::HashSet<_> = input.ids.iter().collect();
    if input.ids.len() < 2 || unique.len() != input.ids.len() {
        return Err(bad("请按顺序选择至少两个不同题目"));
    }
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    let qs: Vec<Question> = input
        .ids
        .iter()
        .map(|id| db::get(&tx, "questions", id))
        .collect::<Result<_, _>>()?;
    let mut q = qs[0].clone();
    if qs.iter().any(|x| x.paper_id != q.paper_id) {
        return Err(bad("只能合并同一来源的题目"));
    }
    q.content = qs
        .iter()
        .map(|q| q.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    q.origins = qs.iter().flat_map(|q| q.origins.clone()).collect();
    q.origins.dedup();
    q.recognition = None;
    q.page_start = qs.iter().map(|q| q.page_start).min().unwrap();
    q.page_end = qs.iter().map(|q| q.page_end).max().unwrap();
    q.status = "pending".into();
    q.verified = false;
    q.answer.clear();
    q.explanation.clear();
    q.revision += 1;
    q.validate()?;
    db::put(&tx, "questions", &q.id, &q)?;
    for id in input.ids.iter().skip(1) {
        tx.execute("DELETE FROM questions WHERE id=?1", [id])
            .map_err(|e| Error::from(e.to_string()))?;
    }
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(q))
}
#[derive(Deserialize)]
struct Split {
    parts: Vec<String>,
    revision: u64,
}
async fn split_question(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<Split>,
) -> Api<Vec<Question>> {
    if input.parts.len() < 2 || input.parts.len() > 100 {
        return Err(bad("拆分需要 2–100 段题干"));
    }
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    let old: Question = db::get(&tx, "questions", &id).map_err(|_| missing())?;
    if old.revision != input.revision {
        return Err(Error(StatusCode::CONFLICT, "题目已修改，请刷新".into()));
    }
    let mut out = Vec::new();
    for (i, part) in input.parts.into_iter().enumerate() {
        let mut q = old.clone();
        q.id = uuid::Uuid::new_v4().to_string();
        q.content = part;
        q.number = format!("{}-{}", old.number, i + 1);
        q.status = "pending".into();
        q.verified = false;
        q.answer.clear();
        q.explanation.clear();
        q.revision = old.revision + 1;
        q.recognition = None;
        q.validate()?;
        db::put(&tx, "questions", &q.id, &q)?;
        out.push(q);
    }
    tx.execute("DELETE FROM questions WHERE id=?1", [&id])
        .map_err(|e| Error::from(e.to_string()))?;
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(out))
}
async fn list_quizzes(State(s): State<Store>) -> Api<Vec<Quiz>> {
    Ok(Json(db::all(&*lock(&s)?, "quizzes")?))
}
async fn read_quiz(State(s): State<Store>, Path(id): Path<String>) -> Api<Quiz> {
    Ok(Json(
        db::get(&*lock(&s)?, "quizzes", &id).map_err(|_| missing())?,
    ))
}

#[derive(Deserialize)]
struct PdfOptions {
    #[serde(default)]
    answers: bool,
    #[serde(default = "default_answer_lines")]
    answer_lines: usize,
    font_id: Option<String>,
    page_size: Option<String>,
    font_size: Option<f32>,
    line_spacing: Option<f32>,
    margin: Option<f32>,
    columns: Option<u8>,
}
fn default_answer_lines() -> usize {
    3
}
async fn export_quiz_pdf(
    State(s): State<Store>,
    Path(id): Path<String>,
    Query(options): Query<PdfOptions>,
) -> Result<Response, Error> {
    let quiz: Quiz = db::get(&*lock(&s)?, "quizzes", &id).map_err(|_| missing())?;
    let mut style = crate::pdf::PdfStyle::default();
    if let Some(v) = options.page_size {
        style.page_size = v;
    }
    if let Some(v) = options.font_size {
        style.font_size = v;
    }
    if let Some(v) = options.line_spacing {
        style.line_spacing = v;
    }
    if let Some(v) = options.margin {
        style.margin = v;
    }
    if let Some(v) = options.columns {
        style.columns = v;
    }
    style.validate()?;
    let bytes_font = crate::fonts::load(
        &s,
        options
            .font_id
            .as_deref()
            .unwrap_or(crate::fonts::DEFAULT_FONT),
    )?;
    let bytes = tokio::task::spawn_blocking(move || {
        crate::pdf::render_with(
            &quiz,
            options.answers,
            options.answer_lines,
            style,
            &bytes_font,
        )
    })
    .await
    .map_err(|_| bad("PDF 生成失败"))??;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=paperdesk-quiz.pdf",
            ),
        ],
        bytes,
    )
        .into_response())
}
async fn preview_quiz(
    State(s): State<Store>,
    Json(input): Json<crate::sampling::QuizInput>,
) -> Api<crate::sampling::Preview> {
    let conn = lock(&s)?;
    let pool = crate::sampling::pool(&conn, &input)?;
    Ok(Json(crate::sampling::preview(&pool, &input)))
}
async fn create_quiz(
    State(s): State<Store>,
    Json(mut input): Json<crate::sampling::QuizInput>,
) -> Api<Quiz> {
    if input.title.trim().is_empty() {
        return Err(bad("请填写测验名称"));
    }
    input.filter.status = Some("valuable".into());
    let conn = lock(&s)?;
    let pool = crate::sampling::sample(crate::sampling::pool(&conn, &input)?, &input)?;
    let quiz = Quiz {
        id: uuid::Uuid::new_v4().to_string(),
        title: input.title,
        questions: pool,
        filter: input.filter,
        created_at: db::now(&conn),
        selection: input.selection,
        quotas: input.quotas,
    };
    db::put(&conn, "quizzes", &quiz.id, &quiz)?;
    Ok(Json(quiz))
}
async fn delete_quiz(State(s): State<Store>, Path(id): Path<String>) -> Api<Value> {
    let conn = lock(&s)?;
    let _: Quiz = db::get(&conn, "quizzes", &id).map_err(|_| missing())?;
    conn.execute("DELETE FROM quizzes WHERE id=?1", [&id])
        .map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(json!({"deleted":id})))
}
async fn list_jobs(State(s): State<Store>) -> Api<Vec<Job>> {
    Ok(Json(db::all(&*lock(&s)?, "jobs")?))
}
#[derive(Deserialize)]
struct JobInput {
    question_id: String,
    kind: String,
    #[serde(default)]
    instruction: String,
}
async fn create_job(State(s): State<Store>, Json(input): Json<JobInput>) -> Api<Job> {
    if !["explanation", "variant", "recognition"].contains(&input.kind.as_str()) {
        return Err(bad("任务类型应为 explanation、variant 或 recognition"));
    }
    let conn = lock(&s)?;
    let q: Question = db::get(&conn, "questions", &input.question_id).map_err(|_| missing())?;
    if let Some(existing) = db::all::<Job>(&conn, "jobs")?.into_iter().find(|j| {
        j.question_id == q.id
            && j.kind == input.kind
            && j.source_revision == q.revision
            && ["queued", "running", "review"].contains(&j.status.as_str())
    }) {
        return Ok(Json(existing));
    }
    let job = Job {
        id: uuid::Uuid::new_v4().to_string(),
        question_id: q.id.clone(),
        kind: input.kind,
        instruction: input.instruction,
        status: "queued".into(),
        source_revision: q.revision,
        source: q,
        result: None,
        created_at: db::now(&conn),
    };
    db::put(&conn, "jobs", &job.id, &job)?;
    Ok(Json(job))
}
async fn claim_job(State(s): State<Store>, Path(id): Path<String>) -> Api<Job> {
    let conn = lock(&s)?;
    let mut job: Job = db::get(&conn, "jobs", &id).map_err(|_| missing())?;
    if job.status != "queued" {
        return Err(Error(StatusCode::CONFLICT, "任务已被领取或已完成".into()));
    }
    job.status = "running".into();
    db::put(&conn, "jobs", &id, &job)?;
    Ok(Json(job))
}
async fn complete_job(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(result): Json<AgentResult>,
) -> Api<Job> {
    let conn = lock(&s)?;
    let mut job: Job = db::get(&conn, "jobs", &id).map_err(|_| missing())?;
    if job.status != "running" {
        return Err(Error(
            StatusCode::CONFLICT,
            "请先领取任务；已完成任务不能重复提交".into(),
        ));
    }
    if job.kind != "recognition"
        && (result.explanation.trim().is_empty() || result.answer.trim().is_empty())
    {
        return Err(bad("答案和解析不能为空"));
    }
    if ["variant", "recognition"].contains(&job.kind.as_str())
        && result
            .content
            .as_ref()
            .is_none_or(|v| v.trim().is_empty() || v.len() > 200_000)
    {
        return Err(bad("变式必须包含新题干"));
    }
    job.result = Some(result);
    job.status = "review".into();
    db::put(&conn, "jobs", &id, &job)?;
    Ok(Json(job))
}
async fn accept_job(State(s): State<Store>, Path(id): Path<String>) -> Api<Question> {
    let mut conn = lock(&s)?;
    let tx = conn.transaction().map_err(|e| Error::from(e.to_string()))?;
    let mut job: Job = db::get(&tx, "jobs", &id).map_err(|_| missing())?;
    if job.status != "review" {
        return Err(bad("只有待审核任务可以采纳"));
    }
    let result = job.result.as_ref().ok_or_else(|| bad("任务缺少结果"))?;
    let mut q: Question = db::get(&tx, "questions", &job.question_id).map_err(|_| missing())?;
    if q.revision != job.source_revision {
        return Err(Error(
            StatusCode::CONFLICT,
            "原题已修改，请重新生成，避免覆盖新内容".into(),
        ));
    }
    if job.kind == "recognition" {
        q.content = result.content.clone().unwrap_or_default();
        q.verified = false;
        q.status = "pending".into();
        q.revision += 1;
        if let Some(recognition) = &mut q.recognition {
            recognition.status = "agent_reviewed".into();
        }
    } else if job.kind == "variant" {
        q.recognition = None;
        q.parent_id = Some(q.id.clone());
        q.id = uuid::Uuid::new_v4().to_string();
        q.paper_id = None;
        q.number = format!("{}·变式", q.number);
        q.page_start = 1;
        q.page_end = 1;
        q.content = result.content.clone().unwrap_or_default();
        q.status = "pending".into();
        q.verified = false;
        q.revision = 0;
    } else {
        q.revision += 1;
    }
    if job.kind != "recognition" {
        q.answer = result.answer.clone();
        q.explanation = result.explanation.clone();
    }
    q.validate()?;
    db::put(&tx, "questions", &q.id, &q)?;
    job.status = "accepted".into();
    db::put(&tx, "jobs", &id, &job)?;
    tx.commit().map_err(|e| Error::from(e.to_string()))?;
    Ok(Json(q))
}
async fn reject_job(State(s): State<Store>, Path(id): Path<String>) -> Api<Job> {
    let conn = lock(&s)?;
    let mut job: Job = db::get(&conn, "jobs", &id).map_err(|_| missing())?;
    if !["queued", "running", "review"].contains(&job.status.as_str()) {
        return Err(bad("任务已结束"));
    }
    job.status = "rejected".into();
    db::put(&conn, "jobs", &id, &job)?;
    Ok(Json(job))
}
async fn export(State(s): State<Store>) -> Api<Value> {
    let conn = lock(&s)?;
    Ok(Json(
        json!({"schema_version":2,"imports":db::all::<crate::importer::ImportPlan>(&conn,"imports")?,"papers":db::all::<Paper>(&conn,"papers")?,"questions":db::all::<Question>(&conn,"questions")?,"quizzes":db::all::<Quiz>(&conn,"quizzes")?,"jobs":db::all::<Job>(&conn,"jobs")?}),
    ))
}

async fn vendor_asset(Path(path): Path<String>) -> Result<Response, Error> {
    let (mime, bytes) = crate::web_assets::asset(&path).ok_or_else(missing)?;
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}
async fn font_file(State(s): State<Store>, Path(id): Path<String>) -> Result<Response, Error> {
    let bytes = crate::fonts::load(&s, &id)?;
    Ok(([(header::CONTENT_TYPE, "font/ttf")], bytes).into_response())
}

async fn print_styles(Query(options): Query<PdfOptions>) -> Result<Response, Error> {
    let mut style = crate::pdf::PdfStyle::default();
    if let Some(value) = options.page_size {
        style.page_size = value;
    }
    if let Some(value) = options.font_size {
        style.font_size = value;
    }
    if let Some(value) = options.line_spacing {
        style.line_spacing = value;
    }
    if let Some(value) = options.margin {
        style.margin = value;
    }
    if let Some(value) = options.columns {
        style.columns = value;
    }
    style.validate()?;
    let font_id = options
        .font_id
        .unwrap_or_else(|| crate::fonts::DEFAULT_FONT.into());
    if font_id != crate::fonts::DEFAULT_FONT && uuid::Uuid::parse_str(&font_id).is_err() {
        return Err(bad("字体 ID 无效"));
    }
    let css = format!(
        "{}\n@font-face{{font-family:QuizText;src:url('/api/fonts/{font_id}/file')}}body{{font-size:{}pt;line-height:{}}}.questions{{column-count:{}}}@page{{size:{};margin:{}mm}}",
        include_str!("../web/print.css"),
        style.font_size,
        style.line_spacing,
        style.columns,
        style.page_size,
        style.margin
    );
    Ok(([(header::CONTENT_TYPE, "text/css; charset=utf-8")], css).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewQuestion {
    revision: u64,
    decision: String,
}
async fn review_question(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<ReviewQuestion>,
) -> Api<Question> {
    let conn = lock(&s)?;
    let mut question: Question = db::get(&conn, "questions", &id).map_err(|_| missing())?;
    if question.revision != input.revision {
        return Err(Error(
            StatusCode::CONFLICT,
            "题目已被修改，请重新打开预览核验".into(),
        ));
    }
    match input.decision.as_str() {
        "approve" => {
            question.verified = true;
            question.status = "valuable".into();
        }
        "return" => {
            question.verified = false;
            question.status = "pending".into();
        }
        "discard" => {
            question.verified = false;
            question.status = "discarded".into();
        }
        _ => return Err(bad("未知审核操作")),
    }
    question.revision += 1;
    question.validate()?;
    db::put(&conn, "questions", &id, &question)?;
    Ok(Json(question))
}
