param([string]$ReleaseExe = "target\release\paperdesk.exe", [string]$InstallDir = "tmp\installed")
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$testRoot = Join-Path (Get-Location) $InstallDir
$testExe = Join-Path $testRoot 'paperdesk.exe'
if (-not (Test-Path -LiteralPath $testExe)) { throw '请先将安装包测试安装到 tmp\installed' }
$env:PAPERDESK_PORT = '3212'
$env:PAPERDESK_DATA = Join-Path (Get-Location) ('tmp\installer-data-' + [guid]::NewGuid().ToString('N'))
$expectedHash = (Get-FileHash $ReleaseExe).Hash
if ((Get-FileHash $testExe).Hash -ne $expectedHash) { throw '安装后的可执行文件与发布版本不一致' }
function Start-TestServer {
    $server = Start-Process -FilePath $testExe -WorkingDirectory $testRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput tmp\installer-server.log -RedirectStandardError tmp\installer-error.log
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        try { $null = Invoke-RestMethod http://127.0.0.1:3212/api/health; return $server } catch { Start-Sleep -Milliseconds 200 }
    }
    if (-not $server.HasExited) { Stop-Process -Id $server.Id }
    throw '安装版本未能启动'
}
$server = Start-TestServer
try {
    $import = Invoke-RestMethod http://127.0.0.1:3212/api/import -Method Post -Headers @{'X-Paperdesk'='1'} -Form @{ file = Get-Item examples\sample-paper.txt }
    if ($import.count -ne 5) { throw '安装版本未正确导入试卷' }
} finally { if (-not $server.HasExited) { Stop-Process -Id $server.Id } }
$server = Start-TestServer
try {
    $questions = Invoke-RestMethod http://127.0.0.1:3212/api/questions
    if ($questions.Count -ne 5) { throw '重启后题库没有持久化' }
    Write-Output '安装文件校验、独立启动、试卷导入及重启持久化：通过'
} finally { if (-not $server.HasExited) { Stop-Process -Id $server.Id } }
