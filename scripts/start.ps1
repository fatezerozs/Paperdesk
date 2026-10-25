param([switch]$NoBrowser)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$expectedVersion = '0.6.0'
$installedExe = Join-Path $root 'paperdesk.exe'
$isInstalled = Test-Path -LiteralPath $installedExe
$localConfig = Join-Path $root 'paperdesk.local.json'
if (-not $isInstalled -and (Test-Path -LiteralPath $localConfig)) {
    $workspace = Get-Content -LiteralPath $localConfig -Raw | ConvertFrom-Json
    if (-not $env:PAPERDESK_DATA -and $workspace.data_dir) { $env:PAPERDESK_DATA = [IO.Path]::GetFullPath((Join-Path $root $workspace.data_dir)) }
    if (-not $env:PAPERDESK_PORT -and $workspace.port) { $env:PAPERDESK_PORT = [string]$workspace.port }
}
$port = if ($env:PAPERDESK_PORT) { [int]$env:PAPERDESK_PORT } elseif ($isInstalled) { 3210 } else { 3215 }
$env:PAPERDESK_PORT = "$port"
$url = "http://127.0.0.1:$port"
$health = $null
try { $health = Invoke-RestMethod "$url/api/health" -TimeoutSec 2 } catch { }
if ($health -and $health.ok) {
    if ($health.version -ne $expectedVersion) { throw "端口 $port 正运行旧版 $($health.version)。请关闭旧版后重试，或设置 PAPERDESK_PORT 使用其他端口。" }
    if (-not $NoBrowser) { Start-Process $url }
    Write-Output "Paperdesk $expectedVersion $url"
    exit 0
}
if ($isInstalled -and -not $env:PAPERDESK_DATA) { $env:PAPERDESK_DATA = Join-Path $env:LOCALAPPDATA 'Paperdesk\data' }
$candidates = @($installedExe, (Join-Path $root 'release\paperdesk.exe'), (Join-Path $root 'target\release\paperdesk.exe'))
$executable = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if ($executable) {
    if ($NoBrowser) { & $executable } else { & $executable --open }
    exit $LASTEXITCODE
}
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoPath)) { throw '请先安装 Rust，或使用已编译的发行程序。' }
if ($NoBrowser) { & $cargoPath run --locked } else { & $cargoPath run --locked -- --open }
exit $LASTEXITCODE
