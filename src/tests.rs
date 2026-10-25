use crate::{
    api,
    db::{self, Store},
    model::Question,
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn setup() -> (tempfile::TempDir, Store, Router) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let app = api::router(store.clone());
    (dir, store, app)
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("X-Paperdesk", "1")
        .header("Content-Type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({})),
    )
}
async fn new_question(app: &Router, content: &str) -> Value {
    let q = Question::new(content.into());
    let (status, data) = call(app, "POST", "/api/questions", json!(q), None).await;
    assert_eq!(status, StatusCode::OK, "{data}");
    data
}

#[tokio::test]
async fn review_filter_sampling_and_snapshots() {
    let (_dir, _store, app) = setup();
    let mut q = new_question(&app, "求函数 f(x)=x² 的导数").await;
    let id = q["id"].as_str().unwrap().to_string();
    q["status"] = json!("valuable");
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/questions/{id}"),
            q.clone(),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    q["verified"] = json!(true);
    q["subject"] = json!("数学");
    q["tags"] = json!(["导数"]);
    let (status, saved) = call(
        &app,
        "PUT",
        &format!("/api/questions/{id}"),
        q.clone(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(&app, "PUT", &format!("/api/questions/{id}"), q, None)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (_, filtered) = call(
        &app,
        "GET",
        "/api/questions?subject=%E6%95%B0%E5%AD%A6&tag=%E5%AF%BC%E6%95%B0&min_difficulty=3",
        Value::Null,
        None,
    )
    .await;
    assert_eq!(filtered.as_array().unwrap().len(), 1);
    let pending = new_question(&app, "不能被抽取的待审题").await;
    let (status, quiz) = call(
        &app,
        "POST",
        "/api/quizzes",
        json!({"title":"导数练习","count":1,"filter":{"subject":"数学"}}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(quiz["questions"][0]["id"], saved["id"]);
    assert_ne!(quiz["questions"][0]["id"], pending["id"]);
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/quizzes",
            json!({"title":"不足","count":2}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/questions/{id}"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, snapshot) = call(
        &app,
        "GET",
        &format!("/api/quizzes/{}", quiz["id"].as_str().unwrap()),
        Value::Null,
        None,
    )
    .await;
    assert_eq!(snapshot["questions"][0]["content"], saved["content"]);
}

#[tokio::test]
async fn agent_claim_auth_revision_and_variant_review() {
    let (_dir, store, app) = setup();
    let q = new_question(&app, "原题").await;
    let (_, job) = call(
        &app,
        "POST",
        "/api/jobs",
        json!({"question_id":q["id"],"kind":"variant","instruction":"更简单"}),
        None,
    )
    .await;
    let id = job["id"].as_str().unwrap();
    assert_eq!(
        call(&app, "GET", "/api/agent/v1/questions", Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let claim = format!("/api/agent/v1/jobs/{id}/claim");
    assert_eq!(
        call(&app, "POST", &claim, Value::Null, Some(&store.token))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "POST", &claim, Value::Null, Some(&store.token))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let submit = format!("/api/agent/v1/jobs/{id}/result");
    let result = json!({"content":"变式题","answer":"2","explanation":"计算得到 2"});
    assert_eq!(
        call(&app, "POST", &submit, result.clone(), Some(&store.token))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "POST", &submit, result, Some(&store.token))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (status, variant) = call(
        &app,
        "POST",
        &format!("/api/jobs/{id}/accept"),
        Value::Null,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{variant}");
    assert_ne!(variant["id"], q["id"]);
    assert_eq!(variant["parent_id"], q["id"]);
    assert_eq!(variant["status"], "pending");
    assert_eq!(variant["verified"], false);
    assert!(variant["paper_id"].is_null());
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/jobs/{id}/accept"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, job2) = call(
        &app,
        "POST",
        "/api/jobs",
        json!({"question_id":q["id"],"kind":"explanation"}),
        None,
    )
    .await;
    let id2 = job2["id"].as_str().unwrap();
    call(
        &app,
        "POST",
        &format!("/api/agent/v1/jobs/{id2}/claim"),
        Value::Null,
        Some(&store.token),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/agent/v1/jobs/{id2}/result"),
        json!({"answer":"1","explanation":"解析"}),
        Some(&store.token),
    )
    .await;
    let mut changed = q.clone();
    changed["content"] = json!("原题已修改");
    call(
        &app,
        "PUT",
        &format!("/api/questions/{}", q["id"].as_str().unwrap()),
        changed,
        None,
    )
    .await;
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/jobs/{id2}/accept"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn split_is_atomic_and_merge_preserves_order() {
    let (_dir, store, app) = setup();
    let q = new_question(&app, "A\nB").await;
    let id = q["id"].as_str().unwrap();
    let path = format!("/api/questions/{id}/split");
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            json!({"parts":["A",""],"revision":0}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        db::all::<Question>(&store.db.lock().unwrap(), "questions")
            .unwrap()
            .len(),
        1
    );
    let (status, parts) = call(
        &app,
        "POST",
        &path,
        json!({"parts":["A","B"],"revision":0}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, merged) = call(
        &app,
        "POST",
        "/api/questions/merge",
        json!({"ids":[parts[1]["id"],parts[0]["id"]]}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(merged["content"], "B\n\nA");
    assert_eq!(merged["verified"], false);
}

#[tokio::test]
async fn multipart_import_and_reopen_persist() {
    let (dir, store, app) = setup();
    let body = "--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.txt\"\r\nContent-Type: text/plain\r\n\r\n1. 计算 1+1\n2. 求 x 的值\r\n--boundary--\r\n";
    let request = Request::builder()
        .method("POST")
        .uri("/api/import")
        .header("X-Paperdesk", "1")
        .header("Content-Type", "multipart/form-data; boundary=boundary")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let data: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(data["count"], 2);
    let reopened = Store::open(dir.path()).unwrap();
    assert_eq!(
        db::all::<Question>(&reopened.db.lock().unwrap(), "questions")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(store.token, reopened.token);
}

#[tokio::test]
async fn cross_site_write_and_rebinding_are_denied() {
    let (_dir, _store, app) = setup();
    let req = Request::builder()
        .method("POST")
        .uri("/api/questions")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let req = Request::builder()
        .uri("/api/export")
        .header("Host", "evil.example:3210")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

// A real, minimal PDF with an actual page tree, font, text stream, xref and trailer.
fn fixture_pdf(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 12 Tf 40 750 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    pdf
}

async fn upload_pdf(app: &Router, bytes: Vec<u8>) -> Value {
    let mut body=b"--pdfboundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"fixture.pdf\"\r\nContent-Type: application/pdf\r\n\r\n".to_vec();
    body.extend_from_slice(&bytes);
    body.extend_from_slice(b"\r\n--pdfboundary--\r\n");
    let request = Request::builder()
        .method("POST")
        .uri("/api/import")
        .header("X-Paperdesk", "1")
        .header("Content-Type", "multipart/form-data; boundary=pdfboundary")
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn real_pdf_extracts_text_and_preserves_source_bytes() {
    let (_dir, store, app) = setup();
    let pdf = fixture_pdf("1. Compute 2+3.");
    let result = upload_pdf(&app, pdf.clone()).await;
    assert_eq!(result["count"], 1);
    assert_eq!(result["paper"]["pages"], 1);
    let id = result["paper"]["id"].as_str().unwrap();
    let qs = db::all::<Question>(&store.db.lock().unwrap(), "questions").unwrap();
    assert!(qs[0].content.contains("Compute 2+3."));
    assert!(!qs[0].verified);
    let req = Request::builder()
        .uri(format!("/api/papers/{id}/source"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.headers()["content-type"], "application/pdf");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'self'")
    );
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        pdf.as_slice()
    );
}

#[tokio::test]
async fn no_text_pdf_can_receive_external_ocr_without_duplicates() {
    let (_dir, _store, app) = setup();
    let result = upload_pdf(&app, fixture_pdf("")).await;
    assert_eq!(result["count"], 0);
    assert!(result["paper"]["warning"].as_str().unwrap().contains("OCR"));
    let id = result["paper"]["id"].as_str().unwrap();
    let path = format!("/api/papers/{id}/text");
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            json!({"pages":["第一页", "不存在的第二页"]}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let body = json!({"pages":["1. OCR 第一题\n2. OCR 第二题"]});
    let (status, attached) = call(&app, "POST", &path, body.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(attached["count"], 2);
    assert_eq!(
        call(&app, "POST", &path, body, None).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn re_recognition_replaces_only_unedited_candidates() {
    let (_dir, store, app) = setup();
    let imported = upload_pdf(&app, fixture_pdf("1. Compute 2+3.")).await;
    let id = imported["paper"]["id"].as_str().unwrap();
    let path = format!("/api/papers/{id}/recognize");
    let (status, result) = call(&app, "POST", &path, json!({}), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["count"], 1);
    let mut q = db::all::<Question>(&store.db.lock().unwrap(), "questions")
        .unwrap()
        .remove(0);
    q.content = "Human corrected formula".into();
    q.revision = 1;
    db::put(&store.db.lock().unwrap(), "questions", &q.id, &q).unwrap();
    let (status, _) = call(&app, "POST", &path, json!({}), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let saved: Question = db::get(&store.db.lock().unwrap(), "questions", &q.id).unwrap();
    assert_eq!(saved.content, q.content);
}

#[tokio::test]
async fn quiz_pdf_endpoint_downloads_student_and_teacher_documents() {
    use crate::model::{Filter, Quiz};
    let (_dir, store, app) = setup();
    let mut question = Question::new("计算 3 + 4。".into());
    question.answer = "SECRET_ANSWER_7".into();
    question.explanation = "将两个数相加。".into();
    let quiz = Quiz {
        id: "pdf-test".into(),
        title: "测验 PDF 接口".into(),
        questions: vec![question],
        filter: Filter::default(),
        created_at: String::new(),
        selection: Default::default(),
        quotas: vec![],
    };
    db::put(&store.db.lock().unwrap(), "quizzes", &quiz.id, &quiz).unwrap();
    for answers in [false, true] {
        let request = Request::builder()
            .uri(format!(
                "/api/quizzes/pdf-test/pdf?answers={answers}&answer_lines=3"
            ))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/pdf");
        assert!(
            response.headers()["content-disposition"]
                .to_str()
                .unwrap()
                .contains("attachment")
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = pdf_extract::extract_text_from_mem(&bytes).unwrap();
        assert!(text.contains("测验 PDF 接口"));
        assert_eq!(text.contains("SECRET_ANSWER_7"), answers);
    }
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/quizzes/pdf-test/pdf?answer_lines=13",
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn source_rename_reassignment_and_old_snapshot_are_safe() {
    let (_dir, _store, app) = setup();
    let (_, a) = call(&app, "POST", "/api/papers", json!({"name":"期中卷"}), None).await;
    let (_, b) = call(&app, "POST", "/api/papers", json!({"name":"教材"}), None).await;
    let mut q = new_question(&app, "来源变更题").await;
    q["paper_id"] = a["id"].clone();
    let (_, mut q) = call(
        &app,
        "PUT",
        &format!("/api/questions/{}", q["id"].as_str().unwrap()),
        q.clone(),
        None,
    )
    .await;
    q["verified"] = json!(true);
    q["status"] = json!("valuable");
    let (_, mut q) = call(
        &app,
        "PUT",
        &format!("/api/questions/{}", q["id"].as_str().unwrap()),
        q.clone(),
        None,
    )
    .await;
    let (_, snapshot) = call(
        &app,
        "POST",
        "/api/quizzes",
        json!({"title":"来源快照","count":1}),
        None,
    )
    .await;
    let path = format!("/api/papers/{}", a["id"].as_str().unwrap());
    assert_eq!(
        call(
            &app,
            "PUT",
            &path,
            json!({"name":"2026期中卷","expected_name":"期中卷"}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &path,
            json!({"name":"过时覆盖","expected_name":"期中卷"}),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    q["paper_id"] = b["id"].clone();
    let (status, moved) = call(
        &app,
        "PUT",
        &format!("/api/questions/{}", q["id"].as_str().unwrap()),
        q,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["paper_id"], b["id"]);
    assert_eq!(moved["verified"], false);
    assert_eq!(moved["status"], "pending");
    let (_, preserved) = call(
        &app,
        "GET",
        &format!("/api/quizzes/{}", snapshot["id"].as_str().unwrap()),
        Value::Null,
        None,
    )
    .await;
    assert_eq!(preserved["questions"][0]["paper_id"], a["id"]);
    let mut legacy = preserved.clone();
    legacy.as_object_mut().unwrap().remove("selection");
    legacy.as_object_mut().unwrap().remove("quotas");
    assert!(serde_json::from_value::<crate::model::Quiz>(legacy).is_ok());
}

#[tokio::test]
async fn advanced_ranges_knowledge_quota_and_history_exclusion() {
    let (_dir, store, app) = setup();
    let (_, p) = call(
        &app,
        "POST",
        "/api/papers",
        json!({"name":"高级组卷卷"}),
        None,
    )
    .await;
    let paper_id = p["id"].as_str().unwrap();
    for (number, difficulty, tags, subject, variant) in [
        ("1", 2, vec!["函数", "导数"], "数学", false),
        ("2", 4, vec!["函数", "积分"], "数学", false),
        ("3", 3, vec!["几何"], "数学", false),
        ("4", 2, vec!["函数"], "物理", false),
        ("5", 1, vec!["函数"], "数学", false),
        ("6", 4, vec!["函数", "导数"], "数学", true),
    ] {
        let mut q = Question::new(format!("题目 {number}"));
        q.number = number.into();
        q.difficulty = difficulty;
        q.subject = subject.into();
        q.tags = tags.into_iter().map(String::from).collect();
        q.paper_id = Some(paper_id.into());
        q.status = "valuable".into();
        q.verified = true;
        if variant {
            q.parent_id = Some("parent".into());
        }
        db::put(&store.db.lock().unwrap(), "questions", &q.id, &q).unwrap();
    }
    let body = json!({"title":"分层练习","count":2,"filter":{"min_difficulty":2,"max_difficulty":4},"selection":{"subjects":["数学"],"paper_ids":[paper_id],"tags":["函数"],"original_only":true,"numbers":["1","2","6"],"min_page":1,"max_page":1},"quotas":[{"difficulty":2,"count":1},{"difficulty":4,"count":1}]});
    let (status, preview) = call(&app, "POST", "/api/quizzes/preview", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["eligible"], 2);
    assert_eq!(preview["by_difficulty"], json!([0, 1, 0, 1, 0]));
    assert_eq!(preview["can_create"], true);
    let (status, quiz) = call(&app, "POST", "/api/quizzes", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    let qs = quiz["questions"].as_array().unwrap();
    assert_eq!(qs.len(), 2);
    assert_ne!(qs[0]["id"], qs[1]["id"]);
    assert_eq!(quiz["quotas"][0]["count"], 1);
    let mut excluding = body.clone();
    excluding["selection"]["exclude_quiz_ids"] = json!([quiz["id"]]);
    let (_, preview) = call(&app, "POST", "/api/quizzes/preview", excluding, None).await;
    assert_eq!(preview["eligible"], 0);
    assert_eq!(preview["can_create"], false);
    let mut all_tags = body.clone();
    all_tags["count"] = json!(1);
    all_tags["quotas"] = json!([]);
    all_tags["selection"]["tags"] = json!(["函数", "导数"]);
    all_tags["selection"]["tag_mode"] = json!("all");
    let (_, preview) = call(&app, "POST", "/api/quizzes/preview", all_tags, None).await;
    assert_eq!(preview["eligible"], 1);
    let mut insufficient = body.clone();
    insufficient["quotas"] = json!([{"difficulty":4,"count":2}]);
    assert_eq!(
        call(&app, "POST", "/api/quizzes", insufficient, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut invalid = body;
    invalid["filter"]["min_difficulty"] = json!(5);
    invalid["filter"]["max_difficulty"] = json!(1);
    assert_eq!(
        call(&app, "POST", "/api/quizzes/preview", invalid, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn structured_import_is_atomic_and_protects_review_work() {
    let (_dir, _store, app) = setup();
    let (_, p) = call(
        &app,
        "POST",
        "/api/papers",
        json!({"name":"解析器输出"}),
        None,
    )
    .await;
    let path = format!("/api/papers/{}/structured", p["id"].as_str().unwrap());
    let mut body = json!({"engine":"fixture-parser","questions":[{"number":"1","content":"第一题","page_start":1,"page_end":1},{"number":"2","content":"第二题","page_start":1,"page_end":1}]});
    assert_eq!(
        call(&app, "POST", &path, body.clone(), None).await.0,
        StatusCode::OK
    );
    body["replace_pending"] = json!(true);
    body["questions"][1]["content"] = json!("");
    assert_eq!(
        call(&app, "POST", &path, body.clone(), None).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, questions) = call(&app, "GET", "/api/questions", Value::Null, None).await;
    assert_eq!(questions.as_array().unwrap().len(), 2);
    let mut q = questions[0].clone();
    q["content"] = json!("人工修订题");
    let id = q["id"].as_str().unwrap().to_string();
    call(&app, "PUT", &format!("/api/questions/{id}"), q, None).await;
    body["questions"][1]["content"] = json!("第二题");
    assert_eq!(
        call(&app, "POST", &path, body, None).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn uploaded_font_is_persistent_and_invalid_fonts_are_rejected() {
    let (dir, store, app) = setup();
    async fn upload(app: &Router, bytes: &[u8]) -> (StatusCode, Value) {
        let mut body=b"--font\r\nContent-Disposition: form-data; name=\"file\"; filename=\"Custom.ttf\"\r\nContent-Type: font/ttf\r\n\r\n".to_vec();
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n--font--\r\n");
        let req = Request::builder()
            .method("POST")
            .uri("/api/fonts")
            .header("X-Paperdesk", "1")
            .header("Content-Type", "multipart/form-data; boundary=font")
            .body(Body::from(body))
            .unwrap();
        let r = app.clone().oneshot(req).await.unwrap();
        let status = r.status();
        let data = r.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&data).unwrap())
    }
    assert_eq!(upload(&app, b"not a font").await.0, StatusCode::BAD_REQUEST);
    let (status, font) = upload(&app, crate::fonts::DEFAULT_BYTES).await;
    assert_eq!(status, StatusCode::OK);
    let reopened = Store::open(dir.path()).unwrap();
    assert_eq!(crate::fonts::list(&reopened).unwrap().len(), 2);
    assert_eq!(
        crate::fonts::load(&store, font["id"].as_str().unwrap()).unwrap(),
        crate::fonts::DEFAULT_BYTES
    );
    assert!(crate::fonts::load(&store, "../paperdesk.sqlite3").is_err());
}

async fn upload_batch(
    app: &Router,
    files: Vec<(&str, Vec<u8>)>,
    options: Value,
) -> (StatusCode, Value) {
    let mut bytes =
        format!("--batch\r\nContent-Disposition: form-data; name=\"options\"\r\n\r\n{options}\r\n")
            .into_bytes();
    for (name, body) in files {
        bytes.extend(format!("--batch\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        bytes.extend(body);
        bytes.extend(b"\r\n");
    }
    bytes.extend(b"--batch--\r\n");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import")
                .header("X-Paperdesk", "1")
                .header("Content-Type", "multipart/form-data; boundary=batch")
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}
fn image_fixture() -> Vec<u8> {
    let mut im = image::RgbImage::from_pixel(120, 160, image::Rgb([255, 255, 255]));
    for y in 20..45 {
        for x in 10..100 {
            im.put_pixel(x, y, image::Rgb([0, 0, 0]));
        }
    }
    for y in 100..125 {
        for x in 10..100 {
            im.put_pixel(x, y, image::Rgb([60, 60, 60]));
        }
    }
    let mut out = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(im)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}
async fn imported_image(app: &Router) -> String {
    let (status, data) = upload_batch(
        app,
        vec![("questions.png", image_fixture())],
        json!({"prepare_only":true,"layout":"manual"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{data}");
    assert_eq!(data["count"], 1);
    data["paper"]["id"].as_str().unwrap().to_string()
}
#[tokio::test]
async fn batch_markdown_text_partial_failures_and_originals() {
    let (_dir, store, app) = setup();
    let md = b"# Notes\n## 1. First question\nformula $x^2$\n## 2. Second question".to_vec();
    let (status, data) = upload_batch(
        &app,
        vec![
            ("lesson.md", md.clone()),
            ("exam.txt", b"1. Text question".to_vec()),
            ("bad.pdf", b"not pdf".to_vec()),
        ],
        json!({"prepare_only":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["count"], 2);
    assert_eq!(results[1]["count"], 1);
    assert!(results[2]["error"].is_string());
    let id = results[0]["paper"]["id"].as_str().unwrap();
    assert_eq!(
        std::fs::read(store.dir.join("papers").join(format!("{id}.md"))).unwrap(),
        md
    );
    let questions = db::questions(
        &store.db.lock().unwrap(),
        &crate::model::Filter {
            paper_id: Some(id.into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(questions[0].origins[0].line_start, Some(2));
    assert_eq!(questions[0].origins[0].line_end, Some(3));
    assert_eq!(questions[0].origins[0].paper_id, id);
    assert!(!questions[0].verified);
}
#[tokio::test]
async fn image_regions_are_saved_before_ocr_and_protected_after_edits() {
    let (_dir, store, app) = setup();
    let id = imported_image(&app).await;
    let (_, before) = call(
        &app,
        "GET",
        &format!("/api/imports/{id}"),
        Value::Null,
        None,
    )
    .await;
    let old = before["questions"][0]["id"].as_str().unwrap();
    let original = std::fs::read(store.dir.join("papers").join(format!("{id}.png"))).unwrap();
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/imports/{id}/regions"),
        json!({"revision":0,"regions":[{"page":1,"rect":{"x":0,"y":0,"width":121,"height":80}}]}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(db::get::<Question>(&store.db.lock().unwrap(), "questions", old).is_ok());
    let regions = json!({"revision":0,"regions":[{"page":1,"rect":{"x":0,"y":0,"width":120,"height":80}},{"page":1,"rect":{"x":0,"y":80,"width":120,"height":80}}]});
    let (status, result) = call(
        &app,
        "POST",
        &format!("/api/imports/{id}/regions"),
        regions.clone(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["questions"].as_array().unwrap().len(), 2);
    let mut q = result["questions"][0].clone();
    let qid = q["id"].as_str().unwrap().to_string();
    assert_eq!(q["recognition"]["status"], "pending");
    assert!(!q["verified"].as_bool().unwrap());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/questions/{qid}/source/0"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let image = response.into_body().collect().await.unwrap().to_bytes();
    let crop = image::load_from_memory(&image).unwrap();
    assert_eq!((crop.width(), crop.height()), (120, 80));
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/agent/v1/questions/{qid}/source/0"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let mut forged = q.clone();
    forged["content"] = json!("Human verified text");
    forged["origins"] = json!([]);
    forged["recognition"] = Value::Null;
    let (status, saved) = call(&app, "PUT", &format!("/api/questions/{qid}"), forged, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["origins"], q["origins"]);
    assert_eq!(saved["recognition"], q["recognition"]);
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{qid}/recognize"),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/imports/{id}/regions"),
            json!({"revision":1,"regions":regions["regions"]}),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    q["content"] = json!("stale");
    assert_eq!(
        call(&app, "PUT", &format!("/api/questions/{qid}"), q, None)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        std::fs::read(store.dir.join("papers").join(format!("{id}.png"))).unwrap(),
        original
    );
}
#[tokio::test]
async fn recognition_agent_jobs_require_review_and_keep_evidence() {
    let (_dir, store, app) = setup();
    let (_, data) = upload_batch(
        &app,
        vec![("text.txt", b"1. Human question".to_vec())],
        json!({"prepare_only":true,"agent_assist":true}),
    )
    .await;
    let id = data["paper"]["id"].as_str().unwrap();
    let (_, plan) = call(
        &app,
        "GET",
        &format!("/api/imports/{id}"),
        Value::Null,
        None,
    )
    .await;
    let qid = plan["questions"][0]["id"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{qid}/recognize"),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{qid}/recognize"),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let jobs = db::all::<crate::model::Job>(&store.db.lock().unwrap(), "jobs").unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, "recognition");
    let job = &jobs[0];
    let token = store.token.to_string();
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/agent/v1/jobs/{}/claim", job.id),
            json!({}),
            Some(&token)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/agent/v1/jobs/{}/result", job.id),
            json!({"content":"Corrected transcription","answer":"A","explanation":"Verified steps"}),
            Some(&token)
        )
        .await
        .0,
        StatusCode::OK
    );
    let before: Question = db::get(&store.db.lock().unwrap(), "questions", qid).unwrap();
    assert_eq!(before.content, "Human question");
    let (status, result) = call(
        &app,
        "POST",
        &format!("/api/jobs/{}/accept", job.id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["content"], "Corrected transcription");
    assert_eq!(result["answer"], "A");
    assert_eq!(result["explanation"], "Verified steps");
    assert_eq!(result["verified"], false);
    assert_eq!(result["status"], "pending");
    assert_eq!(
        result["origins"],
        serde_json::to_value(before.origins).unwrap()
    );
}
#[tokio::test]
async fn manual_split_and_merge_retain_every_original_crop() {
    let (_dir, _store, app) = setup();
    let id = imported_image(&app).await;
    let (_, data) = call(
        &app,
        "GET",
        &format!("/api/imports/{id}"),
        Value::Null,
        None,
    )
    .await;
    let q = &data["questions"][0];
    let qid = q["id"].as_str().unwrap();
    let (status, split) = call(
        &app,
        "POST",
        &format!("/api/questions/{qid}/split"),
        json!({"revision":0,"parts":["Part one","Part two"]}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(split[0]["origins"], q["origins"]);
    assert_eq!(split[1]["origins"], q["origins"]);
    assert!(split[0]["revision"].as_u64().unwrap() > 0);
    let (status, merged) = call(
        &app,
        "POST",
        "/api/questions/merge",
        json!({"ids":[split[0]["id"],split[1]["id"]]}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(merged["origins"], q["origins"]);
}

#[tokio::test]
async fn math_print_styles_validate_options_and_vendor_paths() {
    let (_dir, _store, app) = setup();
    for path in [
        "/vendor/katex/katex.min.js",
        "/vendor/katex/fonts/KaTeX_Main-Regular.woff2",
        "/api/fonts/builtin-sans/file",
        "/print.css?columns=2&page_size=a5&font_size=12",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let csp = response.headers()["content-security-policy"]
            .to_str()
            .unwrap();
        assert!(csp.contains("script-src 'self';"));
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());
        if path.starts_with("/print.css") {
            let css = String::from_utf8(body.to_vec()).unwrap();
            assert!(css.contains("column-count:2"));
            assert!(css.contains("size:a5"));
        }
    }
    for (path, status) in [
        ("/vendor/not-a-bundled-file", StatusCode::NOT_FOUND),
        ("/print.css?columns=3", StatusCode::BAD_REQUEST),
        ("/print.css?font_id=bad-font", StatusCode::BAD_REQUEST),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
    }
}

#[tokio::test]
async fn preview_review_requires_transcription_and_current_revision() {
    let (_dir, store, app) = setup();
    let paper = imported_image(&app).await;
    let q = db::all::<Question>(&store.db.lock().unwrap(), "questions")
        .unwrap()
        .into_iter()
        .find(|q| q.paper_id.as_deref() == Some(&paper))
        .unwrap();
    assert!(q.content.is_empty());
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{}/review", q.id),
            json!({"revision":0,"decision":"approve"}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut edited = q.clone();
    edited.content = "求函数 $f(x)=x^2$ 的导数".into();
    let (status, edited) = call(
        &app,
        "PUT",
        &format!("/api/questions/{}", q.id),
        json!(edited),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{}/review", q.id),
            json!({"revision":0,"decision":"approve"}),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, approved) = call(
        &app,
        "POST",
        &format!("/api/questions/{}/review", q.id),
        json!({"revision":edited["revision"],"decision":"approve"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(approved["status"], "valuable");
    assert_eq!(approved["verified"], true);
    assert_eq!(approved["origins"], json!(q.origins));
    let (status, returned) = call(
        &app,
        "POST",
        &format!("/api/questions/{}/review", q.id),
        json!({"revision":approved["revision"],"decision":"return"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(returned["status"], "pending");
    assert_eq!(returned["verified"], false);
    let mut invalid = returned.clone();
    invalid["content"] = json!(crate::model::LEGACY_OCR_PLACEHOLDER);
    invalid["verified"] = json!(true);
    invalid["status"] = json!("valuable");
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/questions/{}", q.id),
            invalid,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, discarded) = call(
        &app,
        "POST",
        &format!("/api/questions/{}/review", q.id),
        json!({"revision":returned["revision"],"decision":"discard"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(discarded["status"], "discarded");
    assert_eq!(discarded["origins"], json!(q.origins));
}

#[tokio::test]
async fn legacy_placeholder_cannot_be_verified_and_blank_crops_can_be_discarded() {
    let (_dir, store, app) = setup();
    let paper = imported_image(&app).await;
    let mut q = db::all::<Question>(&store.db.lock().unwrap(), "questions")
        .unwrap()
        .into_iter()
        .find(|q| q.paper_id.as_deref() == Some(&paper))
        .unwrap();
    let endpoint = format!("/api/questions/{}/review", q.id);
    let (status, discarded) = call(
        &app,
        "POST",
        &endpoint,
        json!({"revision":0,"decision":"discard"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(discarded["content"].as_str().unwrap().is_empty());
    q.content = crate::model::LEGACY_OCR_PLACEHOLDER.into();
    q.revision = 1;
    db::put(&store.db.lock().unwrap(), "questions", &q.id, &q).unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            &endpoint,
            json!({"revision":1,"decision":"approve"}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &endpoint,
            json!({"revision":1,"decision":"unknown"}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn settings_revision_validation_and_persistence() {
    let (dir, store, app) = setup();
    let (_, mut value) = call(&app, "GET", "/api/settings", Value::Null, None).await;
    value["library_status"] = json!("valuable");
    value["engine"] = json!("service");
    let (status, saved) = call(&app, "PUT", "/api/settings", value.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["revision"], 1);
    assert_eq!(
        call(&app, "PUT", "/api/settings", value, None).await.0,
        StatusCode::CONFLICT
    );
    let mut invalid = saved.clone();
    invalid["layout"] = json!("unsupported");
    assert_eq!(
        call(&app, "PUT", "/api/settings", invalid, None).await.0,
        StatusCode::BAD_REQUEST
    );
    drop(app);
    drop(store);
    let app = api::router(Store::open(dir.path()).unwrap());
    assert_eq!(
        call(&app, "GET", "/api/settings", Value::Null, None)
            .await
            .1,
        saved
    );
}
#[tokio::test]
async fn source_delete_retains_questions_and_cancels_jobs() {
    let (_dir, store, app) = setup();
    let (_, p) = call(&app, "POST", "/api/papers", json!({"name":"Source"}), None).await;
    let mut q = Question::new("Original $x^2$".into());
    q.paper_id = Some(p["id"].as_str().unwrap().into());
    let (_, q) = call(&app, "POST", "/api/questions", json!(q), None).await;
    let job_input = json!({"question_id":q["id"],"kind":"recognition"});
    let (_, job) = call(&app, "POST", "/api/jobs", job_input.clone(), None).await;
    assert_eq!(
        call(&app, "POST", "/api/jobs", job_input, None).await.1["id"],
        job["id"]
    );
    let path = format!("/api/papers/{}", p["id"].as_str().unwrap());
    let mut input =
        json!({"expected_name":"Source","expected_question_ids":[],"delete_questions":false});
    assert_eq!(
        call(&app, "DELETE", &path, input.clone(), None).await.0,
        StatusCode::CONFLICT
    );
    input["expected_question_ids"] = json!([q["id"]]);
    assert_eq!(
        call(&app, "DELETE", &path, input, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/api/papers", Value::Null, None).await.1,
        json!([])
    );
    let (_, kept) = call(
        &app,
        "GET",
        &format!("/api/questions/{}", q["id"].as_str().unwrap()),
        Value::Null,
        None,
    )
    .await;
    assert_eq!(kept["content"], q["content"]);
    assert!(kept["paper_id"].is_null());
    assert_eq!(kept["revision"], 1);
    let jobs = call(&app, "GET", "/api/jobs", Value::Null, None).await.1;
    assert_eq!(jobs[0]["status"], "rejected");
    assert!(
        db::get::<crate::model::Paper>(
            &store.db.lock().unwrap(),
            "papers",
            p["id"].as_str().unwrap()
        )
        .is_ok()
    );
}
#[tokio::test]
async fn verified_question_can_be_sent_for_agent_review_without_overwrite() {
    let (_dir, _store, app) = setup();
    let q = new_question(&app, "Reviewed $x^2$").await;
    let id = q["id"].as_str().unwrap();
    let (_, approved) = call(
        &app,
        "POST",
        &format!("/api/questions/{id}/review"),
        json!({"revision":0,"decision":"approve"}),
        None,
    )
    .await;
    let (status, job) = call(
        &app,
        "POST",
        "/api/jobs",
        json!({"question_id":id,"kind":"recognition","instruction":"Recheck"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job["source_revision"], approved["revision"]);
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/questions/{id}"),
            Value::Null,
            None
        )
        .await
        .1,
        approved
    );
}

#[tokio::test]
async fn deleting_source_with_questions_keeps_other_questions_and_blocks_reimport() {
    let (_dir, _store, app) = setup();
    let (_, p) = call(
        &app,
        "POST",
        "/api/papers",
        json!({"name":"Delete test"}),
        None,
    )
    .await;
    let mut q = Question::new("Attached".into());
    q.paper_id = Some(p["id"].as_str().unwrap().into());
    let (_, q) = call(&app, "POST", "/api/questions", json!(q), None).await;
    let independent = new_question(&app, "Independent").await;
    let path = format!("/api/papers/{}", p["id"].as_str().unwrap());
    assert_eq!(call(&app,"DELETE",&path,json!({"expected_name":"Delete test","expected_question_ids":[q["id"]],"delete_questions":true}),None).await.0,StatusCode::OK);
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/questions/{}", q["id"].as_str().unwrap()),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/questions/{}", independent["id"].as_str().unwrap()),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "POST", &format!("{path}/text"), json!({}), None)
            .await
            .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn automatic_image_regions_must_be_confirmed_before_recognition() {
    let (_dir, _store, app) = setup();
    let id = imported_image(&app).await;
    let (_, before) = call(
        &app,
        "GET",
        &format!("/api/imports/{id}"),
        Value::Null,
        None,
    )
    .await;
    assert_eq!(before["plan"]["regions_confirmed"], false);
    let q = before["questions"][0]["id"].as_str().unwrap();
    let (status, error) = call(
        &app,
        "POST",
        &format!("/api/questions/{q}/recognize"),
        json!({"only_agent":true,"agent_assist":true}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("保存拆分"));
    let (status, saved) = call(
        &app,
        "POST",
        &format!("/api/imports/{id}/regions"),
        json!({"revision":0,"regions":[{"page":1,"rect":{"x":0,"y":0,"width":120,"height":80}}]}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["plan"]["regions_confirmed"], true);
    let id = saved["questions"][0]["id"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{id}/recognize"),
            json!({"only_agent":true,"agent_assist":true}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn operation_progress_remains_readable_while_heavy_work_is_queued() {
    let (_dir, store, app) = setup();
    let id = imported_image(&app).await;
    let (_, created) = call(&app, "POST", "/api/operations", json!({}), None).await;
    let operation = created["id"].as_str().unwrap().to_string();
    let permit = store.import_gate.clone().acquire_owned().await.unwrap();
    let request=Request::builder().method("POST").uri(format!("/api/imports/{id}/regions")).header("X-Paperdesk","1").header("X-Paperdesk-Operation",&operation).header("Content-Type","application/json").body(Body::from(json!({"revision":0,"regions":[{"page":1,"rect":{"x":0,"y":0,"width":120,"height":80}}]}).to_string())).unwrap();
    let task = tokio::spawn(app.clone().oneshot(request));
    let mut queued = false;
    for _ in 0..50 {
        let snapshot = store
            .operations
            .snapshot(&operation)
            .unwrap_or_else(|e| panic!("{}", e.1));
        if snapshot.status == "running" {
            queued = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(queued);
    assert_eq!(
        call(&app, "GET", "/api/health", Value::Null, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/operations/{operation}"),
            Value::Null,
            None
        )
        .await
        .1["status"],
        "running"
    );
    drop(permit);
    assert_eq!(task.await.unwrap().unwrap().status(), StatusCode::OK);
    let finished = store
        .operations
        .snapshot(&operation)
        .unwrap_or_else(|e| panic!("{}", e.1));
    assert_eq!(finished.status, "succeeded");
    assert_eq!(finished.completed, 1);
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("X-Paperdesk-Operation", operation.parse().unwrap());
    assert!(store.operations.claim(&headers).is_err());
}

#[tokio::test]
async fn recognized_question_region_edit_preserves_content_and_revision_history() {
    let (_dir, store, app) = setup();
    let paper = imported_image(&app).await;
    let (_, plan) = call(
        &app,
        "GET",
        &format!("/api/imports/{paper}"),
        Value::Null,
        None,
    )
    .await;
    let mut q = plan["questions"][0].clone();
    let id = q["id"].as_str().unwrap().to_string();
    q["content"] = json!("Edited math question");
    q["answer"] = json!("42");
    q["explanation"] = json!("Original analysis");
    let (_, saved) = call(&app, "PUT", &format!("/api/questions/{id}"), q, None).await;
    let (status, job) = call(
        &app,
        "POST",
        "/api/jobs",
        json!({"question_id":id,"kind":"recognition"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let revision = saved["revision"].as_u64().unwrap();
    let body =
        json!({"revision":revision,"origin_index":0,"rect":{"x":5,"y":10,"width":100,"height":70}});
    let (status, updated) = call(
        &app,
        "POST",
        &format!("/api/questions/{id}/region"),
        body.clone(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    for key in ["id", "number", "content", "answer", "explanation"] {
        assert_eq!(updated[key], saved[key]);
    }
    assert_eq!(updated["revision"], revision + 1);
    assert_eq!(updated["verified"], false);
    assert_eq!(updated["status"], "pending");
    assert_ne!(
        updated["origins"][0]["crop_id"],
        saved["origins"][0]["crop_id"]
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/questions/{id}/region"),
            body,
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let history: Question = db::get(
        &store.db.lock().unwrap(),
        "question_region_history",
        &format!("{id}:{revision}"),
    )
    .unwrap();
    assert_eq!(history.content, "Edited math question");
    let old_job: crate::model::Job = db::get(
        &store.db.lock().unwrap(),
        "jobs",
        job["id"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(old_job.status, "rejected");
    assert_eq!(
        db::all::<Question>(&store.db.lock().unwrap(), "questions")
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn operation_interruption_and_failure_have_terminal_status() {
    let registry = crate::operations::Registry::default();
    let operation = registry.create().unwrap_or_else(|e| panic!("{}", e.1));
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("X-Paperdesk-Operation", operation.id.parse().unwrap());
    let progress = registry
        .claim(&headers)
        .unwrap_or_else(|e| panic!("{}", e.1));
    {
        let _completion = progress.guard();
        progress.set("识别正文与公式", 0, None, "");
    }
    let failed = registry
        .snapshot(&operation.id)
        .unwrap_or_else(|e| panic!("{}", e.1));
    assert_eq!(failed.status, "failed");
    assert!(failed.message.contains("中断"));
    let operation = registry.create().unwrap_or_else(|e| panic!("{}", e.1));
    headers.insert("X-Paperdesk-Operation", operation.id.parse().unwrap());
    let progress = registry
        .claim(&headers)
        .unwrap_or_else(|e| panic!("{}", e.1));
    {
        let _completion = progress.guard();
        progress.set("保存", 0, Some(3), "题");
        progress.finish(None);
    }
    let completed = registry
        .snapshot(&operation.id)
        .unwrap_or_else(|e| panic!("{}", e.1));
    assert_eq!(completed.status, "succeeded");
    assert_eq!(completed.completed, 3);
    assert_eq!(completed.phase, "处理完成");
}
