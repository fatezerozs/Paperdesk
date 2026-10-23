# 试卷解析、OCR 与排版引擎调研

调研时间：2026-10-23。依据各项目官方文档，尚未对这些外部模型在用户试卷上做实测。以下接入建议是工程判断，不代表准确率或性能排名。

## 推荐路线

保留 Rust 主程序与 SQLite。文档解析作为可替换服务，输出标准候选题；PDF 输出作为独立排版模块。优先试验 **MinerU / Docling 解析 + Typst 排版**，中文扫描卷可加入 PaddleOCR 对比。先用匿名测试卷做小规模对比，再决定是否下载模型或购买云服务。

原生 PDF 优先读取文字层与坐标、阅读顺序；扫描页或缺字区域再启用 OCR。解析获得的结构需要再次排版，无法据此承诺无损恢复原始 PDF。若追求原卷图形和公式外观保真，更适合保留 PDF 页面片段 / 题图并与可编辑文本双轨存储；该裁切保真功能尚未实现。

## 候选对比

| 组件 | 能力与原生 PDF | 方便的接法 | 对本项目的用途及边界 |
|---|---|---|---|
| MinerU | 官方提供原生 PDF 文字层模式、模型解析与结构化结果；云端有公式、表格识别选项 | 自托管 CLI / API 或云 API；版本和入口不同，返回结构不同 | 优先评估中文试卷结构、公式与题图；结构化块仍需组装为题目，不能把 Markdown 输出当作公式或数值核验依据 |
| PaddleOCR PP-StructureV3 | PDF / 图片版面分析，支持文字、表格及公式相关模块，输出 JSON / Markdown | 官方 POST `/layout-parsing` 可传 PDF Base64 | 适合中文扫描卷的候选方案；某些部署默认最多处理 10 页，需要核对并设置页数限制；不负责重新排版试卷 |
| Docling | PDF 原生文字提取、布局、表格，支持可选 OCR 及公式 enrichment | 自托管 REST `/v1/convert/file`，也有 Python SDK | 原生 PDF 可设置 `do_ocr=false`；更统一的文档结构便于后续 agent 读取，仍需试卷题号切分适配 |
| Mathpix | 面向 STEM 的公式、文字、表格识别，PDF 输出 MMD、LaTeX 等结构，可再转换 PDF | 云端 `v3/pdf` 异步提交、轮询和下载；需要配置 API 凭据 | 值得作为公式密集卷的云端对照方案；文件上传需用户选择，尚未接入或实测，不承诺原始字体和排版保真 |
| Typst | 排版引擎；字体、页面、栏、数学公式、PDF 输出可配置 | Rust 主程序调用独立 CLI；后续也可做 Rust 库集成 | 适合正式试卷模板与复杂排版。它不把既有 PDF 自动还原为可编辑题目；支持将 PDF 当图像使用，也不等于语义重排 |

### 官方依据

- MinerU 原生 PDF 模式与模型下载说明：[Quick start](https://github.com/opendatalab/MinerU/blob/master/docs/en/quick_start/index.md)。其 `flash` / `txt` 示例仅读取文字层，无扫描页 OCR 回退；具体版本安装后应核对 CLI 帮助。
- MinerU 输出不是单一格式，各 CLI/API 的 JSON 含义不同：[Output contract](https://github.com/opendatalab/MinerU/blob/master/docs/en/reference/output_files.md)。云端接口和 token 说明：[官方 API 文档](https://mineru.net/doc/docs/index_en/)。
- PaddleOCR 的 PDF 请求字段、公式模块和页数限制：[PP-StructureV3 官方说明](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/pipeline_usage/PP-StructureV3.en.md)。
- Docling 原生文字 / OCR / 公式参数：[Pipeline options](https://docling-project.github.io/docling/reference/pipeline_options/)；文件上传及 REST 接口：[API reference](https://docling-project.github.io/docling/usage/api_server/rest_api/)。
- Typst 字体设置：[Text](https://typst.app/docs/reference/text/text/)；PDF 输出：[PDF](https://typst.app/docs/reference/pdf/)；PDF 图像：[Image](https://typst.app/docs/reference/visualize/image/)。
- Mathpix 文档处理与转换：[Process a document](https://docs.mathpix.com/guides/pdf-processing)、[Supported formats](https://docs.mathpix.com/reference/supported-formats)。云端保留策略应在实际配置前阅读 [Data retention](https://docs.mathpix.com/concepts/data-retention)。

## 建议的接入顺序

1. 用原生 PDF、扫描卷、双栏卷、含几何图和公式的卷各选少量样本，记录题目边界、文字、公式、题图、页码是否正确。实际比较结果之后才能确定模型选择。
2. 先接一个本地解析器。Rust 保留原 PDF，解析器处理文件并输出带页码的题目候选；用本版结构化接口导入，人工核验后入库。
3. 保存题干的 Markdown / LaTeX、原页码和后续的题图 / 坐标。单纯字符串无法完整保存原版面，本版标准结构只包含题号、正文、页码、标签，图片与坐标仍待扩展。
4. 正式排版可接 Typst 模板。LaTeX 与 Typst 数学语法不同，不能直接将 OCR 生成的 LaTeX 当作 Typst 语法执行；需要转换器、限定格式，或保留经过核验的题图。模板应将题干视作数据，不能直接拼接为可执行排版代码。
5. 云端 OCR 需要用户配置服务与密钥，并明确选择发送文件。当前程序没有自动上传试卷到第三方，没有内置外部模型或密钥。

## 本版已经具备

- 原生 PDF 文字层导入和候选拆分，保留原文件。
- `/api/papers/{id}/text`：按页文字 / OCR 接入。
- `/api/papers/{id}/structured`：统一的结构化候选导入，契约见 `document-adapter.schema.json`。该接口不直接接受供应商原始返回；适配器先完成题目分组及字段转换。
- 结构化导入仅能替换尚未编辑的待审候选；遇到已审核或有修订的题目返回冲突，避免覆盖人工工作。
- 原生 Rust PDF 输出：用户上传静态中文 TTF、选择字号、行距、页边距、A4 / A5 / Letter、单栏 / 双栏、答案附录和作答空白。
- `/api/engines` 明确报告内置能力与外部候选尚未安装，界面提供说明入口。

## 信任层与剩余风险

调研使用官方 HTML / Markdown 文档，属于 `md_extracted` 级别的接口和能力说明；本次没有外部模型真实试卷准确率的 `computed` 结论，也没有依据供应商演示图做 `pdf_verified` 判断。不能据此承诺公式、题图、双栏顺序或单位准确。

程序本身的题库 JSON 和接口测试属于 `structured_parsed` / `computed`；新导出 PDF 的中文、分页、单双栏和作答区域须另行进行实际渲染检查。用户试卷解析出的公式、数值和图表继续以原始页面为核验依据。
