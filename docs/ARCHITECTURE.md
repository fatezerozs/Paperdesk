# 目录与架构

## 主要模块

| 路径 | 职责 |
| --- | --- |
| `src/main.rs`、`src/api.rs` | 启动、Axum 路由、本机请求保护及 Agent 认证 |
| `src/db.rs`、`src/model.rs`、`migrations/` | SQLite、JSON 记录、修订及存储约束 |
| `src/importer.rs`、`src/layout.rs` | 上传、页面与题目裁切、版面聚合、识别与结果提交 |
| `src/regions.rs` | 单题范围修改、历史存档与冲突保护 |
| `src/operations.rs` | 有界任务进度、阶段与终态 |
| `src/ocr.rs`、`src/ocr_settings.rs` | 子进程调用、资源约束、识别配置与凭据隐藏 |
| `src/pdf.rs`、`src/splitter.rs` | PDF 导出和题号拆分 |
| `web/` | 原生 JavaScript/CSS 界面，编译时嵌入 Rust 程序 |
| `services/math-ocr/` | Python HTTP 服务和独立模型进程 |
| `tests/browser/`、`src/tests.rs` | 浏览器故障注入与 Rust 集成测试 |
| `docs/`、`examples/` | 使用说明、接口与示例 |
| `scripts/`、`packaging/`、`release/` | 启动、构建、安装定义及发行目录说明 |

## 处理流程

上传 → 保存原件 → 渲染/读取坐标 → 检测与聚合题目范围 → 人工确认 → 保存逐题裁切 → OCR / Agent 候选 → 人工核验 → 入库与组卷。

Rust 主进程负责题库、来源证据、修订校验、任务接口与 HTTP 响应。PaddleOCR 服务是可选模块，模型推理使用单独进程；健康检查与模型执行分离。Poppler/Tesseract、图片裁切及外部同步调用通过受控重任务路径执行，避免直接堵塞异步工作线程。

前端通过操作 ID 读取阶段进度，已知页数/题数才显示确定进度。连接失败意味着后台结果尚未确认，不等价于后台强制取消；停止后续任务在当前项结束后生效。详见 [异常恢复](PERFORMANCE-AND-PROGRESS.md)。

## 数据与发布边界

SQLite 与原件在配置的数据目录中；完整备份需要保留整个目录。程序不把运行数据、服务密钥、模型缓存或生成发行文件写进 Git。`release/README.md` 是受版本管理的发行说明，其余 release 生成内容被忽略。setup.exe 和 Docker 镜像由脚本另行构建，不作为普通源码提交。

已识别题目调整范围时保留内容与原证据，写入修订历史，清除核验并要求重新审核。Agent 结果是待采纳候选，不能绕过修订校验或自动覆盖人工确认内容。
