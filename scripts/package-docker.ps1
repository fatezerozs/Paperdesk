$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$dockerCommand = Get-Command docker -ErrorAction SilentlyContinue
$dockerPath = if ($dockerCommand) { $dockerCommand.Source } else { Join-Path $env:ProgramFiles 'Docker\Docker\resources\bin\docker.exe' }
if (-not (Test-Path -LiteralPath $dockerPath)) { throw '请先安装并启动 Docker Desktop' }
$env:PATH = (Split-Path -Parent $dockerPath) + ';' + $env:PATH
& $dockerPath compose build
if ($LASTEXITCODE -ne 0) { throw 'Docker 镜像构建失败' }
New-Item -ItemType Directory -Force dist | Out-Null
& $dockerPath image save -o dist\paperdesk-0.4.0-docker.tar paperdesk:0.4.0
if ($LASTEXITCODE -ne 0) { throw 'Docker 镜像导出失败' }
Get-FileHash dist\paperdesk-0.4.0-docker.tar -Algorithm SHA256
