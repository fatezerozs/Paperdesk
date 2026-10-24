mod api;
mod db;
mod fonts;
mod importer;
mod model;
mod ocr;
mod pdf;
mod sampling;
mod settings;
mod splitter;
#[cfg(test)]
mod tests;
mod web_assets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let open = std::env::args().any(|arg| arg == "--open");
    let dir = std::env::var("PAPERDESK_DATA").unwrap_or_else(|_| "data".into());
    let state = db::Store::open(std::path::Path::new(&dir))?;
    let port: u16 = std::env::var("PAPERDESK_PORT")
        .unwrap_or_else(|_| "3210".into())
        .parse()?;
    let bind: std::net::IpAddr = std::env::var("PAPERDESK_BIND")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()?;
    let listener = match tokio::net::TcpListener::bind((bind, port)).await {
        Ok(listener) => listener,
        Err(error) if open && error.kind() == std::io::ErrorKind::AddrInUse => {
            // Never assume an unrelated service on this port is Paperdesk.
            let mut connection =
                tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await?;
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            connection
                .write_all(
                    format!("GET /api/health HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n")
                        .as_bytes(),
                )
                .await?;
            let mut response = Vec::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                connection.take(8192).read_to_end(&mut response),
            )
            .await??;
            let response = String::from_utf8_lossy(&response);
            if response.contains("\"ok\":true") && response.contains("\"version\"") {
                open_browser(port);
                return Ok(());
            }
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    };
    println!("Paperdesk 已启动：http://127.0.0.1:{port}");
    println!("Agent 凭据存于 {dir}/agent-token.txt；API 说明见 docs/API.md");
    if open {
        open_browser(port);
    }
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn open_browser(port: u16) {
    let url = format!("http://127.0.0.1:{port}");
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        } else {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
