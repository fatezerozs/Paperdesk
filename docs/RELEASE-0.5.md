# 0.5.0

- 题库以题干为主体，状态与难度合并为简洁提示，来源与标签降为次要信息；长题点击打开完整预览。
- Agent 工作台默认显示“待采纳”。右上角菜单切换其他任务，接入说明单独弹窗。
- 工作区设置、来源删除、再次校对与修订冲突保护。
- 多选和 Ctrl/Command、Shift 快捷操作保留。

## 本地发行与启动

源码目录双击“启动拾题.cmd”，默认端口为 3215，优先运行 release/paperdesk.exe。安装版/独立发行目录默认端口 3210，数据保存到用户 LocalAppData/Paperdesk/data。源码启动数据默认在当前目录 data 中；可通过 PAPERDESK_DATA 指定已有题库。

若端口上已有不同版本，启动脚本会提示关闭旧版或更换端口，不再静默打开旧页面。环境变量 PAPERDESK_PORT 可覆盖默认端口。

Windows：scripts/package-windows.ps1 生成 dist/Paperdesk-0.5.0-setup.exe 和 release/ 本地发行目录。Docker：scripts/package-docker.ps1 生成 dist/paperdesk-0.5.0-docker.tar。发布文件不提交到源码 Git 历史。

源码目录可通过不提交 Git 的 paperdesk.local.json 保存本机的 data_dir（相对于项目根目录）和 port。环境变量优先级最高。本次本地配置沿用当前预览题库。
