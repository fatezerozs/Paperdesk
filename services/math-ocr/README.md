# 可选本地数学识别服务

此模块提供 PaddleOCR PP-StructureV3 正文、公式和版面接口。Rust 主程序管理题库与原图，本服务只处理提交的页面或裁切；默认监听本机 `127.0.0.1:8850`。

## Windows

在项目根目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-math-ocr.ps1
```

首次启动安装独立 Python 虚拟环境与依赖，并下载模型，需要网络、磁盘和内存。脚本、模型与依赖安装详情以 [识别配置](../../docs/OCR-CONFIGURATION.md) 为准。主界面选择本地 PaddleOCR，并配置脚本提示的接口地址。

## Docker

```sh
docker compose -f compose.yaml -f compose.math.yaml up -d --build
```

服务构建较大；主镜像内的 Tesseract 与此公式服务相互独立。首次加载可能较慢，不等价于识别任务已卡死；可检查 `/health` 的 ready、state 和 busy。

## 管理与测试

单独模型进程、默认 1–2 CPU 线程、串行推理、超时重启与短期成功缓存详见 [性能与异常恢复](../../docs/PERFORMANCE-AND-PROGRESS.md)。`runtime.py` 是运行依赖，分发时必须与 `server.py` 同目录。

无需下载模型的契约与资源管理测试：

```sh
python -m pip install pillow numpy
python -m unittest discover -s services/math-ocr -p "test_*.py"
```

依赖锁定见 `requirements.txt`；假模型契约测试不代表真实公式准确率测试。复杂数学结果仍需对照原图人工核验。
