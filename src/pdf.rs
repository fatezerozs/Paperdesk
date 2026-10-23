//! Standalone A4 quiz export. A bundled OFL font makes Chinese output independent of host fonts.
use crate::model::Quiz;
use printpdf::{
    IndirectFontRef, Mm, PdfDocument, PdfDocumentReference, PdfLayerIndex, PdfPageIndex,
};
use std::io::Cursor;

const MM_PER_PT: f32 = 25.4 / 72.0;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PdfStyle {
    pub page_size: String,
    pub font_size: f32,
    pub line_spacing: f32,
    pub margin: f32,
    pub columns: u8,
}
impl Default for PdfStyle {
    fn default() -> Self {
        Self {
            page_size: "a4".into(),
            font_size: 11.5,
            line_spacing: 1.65,
            margin: 20.0,
            columns: 1,
        }
    }
}
impl PdfStyle {
    pub fn validate(&self) -> Result<(), String> {
        if !["a4", "a5", "letter"].contains(&self.page_size.as_str())
            || !self.font_size.is_finite()
            || !(9.0..=16.0).contains(&self.font_size)
            || !self.line_spacing.is_finite()
            || !(1.2..=2.4).contains(&self.line_spacing)
            || !self.margin.is_finite()
            || !(12.0..=35.0).contains(&self.margin)
            || !(1..=2).contains(&self.columns)
        {
            return Err(
                "排版参数无效：纸张 A4/A5/Letter，字号 9–16，行距 1.2–2.4，页边距 12–35 mm，1–2 栏"
                    .into(),
            );
        }
        Ok(())
    }
    fn dimensions(&self) -> (f32, f32) {
        match self.page_size.as_str() {
            "a5" => (148.0, 210.0),
            "letter" => (215.9, 279.4),
            _ => (210.0, 297.0),
        }
    }
}

struct Writer<'a> {
    doc: PdfDocumentReference,
    font: IndirectFontRef,
    face: ttf_parser::Face<'a>,
    pages: Vec<(PdfPageIndex, PdfLayerIndex)>,
    y: f32,
    used: std::collections::HashMap<u16, char>,
    style: PdfStyle,
    width: f32,
    height: f32,
    column: u8,
    columns: u8,
    column_top: f32,
}

impl<'a> Writer<'a> {
    fn new(title: &str, bytes: &'a [u8], style: PdfStyle) -> Result<Self, String> {
        style.validate()?;
        let (width, height) = style.dimensions();
        let top = height - style.margin - 3.0;
        let (doc, page, layer) = PdfDocument::new(title, Mm(width), Mm(height), "Content");
        let font = doc
            .add_external_font_with_subsetting(Cursor::new(bytes), true)
            .map_err(|e| e.to_string())?;
        let face = ttf_parser::Face::parse(bytes, 0).map_err(|e| e.to_string())?;
        Ok(Self {
            doc,
            font,
            face,
            pages: vec![(page, layer)],
            y: top,
            used: std::collections::HashMap::new(),
            style,
            width,
            height,
            column: 0,
            columns: 1,
            column_top: top,
        })
    }
    fn next_page(&mut self) {
        self.pages.push(
            self.doc
                .add_page(Mm(self.width), Mm(self.height), "Content"),
        );
        self.y = self.height - self.style.margin - 3.0;
        self.column = 0;
        self.column_top = self.y;
    }
    fn need(&mut self, height: f32) {
        if self.y - height < self.style.margin + 3.0 {
            if self.column + 1 < self.columns {
                self.column += 1;
                self.y = self.column_top;
            } else {
                self.next_page();
            }
        }
    }
    fn line(&mut self, text: &str, size: f32) {
        for c in text.chars() {
            if let Some(g) = self.face.glyph_index(c) {
                self.used.insert(g.0, c);
            }
        }
        let height = size * MM_PER_PT * self.style.line_spacing;
        self.need(height);
        let (page, layer) = *self.pages.last().unwrap();
        self.doc.get_page(page).get_layer(layer).use_text(
            text,
            size,
            Mm(self.style.margin + f32::from(self.column) * (self.column_width() + 8.0)),
            Mm(self.y),
            &self.font,
        );
        self.y -= height;
    }
    fn block(&mut self, text: &str, size: f32) -> Result<(), String> {
        let max_width = self.column_width() / MM_PER_PT;
        for paragraph in text.lines() {
            let mut current = String::new();
            let mut width = 0.0;
            for character in paragraph
                .replace('\t', "    ")
                .chars()
                .filter(|c| !c.is_control())
            {
                let glyph = self.face.glyph_index(character).ok_or_else(|| {
                    format!("PDF 字体暂不支持字符「{character}」，请在题目中替换该字符后重试")
                })?;
                let advance = f32::from(self.face.glyph_hor_advance(glyph).unwrap_or(1000))
                    / f32::from(self.face.units_per_em())
                    * size;
                if width + advance > max_width && !current.is_empty() {
                    let mut carry = String::new();
                    if "，。！？；：、）》】〕〉」』,.!?:;)]}".contains(character)
                        && current.chars().count() > 1
                    {
                        while current.ends_with(' ') {
                            current.pop();
                        }
                        if let Some(c) = current.pop() {
                            carry.push(c);
                        }
                    } else if current.ends_with(['（', '(', '《', '「', '『', '【', '['])
                        && let Some(c) = current.pop()
                    {
                        carry.push(c);
                    }
                    self.line(&current, size);
                    current = carry;
                    width = current
                        .chars()
                        .filter_map(|c| self.face.glyph_index(c))
                        .map(|g| {
                            f32::from(self.face.glyph_hor_advance(g).unwrap_or(1000))
                                / f32::from(self.face.units_per_em())
                                * size
                        })
                        .sum();
                }
                current.push(character);
                width += advance;
            }
            self.line(&current, size);
        }
        Ok(())
    }
    fn space(&mut self, height: f32) {
        self.y = (self.y - height).max(self.style.margin + 3.0);
    }
    fn column_width(&self) -> f32 {
        (self.width - 2.0 * self.style.margin - 8.0 * f32::from(self.columns - 1))
            / f32::from(self.columns)
    }
    fn start_columns(&mut self) {
        self.columns = self.style.columns;
        self.column_top = self.y;
        self.column = 0;
    }
    fn finish(mut self) -> Result<Vec<u8>, String> {
        let total = self.pages.len();
        for (i, (page, layer)) in self.pages.iter().enumerate() {
            for c in "拾题 Paperdesk  |  0123456789/".chars() {
                if let Some(g) = self.face.glyph_index(c) {
                    self.used.insert(g.0, c);
                }
            }
            self.doc.get_page(*page).get_layer(*layer).use_text(
                format!("拾题 Paperdesk  |  {} / {}", i + 1, total),
                8.0,
                Mm(self.style.margin),
                Mm((self.style.margin - 7.0).max(7.0)),
                &self.font,
            );
        }
        let bytes = self.doc.save_to_bytes().map_err(|e| e.to_string())?;
        repair_unicode_mapping(bytes, &self.face, &self.used)
    }
}

// CJK fonts share glyphs between normal ideographs and Unicode radical aliases.
// printpdf selects the first alias for ToUnicode. Prefer the actual text we printed,
// so selecting/copying/searching a PDF returns 文 instead of the visually identical ⽂.
fn repair_unicode_mapping(
    bytes: Vec<u8>,
    face: &ttf_parser::Face<'_>,
    used: &std::collections::HashMap<u16, char>,
) -> Result<Vec<u8>, String> {
    let mut doc = pdf_extract::Document::load_mem(&bytes).map_err(|e| e.to_string())?;
    let ids: Vec<_> = doc
        .objects
        .values()
        .filter_map(|o| {
            o.as_dict()
                .ok()?
                .get(b"ToUnicode")
                .ok()?
                .as_reference()
                .ok()
        })
        .collect();
    let pattern = regex::Regex::new(r"(?m)^<([0-9a-fA-F]+)> <([0-9a-fA-F]{4,8})>\r?$").unwrap();
    for id in ids {
        let stream = doc
            .get_object_mut(id)
            .and_then(|o| o.as_stream_mut())
            .map_err(|e| e.to_string())?;
        let content = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let content = String::from_utf8(content).map_err(|e| e.to_string())?;
        let corrected = pattern.replace_all(&content, |capture: &regex::Captures<'_>| {
            let Some(c) = u32::from_str_radix(&capture[2], 16)
                .ok()
                .and_then(char::from_u32)
            else {
                return capture[0].to_string();
            };
            let preferred = face
                .glyph_index(c)
                .and_then(|g| used.get(&g.0))
                .copied()
                .unwrap_or(c);
            let mut buffer = [0u16; 2];
            let encoded: String = preferred
                .encode_utf16(&mut buffer)
                .iter()
                .map(|u| format!("{u:04X}"))
                .collect();
            format!("<{}> <{encoded}>", &capture[1])
        });
        stream.dict.remove(b"Filter");
        stream.dict.remove(b"DecodeParms");
        stream.set_content(corrected.as_bytes().to_vec());
    }
    doc.compress();
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
pub fn render(quiz: &Quiz, answers: bool, answer_lines: usize) -> Result<Vec<u8>, String> {
    render_with(
        quiz,
        answers,
        answer_lines,
        PdfStyle::default(),
        crate::fonts::DEFAULT_BYTES,
    )
}
pub fn render_with(
    quiz: &Quiz,
    answers: bool,
    answer_lines: usize,
    style: PdfStyle,
    font_bytes: &[u8],
) -> Result<Vec<u8>, String> {
    if answer_lines > 12 {
        return Err("作答空白最多 12 行".into());
    }
    let size = style.font_size;
    let mut writer = Writer::new(&quiz.title, font_bytes, style)?;
    writer.block(&quiz.title, 19.0)?;
    writer.space(3.0);
    writer.block(
        &format!(
            "共 {} 题    姓名：________________    日期：________________",
            quiz.questions.len()
        ),
        10.0,
    )?;
    writer.space(7.0);
    writer.start_columns();
    for (i, question) in quiz.questions.iter().enumerate() {
        writer.need(25.0);
        writer.block(
            &format!(
                "第 {} 题  ·  {}  ·  难度 {}",
                i + 1,
                question.subject,
                question.difficulty
            ),
            11.0,
        )?;
        writer.space(2.0);
        writer.block(&question.content, size)?;
        writer.space(3.0);
        for _ in 0..answer_lines {
            let glyph = writer.face.glyph_index('_').ok_or("字体缺少下划线")?;
            let width = f32::from(writer.face.glyph_hor_advance(glyph).unwrap_or(500))
                / f32::from(writer.face.units_per_em())
                * 10.0;
            writer.line(
                &"_".repeat(((writer.column_width() / MM_PER_PT) / width).floor() as usize),
                10.0,
            );
        }
        writer.space(7.0);
    }
    if answers {
        writer.next_page();
        writer.columns = 1;
        writer.block("参考答案与解析", 18.0)?;
        writer.space(6.0);
        writer.start_columns();
        for (i, question) in quiz.questions.iter().enumerate() {
            writer.need(25.0);
            writer.block(&format!("第 {} 题", i + 1), 12.0)?;
            writer.block(
                &format!(
                    "答案：{}",
                    if question.answer.trim().is_empty() {
                        "暂无答案"
                    } else {
                        &question.answer
                    }
                ),
                size,
            )?;
            writer.space(2.0);
            writer.block(
                &format!(
                    "解析：{}",
                    if question.explanation.trim().is_empty() {
                        "暂无解析"
                    } else {
                        &question.explanation
                    }
                ),
                size,
            )?;
            writer.space(7.0);
        }
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Filter, Question};
    fn quiz() -> Quiz {
        let mut q = Question::new("解方程：2x + 5 = 17。".into());
        q.subject = "数学".into();
        q.answer = "ONLY_IN_ANSWER".into();
        q.explanation = "两边同时减去五，再除以二。".into();
        Quiz {
            id: "test".into(),
            title: "中文导出测试".into(),
            questions: vec![q],
            filter: Filter::default(),
            created_at: String::new(),
            selection: Default::default(),
            quotas: vec![],
        }
    }
    #[test]
    fn chinese_font_and_answer_privacy() {
        let q = quiz();
        let student = render(&q, false, 3).unwrap();
        let text = pdf_extract::extract_text_from_mem(&student).unwrap();
        assert!(text.contains("中文导出测试"), "extracted: {text:?}");
        assert!(text.contains("解方程"));
        assert!(!text.contains("ONLY_IN_ANSWER"));
        let teacher = render(&q, true, 0).unwrap();
        assert!(
            pdf_extract::extract_text_from_mem(&teacher)
                .unwrap()
                .contains("ONLY_IN_ANSWER")
        );
        assert!(
            pdf_extract::extract_text_from_mem_by_pages(&teacher)
                .unwrap()
                .len()
                >= 2
        );
    }
    #[test]
    fn long_chinese_text_paginates_without_losing_tail() {
        let mut q = quiz();
        q.questions[0].content = "这是需要自动换行的中文长题干。".repeat(200) + "END_OF_QUESTION";
        let bytes = render(&q, false, 0).unwrap();
        let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes).unwrap();
        assert!(pages.len() > 1);
        assert!(pages.join("").contains("END_OF_QUESTION"));
    }

    #[test]
    fn custom_a5_columns_keep_all_question_text() {
        let mut q = quiz();
        q.questions = (1..=12)
            .map(|i| {
                Question::new(format!(
                    "QUESTION_{i}_START\n{}\nQUESTION_{i}_END",
                    "中文双栏排版验证。".repeat(15)
                ))
            })
            .collect();
        let style = PdfStyle {
            page_size: "a5".into(),
            columns: 2,
            margin: 15.0,
            font_size: 10.0,
            line_spacing: 1.4,
        };
        let bytes = render_with(&q, true, 3, style, crate::fonts::DEFAULT_BYTES).unwrap();
        let text = pdf_extract::extract_text_from_mem(&bytes).unwrap();
        for i in 1..=12 {
            assert!(text.contains(&format!("QUESTION_{i}_END")));
        }
        let bad = PdfStyle {
            columns: 3,
            ..Default::default()
        };
        assert!(render_with(&q, false, 0, bad, crate::fonts::DEFAULT_BYTES).is_err());
    }
}
