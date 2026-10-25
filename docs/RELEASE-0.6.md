# 0.6.0

本版集中改进 PDF 分区、公式识别、来源范围编辑和异常恢复。功能变化见 [更新记录](../CHANGELOG.md)，完整说明见 [文档索引](README.md)。

## 升级

停止旧服务并备份整个数据目录，再替换程序及配套启动脚本。数据目录继续使用原路径；首次打开会创建新增的识别范围历史表，保留现有题库、原图与测验。新增数据不保证旧版程序能完整使用，回退前请恢复对应备份。

已识别题目在预览中可调整原图范围，保存保留题干、答案和解析，但核验状态会退回待审核；再次识别后需人工核对。

## 启动与构建

源码目录使用 `scripts/start.ps1` 或根目录启动脚本，默认端口 3215；安装版与独立发行目录默认端口 3210。本机配置不提交 Git。

- `cargo build --release --locked`：构建主程序。
- `scripts/package-windows.ps1`：生成 `dist/Paperdesk-0.6.0-setup.exe` 和本地 `release/` 目录；需要 Inno Setup、Windows OCR 组件与 Rust MSVC 工具链。
- `scripts/package-docker.ps1`：构建并导出 `dist/paperdesk-0.6.0-docker.tar`；需要可用的 Docker 服务。
- 本地 PaddleOCR 模型通过 `scripts/start-math-ocr.ps1` 或可选 Compose 服务单独启动；它不包含在主程序 exe 中。

当前源码提交包含打包定义，不代表已经生成或上传对应二进制附件。生成物应放在 GitHub Releases，不放入源码历史。

## 已知边界

本地 CPU 公式精细识别仍可能较慢；快速文字模式降低公式能力。自动分区、OCR 与 Agent 答案解析均需人工核验。中断页面等待不会强制终止已经运行的后台任务；服务异常后应确认当前来源的最新结果再重试。
