# 安装、打包与容器部署

## Windows setup.exe

发行产物：`dist/Paperdesk-0.4.0-setup.exe`，支持 Windows 10/11 x64。当前安装包未做商业代码签名。OCR 运行时已包含在安装包，不需要单独部署。

安装在当前用户的 `%LOCALAPPDATA%\Programs\Paperdesk`，不需要管理员权限。开始菜单中提供启动入口、数据目录入口和卸载入口；可选桌面快捷方式。启动会打开浏览器，同时保留服务窗口，关闭窗口或按 Ctrl+C 停止本机服务。重复点击启动入口会直接打开已运行的界面。

数据单独存放于 `%LOCALAPPDATA%\Paperdesk\data`，升级、卸载不删除该目录。便携程序在其工作目录的 `data/` 存储，和安装版的默认目录相互独立。

源码打包（已安装 Rust MSVC + C++ Build Tools + Inno Setup 6/7，并把 Poppler、Tesseract 与 chi_sim 模型放入本地 `tools/ocr/`）：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1
```

可通过 `-CompilerPath C:\path\ISCC.exe` 指定 Inno 编译器。项目包含便携编译器时自动查找 `tools/inno/ISCC.exe`，该目录不进入源码版本管理。`assets/fonts/` 下字体及 OFL 许可证必须保留，字体嵌入最终程序和导出 PDF。

## Docker

```sh
docker compose up -d --build
```

打开 `http://127.0.0.1:3210`。镜像采用 Rust 官方镜像进行多阶段构建，运行层为 Debian slim，以非 root 用户运行。镜像内置 Poppler、Tesseract 中文 OCR；无需外部 OCR 服务、Node.js、Python 或数据库。SQLite、原卷、agent 令牌都在 `/data`，使用命名卷持久化。

查看状态与日志：

```sh
docker compose ps
docker compose logs --tail=100 paperdesk
```

已有本机版占用端口时，可改宿主端口。PowerShell：

```powershell
$env:PAPERDESK_HTTP_PORT = '3211'
docker compose up -d --build
```

然后打开 `http://127.0.0.1:3211`。容器内部仍使用 3210。Compose 保持宿主机回环地址绑定；本版本为个人本机工具，不提供账户隔离。

停止与再次启动：

```sh
docker compose stop
docker compose start
```

`docker compose down` 删除容器但保留命名卷。**不要使用 `docker compose down -v`，除非你明确要删除全部题库。**

## 离线分发镜像

构建并导出：

```sh
docker compose build
docker image save -o dist/paperdesk-0.4.0-docker.tar paperdesk:0.4.0
```

目标机器导入镜像，并把 compose.yaml 放在工作目录：

```sh
docker image load -i paperdesk-0.4.0-docker.tar
docker compose up -d --no-build
```

源码分发也可以只提供 Dockerfile、compose.yaml 与源码、字体文件，目标机器执行联网构建。

## 备份

Windows 关闭服务后复制整个数据目录。容器先停止，再复制 `/data`：

```sh
docker compose stop
docker compose cp paperdesk:/data ./paperdesk-backup
docker compose start
```

结构化 JSON 导出不含原始 PDF、agent token 或浏览器作答，不能替代整个目录的备份。恢复到 Docker 命名卷时保持运行用户 UID/GID 10001 的读写权限。浏览器作答在对应 origin 的 localStorage 内，不随题库目录迁移。

Windows 完整安装包包含独立 OCR 子进程与中文模型，位于安装目录 `ocr/`；请勿仅复制主 exe 后期待扫描识别工作。源码入口双击 `启动拾题.cmd`；安装后的快捷方式同样使用默认浏览器。OCR 进程隐藏运行，主服务窗口需保持运行。
