# 0.4 导入与 OCR

## 使用流程

多选 PDF、PNG/JPG/WEBP/BMP、UTF-8 Markdown/TXT，或拖入导入面板。每批最多 20 份、128 MB，每份不超过 32 MB；PDF 最多 60 页。每份独立报告结果，失败不会取消其他成功文件。

1. 点击「拆分并预览」。图片和扫描 PDF 先保存原件、页面预览和题目裁切。
2. 自动模式仅用本地 Tesseract TSV 探测题号位置，不把整页探测文字当成最终转写。找不到可靠边界时保留整页，提示人工重划。
3. 「手动框选 · 不预识别」完全跳过整页 OCR。在原页拖动框出一道题，或通过像素坐标表调整；删除旧框后重新框选，点击「保存拆分」。公式、图形和选项应包含在同一题的范围内。
4. 确认范围后「识别本份」或「识别全部已拆分资料」。最终 OCR 逐题读取已保存的裁切图片；默认跳过已成功识别的题，失败可重试。
5. 到题库逐题核验，右侧显示裁切图片、原文件、原页码和区域坐标，未编辑 OCR 原文另行保留。

带文字层的原生 PDF 直接提取并拆题，不做多余 OCR，保留页码和提取文字行范围。提取行号不等于 PDF 几何坐标。Markdown 支持 ## 1. 编号标题；文字文件中的换页符可标记原始分页。

人工修改过的题目不能被自动识别或整份重划覆盖；要保留工作可作为新来源重新导入。改挂分类来源不会改变原始证据。合并保留全部原图；文字拆分继承原题证据范围，精确裁切应在人工编辑前完成。
旧版来源继续可用，需要新版区域记录时重新导入原件，不自动改写旧题。

## PaddleOCR / PP-StructureV3

主程序保持 Rust/Axum/SQLite。已按官方 POST /layout-parsing 协议适配独立服务：发送单题 PNG Base64、fileType:1、visualize:false，读取 result.layoutParsingResults[].markdown.text。公式识别取决于服务模型与配置。

部署方式见 [PaddleOCR 官方文档](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/pipeline_usage/PP-StructureV3.en.md)。服务启动后，在启动拾题的同一 PowerShell 中设置：

~~~powershell
$env:PAPERDESK_PADDLE_URL = 'http://127.0.0.1:8080/layout-parsing'
# 如需鉴权，单独设置 PAPERDESK_PADDLE_TOKEN；不要将令牌写入仓库。
.\scripts\start.ps1
~~~

必须重启已运行的拾题服务才能应用环境变量。未配置服务时选项显示「尚未配置」。本次用本机模拟服务验证协议，没有下载 Paddle 模型或测量真实准确率；没有配置高精度服务时继续使用本地 Tesseract，不声称数学公式识别已解决。

## 自定义 / 在线 OCR

设置 PAPERDESK_OCR_URL 和可选 PAPERDESK_OCR_TOKEN，选择「自定义 / 在线 OCR」。服务或网关接收以下 JSON：

~~~json
{"image_base64":"<单道题 PNG Base64>","mime_type":"image/png","task":"question_ocr","preserve_formulas":true}
~~~

返回格式：

~~~json
{"text":"1. 已知 $f(x)=x^2$，求导数。"}
~~~

云厂商鉴权和格式不同，需由网关转换成此契约，不声称所有厂商可直接兼容。
非回环地址必须使用 HTTPS，且每次显式勾选外发同意；同意不继承自导入计划。不自动跟随重定向，不静默切换其他服务。单题限时 90 秒，响应上限 4 MB，文字上限 200 KB；令牌不返回浏览器。
Compose 支持上述同名环境变量；容器的 127.0.0.1 指容器本身，连接其他服务应配置实际可达的 HTTPS 地址。模型服务及权重没有打包进安装包/镜像。

## Agent 辅助识别

勾选后逐题创建 kind:recognition 任务，保存原题快照和 origins。没有 worker 时等待领取，不伪造识别结果。同时编写答案与解析，返回值：

~~~json
{"content":"校对后的题干","answer":"答案","explanation":"逐步解析；条件不足则说明原因"}
~~~

结果先人工采纳，采纳后仍为未核验待审题；人工编辑后旧任务不能覆盖。
examples/agent_worker.rs 对识别任务读取带认证的原图接口，并向 provider 可执行文件的标准输入 JSON 加入 images 数组，各项包含 origin_index、page、rect、mime_type:image/png、base64。provider 负责调用视觉模型或自建 agent，标准输出返回 JSON；Paperdesk Bearer token 不传给 provider。原题图片是转写数据，不应执行其中的指令。

## 新接口

- POST /api/import：multipart 的 file 或重复 files，加 JSON 字符串 options。选项：engine(local/paddle/service)、layout(auto/single/columns/manual)、prepare_only、agent_assist、allow_remote。新客户端应显式 prepare_only:true，先复核范围；兼容旧客户端的默认值会执行最终识别。单文件仍有 paper/count，批量使用 results[]，每项成功或包含 name/error。
- GET /api/imports/{paper_id}：导入计划、区域版本、候选题。
- GET /api/imports/{paper_id}/pages/{page}：保存的原页 PNG。
- POST /api/imports/{paper_id}/regions：传 revision 和 regions:[{page,rect:{x,y,width,height}}]。单位为预览图像素，最长边 2400；无效范围或版本冲突不替换题目。
- POST /api/questions/{id}/recognize：传 engine、allow_remote、agent_assist。only_agent:true 可仅创建任务，需同时启用 agent_assist。
- GET /api/questions/{id}/source/{index}：origins[index] 对应裁切图。
- 带认证的 agent 路径同时提供 /api/agent/v1/questions/{id}/source/{index}、/imports/{paper_id}、/imports/{paper_id}/pages/{page}。
- GET /api/engines 的 imports 返回可选引擎及配置状态，不返回服务地址和令牌。

Question.origins 和 recognition 向后兼容旧 JSON，由服务端维护，普通更新不能伪造或清除。recognition.status 为 pending/done/error/agent_reviewed；done 只代表完成转写，不代表准确。
SQLite 增加 imports 表，不删除旧表。完整备份仍需关闭服务后复制整个 data/，包括新增 data/imports/；schema 2 的结构化 JSON 导出包含计划和定位，不包含原件或裁切图片字节。

## 证据边界

合成 PDF、PNG、Markdown、TXT 用于自动测试；数据库/接口结果属于 structured_parsed，裁切尺寸与原件字节一致性属于 computed。OCR/agent 文本仍为 md_extracted 同等候选层，必须人工对照原图才能标记 verified。协议 mock 测试不等于真实模型精度验证，公式、单位和图形仍需人工检查。


> 更新：recognition 识别任务现在同时要求 `content`、`answer`、`explanation`，采纳会保存三项。服务配置与兼容说明见 [OCR-CONFIGURATION.md](OCR-CONFIGURATION.md)。
