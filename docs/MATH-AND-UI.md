# 数学识别与界面更新

题库中的“来源与资料”负责来源筛选、重命名和手工来源管理。Agent 工作台位于左侧底部；手机窄屏位于导航下方。手工拆题使用彩色边框、编号、选中高亮和实时绘框，右侧坐标卡与原图颜色对应。

## 数学导入

导入面板默认勾选“公式增强”。开启时，即使 PDF 含文字层，也会渲染页面、保存拆题区域后再识别，避免把正文提取成功误认为公式也完整。关闭时保留文字直提的快速路径。Markdown/TXT 仍按文本导入。

- 本地 Tesseract：通用文字识别，不承诺正确识别数学结构。
- PaddleOCR：PP-StructureV3 正文、版面与专用公式模型。每次请求明确带 `useFormulaRecognition:true`，返回 Markdown + LaTeX。现有 PP-StructureV3 服务仍可连接。
- Mathpix：内置 `/v3/text` 适配，不再要求自建转发网关。需要 `PAPERDESK_MATHPIX_APP_ID`、`PAPERDESK_MATHPIX_APP_KEY` 环境变量，并逐次勾选远程传输同意。服务可能收费；程序不自动切换或发送到其他服务。单题编码图片上限 2 MB。
- Agent：复核任务需要 worker，结果由用户采纳。原题图片、原始识别文本及人工修改保护继续保留。

几何图仍以原题裁切为准，图形矢量化不包含在公式识别中。任何模型均需要人工核验，不能根据 LaTeX 可渲染就判定公式正确。

## 本地公式服务

主程序和数据库继续使用 Rust。模型运行环境独立，避免把机器学习依赖塞进主程序。

Windows（Python 3.10–3.12）：

```powershell
.\scripts\start-math-ocr.ps1 -Python C:\path\to\python.exe
```

首次安装下载依赖、模型权重，启动期间控制台会显示进度。等 `Ready` 后，在启动拾题的 PowerShell 中设置：

```powershell
$env:PAPERDESK_PADDLE_URL = 'http://127.0.0.1:8850/layout-parsing'
.\target\release\paperdesk.exe --open
```

CPU 作为默认设备；速度和内存依硬件而异。`PAPERDESK_FORMULA_DEVICE` 可配置兼容环境的设备。服务只监听 127.0.0.1:8850，按顺序处理请求，不接收文件路径或远程 URL。

Docker：

```shell
docker compose -f compose.yaml -f compose.math.yaml up -d --build
```

公式服务与主程序共享容器网络命名空间，通过 loopback 通信；模型缓存保留在独立命名卷。首次下载可能耗时，需等公式服务健康后使用。普通 `compose.yaml` 不自动安装模型服务。

## 公式预览与 PDF

内置 KaTeX 0.19.1、字体及 MIT 许可证，无需 CDN。支持 `$...$`、`$$...$$`、`\(...\)`、`\[...\]`；正文、答案和解析均可预览。代码片段不作为公式处理；无法渲染的公式保留源码并显示“公式待检查”。不支持任意 LaTeX 文档或所有宏包。

测验的“导出 PDF”提供两种方式：

- 公式排版：打开本地排版窗口，加载已选中文字体，支持纸张、字号、行距、单双栏、页边距、答题空白及答案另页；点击“打印 / 保存为 PDF”，在浏览器选择另存为 PDF。建议关闭浏览器页眉页脚。公式使用 KaTeX 数学字体，正文使用所选字体。
- 原生 PDF：继续由 Rust 直接下载，公式保留 LaTeX 源码。

浏览器分页对超长单题会自动分割；过宽公式仍需人工调整或改为单栏。题图自动嵌入不包含在本次排版更新中，原图可在编辑器对照。

## 验证边界

Mathpix 的结果解析、错误和传输同意已有测试，但付费账户实测需要有效密钥。模型服务的协议测试不能代替真实模型准确率评测。建议以实际试卷逐题比较公式结构，尤其是上下标、分母、区间和不等号。

协议参考：[PaddleOCR PP-StructureV3](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/pipeline_usage/PP-StructureV3.en.md)、[Mathpix 图片 API](https://docs.mathpix.com/reference/post-v3-text)、[KaTeX](https://katex.org/docs/options.html)。

### 本机实际抽样

已用 Windows CPU、PaddleOCR 3.7.0 / PaddleX 3.7.2 / PaddlePaddle 3.3.1 跑通两份裁切：选择题约 26.5 秒，主公式 `f(x)=x^{2}` 正确，但选项行被错误转写为分式；原生 PDF 的题干和导数公式裁切约 167.7 秒，识别出了 `\frac{d}{dx}x^2=2x`，行内文字分界仍需核验。这是两份功能抽样，不是准确率基准。CPU 速度较慢，批量使用可考虑兼容 GPU 环境或在线服务。默认关闭 oneDNN，以避开本机实际遇到的 PIR 属性转换错误。

验证包括 31 项 Rust 测试、2 项模型服务协议测试，实际浏览器下的多色绘框、选中同步、来源整合、底部 Agent 入口、三主题及窄屏，以及公式 PDF 的渲染核验。Windows 安装包经过独立目录安装、启动和重启持久化验证；Docker 主程序在独立容器完成导入与离线资源验证。独立的 Linux 模型服务镜像尚未执行真实模型推理；在线 Mathpix 尚未用付费密钥实测。
