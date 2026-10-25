//! Page layout adapters and conservative question aggregation, independent of OCR transcription.
use crate::{
    importer::{PagePreview, RegionInput},
    model::RegionRect,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub bbox: [f64; 4],
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub order: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageLayout {
    pub width: u32,
    pub height: u32,
    pub engine: String,
    pub lines: Vec<Block>,
    pub blocks: Vec<Block>,
}
impl PageLayout {
    pub fn validate(&self, p: &PagePreview) -> Result<(), String> {
        if self.width != p.width
            || self.height != p.height
            || self.lines.len() + self.blocks.len() > 20000
        {
            return Err("版面坐标尺寸或块数量无效".into());
        }
        for b in self.lines.iter().chain(&self.blocks) {
            let [l, t, r, d] = b.bbox;
            if !b.bbox.iter().all(|v| v.is_finite())
                || l < 0.0
                || t < 0.0
                || r > p.width as f64 + 1.0
                || d > p.height as f64 + 1.0
                || r <= l
                || d <= t
                || b.text.len() > 200000
                || b.confidence
                    .is_some_and(|c| !c.is_finite() || !(0.0..=1.0).contains(&c))
            {
                return Err("版面服务返回了越界或无效坐标".into());
            }
        }
        Ok(())
    }
}
pub fn request(
    url: reqwest::Url,
    token: Option<&str>,
    payload: Value,
    seconds: u64,
) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(seconds.max(1)))
        .build()
        .map_err(|_| "无法创建版面服务连接")?;
    let mut req = client.post(url).json(&payload);
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let response = req.send().map_err(|_| "版面服务连接失败或超时")?;
    if !response.status().is_success() {
        return Err(format!("版面服务返回 HTTP {}", response.status().as_u16()));
    }
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "版面结果读取失败")?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("版面结果超过 4 MB".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "版面服务没有返回有效 JSON".into())
}
/// Full PDFs are sent only to the loopback companion, never an external OCR provider.
pub fn companion(
    config: &crate::ocr_settings::Values,
    route: &str,
) -> Option<(reqwest::Url, Option<String>)> {
    let raw = crate::ocr_settings::value(config, "PAPERDESK_PADDLE_URL")?;
    let mut url = reqwest::Url::parse(&raw).ok()?;
    if !matches!(
        url.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    ) || !["http", "https"].contains(&url.scheme())
        || url.path() != "/layout-parsing"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
    {
        return None;
    }
    url.set_path(route);
    Some((
        url,
        crate::ocr_settings::value(config, "PAPERDESK_PADDLE_TOKEN"),
    ))
}
pub fn native(
    config: &crate::ocr_settings::Values,
    bytes: &[u8],
    pages: &[PagePreview],
    seconds: u64,
) -> Result<Vec<(usize, PageLayout)>, String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let (url, token) = companion(config, "/pdf-layout").ok_or("尚未配置本地版面服务")?;
    let value = request(
        url,
        token.as_deref(),
        json!({"file":STANDARD.encode(bytes),"pages":pages,"budget_seconds":seconds.saturating_sub(2).max(1)}),
        seconds,
    )?;
    #[derive(Deserialize)]
    struct Entry {
        page: usize,
        layout: PageLayout,
    }
    #[derive(Deserialize)]
    struct Document {
        pages: Vec<Entry>,
    }
    let doc: Document = serde_json::from_value(value).map_err(|_| "原生 PDF 版面格式无效")?;
    let mut out = Vec::new();
    for e in doc.pages {
        let p = pages
            .iter()
            .find(|p| p.page == e.page)
            .ok_or("版面页码无效")?;
        e.layout.validate(p)?;
        if out.iter().any(|(page, _)| *page == e.page) {
            return Err("版面页码重复".into());
        }
        out.push((e.page, e.layout));
    }
    Ok(out)
}
pub fn visual(
    config: &crate::ocr_settings::Values,
    path: &std::path::Path,
    p: &PagePreview,
    seconds: u64,
) -> Result<PageLayout, String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let (url, token) = companion(config, "/page-layout").ok_or("尚未配置本地版面服务")?;
    let bytes = std::fs::read(path).map_err(|_| "原页读取失败")?;
    let value = request(
        url,
        token.as_deref(),
        json!({"file":STANDARD.encode(bytes),"budget_seconds":seconds.saturating_sub(2).max(1)}),
        seconds,
    )?;
    let result: PageLayout = serde_json::from_value(value).map_err(|_| "视觉版面结果格式无效")?;
    result.validate(p)?;
    Ok(result)
}
/// Reuse only fully contained, high-confidence page OCR; otherwise run the full crop pipeline.
pub fn crop_hint(dir: &std::path::Path, origin: &crate::model::SourceSpan) -> Option<Value> {
    let rect = origin.rect?;
    let file = dir.join(format!("layout-{}.json", origin.page));
    if std::fs::metadata(&file).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    let layout: PageLayout = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    if layout.engine != "PP-StructureV3-layout" || !rect.valid(layout.width, layout.height) {
        return None;
    }
    let p = PagePreview {
        page: origin.page,
        width: layout.width,
        height: layout.height,
    };
    layout.validate(&p).ok()?;
    let translate = |b: &Block| -> Option<Block> {
        let [l, t, r, d] = b.bbox;
        let x = rect.x as f64;
        let y = rect.y as f64;
        if r <= x || d <= y || l >= x + rect.width as f64 || t >= y + rect.height as f64 {
            return None;
        }
        let mut block = b.clone();
        block.bbox = [l - x, t - y, r - x, d - y];
        Some(block)
    };
    let mut lines = vec![];
    let mut formulas = vec![];
    for block in &layout.lines {
        if let Some(b) = translate(block) {
            if b.confidence.is_none_or(|v| v < 0.9)
                || b.bbox[0] < 0.0
                || b.bbox[1] < 0.0
                || b.bbox[2] > rect.width as f64
                || b.bbox[3] > rect.height as f64
            {
                return None;
            }
            lines.push(b);
        }
    }
    for block in &layout.blocks {
        if block.category.contains("formula")
            && let Some(b) = translate(block)
        {
            if b.bbox[0] < 0.0
                || b.bbox[1] < 0.0
                || b.bbox[2] > rect.width as f64
                || b.bbox[3] > rect.height as f64
            {
                return None;
            }
            formulas.push(b);
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(json!({"lines":lines,"formulas":formulas}))
}
/// Use line geometry for anchors; layout blocks protect figures/formulas from cuts.
pub fn aggregate(
    layout: &PageLayout,
    page: &PagePreview,
    cols: &[(u32, u32)],
) -> (Vec<RegionInput>, Vec<String>) {
    if cols.len() > 1
        && layout.blocks.iter().any(|b| {
            matches!(b.category.as_str(), "image" | "chart" | "table" | "formula")
                && cols.windows(2).any(|c| {
                    b.bbox[0] < (c[0].1 as f64) - 12.0 && b.bbox[2] > (c[1].0 as f64) + 12.0
                })
        })
    {
        return (
            vec![RegionInput {
                page: page.page,
                rect: RegionRect {
                    x: 0,
                    y: 0,
                    width: page.width,
                    height: page.height,
                },
            }],
            vec![format!(
                "第 {} 页存在跨栏图表或公式，保留整页候选，请人工确认边界。",
                page.page
            )],
        );
    }
    let number = regex::Regex::new(r"^(?:第\s*)?([0-9０-９]{1,3})\s*[.．、:：题]").unwrap();
    let mut result = vec![];
    let mut warnings = vec![];
    if layout
        .lines
        .iter()
        .any(|b| b.text.contains("参考答案") || b.text.contains("答案与解析"))
    {
        warnings.push(format!(
            "第 {} 页包含答案区，请将其与试题范围分开确认。",
            page.page
        ));
    }

    for &(left, right) in cols {
        let mut candidates: Vec<(u32, u32)> = layout
            .lines
            .iter()
            .filter_map(|b| {
                let [x, y, _, _] = b.bbox;
                if x < left as f64
                    || x > (left + (right - left) / 4) as f64
                    || b.confidence.is_some_and(|c| c < 0.65)
                {
                    return None;
                }
                let cap = number.captures(b.text.trim())?;
                if b.text.trim()[cap.get(0)?.end()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    return None;
                }
                let digits: String = cap[1]
                    .chars()
                    .map(|c| {
                        if ('０'..='９').contains(&c) {
                            char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap()
                        } else {
                            c
                        }
                    })
                    .collect();
                Some((y.floor() as u32, digits.parse().ok()?))
            })
            .collect();
        candidates.sort_unstable();
        let mut anchors = vec![];
        let mut last = 0;
        for (y, n) in candidates {
            if n == 0 || n <= last || anchors.last().is_some_and(|(a, _)| y <= a + 12) {
                continue;
            }
            if last > 0 && n != last + 1 {
                warnings.push(format!(
                    "第 {} 页题号从 {} 跳到 {}，请核对是否漏拆或包含答案区。",
                    page.page, last, n
                ));
            }
            anchors.push((y, n));
            last = n;
        }
        if anchors.is_empty() {
            warnings.push(format!(
                "第 {} 页的一个栏未找到可靠题号，保留完整栏；请检查共享材料或跨页续题。",
                page.page
            ));
        }
        let mut starts = vec![0];
        for &(y, _) in anchors.iter().skip(1) {
            let cut = y.saturating_sub(8);
            let crosses = layout.blocks.iter().any(|b| {
                matches!(b.category.as_str(), "image" | "chart" | "table" | "formula")
                    && b.bbox[0] < right as f64
                    && b.bbox[2] > left as f64
                    && b.bbox[1] < cut as f64
                    && b.bbox[3] > cut as f64
            });
            if crosses {
                warnings.push(format!(
                    "第 {} 页题目边界穿过图表或公式，保留合并区域等待人工拆分。",
                    page.page
                ));
            } else {
                starts.push(cut);
            }
        }
        if page.page > 1 && anchors.first().is_some_and(|(y, _)| *y > page.height / 8) {
            warnings.push(format!(
                "第 {} 页页首可能是上一页续题，未自动合并，请对照两页确认。",
                page.page
            ));
        }
        for (i, &top) in starts.iter().enumerate() {
            let bottom = starts.get(i + 1).copied().unwrap_or(page.height);
            if bottom > top + 8 {
                result.push(RegionInput {
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
    (result, warnings)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn block(x: f64, y: f64, text: &str) -> Block {
        Block {
            bbox: [x, y, x + 100.0, y + 20.0],
            text: text.into(),
            category: "text".into(),
            confidence: Some(0.99),
            order: None,
        }
    }
    #[test]
    fn groups_columns_without_splitting_options_or_subquestions() {
        let page = PagePreview {
            page: 1,
            width: 1200,
            height: 1000,
        };
        let layout = PageLayout {
            width: 1200,
            height: 1000,
            engine: "fixture".into(),
            lines: vec![
                block(10.0, 20.0, "1. 题"),
                block(10.0, 100.0, "A. 选项"),
                block(10.0, 180.0, "(1) 子问"),
                block(10.0, 400.0, "2. 题"),
                block(610.0, 20.0, "3. 题"),
                block(610.0, 500.0, "4. 题"),
            ],
            blocks: vec![],
        };
        layout.validate(&page).unwrap();
        let (r, _) = aggregate(&layout, &page, &[(0, 600), (600, 1200)]);
        assert_eq!(r.len(), 4);
        assert_eq!(r[1].rect.y, 392);
        assert_eq!(r[2].rect.x, 600);
    }
    #[test]
    fn ambiguous_figure_boundary_is_not_cut() {
        let page = PagePreview {
            page: 1,
            width: 1200,
            height: 1000,
        };
        let mut figure = block(10.0, 350.0, "");
        figure.bbox[3] = 450.0;
        figure.category = "image".into();
        let mut layout = PageLayout {
            width: 1200,
            height: 1000,
            engine: "fixture".into(),
            lines: vec![block(10.0, 20.0, "1. 题"), block(10.0, 400.0, "2. 题")],
            blocks: vec![figure],
        };
        let (r, w) = aggregate(&layout, &page, &[(0, 1200)]);
        assert_eq!(r.len(), 1);
        assert!(!w.is_empty());
        layout.lines[0].bbox[2] = f64::NAN;
        assert!(layout.validate(&page).is_err());
    }
}
