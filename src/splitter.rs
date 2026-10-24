//! Conservative text boundary detection. Every candidate requires original-layout review.
use crate::model::{Question, SourceSpan};
use regex::Regex;

pub fn split(pages: &[String]) -> Vec<Question> {
    let boundary = Regex::new(
        r"^\s*(?:#{1,6}\s+)?(?:第\s*([0-9０-９]{1,3})\s*题|([0-9０-９]{1,3})\s*[.．、，,:：)）])\s*(.*)$",
    )
    .unwrap();
    let section = Regex::new(
        r"^\s*[一二三四五六七八九十]+\s*[、，,．.]?\s*(?:选择|填空|解答|计算|判断|综合|阅读|证明)",
    )
    .unwrap();
    let mut out = Vec::new();
    let mut current: Option<Question> = None;
    let mut preamble = Vec::new();
    let mut last_number = None;
    // Numbered exam instructions are not questions when a section heading follows.
    let mut started = !pages
        .iter()
        .any(|p| p.lines().any(|line| section.is_match(line)));
    for (page, text) in pages.iter().enumerate() {
        for (line_index, line) in text.lines().enumerate() {
            if section.is_match(line) {
                if let Some(q) = current.take() {
                    out.push(q);
                }
                started = true;
                last_number = None;
                continue;
            }
            if !started {
                continue;
            }
            let candidate = boundary.captures(line).filter(|c| {
                let number = normalize_number(c.get(1).or_else(|| c.get(2)).unwrap().as_str());
                last_number.is_none_or(|last| number.parse::<u16>().unwrap_or(0) > last)
            });
            if let Some(c) = candidate {
                if let Some(q) = current.take() {
                    out.push(q);
                }
                let mut q = Question::new(c[3].trim().to_string());
                q.number = normalize_number(c.get(1).or_else(|| c.get(2)).unwrap().as_str());
                last_number = q.number.parse::<u16>().ok();
                q.page_start = page + 1;
                q.page_end = page + 1;
                q.origins.push(SourceSpan {
                    paper_id: String::new(),
                    page: page + 1,
                    rect: None,
                    crop_id: None,
                    line_start: Some(line_index + 1),
                    line_end: Some(line_index + 1),
                });
                current = Some(q);
            } else if let Some(q) = current.as_mut() {
                if !line.trim().is_empty() {
                    q.content.push('\n');
                    q.content.push_str(line);
                    q.page_end = page + 1;
                    if let Some(span) = q.origins.last_mut().filter(|span| span.page == page + 1) {
                        span.line_end = Some(line_index + 1);
                    } else {
                        q.origins.push(SourceSpan {
                            paper_id: String::new(),
                            page: page + 1,
                            rect: None,
                            crop_id: None,
                            line_start: Some(line_index + 1),
                            line_end: Some(line_index + 1),
                        });
                    }
                }
            } else {
                preamble.push(line);
            }
        }
    }
    if let Some(q) = current {
        out.push(q);
    }
    if out.is_empty() && !preamble.join("\n").trim().is_empty() {
        let mut q = Question::new(preamble.join("\n").trim().to_string());
        q.page_end = pages.len().max(1);
        q.origins = pages
            .iter()
            .enumerate()
            .map(|(i, p)| SourceSpan {
                paper_id: String::new(),
                page: i + 1,
                rect: None,
                crop_id: None,
                line_start: Some(1),
                line_end: Some(p.lines().count().max(1)),
            })
            .collect();
        out.push(q);
    }
    out.into_iter()
        .filter(|q| !q.content.trim().is_empty())
        .collect()
}

fn normalize_number(number: &str) -> String {
    number
        .chars()
        .map(|c| {
            if ('０'..='９').contains(&c) {
                char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap()
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spans_pages_and_preserves_subquestions() {
        let q = split(&[
            "数学试卷\n一、选择题\n1. 第一题\n（1）子问题\nA. 选项\n2、第二题".into(),
            "续行\n第3题 第三题".into(),
        ]);
        assert_eq!(q.len(), 3);
        assert!(q[0].content.contains("（1）"));
        assert_eq!(q[1].page_end, 2);
        assert_eq!(q[2].page_start, 2);
    }
    #[test]
    fn unrecognized_layout_is_one_reviewable_candidate() {
        assert_eq!(split(&["没有题号的正文".into()]).len(), 1);
        assert!(split(&[String::new()]).is_empty());
    }
    #[test]
    fn ignores_numbered_instructions_and_ocr_section_punctuation() {
        let q = split(&["注意事项\n1. 填写姓名\n2. 填涂答题卡\n一选择题:本题共8小题\n合题目要求\n1. 题干\nA. 选项\n2. 题干二\n二,选择题:多项符合\n全部选对得分\n3. 题干三".into()]);
        assert_eq!(
            q.iter().map(|q| q.number.as_str()).collect::<Vec<_>>(),
            ["1", "2", "3"]
        );
        assert!(!q[1].content.contains("全部选对"));
        assert!(!q[0].content.contains("填写姓名"));
    }
    #[test]
    fn ocr_comma_and_formula_line_do_not_create_false_questions() {
        let q = split(&["９. 第九题\n10, 第十题\n11. 第十一题\n1 ) -f(x) < 0\nA. 选项".into()]);
        assert_eq!(
            q.iter().map(|q| q.number.as_str()).collect::<Vec<_>>(),
            ["9", "10", "11"]
        );
        assert!(q[2].content.contains("1 ) -f(x)"));
    }
}
