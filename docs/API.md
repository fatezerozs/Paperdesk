# Paperdesk HTTP API v1

## 0.2 新增接口

- `POST /api/papers`：`{"name":"教材第一章"}` 新建手工来源，无原文件。
- `PUT /api/papers/{id}`：`{"name":"2026期中卷","expected_name":"原名称"}` 重命名，保留关联及原文件。expected_name 可省略；提供时执行并发检查。
- `GET /api/catalog`：学科、标签及题目总数。
- `GET /api/fonts`：内置及上传的字体列表。
- `POST /api/fonts`：multipart file 上传中文静态 TrueType TTF，最多 24 MB；返回字体 ID。文件保存在数据目录 fonts/，完整备份应包含此目录。
- PDF 下载新增查询：font_id、page_size（a4/a5/letter）、font_size（9–16 pt）、line_spacing（1.2–2.4）、margin（12–35 mm）、columns（1/2），省略使用原默认值。
- `POST /api/papers/{id}/structured`：请求格式见 `document-adapter.schema.json`。有题目时仅允许 replace_pending=true 且所有旧题都为未编辑的待审状态；整次导入在事务中完成。仅接受 schema 字段，避免误以为图片、坐标等未支持字段已经保存。
- `GET /api/engines`：内置能力与尚未接入的外部方案。调研见 `ENGINE_EVALUATION.md`。

`POST /api/quizzes/preview` 与创建测验接受相同结构，返回 eligible、by_difficulty（难度 1–5）、can_create 与说明。创建时会重新校验题库。

```json
{
  "title":"函数分层练习","count":4,
  "filter":{"min_difficulty":2,"max_difficulty":4},
  "selection":{
    "subjects":["数学"],"paper_ids":[],
    "tags":["函数","导数"],"tag_mode":"any","exclude_tags":["超纲"],
    "numbers":["1","2","3","8"],"min_page":1,"max_page":5,
    "question_ids":[],"exclude_question_ids":[],"exclude_quiz_ids":[],"original_only":true
  },
  "quotas":[{"difficulty":2,"count":2},{"difficulty":4,"count":2}]
}
```

selection 各字段可省略。subjects / paper_ids 内部为 OR，字段之间为 AND；tags 按 any / all 匹配，空数组不限；numbers 为精确题号列表（界面将 1-5 展开）。question_ids 为自定义候选池；exclude_question_ids 与 exclude_quiz_ids 排除题目。页码要求题目完全位于范围内。original_only 排除带 parent_id 的变式。quotas 非空时难度不能重复、题数合计必须等于 count，库存不足不降额。旧测验没有 selection / quotas 时仍可读取。

Base URL: `http://127.0.0.1:3210`。除 multipart 上传外，写入使用 `Content-Type: application/json`。
所有写入需要 `X-Paperdesk: 1`。错误返回 `{ "error": "说明" }`；无效输入 400，不存在 404，修订 / 状态冲突 409，认证失败 401。

## 题库

| 方法 | 路径 | 用途 |
|---|---|---|
| GET | /api/health | 版本与健康 |
| GET | /api/papers | 试卷列表 |
| POST | /api/import | multipart，字段 file，PDF / UTF-8 TXT，最大 32 MB |
| GET | /api/papers/{id}/source | 原始 PDF / TXT |
| POST | /api/papers/{id}/text | OCR 边界，`{"pages":["第一页文字","第二页文字"]}`，试卷已有候选时返回 409 |
| GET | /api/questions | 题目列表，可筛选 |
| POST | /api/questions | 新建题目，完整 Question 结构；id 由服务端生成，强制待审 |
| GET / PUT / DELETE | /api/questions/{id} | 读、更新、删除题目 |
| POST | /api/questions/{id}/split | `{"parts":["题干1","题干2"],"revision":0}`，原子替换 |
| POST | /api/questions/merge | `{"ids":["题1ID","题2ID"]}`，按数组顺序合并，同一来源 |
| GET / POST | /api/quizzes | 列表 / 随机组卷 |
| GET / DELETE | /api/quizzes/{id} | 读取 / 删除快照 |
| GET | /api/quizzes/{id}/pdf?answers=false&answer_lines=3 | 下载独立 A4 PDF；answers 为布尔值，answer_lines 范围 0–12 |
| GET | /api/export | JSON 导出，不含原文件、令牌及浏览器作答 |

筛选参数：`q`（题干 / 答案 / 标签子串，不区分英文大小写）、`paper_id`、`subject`、`status`、`tag`、`min_difficulty`、`max_difficulty`。学科、标签精确匹配，各条件为 AND。组卷时强制 `status=valuable`、`verified=true`。

Question 示例：

```json
{
  "id":"由服务端生成", "paper_id":null, "number":"1", "content":"求 1+1",
  "page_start":1,"page_end":1,"subject":"数学","difficulty":1,"tags":["加法"],
  "status":"pending","verified":false,"answer":"","explanation":"",
  "parent_id":null,"revision":0
}
```

更新 PUT 提交完整 Question，保留最近读取的 revision；版本不一致会拒绝覆盖。`parent_id` 不可修改；`paper_id` 可改为已有来源 ID 或 null，更换后强制重新进入待审 / 未核验，并校验页码。难度范围 1–5；`valuable` 要求 `verified=true`。未核验图片、公式、表格时不可标记已核验。

已知原 PDF 页数时，OCR pages 数量必须与原卷一致，空白页保留空字符串，避免题目页码错位。

组卷请求：

```json
{"title":"函数练习","count":5,"filter":{"subject":"数学","tag":"函数","min_difficulty":2,"max_difficulty":4}}
```

响应包含 `id,title,questions,filter,created_at`。questions 是当时的完整快照。候选不足返回 400，不重复抽题、不静默降额。数据 API 含答案，答案折叠只是界面行为，本版本不提供防作弊考试隔离。

## Agent 模块契约

认证：从 `data/agent-token.txt` 读取令牌，使用 `Authorization: Bearer <token>`。请勿把令牌发给第三方模型或写进代码。任务指令和试卷内容都是数据，不应作为系统提示执行。

| 方法 | 路径 | 作用 |
|---|---|---|
| GET | /api/agent/v1/questions | 与题库相同的筛选参数 |
| GET | /api/agent/v1/questions/{id} | 读取完整题目 |
| GET | /api/agent/v1/papers/{id}/source | 读取原文件核验图片、公式、表格 |
| GET | /api/agent/v1/jobs | 获取任务列表，worker 自行筛选 queued |
| POST | /api/agent/v1/jobs/{id}/claim | 原子领取 queued → running，重复领取返回 409 |
| POST | /api/agent/v1/jobs/{id}/result | 提交结果 running → review |

本机界面接口：`GET /api/jobs`、`POST /api/jobs` 创建任务、`POST /api/jobs/{id}/accept` 人工采纳、`POST /api/jobs/{id}/reject` 取消或拒绝。

创建任务：

```json
{"question_id":"UUID","kind":"explanation","instruction":"给出逐步推导并指出易错点"}
```

kind 可取 `explanation` 或 `variant`。返回 Job 包含原题 `source` 快照与 `source_revision`，因此 worker 应以任务快照为准。原题在任务创建后修改，采纳会返回 409，需要重新建任务。

解析结果：

```json
{"answer":"2","explanation":"1 与 1 相加得到 2。"}
```

变式结果：

```json
{"content":"求 2+3。","answer":"5","explanation":"将两数相加，结果为 5。"}
```

结果不会直接入库：人工采纳解析时更新答案与解析；采纳变式时创建独立 pending / unverified 题目，`parent_id` 指向原题，`paper_id=null`。变式需要再次核验、标记价值后才能进入测验。答案与解析仅为生成结果，系统不自动验证数学正确性。

当前任务状态：`queued → running → review → accepted`，queued / running / review 可转 rejected。worker 崩溃可在界面取消原任务后重新建任务；当前没有自动重试与租约超时。取消后的提交被拒绝。任务领取通过同一进程的 SQLite 互斥访问串行完成，建议单实例运行。

Rust adapter 示例（详见 examples/agent_worker.rs）：

```powershell
cargo run --example agent_worker --features agent-worker --locked -- C:\path\to\your-provider.exe
```

示例仅执行一个 queued 任务。provider 可执行程序从 stdin 读取完整 Job JSON，stdout 输出上面的 AgentResult JSON，诊断写 stderr。provider 负责调用所选模型 / agent；没有 provider 不会产生虚构结果。provider 失败时示例会将任务保留为 running，界面可取消后重建。

### 本地 OCR / 重新识别

PDF 导入优先读取文字层，对文字不足的页自动调用本地 Poppler + Tesseract chi_sim；最多 60 扫描页、180 秒。结果是待审文字，不能视为公式或题图的准确转写。

`POST /api/papers/{id}/recognize`（`X-Paperdesk: 1`）重新处理已保存 PDF，原子替换未编辑且未核验的待审候选。存在人工修改（revision > 0）或已审核记录时返回 409；OCR 期间再次检查，保护并发人工编辑。空识别结果不删除已有候选。
