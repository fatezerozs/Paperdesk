$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$port = if ($env:PAPERDESK_PORT) { [int]$env:PAPERDESK_PORT } else { 3210 }
$url = "http://127.0.0.1:$port"
try {
    $health = Invoke-RestMethod "$url/api/health" -TimeoutSec 2
    if ($health.ok -and $health.version) { Start-Process $url; exit 0 }
} catch { }
$installedExe = Join-Path (Get-Location) 'paperdesk.exe'
$releaseExe = Join-Path (Get-Location) 'target\release\paperdesk.exe'
if (Test-Path -LiteralPath $installedExe) {
    if (-not $env:PAPERDESK_DATA) { $env:PAPERDESK_DATA = Join-Path $env:LOCALAPPDATA 'Paperdesk\data' }
    & $installedExe --open
    exit $LASTEXITCODE
}
if (Test-Path -LiteralPath $releaseExe) {
    & $releaseExe --open
    exit $LASTEXITCODE
}
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoPath)) { throw '请先从 https://rustup.rs 安装 Rust，或使用已编译的 paperdesk.exe。' }
& $cargoPath run --locked -- --open
exit $LASTEXITCODE
