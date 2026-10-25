# 贡献与提交约定

## 开发与目录

先阅读 [README](README.md)、[文档索引](docs/README.md) 与 [架构](docs/ARCHITECTURE.md)。Rust 源码放入 `src/`，前端资源放入 `web/`，数据库结构放入 `migrations/`，外部识别服务放入 `services/`，使用与部署说明放入 `docs/`。脚本、安装定义、示例和测试分别使用现有目录，不将临时文件混入源码。

## 提交要求

- 提交前运行 `cargo fmt --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`。
- 修改 OCR 服务时运行 `python -m unittest discover -s services/math-ocr -p "test_*.py"`，依赖为 Pillow 与 NumPy；契约测试不下载模型。
- 修改异步进度、失败恢复或交互时检查对应浏览器回归脚本；说明已验证情形和未验证限制。
- 不提交题库、试卷、密钥、令牌、日志、虚拟环境、模型权重或本机路径配置。使用无个人数据的测试样例。
- 提交采用 Conventional Commits：`feat(import): ...`、`fix(ocr): ...`、`docs: ...`、`chore(release): ...`。标题概括实际行为；正文说明必要的验证与兼容性。
- PR 描述说明问题、最终行为、验证结果与限制。分支可用 `codex/` 或项目协作者约定的前缀。

## 版本与发行

更新 Cargo.toml 与 Cargo.lock 中本项目版本，同时同步启动脚本、Windows 安装定义、打包脚本、Compose 镜像标签、README 和更新记录。旧版本说明保留为历史文档。

先通过检查，再生成发行程序。`release/` 保留目录说明；可执行文件、setup.exe、OCR 二进制和 Docker 导出镜像属于生成物，不提交到源码 Git 历史。需要公开二进制时，将它们作为对应 GitHub Release 的附件并附校验值。未实际构建的安装包或镜像不可标记为已发布。

## 数据与精度

修改题干、来源或识别范围时保留修订冲突保护、原图溯源与人工核验；失败不得覆盖已有内容为“成功”。外部 OCR 上传必须保留本次授权。不要将自动 OCR 结果宣称为已完成公式、单位或图表核验。
