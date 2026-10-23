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
