param([string]$CompilerPath)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
& $cargoPath build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'Rust 发布编译失败' }
if (-not $CompilerPath) {
    $candidates = @((Join-Path (Get-Location) 'tools\inno\ISCC.exe'), (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'), 'C:\Program Files\Inno Setup 7\ISCC.exe')
    $CompilerPath = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $CompilerPath) { throw '请安装 Inno Setup 6/7，或使用 -CompilerPath 指定 ISCC.exe。' }
& $CompilerPath packaging\windows\paperdesk.iss
if ($LASTEXITCODE -ne 0) { throw '安装包生成失败' }
Get-FileHash dist\Paperdesk-0.3.0-setup.exe -Algorithm SHA256
