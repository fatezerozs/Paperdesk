use crate::{db, model::*};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct QuizInput {
    #[serde(default)]
    pub title: String,
    pub count: usize,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub quotas: Vec<DifficultyQuota>,
}
#[derive(Serialize)]
pub struct Preview {
    pub eligible: usize,
    pub by_difficulty: [usize; 5],
    pub can_create: bool,
    pub message: String,
}

pub fn pool(conn: &rusqlite::Connection, input: &QuizInput) -> Result<Vec<Question>, String> {
    if input.count == 0 || input.count > 200 {
        return Err("每份测验需要 1–200 题".into());
    }
    if input
        .filter
        .min_difficulty
        .is_some_and(|v| !(1..=5).contains(&v))
        || input
            .filter
            .max_difficulty
            .is_some_and(|v| !(1..=5).contains(&v))
        || input
            .filter
            .min_difficulty
            .zip(input.filter.max_difficulty)
            .is_some_and(|(a, b)| a > b)
    {
        return Err("难度范围应在 1–5 之间，且下限不能高于上限".into());
    }
    let s = &input.selection;
    if !["", "any", "all"].contains(&s.tag_mode.as_str()) {
        return Err("知识点匹配方式应为 any 或 all".into());
    }
    if s.min_page == Some(0)
        || s.max_page == Some(0)
        || s.min_page.zip(s.max_page).is_some_and(|(a, b)| a > b)
    {
        return Err("页码范围无效".into());
    }
    if s.question_ids.len() > 5000 || s.numbers.len() > 5000 || s.exclude_quiz_ids.len() > 100 {
        return Err("自定义范围过大".into());
    }
    if !input.quotas.is_empty() {
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        for quota in &input.quotas {
            if !(1..=5).contains(&quota.difficulty)
                || quota.count > 200
                || !seen.insert(quota.difficulty)
            {
                return Err("难度配额含无效难度、重复难度或过大数量".into());
            }
            total += quota.count;
        }
        if total != input.count {
            return Err("各难度配额之和必须等于测验题目数量".into());
        }
    }
    let mut excluded = std::collections::HashSet::new();
    for id in &s.exclude_quiz_ids {
        let quiz: Quiz = db::get(conn, "quizzes", id)?;
        for q in quiz.questions {
            excluded.insert(q.id);
        }
    }
    let mut filter = input.filter.clone();
    filter.status = Some("valuable".into());
    Ok(db::questions(conn, &filter)?
        .into_iter()
        .filter(|q| q.verified && s.matches(q) && !excluded.contains(&q.id))
        .collect())
}
pub fn preview(pool: &[Question], input: &QuizInput) -> Preview {
    let mut by = [0usize; 5];
    for q in pool {
        by[usize::from(q.difficulty - 1)] += 1;
    }
    let message = if pool.len() < input.count {
        format!(
            "符合条件的已核验题目只有 {} 道，无法抽取 {} 道",
            pool.len(),
            input.count
        )
    } else if let Some(q) = input
        .quotas
        .iter()
        .find(|q| by[usize::from(q.difficulty - 1)] < q.count)
    {
        format!(
            "难度 {} 只有 {} 道，配额需要 {} 道",
            q.difficulty,
            by[usize::from(q.difficulty - 1)],
            q.count
        )
    } else {
        "候选充足，可以生成测验".into()
    };
    let can_create = pool.len() >= input.count
        && input
            .quotas
            .iter()
            .all(|q| by[usize::from(q.difficulty - 1)] >= q.count);
    Preview {
        eligible: pool.len(),
        by_difficulty: by,
        can_create,
        message,
    }
}
pub fn sample(mut pool: Vec<Question>, input: &QuizInput) -> Result<Vec<Question>, String> {
    let p = preview(&pool, input);
    if !p.can_create {
        return Err(p.message);
    }
    let mut rng = rand::rng();
    pool.shuffle(&mut rng);
    if input.quotas.is_empty() {
        pool.truncate(input.count);
        return Ok(pool);
    }
    let mut selected = Vec::new();
    for quota in &input.quotas {
        selected.extend(
            pool.iter()
                .filter(|q| q.difficulty == quota.difficulty)
                .take(quota.count)
                .cloned(),
        );
    }
    selected.shuffle(&mut rng);
    Ok(selected)
}
