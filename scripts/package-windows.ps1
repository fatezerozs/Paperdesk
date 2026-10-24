param([string]$CompilerPath, [string]$TargetDir = "target")
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
& $cargoPath build --release --locked --target-dir $TargetDir
if ($LASTEXITCODE -ne 0) { throw 'Rust 发布编译失败' }
if (-not $CompilerPath) {
    $candidates = @((Join-Path (Get-Location) 'tools\inno\ISCC.exe'), (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'), 'C:\Program Files\Inno Setup 7\ISCC.exe')
    $CompilerPath = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $CompilerPath) { throw '请安装 Inno Setup 6/7，或使用 -CompilerPath 指定 ISCC.exe。' }
$appExecutable = (Resolve-Path (Join-Path $TargetDir "release\paperdesk.exe")).Path
& $CompilerPath "/DAppExecutable=$appExecutable" packaging\windows\paperdesk.iss
if ($LASTEXITCODE -ne 0) { throw '安装包生成失败' }
Get-FileHash dist\Paperdesk-0.5.0-setup.exe -Algorithm SHA256

# A self-contained local release folder, also used by the repository launcher.
New-Item -ItemType Directory -Force release, release/scripts | Out-Null
Copy-Item -LiteralPath $appExecutable -Destination release/paperdesk.exe -Force
Copy-Item -LiteralPath '启动拾题.cmd' -Destination release -Force
Copy-Item -LiteralPath scripts/start.ps1 -Destination release/scripts -Force
Copy-Item -LiteralPath README.md -Destination release -Force
New-Item -ItemType Directory -Force release/docs | Out-Null
Get-ChildItem docs -File | Where-Object { $_.Name -notlike "VALIDATION-*" } | Copy-Item -Destination release/docs -Force
Copy-Item -Path tools/ocr -Destination release -Recurse -Force
Set-Content -LiteralPath release/VERSION -Value '0.5.0' -Encoding ascii

New-Item -ItemType Directory -Force release/services/math-ocr, release/licenses, release/examples | Out-Null
Copy-Item -LiteralPath scripts/start-math-ocr.ps1 -Destination release/scripts -Force
Copy-Item -LiteralPath services/math-ocr/server.py, services/math-ocr/requirements.txt -Destination release/services/math-ocr -Force
Copy-Item -LiteralPath assets/fonts/OFL.txt -Destination release/licenses/NotoSansSC-OFL.txt -Force
Copy-Item -LiteralPath web/vendor/katex/LICENSE -Destination release/licenses/KaTeX-MIT.txt -Force
Copy-Item -LiteralPath examples/sample-paper.txt -Destination release/examples -Force
