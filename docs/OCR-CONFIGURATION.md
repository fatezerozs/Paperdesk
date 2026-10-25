# OCR 配置与 AI 解题

设置 → 配置识别服务，可保存 PaddleOCR、Mathpix、自定义 OCR 的连接参数。配置保存在当前工作区 SQLite 的 settings 表，不包含在 `/api/export` 导出中；备份整个数据库仍包含这些凭据，应妥善保管。

`GET /api/ocr-settings` 只返回地址、是否配置及环境变量覆盖状态，凭据不回显。`PUT /api/ocr-settings` 接收 `{ "values": { "PAPERDESK_PADDLE_URL": "http://127.0.0.1:8850/layout-parsing" }, "clear": [] }`。空白保留原值，clear 显式清除；环境变量优先于工作区配置。保存后无需重启 Paperdesk。

## PaddleOCR

运行 `powershell -ExecutionPolicy Bypass -File scripts/start-math-ocr.ps1`，保持服务运行。设置中点击“填入本地服务地址”，保存后选择 PaddleOCR 并保存设置。初次运行需要 Python 3.10–3.12、安装依赖和下载模型；配置完成不代表模型已经就绪。Docker 的 OCR 独立服务仍按原部署文档启动，URL 应指向容器可达地址。

## Mathpix

在 Mathpix 控制台创建 App ID / App Key，填入配置窗口。使用官方 `https://api.mathpix.com/v3/text`，凭据在请求头传送。参考：https://docs.mathpix.com/reference/authentication 。需要有效账户与额度，每次图片发送仍须授权。

## 自定义服务

POST JSON：`image_base64`、`mime_type: image/png`、`task: question_ocr`、`preserve_formulas: true`。返回 `{ "text": "含 LaTeX 的题干" }`。可选 Bearer 令牌。外部地址必须 HTTPS，回环地址允许 HTTP；不会跟随重定向。

## Agent 识别与解题

新提交的 recognition 结果必须包含非空 `content`、`answer`、`explanation`。任务统一要求先核对原图，再写答案和逐步解析。证据不足时答案标注【待核验】，解析说明缺失条件。采纳会一并保存题干、答案和解析，题目保持待人工核验。历史已完成的仅题干结果仍可采纳，保留原答案解析。尚未完成的旧任务须按新契约提交三字段，必要时取消后重新创建。

Agent 仍由外部 worker 执行；开启辅助识别只会排队，不会凭空调用模型。OCR 引擎负责转写，解题由 Agent 完成。
