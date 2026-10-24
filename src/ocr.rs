//! Local subprocess adapter: original bytes never leave the machine.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn executable(name: &str) -> PathBuf {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("PAPERDESK_OCR_BIN") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        roots.push(parent.join("ocr"));
    }
    roots.push(PathBuf::from("tools/ocr"));
    roots
        .into_iter()
        .flat_map(|root| [root.join(name).join(&file), root.join(&file)])
        .find(|p| p.is_file())
        .unwrap_or_else(|| file.into())
}

pub(crate) fn run(command: &mut Command, deadline: Instant) -> Result<(), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "本地 OCR 组件缺失，请使用含 OCR 的完整安装包。".to_string())?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err("本地 OCR 未完成，请确认 PDF 可打开、未加密，并安装中文识别模型。".into())
                };
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
            Ok(None) => {}
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("OCR 超时，请缩小本次识别范围后重试。".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn recognize(bytes: &[u8], pages: &[usize]) -> Result<Vec<String>, String> {
    if pages.len() > 60 {
        return Err("一次最多识别 60 页扫描件，请分批导入。".into());
    }
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let input = temp.path().join("source.pdf");
    std::fs::write(&input, bytes).map_err(|e| e.to_string())?;
    let renderer = executable("pdftoppm");
    let tesseract = executable("tesseract");
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut output = Vec::new();
    for page in pages {
        let image = temp.path().join("page");
        run(
            Command::new(&renderer)
                .args([
                    "-f",
                    &page.to_string(),
                    "-l",
                    &page.to_string(),
                    "-scale-to",
                    "2400",
                    "-singlefile",
                    "-png",
                ])
                .arg(&input)
                .arg(&image),
            deadline,
        )?;
        let text = temp.path().join("recognized");
        let mut command = Command::new(&tesseract);
        command
            .arg(image.with_extension("png"))
            .arg(&text)
            .args(["-l", "chi_sim", "--psm", "6"]);
        if let Some(parent) = tesseract.parent() {
            let models = parent.join("tessdata");
            if models.is_dir() {
                command.arg("--tessdata-dir").arg(models);
            }
        }
        run(&mut command, deadline)?;
        output
            .push(std::fs::read_to_string(text.with_extension("txt")).map_err(|e| e.to_string())?);
    }
    Ok(output)
}

pub fn extract(bytes: &[u8]) -> (Vec<String>, String) {
    let mut pages = match pdf_extract::extract_text_from_mem_by_pages(bytes) {
        Ok(pages) => pages,
        Err(_) => {
            return (
                vec![],
                "PDF 文字解析失败，可能已加密或损坏；原件已保留。".into(),
            );
        }
    };
    let scanned: Vec<usize> = pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p.chars().filter(|c| !c.is_whitespace()).count() < 12)
        .map(|(i, _)| i + 1)
        .collect();
    if scanned.is_empty() {
        return (pages, String::new());
    }
    match recognize(bytes, &scanned) {
        Ok(recognized) => {
            for (page, text) in scanned.iter().zip(recognized) {
                pages[page - 1] = text;
            }
            (
                pages,
                format!(
                    "已对 {} 页执行本地 OCR 并按题号拆分。公式、上下标、选项顺序与题图尚未核验，请逐题对照原卷；题图不会自动转成文字。",
                    scanned.len()
                ),
            )
        }
        Err(error) => (
            pages,
            format!("扫描页未能识别：{error} 可导入分页文字或结构化结果。"),
        ),
    }
}

/// A layout probe only proposes boundaries; final recognition runs on each crop.
pub fn image_output(path: &std::path::Path, tsv: bool) -> Result<String, String> {
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let output = temp.path().join("result");
    let exe = executable("tesseract");
    let mut command = Command::new(&exe);
    command
        .arg(path)
        .arg(&output)
        .args(["-l", "chi_sim", "--psm", if tsv { "3" } else { "6" }]);
    if let Some(parent) = exe.parent() {
        let models = parent.join("tessdata");
        if models.is_dir() {
            command.arg("--tessdata-dir").arg(models);
        }
    }
    if tsv {
        command.arg("tsv");
    }
    run(
        &mut command,
        Instant::now() + Duration::from_secs(if tsv { 25 } else { 90 }),
    )?;
    std::fs::read_to_string(output.with_extension(if tsv { "tsv" } else { "txt" }))
        .map_err(|e| e.to_string())
}
