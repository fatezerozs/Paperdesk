use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub pages: usize,
    pub warning: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub paper_id: Option<String>,
    pub number: String,
    pub content: String,
    pub page_start: usize,
    pub page_end: usize,
    pub subject: String,
    pub difficulty: u8,
    pub tags: Vec<String>,
    pub status: String,
    pub verified: bool,
    pub answer: String,
    pub explanation: String,
    pub parent_id: Option<String>,
    pub revision: u64,
    #[serde(default)]
    pub origins: Vec<SourceSpan>,
    #[serde(default)]
    pub recognition: Option<Recognition>,
}

pub const LEGACY_OCR_PLACEHOLDER: &str = "【待识别】请先确认原图范围，再逐题识别或人工录入。";

impl Question {
    pub fn new(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            paper_id: None,
            number: String::new(),
            content,
            page_start: 1,
            page_end: 1,
            subject: "未分类".into(),
            difficulty: 3,
            tags: vec![],
            status: "pending".into(),
            verified: false,
            answer: String::new(),
            explanation: String::new(),
            parent_id: None,
            revision: 0,
            origins: vec![],
            recognition: None,
        }
    }
    pub fn has_transcription(&self) -> bool {
        !self.content.trim().is_empty() && self.content.trim() != LEGACY_OCR_PLACEHOLDER
    }
    pub fn validate(&self) -> Result<(), String> {
        let staged_empty = !self.verified
            && self.status != "valuable"
            && self.origins.iter().any(|o| o.crop_id.is_some())
            && self
                .recognition
                .as_ref()
                .is_some_and(|r| ["pending", "error"].contains(&r.status.as_str()));
        if !self.has_transcription() && (self.verified || self.status == "valuable") {
            return Err("题干尚未识别或录入，不能通过审核".into());
        }
        if (self.content.trim().is_empty() && !staged_empty) || self.content.len() > 200_000 {
            return Err("题干不能为空，且不能超过 200 KB".into());
        }
        if !(1..=5).contains(&self.difficulty) {
            return Err("难度必须为 1–5".into());
        }
        if !["pending", "valuable", "discarded"].contains(&self.status.as_str()) {
            return Err("未知审核状态".into());
        }
        if self.status == "valuable" && !self.verified {
            return Err("请先对照原卷核验，再标记为有价值".into());
        }
        if self.page_start == 0 || self.page_end < self.page_start {
            return Err("页码范围无效".into());
        }
        if self.tags.len() > 30 || self.tags.iter().any(|t| t.len() > 100) {
            return Err("标签过多或过长".into());
        }
        Ok(())
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub q: Option<String>,
    pub paper_id: Option<String>,
    pub subject: Option<String>,
    pub status: Option<String>,
    pub tag: Option<String>,
    pub min_difficulty: Option<u8>,
    pub max_difficulty: Option<u8>,
}
impl Filter {
    pub fn matches(&self, q: &Question) -> bool {
        self.q.as_ref().is_none_or(|v| {
            format!("{} {} {}", q.content, q.answer, q.tags.join(" "))
                .to_lowercase()
                .contains(&v.to_lowercase())
        }) && self
            .paper_id
            .as_ref()
            .is_none_or(|v| q.paper_id.as_ref() == Some(v))
            && self.subject.as_ref().is_none_or(|v| &q.subject == v)
            && self.status.as_ref().is_none_or(|v| &q.status == v)
            && self.tag.as_ref().is_none_or(|v| q.tags.contains(v))
            && self.min_difficulty.is_none_or(|v| q.difficulty >= v)
            && self.max_difficulty.is_none_or(|v| q.difficulty <= v)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quiz {
    pub id: String,
    pub title: String,
    pub questions: Vec<Question>,
    pub filter: Filter,
    pub created_at: String,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub quotas: Vec<DifficultyQuota>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub subjects: Vec<String>,
    pub paper_ids: Vec<String>,
    pub tags: Vec<String>,
    pub tag_mode: String,
    pub exclude_tags: Vec<String>,
    pub numbers: Vec<String>,
    pub question_ids: Vec<String>,
    pub exclude_question_ids: Vec<String>,
    pub min_page: Option<usize>,
    pub max_page: Option<usize>,
    pub exclude_quiz_ids: Vec<String>,
    pub original_only: bool,
}
impl Selection {
    pub fn matches(&self, q: &Question) -> bool {
        (self.subjects.is_empty() || self.subjects.contains(&q.subject))
            && (self.paper_ids.is_empty()
                || q.paper_id
                    .as_ref()
                    .is_some_and(|p| self.paper_ids.contains(p)))
            && (self.tags.is_empty()
                || if self.tag_mode == "all" {
                    self.tags.iter().all(|t| q.tags.contains(t))
                } else {
                    self.tags.iter().any(|t| q.tags.contains(t))
                })
            && !self.exclude_tags.iter().any(|t| q.tags.contains(t))
            && (self.numbers.is_empty()
                || self
                    .numbers
                    .iter()
                    .any(|n| normalized_number(n) == normalized_number(&q.number)))
            && (self.question_ids.is_empty() || self.question_ids.contains(&q.id))
            && !self.exclude_question_ids.contains(&q.id)
            && self.min_page.is_none_or(|p| q.page_start >= p)
            && self.max_page.is_none_or(|p| q.page_end <= p)
            && (!self.original_only || q.parent_id.is_none())
    }
}
fn normalized_number(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| {
            if ('０'..='９').contains(&c) {
                char::from_u32(u32::from(c) - u32::from('０') + u32::from('0')).unwrap()
            } else {
                c
            }
        })
        .collect()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifficultyQuota {
    pub difficulty: u8,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub content: Option<String>,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub question_id: String,
    pub kind: String,
    pub instruction: String,
    pub status: String,
    pub source_revision: u64,
    pub source: Question,
    pub result: Option<AgentResult>,
    pub created_at: String,
}

/// Immutable original evidence, independent of the editable source category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceSpan {
    pub paper_id: String,
    pub page: usize,
    pub rect: Option<RegionRect>,
    pub crop_id: Option<String>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RegionRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl RegionRect {
    pub fn valid(&self, width: u32, height: u32) -> bool {
        self.width >= 8
            && self.height >= 8
            && self.x.checked_add(self.width).is_some_and(|v| v <= width)
            && self.y.checked_add(self.height).is_some_and(|v| v <= height)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recognition {
    pub status: String,
    pub engine: String,
    pub raw_text: String,
    pub error: String,
    pub attempt: u64,
}
