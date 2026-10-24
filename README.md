# 拾题 · Paperdesk

Rust + Axum + SQLite 的本地试卷整理工具，使用接近 Typora 的文稿式界面。当前版本 0.5.0。发行与打包说明见 [`release/`](release/README.md) 和 [`docs/RELEASE-0.5.md`](docs/RELEASE-0.5.md)。

0.5 新增：简化题库信息层级、Agent 待采纳单栏工作台、任务弹出菜单、独立接入说明、持久化设置和来源删除。发行与启动说明见 [0.5.0 说明](docs/RELEASE-0.5.md)。

0.4 新增：多选图片 / Markdown / TXT / PDF 导入、先框选拆题再逐题识别、原图区域溯源、PaddleOCR / 在线 OCR 服务适配，以及可选的 Agent 识别校对。具体使用和配置见 [导入与 OCR 指南](docs/IMPORT-0.4.md)。

0.3 新增：纸页与选题书签图标、一键浏览器启动、扫描 PDF 的本地中文 OCR，以及只替换未编辑候选的安全重新识别。旧题库和旧测验可以继续使用。

0.2 功能：米纸书卷主题、来源重命名与题目改挂、高级随机组卷、字体上传与 PDF 自定义排版、结构化解析候选导入。

高级组卷可以设置难度区间、多学科 / 来源、知识点任一或全部命中、排除标签、题号 / 页码范围、仅勾选题目、排除变式、排除已有测验用题，以及按难度配额；界面实时预览候选数量。来源右侧「⋯」可重命名，「＋」可添加教材等手工来源，编辑器可改挂题目；改挂后重新审核。

PDF 导出增加中文静态 TTF 字体上传、字号、行距、页边距、A4 / A5 / Letter、单栏 / 双栏。结构化解析接口与外部引擎比较见 [引擎调研](docs/ENGINE_EVALUATION.md)；这些外部引擎尚未集成，当前完整公式排版和题图提取仍需后续适配。

右上角支持跟随系统 / 日间 / 米纸书卷 / 夜间。米纸主题使用浅米黄色背景与细微纸纹；选择会保存在当前浏览器。打印与 PDF 使用白色底。

## 启动

Windows 在项目目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start.ps1
```

也可以双击根目录「启动拾题.cmd」，源码目录默认打开 <http://127.0.0.1:3215>；安装版与独立发行目录默认端口为 3210。重复启动仅复用同版本界面，遇到旧版会提示。源码目录优先运行 `release/paperdesk.exe`，本机配置 `paperdesk.local.json` 可指定现有题库路径。命令窗口需保持运行，按 Ctrl+C 关闭。

安装了 Rust 的其他系统：

```sh
cargo run --locked
```

首次从源码编译需要联网下载依赖。Windows 使用 Rust MSVC 工具链及 Visual Studio C++ Build Tools；SQLite 已随程序静态编译，无需另装数据库。界面资源嵌入可执行文件，无需 Node.js 或前端构建。

发布编译：`cargo build --release --locked`。Windows 产物是 `target/release/paperdesk.exe`，可复制到任意可写目录运行；数据库默认创建在工作目录的 `data/` 中。

## 使用流程

1. 点击「导入试卷」，多选 PDF、图片、UTF-8 Markdown / TXT。可用 `examples/sample-paper.txt` 体验。
2. 系统按页提取文字，再按 `1.`、`1、`、`第1题` 等行首题号拆为候选题。审核区可按勾选顺序合并，也可在编辑器用独占一行的 `---` 手工拆分。
3. 打开题目，点击「对照原卷」。编辑题干、页码、学科、难度、标签、答案、解析；核验后勾选确认，再标记「有价值」。Ctrl+S 保存。子题、重复题号、双栏与数学公式仍需人工检查。
4. 按关键词、来源、学科、标签、难度、审核状态筛选题库。点击「按当前筛选组卷」，指定数量，随机无放回抽取已核验且有价值的题目。候选不足会报错，不会悄悄减少数量。
5. 测验保存完整题目快照，之后修改或删除原题不会改变已生成的测验。作答保存在当前浏览器 localStorage，答案默认折叠。点击「导出 PDF」可下载独立 A4 文档，选择是否附答案解析及每题作答行数；原生 Rust 生成，嵌入开源中文字体，自动换行、分页、页码。也可打印当前页面或导出 Markdown。含图片或复杂公式的题目需先完善题干，或同时使用原卷链接；LaTeX 保留源码。
6. 题目编辑器与测验中可以创建 agent 解析 / 变式任务。外部 worker 通过接口领取、提交，用户在 Agent 工作台审核。没有 worker 时不会假装生成内容。

## PDF、OCR 与证据边界

程序优先通过 Rust `pdf-extract` 读取文字层；文字不足的页自动使用 Poppler + Tesseract 中文 OCR，本地处理、不上传文件。发布安装包和 Docker 镜像已包含 OCR 组件。源码运行须准备未纳入版本控制的 `tools/ocr/`，或在 PATH 提供 pdftoppm、tesseract 及 chi_sim 模型，也可用 `PAPERDESK_OCR_BIN` 指定组件目录。OCR 最多 60 页、总时限 180 秒。数学公式、上下标、图形和复杂双栏仍需对照原卷，当前不包含图形裁切、公式识别与 LaTeX 排版引擎。来源栏「重新识别 PDF」仅可替换尚未人工编辑的待审候选；已编辑或审核的来源请作为新来源重新导入。外部高精度引擎仍可通过结构化接口接入。

题干属于 `md_extracted` 同等信任层的文字候选，不代表公式、单位、图片、表格准确；原卷 iframe / 新窗口是人工核验入口，程序不会声称完成了自动 `pdf_verified`。只有用户勾选核验时才记录 `verified: true`。原始文件始终保留。SQLite 中的 JSON 导出属于 `structured_parsed` 数据结构，题目文本是否准确仍由其核验状态决定。

## Agent 扩展

接口契约见 [docs/API.md](docs/API.md)，机器可读契约见 [docs/agent-contract.json](docs/agent-contract.json)，Rust worker 示例见 [examples/agent_worker.rs](examples/agent_worker.rs)。框架不限，可接自建 agent、任意大模型 SDK 或本地模型。程序本身不携带大模型密钥。

OCR 是独立适配边界：外部模块读取原 PDF，输出 `pages: string[]`，提交到 `/api/papers/{id}/text` 后统一进入同一套人工审核流程。

## 存储与备份

`data/paperdesk.sqlite3` 保存试卷元数据、题目、测验和 agent 任务。`data/papers/` 保存原文件；`data/agent-token.txt` 保存访问 agent 接口的令牌。默认随机生成，也可设置 `PAPERDESK_AGENT_TOKEN`（至少 24 字符）。请将此目录视作私人文件夹。

页面「导出数据」只导出结构化 JSON，**不包含原卷和浏览器作答**。完整备份应先关闭程序，再复制整个 `data/` 目录；恢复时替换该目录。导出的 JSON 当前没有一键导入功能。不要只在服务运行时复制 sqlite 主文件，WAL 文件可能仍有未合并的数据。

环境变量：`PAPERDESK_PORT`（默认 3210）、`PAPERDESK_DATA`（默认 data）、`PAPERDESK_AGENT_TOKEN`、`PAPERDESK_BIND`（默认 127.0.0.1）。容器内部绑定 0.0.0.0，Compose 仅将端口发布到宿主机 127.0.0.1。

## 安装包与 Docker

Windows 安装包由 `scripts/package-windows.ps1` 构建，输出 `dist/Paperdesk-0.5.0-setup.exe`。安装后从开始菜单启动，浏览器自动打开；数据位于 `%LOCALAPPDATA%\Paperdesk\data`，卸载保留题库。发布程序静态链接 MSVC 运行库，不需额外安装 Rust、Node.js 或数据库。安装包打包前需把 Poppler、Tesseract 和 chi_sim 模型放入未跟踪的 `tools/ocr/` 子目录；完整组件不提交至源码仓库。

Docker 使用 `docker compose up -d --build`，打开 <http://127.0.0.1:3210>。题库通过命名卷 `paperdesk-data` 持久化。完整构建、离线镜像、端口设置与备份说明见 [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)。

## 验证与范围

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

测试覆盖题号拆分与跨页、人工核验约束、筛选、随机组卷资格与快照、agent 身份验证及任务状态、并发修订冲突、变式来源、拆分事务回滚、合并顺序、真实 PDF 上传与原文件保留、OCR 页码校验、持久化、本机写入保护、中文 PDF 导出、长题分页与答案隔离。

这是单人本机版本：只绑定 127.0.0.1，非多人公网服务。浏览器界面没有登录；agent 接口有 Bearer 认证，写入需要 `X-Paperdesk: 1` 防跨站表单。无 CORS 放行。当前筛选在 Rust 内对题目集合执行，适合个人题库；大规模部署应改为 SQL 分页 / 全文索引、数据库连接池，并增加账户隔离与审计。Markdown 预览仅支持基础标题、粗体、行内代码与引用；LaTeX 以源码保存。

依赖接口参考：[Axum 文档](https://docs.rs/axum/0.8.9/axum/)、[rusqlite 文档](https://docs.rs/rusqlite/0.40.2/rusqlite/)、[PDF 分页提取](https://docs.rs/pdf-extract/0.12.1/pdf_extract/fn.extract_text_from_mem_by_pages.html)。

## 0.3 更新

新增纸页与选题书签图标（SVG / PNG / 多尺寸 ICO），用于网页、程序和安装包；新增一键默认浏览器启动脚本；扫描 PDF 自动本地 OCR，处理题号逗号、编号答题须知及公式误拆。

数学公式识别、离线预览、公式 PDF 和界面调整见 [使用说明](docs/MATH-AND-UI.md)。

列表多选、快捷键与预览审核见 [交互说明](docs/SELECTION-AND-REVIEW.md)。

### 工作区设置与任务审核

支持持久化导入偏好、题库默认状态、Agent 结果分组与公式预览、已有题目重新校对，以及带关联题目选择的来源删除。操作说明见 [工作区设置与 Agent 审核](docs/WORKSPACE-SETTINGS.md)。
