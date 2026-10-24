//! A framework-neutral one-job adapter. Model credentials belong in the provider, not this service.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = std::env::args()
        .nth(1)
        .ok_or("Usage: agent_worker <provider-executable>")?;
    let port = std::env::var("PAPERDESK_PORT").unwrap_or_else(|_| "3210".into());
    let base = format!("http://127.0.0.1:{port}/api/agent/v1");
    let data = std::env::var("PAPERDESK_DATA").unwrap_or_else(|_| "data".into());
    let token = std::env::var("PAPERDESK_AGENT_TOKEN")
        .or_else(|_| std::fs::read_to_string(format!("{data}/agent-token.txt")))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let jobs: Vec<Value> = client
        .get(format!("{base}/jobs"))
        .bearer_auth(token.trim())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(job) = jobs.iter().find(|j| j["status"] == "queued") else {
        println!("没有待处理任务");
        return Ok(());
    };
    let id = job["id"].as_str().ok_or("missing job ID")?;
    let mut job: Value = client
        .post(format!("{base}/jobs/{id}/claim"))
        .bearer_auth(token.trim())
        .header("X-Paperdesk", "1")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    // Only explicitly queued recognition jobs attach original crop images.
    // The provider receives image data, never the Paperdesk Bearer token.
    if job["kind"] == "recognition" {
        let question_id = job["question_id"].as_str().ok_or("missing question ID")?;
        let mut images = vec![];
        if let Some(origins) = job["source"]["origins"].as_array() {
            for (index, origin) in origins.iter().enumerate() {
                if !origin["crop_id"].is_string() {
                    continue;
                }
                let response = client
                    .get(format!("{base}/questions/{question_id}/source/{index}"))
                    .bearer_auth(token.trim())
                    .send()
                    .await?
                    .error_for_status()?;
                if response
                    .content_length()
                    .is_some_and(|size| size > 32 * 1024 * 1024)
                {
                    return Err("crop exceeds 32 MB".into());
                }
                let bytes = response.bytes().await?;
                if bytes.len() > 32 * 1024 * 1024 {
                    return Err("crop exceeds 32 MB".into());
                }
                images.push(json!({"origin_index":index,"page":origin["page"],"rect":origin["rect"],"mime_type":"image/png","base64":STANDARD.encode(bytes)}));
            }
        }
        job["images"] = json!(images);
    }
    // Arguments are never interpolated into a shell. Provider stdout is strictly JSON.
    let mut child = Command::new(provider)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("missing stdin")?
        .write_all(&serde_json::to_vec(&job)?)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err("provider failed; cancel this task in the UI and create a new one".into());
    }
    let result: Value = serde_json::from_slice(&output.stdout)?;
    client
        .post(format!("{base}/jobs/{id}/result"))
        .bearer_auth(token.trim())
        .header("X-Paperdesk", "1")
        .json(&result)
        .send()
        .await?
        .error_for_status()?;
    println!("结果已提交，等待人工审核");
    Ok(())
}
