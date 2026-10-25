# 拾题 · Paperdesk

用 Rust + Axum + SQLite 构建的本地试卷整理与题库工具。导入试卷后先拆题，再识别、人工核验与筛选组卷；为外部 Agent 保留解析、答案和变式任务接口。界面采用 Typora 式文稿布局，提供日间、米纸、夜间与跟随系统主题。

当前源码版本：**0.6.0**。查看 [更新记录](CHANGELOG.md)、[本版说明](docs/RELEASE-0.6.md) 和 [完整文档](docs/README.md)。

## 主要功能

- 多选导入 PDF、图片、UTF-8 Markdown / TXT。支持原生 PDF 坐标提取、版面检测与人工框选拆题，保留原件、页码和题目裁切图。
- 接入本地 Tesseract、PaddleOCR、Mathpix 或自定义 OCR 服务；可选择快速文字模式或公式精细识别。在线服务需要独立配置和本次上传授权。
- 已识别题目可拖动、缩放或重画识别范围，保存时保留题干、答案和解析；可另行发起重新识别并再次审核。
- 人工核验、价值标注、标签、学科、难度、来源重命名/删除/改挂；多选、Ctrl/Command 点击和 Shift 范围选择。
- 按难度范围、知识点、来源、标签、题号/页码、自定义题目范围与难度配额随机组卷；保存题目快照，支持 Markdown、打印和 PDF 导出。
- 外部 Agent 领取识别校对、解析与变式任务；辅助识别结果包含题干、答案和解析，由用户采纳后入库。
- 切分与识别显示动态阶段进度，支持后台查看、停止后续任务、错误详情折叠及失败项重试；本地模型独立进程、限线程、超时恢复。

## 快速开始

### Windows

已有便携发行目录时，双击其中的 **启动拾题.cmd**。源码目录也可以运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start.ps1
```

源码入口默认端口 `3215`，优先使用 `release/paperdesk.exe`；安装版与独立发行目录默认端口 `3210`。源码目录可用不提交 Git 的 `paperdesk.local.json` 指定数据目录和端口；环境变量优先。服务窗口需要保持运行。

从源码编译需要 Rust stable（edition 2024）；Windows 还需要 MSVC C++ Build Tools 与 Windows SDK。SQLite 和界面资源嵌入程序，无需另装数据库或 Node.js。

```sh
cargo build --release --locked
cargo run --locked -- --open
```

程序默认打开 `http://127.0.0.1:3210`，数据保存在工作目录 `data/`。纯源码运行扫描识别需在 PATH 提供 Poppler、Tesseract 和中文模型，或设置 `PAPERDESK_OCR_BIN`；源码仓库不包含这些运行时二进制。配置公式模型见 [OCR 服务指南](docs/OCR-CONFIGURATION.md)。

### Docker

```sh
docker compose up -d --build
```

打开 `http://127.0.0.1:3210`。Compose 使用命名卷保存数据，仅向宿主机回环地址发布端口。主镜像内置 Poppler 和 Tesseract 中文 OCR；PaddleOCR 公式模型是独立可选服务，详见 [数学识别服务](services/math-ocr/README.md)。

部署、备份、端口与安装包构建见 [部署教程](docs/DEPLOYMENT.md)。

## 使用流程

1. 多选导入 PDF、图片、Markdown 或 TXT。可先使用 `examples/sample-paper.txt` 体验。
2. 检查每页的题目范围，手动补框或调整后保存拆分，再开始逐题识别。分区提示默认折叠，原件始终保留。
3. 在题目预览中对照原图，编辑题干、学科、难度、标签、答案和解析；确认内容后完成核验并标记价值。
4. 对识别失败项重试；需要修正原图范围时，在题目预览选择“调整识别范围”。人工修订及已核验内容有覆盖保护。
5. 通过筛选或勾选题目组卷。测验保存完整快照，后续原题修改不会改变已有测验；可导出 PDF、Markdown 或使用浏览器打印。
6. Agent 工作台默认显示待采纳结果，其他任务与接入说明放在二级入口。没有外部 worker 时不会自动生成答案或解析。

## 项目结构

```text
src/                 Rust API、数据、导入、版面聚合、任务进度与 PDF
web/                 嵌入式 HTML / CSS / JavaScript 与本地前端依赖
migrations/          SQLite 结构
services/math-ocr/   可选 Python 版面与公式识别服务及测试
assets/              图标、字体与第三方许可证
examples/            示例试卷与 Agent worker
scripts/             启动、打包与验证脚本
packaging/windows/   Windows setup.exe 定义
tests/browser/      浏览器故障恢复回归测试
docs/               部署、API、使用说明与设计边界
release/            发行目录说明；生成程序不进入 Git
.github/workflows/   Rust 与 OCR 服务契约检查
```

详细模块职责见 [目录与架构](docs/ARCHITECTURE.md)。`data/`、`tmp/`、`target/`、`tools/`、`dist/`、虚拟环境与本机配置均不提交到 Git。

## Agent 与识别服务

Agent 框架不限，通过 Bearer 令牌领取与提交任务。接口见 [API](docs/API.md)、[机器可读契约](docs/agent-contract.json) 和 [Rust worker 示例](examples/agent_worker.rs)。识别服务配置与 Agent 的模型调用各自独立，程序不内置模型服务密钥。

OCR 凭据可在工作区设置中配置；环境变量优先，读取接口隐藏密钥。配置存入本地题库，请保护整个数据目录；Mathpix 等在线服务的可用性与费用由提供方决定。未经本次授权不会上传原图。

## 数据与备份

`data/paperdesk.sqlite3` 保存来源、题目、测验、设置和任务；`data/papers/` 保存原件，`data/imports/` 保存页面及裁切证据；`data/agent-token.txt` 保存 Agent 访问令牌。程序默认生成随机令牌，也可用 `PAPERDESK_AGENT_TOKEN` 指定至少 24 字符的令牌。

完整备份请先停止服务，再复制整个数据目录。结构化 JSON 导出不含原卷、令牌或浏览器作答，不能替代完整备份；浏览器作答保存在当前 origin 的 localStorage。

| 环境变量 | 默认值 / 用途 |
| --- | --- |
| `PAPERDESK_DATA` | `data`；题库与原件目录 |
| `PAPERDESK_PORT` | `3210`；源码启动脚本可改为 `3215` |
| `PAPERDESK_BIND` | `127.0.0.1`；容器内为 `0.0.0.0` |
| `PAPERDESK_AGENT_TOKEN` | 可选，自定义 Agent 令牌 |
| `PAPERDESK_OCR_BIN` | 可选，Poppler / Tesseract 目录 |
| `PAPERDESK_OCR_THREADS` | 本地公式模型线程数，默认 1–2，上限 4 |

其他 OCR 变量见 [识别配置](docs/OCR-CONFIGURATION.md)。

## 验证与贡献

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

OCR 服务契约及资源管理测试不需要下载真实模型：

```sh
python -m pip install pillow numpy
python -m unittest discover -s services/math-ocr -p "test_*.py"
```

浏览器故障注入测试见 [性能与进度](docs/PERFORMANCE-AND-PROGRESS.md)。提交格式、测试要求和发布规则见 [贡献指南](CONTRIBUTING.md)。

## 适用范围与精度边界

本项目面向个人本机使用，浏览器界面没有账户系统；Agent 接口使用 Bearer 认证，本机写入使用跨站请求保护。请勿直接将服务暴露到公网。

OCR 与自动分区都是待审核候选，数学符号、图形、表格、单位及多栏阅读顺序需要人工核验。候选文本相当于 `md_extracted`；原图用于核对，程序不会自动宣称 `pdf_verified`。只有用户核验后才记录 `verified: true`。结构化导出属于 `structured_parsed`，其中题目准确性取决于核验状态。

公式可在界面渲染，复杂 PDF 可使用浏览器打印；原生 Rust PDF 导出不等价于完整 LaTeX 排版。快速文字模式不保证数学公式准确，本地 CPU 公式精细识别仍可能较慢。服务的单模型进程与资源限制不能替代实际硬件容量规划。
